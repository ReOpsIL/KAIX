//! Google Gemini LLM provider implementation

use super::{
    FunctionDefinition, GenerationConfig, LlmError, LlmProvider, LlmResponse, Message,
    MessageRole, ModelInfo, ModelPricing, TokenUsage, ToolCall, ToolDefinition,
    TaskAnalysis, TaskExecutionResult, TaskRefinementContext,
};
use async_trait::async_trait;
use futures::future::BoxFuture;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Google Gemini API provider
pub struct GeminiProvider {
    client: Client,
    api_key: String,
    base_url: String,
    retry_attempts: usize,
    retry_delay: Duration,
}

/// Gemini-specific request structures
#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<GeminiTool>>,
    #[serde(rename = "generationConfig", skip_serializing_if = "Option::is_none")]
    generation_config: Option<GeminiGenerationConfig>,
    #[serde(rename = "systemInstruction", skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiContent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum GeminiPart {
    Text { text: String },
    FunctionCall {
        #[serde(rename = "functionCall")]
        function_call: GeminiFunctionCall,
    },
    FunctionResponse {
        #[serde(rename = "functionResponse")]
        function_response: GeminiFunctionResponse,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiFunctionCall {
    name: String,
    args: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiFunctionResponse {
    name: String,
    response: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiTool {
    #[serde(rename = "functionDeclarations")]
    function_declarations: Vec<GeminiFunctionDeclaration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiFunctionDeclaration {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeminiGenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(rename = "maxOutputTokens", skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
    #[serde(rename = "topP", skip_serializing_if = "Option::is_none")]
    top_p: Option<f32>,
    #[serde(rename = "stopSequences", skip_serializing_if = "Option::is_none")]
    stop_sequences: Option<Vec<String>>,
}

/// Gemini API response structures
#[derive(Debug, Clone, Deserialize)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
    #[serde(rename = "usageMetadata", skip_serializing_if = "Option::is_none")]
    usage_metadata: Option<GeminiUsageMetadata>,
}

#[derive(Debug, Clone, Deserialize)]
struct GeminiCandidate {
    content: GeminiContent,
    #[serde(rename = "finishReason", skip_serializing_if = "Option::is_none")]
    finish_reason: Option<String>,
    index: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
struct GeminiUsageMetadata {
    #[serde(rename = "promptTokenCount")]
    prompt_token_count: u32,
    #[serde(rename = "candidatesTokenCount")]
    candidates_token_count: u32,
    #[serde(rename = "totalTokenCount")]
    total_token_count: u32,
}

/// Models response structure
#[derive(Debug, Clone, Deserialize)]
struct GeminiModelsResponse {
    models: Vec<GeminiModelInfo>,
}

#[derive(Debug, Clone, Deserialize)]
struct GeminiModelInfo {
    name: String,
    #[serde(rename = "displayName")]
    display_name: String,
    description: Option<String>,
    #[serde(rename = "inputTokenLimit")]
    input_token_limit: Option<u32>,
    #[serde(rename = "outputTokenLimit")]
    output_token_limit: Option<u32>,
    #[serde(rename = "supportedGenerationMethods")]
    supported_generation_methods: Option<Vec<String>>,
}

impl GeminiProvider {
    /// Create a new Gemini provider
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(120))
                .build()
                .unwrap_or_else(|_| Client::new()),
            api_key,
            base_url: "https://generativelanguage.googleapis.com/v1beta".to_string(),
            retry_attempts: 3,
            retry_delay: Duration::from_millis(1000),
        }
    }

    /// Create a new Gemini provider with custom configuration
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
            base_url: base_url.unwrap_or_else(|| "https://generativelanguage.googleapis.com/v1beta".to_string()),
            retry_attempts: retry_attempts.unwrap_or(3),
            retry_delay: retry_delay.unwrap_or_else(|| Duration::from_millis(1000)),
        }
    }

    /// Convert our Message format to Gemini's format
    fn convert_messages(
        messages: &[Message],
    ) -> Result<(Option<GeminiContent>, Vec<GeminiContent>), LlmError> {
        let mut system_instruction = None;
        let mut contents = Vec::new();

        for message in messages {
            match message.role {
                MessageRole::System => {
                    system_instruction = Some(GeminiContent {
                        parts: vec![GeminiPart::Text {
                            text: message.content.clone(),
                        }],
                        role: None,
                    });
                }
                MessageRole::User => {
                    contents.push(GeminiContent {
                        parts: vec![GeminiPart::Text {
                            text: message.content.clone(),
                        }],
                        role: Some("user".to_string()),
                    });
                }
                MessageRole::Assistant => {
                    let mut parts = Vec::new();
                    
                    if !message.content.is_empty() {
                        parts.push(GeminiPart::Text {
                            text: message.content.clone(),
                        });
                    }
                    
                    // Add function calls if present
                    if let Some(tool_calls) = &message.tool_calls {
                        for tool_call in tool_calls {
                            parts.push(GeminiPart::FunctionCall {
                                function_call: GeminiFunctionCall {
                                    name: tool_call.function.name.clone(),
                                    args: tool_call.function.arguments.clone(),
                                },
                            });
                        }
                    }
                    
                    contents.push(GeminiContent {
                        parts,
                        role: Some("model".to_string()),
                    });
                }
                MessageRole::Tool => {
                    // Tool responses in Gemini are function responses
                    let tool_name = message.tool_call_id
                        .as_ref()
                        .ok_or_else(|| LlmError::InvalidResponse {
                            message: "Tool message missing tool_call_id".to_string(),
                        })?;
                    
                    contents.push(GeminiContent {
                        parts: vec![GeminiPart::FunctionResponse {
                            function_response: GeminiFunctionResponse {
                                name: tool_name.clone(),
                                response: serde_json::json!({
                                    "result": message.content
                                }),
                            },
                        }],
                        role: Some("user".to_string()),
                    });
                }
            }
        }

        Ok((system_instruction, contents))
    }

    /// Convert tool definitions to Gemini format
    fn convert_tools(tools: &[ToolDefinition]) -> Result<Vec<GeminiTool>, LlmError> {
        let function_declarations = tools
            .iter()
            .map(|tool| GeminiFunctionDeclaration {
                name: tool.function.name.clone(),
                description: tool.function.description.clone(),
                parameters: tool.function.parameters.clone(),
            })
            .collect();

        Ok(vec![GeminiTool {
            function_declarations,
        }])
    }

    /// Convert generation config to Gemini format
    fn convert_generation_config(config: &GenerationConfig) -> GeminiGenerationConfig {
        GeminiGenerationConfig {
            temperature: config.temperature,
            max_output_tokens: config.max_tokens,
            top_p: config.top_p,
            stop_sequences: config.stop_sequences.clone(),
        }
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
                                LlmError::RateLimit { .. } => {
                                    // Wait longer for rate limits
                                    tokio::time::sleep(self.retry_delay * (attempt as u32 + 1) * 2).await;
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
}

#[async_trait]
impl LlmProvider for GeminiProvider {
    fn provider_name(&self) -> &str {
        "gemini"
    }

    async fn list_models(&self) -> Result<Vec<ModelInfo>, LlmError> {
        let operation = || async {
                let url = format!(
                    "{}/models?key={}",
                    self.base_url, self.api_key
                );

                let response = self.client.get(&url).send().await?;

                if !response.status().is_success() {
                    let status = response.status().as_u16();
                    let text = response.text().await.unwrap_or_default();
                    
                    return match status {
                        429 => Err(LlmError::RateLimit { retry_after: Some(60) }),
                        401 | 403 => Err(LlmError::Authentication {
                            message: "Invalid API key".to_string(),
                        }),
                        _ => Err(LlmError::RequestFailed {
                            status,
                            message: text,
                        }),
                    };
                }

                let models_response: GeminiModelsResponse = response.json().await
                    .map_err(|e| LlmError::InvalidResponse {
                        message: format!("Failed to parse models response: {}", e),
                    })?;

                let mut models = Vec::new();
                for model in models_response.models {
                    // Extract model ID from the full name (e.g., "models/gemini-pro" -> "gemini-pro")
                    let model_id = model.name
                        .strip_prefix("models/")
                        .unwrap_or(&model.name)
                        .to_string();

                    // Only include models that support generateContent
                    if model.supported_generation_methods
                        .as_ref()
                        .map(|methods| methods.contains(&"generateContent".to_string()))
                        .unwrap_or(false)
                    {
                        models.push(ModelInfo {
                            id: model_id,
                            name: model.display_name,
                            description: model.description,
                            context_length: model.input_token_limit,
                            max_output_tokens: model.output_token_limit,
                            pricing: None, // Gemini doesn't provide pricing in model info
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
                let (system_instruction, contents) = Self::convert_messages(messages)?;
                
                let mut request = GeminiRequest {
                    contents,
                    tools: None,
                    generation_config: config.map(Self::convert_generation_config),
                    system_instruction,
                };

                if let Some(tools) = tools {
                    request.tools = Some(Self::convert_tools(tools)?);
                }

                let url = format!(
                    "{}/models/{}:generateContent?key={}",
                    self.base_url, model, self.api_key
                );

                let response = self
                    .client
                    .post(&url)
                    .header("Content-Type", "application/json")
                    .json(&request)
                    .send()
                    .await?;

                if !response.status().is_success() {
                    let status = response.status().as_u16();
                    let text = response.text().await.unwrap_or_default();
                    
                    return match status {
                        429 => Err(LlmError::RateLimit { retry_after: Some(60) }),
                        401 | 403 => Err(LlmError::Authentication {
                            message: "Invalid API key".to_string(),
                        }),
                        400 => {
                            if text.contains("model not found") || text.contains("model does not exist") {
                                Err(LlmError::InvalidModel {
                                    model: model.to_string(),
                                })
                            } else {
                                Err(LlmError::RequestFailed { status, message: text })
                            }
                        }
                        _ => Err(LlmError::RequestFailed { status, message: text }),
                    };
                }

                let gemini_response: GeminiResponse = response.json().await
                    .map_err(|e| LlmError::InvalidResponse {
                        message: format!("Failed to parse generation response: {}", e),
                    })?;

                // Extract the first candidate
                let candidate = gemini_response.candidates
                    .first()
                    .ok_or_else(|| LlmError::InvalidResponse {
                        message: "No candidates in response".to_string(),
                    })?;

                // Process the response parts
                let mut content_text = String::new();
                let mut tool_calls = Vec::new();

                for part in &candidate.content.parts {
                    match part {
                        GeminiPart::Text { text } => {
                            if !content_text.is_empty() {
                                content_text.push(' ');
                            }
                            content_text.push_str(text);
                        }
                        GeminiPart::FunctionCall { function_call } => {
                            tool_calls.push(ToolCall {
                                id: format!("call_{}", uuid::Uuid::new_v4()),
                                r#type: "function".to_string(),
                                function: crate::llm::FunctionCall {
                                    name: function_call.name.clone(),
                                    arguments: function_call.args.clone(),
                                },
                            });
                        }
                        GeminiPart::FunctionResponse { .. } => {
                            // Function responses shouldn't appear in model outputs
                            // They're used for inputs when continuing conversations
                        }
                    }
                }

                let usage = gemini_response.usage_metadata.map(|usage| TokenUsage {
                    prompt_tokens: usage.prompt_token_count,
                    completion_tokens: usage.candidates_token_count,
                    total_tokens: usage.total_token_count,
                });

                Ok(LlmResponse {
                    content: if content_text.is_empty() { None } else { Some(content_text) },
                    tool_calls: if tool_calls.is_empty() { None } else { Some(tool_calls) },
                    finish_reason: candidate.finish_reason
                        .clone()
                        .unwrap_or_else(|| "stop".to_string()),
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
        let system_message = r#"
You are a task planning AI. Generate a structured plan to accomplish the user's request.
Output your response as a JSON object with this exact structure:
{
    "description": "Brief description of what the plan accomplishes",
    "tasks": [
        {
            "id": "unique_task_id",
            "description": "Human readable task description",
            "task_type": "read_file|write_file|execute_command|generate_content|analyze_code|list_files|create_directory|delete",
            "parameters": {
                // Task-specific parameters as key-value pairs
            },
            "dependencies": ["task_id_1", "task_id_2"]
        }
    ]
}

Available task types:
- read_file: Read content from a file (parameters: {"path": "file/path"})
- write_file: Write content to a file (parameters: {"path": "file/path", "content": "..."})
- execute_command: Run a shell command (parameters: {"command": "...", "args": [...]})
- generate_content: Generate code/text (parameters: {"prompt": "...", "output_file": "optional"})
- analyze_code: Analyze existing code (parameters: {"path": "file/path", "focus": "what to analyze"})
- list_files: List files in a directory (parameters: {"path": "directory/path", "pattern": "optional_glob"})
- create_directory: Create a directory (parameters: {"path": "directory/path"})
- delete: Delete a file or directory (parameters: {"path": "file/directory/path"})

Ensure all tasks have unique IDs and proper dependencies.
"#;

        let messages = vec![
            Message {
                role: MessageRole::System,
                content: system_message.to_string(),
                tool_calls: None,
                tool_call_id: None,
            },
            Message {
                role: MessageRole::User,
                content: format!("Context:\n{}\n\nRequest:\n{}", context, prompt),
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
                role: MessageRole::User,
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
        // Use the task refinement prompt template
        let template = super::prompts::PromptTemplates::task_refinement();
        let prompt_context = super::prompts::PromptContext::new()
            .with_variable("plan_description", &context.plan_description)
            .with_variable("task_id", &task.id)
            .with_variable("task_type", &format!("{:?}", task.task_type))
            .with_variable("task_description", &task.description)
            .with_variable("task_parameters", &serde_json::to_string_pretty(&task.parameters).unwrap_or_default())
            .with_variable("global_context", &context.global_context)
            .with_variable("plan_context", &context.plan_context)
            .with_variable("dependency_outputs", &serde_json::to_string_pretty(&context.dependency_outputs).unwrap_or_default());
        
        let (system_message, user_message) = template.fill(&prompt_context)
            .map_err(|e| LlmError::InvalidResponse {
                message: format!("Failed to fill task refinement template: {}", e),
            })?;

        let messages = vec![
            Message {
                role: MessageRole::System,
                content: system_message,
                tool_calls: None,
                tool_call_id: None,
            },
            Message {
                role: MessageRole::User,
                content: user_message,
                tool_calls: None,
                tool_call_id: None,
            },
        ];

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
        // Use the execution analysis prompt template
        let template = super::prompts::PromptTemplates::execution_analysis();
        let prompt_context = super::prompts::PromptContext::new()
            .with_variable("task_id", &task.id)
            .with_variable("task_type", &format!("{:?}", task.task_type))
            .with_variable("task_description", &task.description)
            .with_variable("expected_outcome", expected_outcome)
            .with_variable("task_parameters", &serde_json::to_string_pretty(&task.parameters).unwrap_or_default())
            .with_variable("success", &execution_result.success.to_string())
            .with_variable("exit_code", &execution_result.exit_code.map(|c| c.to_string()).unwrap_or_else(|| "N/A".to_string()))
            .with_variable("execution_time_ms", &execution_result.execution_time_ms.to_string())
            .with_variable("stdout", execution_result.stdout.as_deref().unwrap_or(""))
            .with_variable("stderr", execution_result.stderr.as_deref().unwrap_or(""))
            .with_variable("output_data", &serde_json::to_string_pretty(&execution_result.output).unwrap_or_else(|_| "null".to_string()))
            .with_variable("error_message", execution_result.error.as_deref().unwrap_or(""))
            .with_variable("plan_description", "")
            .with_variable("plan_context", "")
            .with_variable("task_dependencies", &serde_json::to_string(&task.dependencies).unwrap_or_default());
        
        let (system_message, user_message) = template.fill(&prompt_context)
            .map_err(|e| LlmError::InvalidResponse {
                message: format!("Failed to fill execution analysis template: {}", e),
            })?;

        let messages = vec![
            Message {
                role: MessageRole::System,
                content: system_message,
                tool_calls: None,
                tool_call_id: None,
            },
            Message {
                role: MessageRole::User,
                content: user_message,
                tool_calls: None,
                tool_call_id: None,
            },
        ];

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

        // Parse the JSON response into TaskAnalysis
        let analysis_json: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| LlmError::InvalidResponse {
                message: format!("Failed to parse task analysis JSON: {}. Content: {}", e, content),
            })?;

        // Extract fields with robust defaults
        let success = analysis_json["success"].as_bool().unwrap_or(false);
        let summary = analysis_json["summary"].as_str().unwrap_or("Analysis unavailable").to_string();
        let details = analysis_json["details"].as_str().unwrap_or("").to_string();
        let extracted_data = analysis_json.get("extracted_data").cloned();
        let next_steps = analysis_json["next_steps"].as_array()
            .map(|arr| arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<String>>());
        let context_updates = analysis_json["context_updates"].as_object()
            .map(|obj| obj.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect());
        let modified_files = analysis_json["modified_files"].as_array()
            .map(|arr| arr.iter()
                .filter_map(|v| v.as_str().map(|s| PathBuf::from(s)))
                .collect::<Vec<PathBuf>>());
        let error = if !success {
            Some(analysis_json["error"].as_str().unwrap_or("Task failed").to_string())
        } else {
            None
        };
        let metadata = analysis_json["metadata"].as_object()
            .map(|obj| obj.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect())
            .unwrap_or_default();

        Ok(TaskAnalysis {
            success,
            summary,
            details,
            extracted_data,
            next_steps,
            context_updates,
            modified_files,
            error,
            metadata,
        })
    }
}