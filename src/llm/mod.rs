//! LLM provider abstraction and implementations

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

pub mod openrouter;
pub mod gemini;
pub mod prompts;
pub mod streaming;
pub mod utils;

#[cfg(test)]
pub mod examples;

// Re-export commonly used types for convenience
pub use prompts::{PromptContext, PromptTemplate, PromptTemplates};
pub use streaming::{LlmStream, StreamChunk, StreamCollector, StreamingLlmProvider};
pub use utils::{CostBreakdown, CostEstimator, TokenCounter, UsageTracker};

// Re-export agentic loop types (defined below)
// pub use {TaskAnalysis, TaskExecutionResult, TaskRefinementContext};

/// Error types for LLM operations
#[derive(Error, Debug)]
pub enum LlmError {
    #[error("Authentication failed: {message}")]
    Authentication { message: String },

    #[error("Rate limit exceeded: {retry_after:?}")]
    RateLimit { retry_after: Option<u64> },

    #[error("Invalid model: {model}")]
    InvalidModel { model: String },

    #[error("Request failed: {status}: {message}")]
    RequestFailed { status: u16, message: String },

    #[error("Invalid response format: {message}")]
    InvalidResponse { message: String },

    #[error("Tool execution error: {message}")]
    ToolExecution { message: String },

    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Unknown error: {message}")]
    Unknown { message: String },
}

/// Represents a message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
}

/// Role of a message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

/// Represents a tool call request from the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String, // Usually "function"
    pub function: FunctionCall,
}

/// Function call details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Available tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub r#type: String, // Usually "function"
    pub function: FunctionDefinition,
}

/// Function definition for tools
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Configuration for LLM generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationConfig {
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f32>,
    pub frequency_penalty: Option<f32>,
    pub presence_penalty: Option<f32>,
    pub stop_sequences: Option<Vec<String>>,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            temperature: Some(0.7),
            max_tokens: Some(2048),
            top_p: Some(0.9),
            frequency_penalty: None,
            presence_penalty: None,
            stop_sequences: None,
        }
    }
}

/// Response from LLM generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub finish_reason: String,
    pub usage: Option<TokenUsage>,
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Raw execution result from task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskExecutionResult {
    /// Whether the task succeeded at the system level
    pub success: bool,
    /// Standard output from the task
    pub stdout: Option<String>,
    /// Standard error from the task
    pub stderr: Option<String>,
    /// Exit code for command tasks
    pub exit_code: Option<i32>,
    /// Raw output data
    pub output: Option<serde_json::Value>,
    /// Error message if task failed
    pub error: Option<String>,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Analyzed task result after LLM analysis
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAnalysis {
    /// Whether the task achieved its intended outcome
    pub success: bool,
    /// Brief summary of what happened
    pub summary: String,
    /// Detailed analysis of the results
    pub details: String,
    /// Key information extracted from execution
    pub extracted_data: Option<serde_json::Value>,
    /// Suggested next steps if any
    pub next_steps: Option<Vec<String>>,
    /// Information to add to plan context
    pub context_updates: Option<HashMap<String, serde_json::Value>>,
    /// Files that were modified during execution
    pub modified_files: Option<Vec<PathBuf>>,
    /// Error message if analysis indicates failure
    pub error: Option<String>,
    /// Additional metadata from analysis
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Context for task refinement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRefinementContext {
    /// Global project context summary
    pub global_context: String,
    /// Current plan context
    pub plan_context: String,
    /// Outputs from dependent tasks
    pub dependency_outputs: HashMap<String, serde_json::Value>,
    /// Plan description for context
    pub plan_description: String,
}

/// Model information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub context_length: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub pricing: Option<ModelPricing>,
}

/// Model pricing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    pub prompt: Option<f64>, // per 1M tokens
    pub completion: Option<f64>, // per 1M tokens
}

/// The main LLM provider trait that all implementations must implement
/// 
/// This trait provides a common interface for interacting with different LLM providers
/// such as OpenRouter, Gemini, OpenAI, etc. It abstracts away provider-specific details
/// and provides a consistent API for generating content, creating plans, and managing models.
/// 
/// # Example
/// 
/// ```rust,no_run
/// use kai_x::llm::{LlmProvider, Message, MessageRole};
/// use std::collections::HashMap;
/// 
/// async fn example_usage(provider: &dyn LlmProvider) -> Result<(), Box<dyn std::error::Error>> {
///     // List available models
///     let models = provider.list_models().await?;
///     println!("Available models: {:?}", models);
///     
///     // Generate content
///     let messages = vec![Message {
///         role: MessageRole::User,
///         content: "Hello, how are you?".to_string(),
///         tool_calls: None,
///         tool_call_id: None,
///     }];
///     
///     let response = provider.generate(
///         &messages,
///         "gpt-3.5-turbo",
///         None, // No tools
///         None, // Default config
///     ).await?;
///     
///     println!("Response: {:?}", response.content);
///     Ok(())
/// }
/// ```
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Get the provider name (e.g., "openrouter", "gemini")
    fn provider_name(&self) -> &str;

    /// List available models
    async fn list_models(&self) -> Result<Vec<ModelInfo>, LlmError>;

    /// Generate content with optional tool use
    async fn generate(
        &self,
        messages: &[Message],
        model: &str,
        tools: Option<&[ToolDefinition]>,
        config: Option<&GenerationConfig>,
    ) -> Result<LlmResponse, LlmError>;

    /// Generate a plan from a user prompt and context
    async fn generate_plan(
        &self,
        prompt: &str,
        context: &str,
        model: &str,
    ) -> Result<crate::planning::Plan, LlmError>;

    /// Generate content for a specific task
    async fn generate_content(
        &self,
        prompt: &str,
        context: &str,
        model: &str,
        config: Option<&GenerationConfig>,
    ) -> Result<String, LlmError>;

    /// Refine a high-level task into concrete execution instructions
    /// 
    /// Takes a task with its context and dependency outputs, and generates
    /// precise, executable instructions. This is used in the agentic loop
    /// before task execution to transform abstract tasks into concrete actions.
    /// 
    /// # Arguments
    /// * `task` - The task to refine
    /// * `context` - Context including global and plan context plus dependencies
    /// * `model` - The model to use for refinement
    /// 
    /// # Returns
    /// Concrete, executable instruction as a string
    async fn refine_task_for_execution(
        &self,
        task: &crate::planning::Task,
        context: &TaskRefinementContext,
        model: &str,
    ) -> Result<String, LlmError>;

    /// Analyze raw task execution results and extract insights
    /// 
    /// Takes the raw results from task execution and performs intelligent
    /// analysis to determine success/failure, extract key information,
    /// and provide structured feedback. This is used in the agentic loop
    /// after task execution to update context and plan state.
    /// 
    /// # Arguments
    /// * `task` - The original task that was executed
    /// * `execution_result` - Raw results from task execution
    /// * `expected_outcome` - What the task was supposed to achieve
    /// * `model` - The model to use for analysis
    /// 
    /// # Returns
    /// Structured analysis with success determination and extracted data
    async fn analyze_task_result(
        &self,
        task: &crate::planning::Task,
        execution_result: &TaskExecutionResult,
        expected_outcome: &str,
        model: &str,
    ) -> Result<TaskAnalysis, LlmError>;

    /// Validate that a model is available
    async fn validate_model(&self, model: &str) -> Result<ModelInfo, LlmError>;
}

/// Factory for creating LLM providers
pub struct LlmProviderFactory;

impl LlmProviderFactory {
    /// Create a provider by name
    pub fn create_provider(
        provider_name: &str,
        config: HashMap<String, String>,
    ) -> Result<Box<dyn LlmProvider>, LlmError> {
        match provider_name.to_lowercase().as_str() {
            "openrouter" => {
                let api_key = config.get("api_key")
                    .ok_or_else(|| LlmError::Authentication {
                        message: "OpenRouter API key not provided".to_string(),
                    })?;
                
                Ok(Box::new(openrouter::OpenRouterProvider::new(api_key.clone())))
            }
            "gemini" => {
                let api_key = config.get("api_key")
                    .ok_or_else(|| LlmError::Authentication {
                        message: "Gemini API key not provided".to_string(),
                    })?;
                
                Ok(Box::new(gemini::GeminiProvider::new(api_key.clone())))
            }
            _ => Err(LlmError::Unknown {
                message: format!("Unknown provider: {}", provider_name),
            }),
        }
    }

    /// List all available provider names
    pub fn list_providers() -> Vec<&'static str> {
        vec!["openrouter", "gemini"]
    }
}