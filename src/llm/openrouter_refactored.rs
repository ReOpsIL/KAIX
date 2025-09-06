//! Refactored OpenRouter LLM provider implementation using shared utilities

use super::{
    FunctionDefinition, GenerationConfig, LlmError, LlmProvider, LlmResponse, Message,
    ModelInfo, TokenUsage, ToolCall, ToolDefinition, TaskAnalysis, TaskExecutionResult, TaskRefinementContext,
};
use crate::utils::http::{HttpClient, HttpClientConfig, execute_with_retry, parse_http_error};
use crate::utils::http::headers::{OpenRouterHeaders, ProviderHeaders};
use crate::utils::templates::handlers::{TemplateHandler, StandardTemplateHandler};
use crate::utils::errors::KaiError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// OpenRouter API provider with shared utilities
pub struct OpenRouterProvider {
    http_client: HttpClient,
    api_key: String,
    base_url: String,
    template_handler: StandardTemplateHandler,
    headers_provider: OpenRouterHeaders,
}

impl OpenRouterProvider {
    /// Create a new OpenRouter provider
    pub fn new(api_key: String) -> Result<Self, LlmError> {
        let config = HttpClientConfig::default();
        let http_client = HttpClient::new(config)?;
        
        Ok(Self {
            http_client,
            api_key,
            base_url: "https://openrouter.ai/api/v1".to_string(),
            template_handler: StandardTemplateHandler,
            headers_provider: OpenRouterHeaders::new(),
        })
    }

    /// Create a new OpenRouter provider with custom configuration
    pub fn with_config(
        api_key: String,
        base_url: Option<String>,
        client_config: Option<HttpClientConfig>,
    ) -> Result<Self, LlmError> {
        let config = client_config.unwrap_or_default();
        let http_client = HttpClient::new(config)?;
        
        Ok(Self {
            http_client,
            api_key,
            base_url: base_url.unwrap_or_else(|| "https://openrouter.ai/api/v1".to_string()),
            template_handler: StandardTemplateHandler,
            headers_provider: OpenRouterHeaders::new(),
        })
    }

    /// Execute HTTP request with retry logic
    async fn execute_with_retry<F, T>(&self, operation: F) -> Result<T, LlmError>
    where
        F: Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T, LlmError>> + Send>> + Send + Sync,
    {
        let retry_config = self.http_client.retry_config();
        execute_with_retry(|| operation(), &retry_config).await
    }
}

#[async_trait]
impl LlmProvider for OpenRouterProvider {
    fn provider_name(&self) -> &str {
        "openrouter"
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, LlmError> {
        let operation = || {
            let client = self.http_client.client();
            let headers = self.headers_provider.create_headers(&self.api_key);
            let url = format!("{}/models", self.base_url);
            
            Box::pin(async move {
                let response = client
                    .get(&url)
                    .headers(headers)
                    .send()
                    .await?;

                if !response.status().is_success() {
                    let status = response.status().as_u16();
                    let body = response.text().await.unwrap_or_default();
                    return Err(parse_http_error(status, &body, None));
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
            })
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
        let operation = || {
            let client = self.http_client.client();
            let headers = self.headers_provider.create_headers(&self.api_key);
            let url = format!("{}/chat/completions", self.base_url);
            let messages = messages.to_vec();
            let tools = tools.map(|t| t.to_vec());
            let config = config.cloned();
            
            Box::pin(async move {
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
                        request_body["stop"] = serde_json::to_value(stop).unwrap_or(serde_json::Value::Null);
                    }
                }

                // Add tools if provided
                if let Some(tools) = tools {
                    request_body["tools"] = serde_json::to_value(tools).unwrap_or(serde_json::Value::Null);
                    request_body["tool_choice"] = "auto".into();
                }

                let response = client
                    .post(&url)
                    .headers(headers)
                    .json(&request_body)
                    .send()
                    .await?;

                if !response.status().is_success() {
                    let status = response.status().as_u16();
                    let body = response.text().await.unwrap_or_default();
                    return Err(parse_http_error(status, &body, Some(model)));
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
        // Use shared template handler
        let messages = self.template_handler.plan_generation_messages(prompt, context)?;

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

    async fn refine_task_for_execution(
        &self,
        task: &crate::planning::Task,
        context: &TaskRefinementContext,
        model: &str,
    ) -> Result<String, LlmError> {
        // Use shared template handler
        let messages = self.template_handler.task_refinement_messages(
            task,
            &context.plan_description,
            &context.global_context,
            &context.plan_context,
            &context.dependency_outputs,
        )?;

        // Use focused configuration for refinement
        let config = GenerationConfig {
            temperature: Some(0.3), // Lower temperature for focused refinement
            max_tokens: Some(4000),
            top_p: Some(0.9),
            frequency_penalty: None,
            presence_penalty: None,
            stop_sequences: None,
        };

        let response = self.generate(&messages, model, None, Some(&config)).await?;
        
        response.content
            .ok_or_else(|| LlmError::InvalidResponse {
                message: "No content in task refinement response".to_string(),
            })
    }

    async fn analyze_task_result(
        &self,
        task: &crate::planning::Task,
        execution_result: &TaskExecutionResult,
        expected_outcome: &str,
        model: &str,
    ) -> Result<TaskAnalysis, LlmError> {
        // Use shared template handler
        let messages = self.template_handler.execution_analysis_messages(
            task,
            execution_result,
            expected_outcome,
        )?;

        // Use lower temperature for consistent analysis
        let config = GenerationConfig {
            temperature: Some(0.1),
            max_tokens: Some(2048),
            top_p: Some(0.8),
            frequency_penalty: None,
            presence_penalty: None,
            stop_sequences: None,
        };

        let response = self.generate(&messages, model, None, Some(&config)).await?;
        
        let content = response.content
            .ok_or_else(|| LlmError::InvalidResponse {
                message: "No content in task analysis response".to_string(),
            })?;

        // Use shared template handler for parsing
        self.template_handler.parse_task_analysis(&content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_provider_creation() {
        let provider = OpenRouterProvider::new("test-api-key".to_string());
        assert!(provider.is_ok());
        
        let provider = provider.unwrap();
        assert_eq!(provider.provider_name(), "openrouter");
    }

    #[test]
    fn test_provider_with_config() {
        let config = HttpClientConfig {
            timeout: Duration::from_secs(60),
            retry_attempts: 2,
            retry_delay: Duration::from_millis(500),
            user_agent: Some("Test/1.0".to_string()),
            max_redirects: Some(5),
        };
        
        let provider = OpenRouterProvider::with_config(
            "test-api-key".to_string(),
            Some("https://custom.api.com".to_string()),
            Some(config),
        );
        
        assert!(provider.is_ok());
        let provider = provider.unwrap();
        assert_eq!(provider.base_url, "https://custom.api.com");
    }
}