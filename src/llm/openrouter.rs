//! OpenRouter LLM provider implementation

use super::{
    FunctionDefinition, GenerationConfig, LlmError, LlmProvider, LlmResponse, Message,
    ModelInfo, TokenUsage, ToolCall, ToolDefinition,
};
use async_trait::async_trait;
use futures::future::BoxFuture;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// OpenRouter API provider
pub struct OpenRouterProvider {
    client: Client,
    api_key: String,
    base_url: String,
    retry_attempts: usize,
    retry_delay: Duration,
}

impl OpenRouterProvider {
    /// Create a new OpenRouter provider
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap_or_else(|_| Client::new()),
            api_key,
            base_url: "https://openrouter.ai/api/v1".to_string(),
            retry_attempts: 3,
            retry_delay: Duration::from_millis(1000),
        }
    }

    /// Create a new OpenRouter provider with custom configuration
    pub fn with_config(
        api_key: String,
        base_url: Option<String>,
        retry_attempts: Option<usize>,
        retry_delay: Option<Duration>,
    ) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap_or_else(|_| Client::new()),
            api_key,
            base_url: base_url.unwrap_or_else(|| "https://openrouter.ai/api/v1".to_string()),
            retry_attempts: retry_attempts.unwrap_or(3),
            retry_delay: retry_delay.unwrap_or_else(|| Duration::from_millis(1000)),
        }
    }

    /// Create OpenRouter request headers
    fn create_headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Authorization",
            format!("Bearer {}", self.api_key).parse().unwrap(),
        );
        headers.insert(
            "HTTP-Referer",
            "https://github.com/your-org/KAI-X".parse().unwrap(),
        );
        headers.insert(
            "X-Title",
            "KAI-X".parse().unwrap(),
        );
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        headers
    }

    /// Execute a request with retry logic
    async fn execute_with_retry<F, Fut, T>(&self, operation: F) -> Result<T, LlmError>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T, LlmError>> + Send,
    {
        let mut last_error = None;
        
        for attempt in 0..=self.retry_attempts {
            match operation().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    last_error = Some(e);
                    
                    if attempt < self.retry_attempts {
                        // Check if error is retryable
                        if let Some(ref error) = last_error {
                            match error {
                                LlmError::RateLimit { retry_after } => {
                                    // Use retry_after if provided, otherwise use exponential backoff
                                    let delay = if let Some(retry_after) = retry_after {
                                        Duration::from_secs(*retry_after)
                                    } else {
                                        self.retry_delay * (attempt as u32 + 1) * 2
                                    };
                                    tokio::time::sleep(delay).await;
                                }
                                LlmError::Network(_) => {
                                    tokio::time::sleep(self.retry_delay * (attempt as u32 + 1)).await;
                                }
                                LlmError::RequestFailed { status, .. } if *status >= 500 => {
                                    tokio::time::sleep(self.retry_delay * (attempt as u32 + 1)).await;
                                }
                                _ => {
                                    // Non-retryable error, break immediately
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
        
        Err(last_error.unwrap_or_else(|| LlmError::Unknown {
            message: "Retry operation failed with no error".to_string(),
        }))
    }

    /// Parse error response and return appropriate LlmError
    fn parse_error_response(status: u16, body: &str) -> LlmError {
        match status {
            429 => {
                // Try to extract retry-after from the response
                let retry_after = if body.contains("retry_after") {
                    // Try to parse retry_after from JSON response
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
                        json.get("retry_after")
                            .and_then(|v| v.as_u64())
                            .or_else(|| {
                                json.get("error")
                                    .and_then(|e| e.get("retry_after"))
                                    .and_then(|v| v.as_u64())
                            })
                    } else {
                        None
                    }
                } else {
                    None
                };
                LlmError::RateLimit { retry_after }
            }
            401 | 403 => LlmError::Authentication {
                message: "Invalid API key or insufficient permissions".to_string(),
            },
            400 => {
                if body.contains("model") && (body.contains("not found") || body.contains("invalid")) {
                    LlmError::InvalidModel {
                        model: "unknown".to_string(), // We don't have the model name in context
                    }
                } else {
                    LlmError::RequestFailed {
                        status,
                        message: body.to_string(),
                    }
                }
            }
            _ => LlmError::RequestFailed {
                status,
                message: body.to_string(),
            },
        }
    }
}

#[async_trait]
impl LlmProvider for OpenRouterProvider {
    fn provider_name(&self) -> &str {
        "openrouter"
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, LlmError> {
        let operation = || async {
                let url = format!("{}/models", self.base_url);
                let response = self
                    .client
                    .get(&url)
                    .headers(self.create_headers())
                    .send()
                    .await?;

                if !response.status().is_success() {
                    let status = response.status().as_u16();
                    let body = response.text().await.unwrap_or_default();
                    return Err(Self::parse_error_response(status, &body));
                }

                let body: serde_json::Value = response.json().await
                    .map_err(|e| LlmError::InvalidResponse {
                        message: format!("Failed to parse models response: {}", e),
                    })?;
                
                let data = body["data"].as_array()
                    .ok_or_else(|| LlmError::InvalidResponse {
                        message: "Expected 'data' array in response".to_string(),
                    })?;

                let mut models = Vec::new();
                for model in data {
                    if let Some(id) = model["id"].as_str() {
                        // Extract pricing information if available
                        let pricing = model.get("pricing").and_then(|p| {
                            let prompt = p.get("prompt").and_then(|v| v.as_str())
                                .and_then(|s| s.parse::<f64>().ok());
                            let completion = p.get("completion").and_then(|v| v.as_str())
                                .and_then(|s| s.parse::<f64>().ok());
                            
                            if prompt.is_some() || completion.is_some() {
                                Some(super::ModelPricing { prompt, completion })
                            } else {
                                None
                            }
                        });
                        
                        models.push(ModelInfo {
                            id: id.to_string(),
                            name: model["name"].as_str().unwrap_or(id).to_string(),
                            description: model["description"].as_str().map(|s| s.to_string()),
                            context_length: model["context_length"].as_u64().map(|n| n as u32),
                            max_output_tokens: model["max_completion_tokens"]
                                .as_u64()
                                .or_else(|| model["top_provider"].as_object()
                                    .and_then(|tp| tp["max_completion_tokens"].as_u64()))
                                .map(|n| n as u32),
                            pricing,
                        });
                    }
                }

                Ok(models)
        };
        
        self.execute_with_retry(operation).await
    }

    async fn generate(
        &self,
        messages: &[Message],
        model: &str,
        tools: Option<&[ToolDefinition]>,
        config: Option<&GenerationConfig>,
    ) -> Result<LlmResponse, LlmError> {
        let operation = || async {
                let url = format!("{}/chat/completions", self.base_url);
                
                let mut request_body = serde_json::json!({
                    "model": model,
                    "messages": messages
                });

                // Apply generation configuration
                if let Some(config) = config {
                    if let Some(temp) = config.temperature {
                        request_body["temperature"] = temp.into();
                    }
                    if let Some(max_tokens) = config.max_tokens {
                        request_body["max_tokens"] = max_tokens.into();
                    }
                    if let Some(top_p) = config.top_p {
                        request_body["top_p"] = top_p.into();
                    }
                    if let Some(freq_penalty) = config.frequency_penalty {
                        request_body["frequency_penalty"] = freq_penalty.into();
                    }
                    if let Some(pres_penalty) = config.presence_penalty {
                        request_body["presence_penalty"] = pres_penalty.into();
                    }
                    if let Some(stop) = &config.stop_sequences {
                        request_body["stop"] = serde_json::to_value(stop)?;
                    }
                }

                // Add tools if provided
                if let Some(tools) = tools {
                    request_body["tools"] = serde_json::to_value(tools)?;
                    request_body["tool_choice"] = "auto".into();
                }

                let response = self
                    .client
                    .post(&url)
                    .headers(self.create_headers())
                    .json(&request_body)
                    .send()
                    .await?;

                if !response.status().is_success() {
                    let status = response.status().as_u16();
                    let body = response.text().await.unwrap_or_default();
                    
                    // Special handling for model not found errors
                    if status == 400 && (body.contains("model") && body.contains("not found")) {
                        return Err(LlmError::InvalidModel {
                            model: model.to_string(),
                        });
                    }
                    
                    return Err(Self::parse_error_response(status, &body));
                }

                let body: serde_json::Value = response.json().await
                    .map_err(|e| LlmError::InvalidResponse {
                        message: format!("Failed to parse generation response: {}", e),
                    })?;
                
                // Check for API errors in successful response
                if let Some(error) = body.get("error") {
                    return Err(LlmError::RequestFailed {
                        status: 400,
                        message: error.to_string(),
                    });
                }
                
                let choice = body["choices"][0].as_object()
                    .ok_or_else(|| LlmError::InvalidResponse {
                        message: "No choices in response".to_string(),
                    })?;

                let message = &choice["message"];
                let content = message["content"].as_str().map(|s| s.to_string());
                
                let tool_calls = if let Some(calls) = message["tool_calls"].as_array() {
                    let parsed_calls: Result<Vec<ToolCall>, _> = serde_json::from_value(serde_json::Value::Array(calls.clone()));
                    Some(parsed_calls.map_err(|e| LlmError::InvalidResponse {
                        message: format!("Failed to parse tool calls: {}", e),
                    })?)
                } else {
                    None
                };

                let finish_reason = choice["finish_reason"].as_str().unwrap_or("unknown").to_string();

                let usage = body["usage"].as_object().map(|u| TokenUsage {
                    prompt_tokens: u["prompt_tokens"].as_u64().unwrap_or(0) as u32,
                    completion_tokens: u["completion_tokens"].as_u64().unwrap_or(0) as u32,
                    total_tokens: u["total_tokens"].as_u64().unwrap_or(0) as u32,
                });

                Ok(LlmResponse {
                    content,
                    tool_calls,
                    finish_reason,
                    usage,
                })
        };
        
        self.execute_with_retry(operation).await
    }

    async fn generate_plan(
        &self,
        prompt: &str,
        context: &str,
        model: &str,
    ) -> Result<crate::planning::Plan, LlmError> {
        // Use the structured prompt template for plan generation
        let template = super::prompts::PromptTemplates::plan_generation();
        let prompt_context = super::prompts::PromptContext::new()
            .with_variable("context", context)
            .with_variable("request", prompt);
        
        let (system_message, user_message) = template.fill(&prompt_context)
            .map_err(|e| LlmError::InvalidResponse {
                message: format!("Failed to fill prompt template: {}", e),
            })?;

        let messages = vec![
            Message {
                role: super::MessageRole::System,
                content: system_message,
                tool_calls: None,
                tool_call_id: None,
            },
            Message {
                role: super::MessageRole::User,
                content: user_message,
                tool_calls: None,
                tool_call_id: None,
            },
        ];

        let response = self.generate(&messages, model, None, None).await?;
        
        let content = response.content
            .ok_or_else(|| LlmError::InvalidResponse {
                message: "No content in planning response".to_string(),
            })?;

        // Parse the JSON response into a Plan
        let plan_json: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| LlmError::InvalidResponse {
                message: format!("Failed to parse plan JSON: {}", e),
            })?;

        crate::planning::Plan::from_json(&plan_json)
            .map_err(|e| LlmError::InvalidResponse {
                message: format!("Invalid plan structure: {}", e),
            })
    }

    async fn generate_content(
        &self,
        prompt: &str,
        context: &str,
        model: &str,
        config: Option<&GenerationConfig>,
    ) -> Result<String, LlmError> {
        let messages = vec![
            Message {
                role: super::MessageRole::User,
                content: format!("Context:\n{}\n\nRequest:\n{}", context, prompt),
                tool_calls: None,
                tool_call_id: None,
            },
        ];

        let response = self.generate(&messages, model, None, config).await?;
        
        response.content
            .ok_or_else(|| LlmError::InvalidResponse {
                message: "No content in response".to_string(),
            })
    }

    async fn validate_model(&self, model: &str) -> Result<ModelInfo, LlmError> {
        let models = self.list_models().await?;
        models
            .into_iter()
            .find(|m| m.id == model)
            .ok_or_else(|| LlmError::InvalidModel {
                model: model.to_string(),
            })
    }
}