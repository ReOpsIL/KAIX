//! Shared template utilities
//! 
//! This module provides common template handling utilities used across LLM providers,
//! eliminating duplication in template context building and filling patterns.

use crate::llm::{LlmError, Message, MessageRole};
use crate::llm::prompts::{PromptContext, PromptTemplate, PromptTemplates};
use crate::planning::Task;
use std::collections::HashMap;

pub mod builders;
pub mod handlers;

pub use builders::{TemplateContextBuilder, MessageBuilder, LlmMessageBuilder};
pub use handlers::{TemplateHandler, StandardTemplateHandler};

/// Common template operations for LLM providers
pub trait LlmTemplateHandler {
    /// Fill template and create messages for plan generation
    fn create_plan_messages(
        &self,
        prompt: &str,
        context: &str,
        additional_vars: Option<&HashMap<String, String>>,
    ) -> Result<Vec<Message>, LlmError>;
    
    /// Fill template and create messages for task refinement
    fn create_task_refinement_messages(
        &self,
        task: &Task,
        plan_description: &str,
        global_context: &str,
        plan_context: &str,
        dependency_outputs: &HashMap<String, serde_json::Value>,
    ) -> Result<Vec<Message>, LlmError>;
    
    /// Fill template and create messages for execution analysis
    fn create_execution_analysis_messages(
        &self,
        task: &Task,
        execution_result: &crate::llm::TaskExecutionResult,
        expected_outcome: &str,
    ) -> Result<Vec<Message>, LlmError>;
    
    /// Create messages for content generation
    fn create_content_generation_messages(
        &self,
        prompt: &str,
        context: &str,
        _config: Option<&GenerationConfig>,
    ) -> Result<Vec<Message>, LlmError>;
}

/// Configuration for content generation templates
pub struct GenerationConfig {
    pub content_type: Option<String>,
    pub language: Option<String>,
    pub style_guidelines: Option<String>,
    pub output_format: Option<String>,
}

impl Default for GenerationConfig {
    fn default() -> Self {
        Self {
            content_type: None,
            language: None,
            style_guidelines: None,
            output_format: None,
        }
    }
}

/// Standard implementation of LLM template handling
pub struct StandardLlmTemplateHandler;

impl LlmTemplateHandler for StandardLlmTemplateHandler {
    fn create_plan_messages(
        &self,
        prompt: &str,
        context: &str,
        additional_vars: Option<&HashMap<String, String>>,
    ) -> Result<Vec<Message>, LlmError> {
        let template = PromptTemplates::plan_generation();
        
        let mut prompt_context = PromptContext::new()
            .with_variable("context", context)
            .with_variable("request", prompt)
            .with_variable("working_directory", std::env::current_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "unknown".to_string()))
            .with_variable("project_type", "unknown")
            .with_variable("current_state", "unknown");
        
        // Add any additional variables provided
        if let Some(vars) = additional_vars {
            for (key, value) in vars {
                prompt_context.set_variable(key, value);
            }
        }
        
        let (system_message, user_message) = template.fill(&prompt_context)
            .map_err(|e| LlmError::InvalidResponse {
                message: format!("Failed to fill plan generation template: {}", e),
            })?;

        Ok(vec![
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
        ])
    }

    fn create_task_refinement_messages(
        &self,
        task: &Task,
        plan_description: &str,
        global_context: &str,
        plan_context: &str,
        dependency_outputs: &HashMap<String, serde_json::Value>,
    ) -> Result<Vec<Message>, LlmError> {
        let template = PromptTemplates::task_refinement();
        let prompt_context = PromptContext::new()
            .with_variable("plan_description", plan_description)
            .with_variable("task_id", &task.id)
            .with_variable("task_type", &format!("{:?}", task.task_type))
            .with_variable("task_description", &task.description)
            .with_variable("task_parameters", &serde_json::to_string_pretty(&task.parameters).unwrap_or_default())
            .with_variable("global_context", global_context)
            .with_variable("plan_context", plan_context)
            .with_variable("dependency_outputs", &serde_json::to_string_pretty(dependency_outputs).unwrap_or_default());
        
        let (system_message, user_message) = template.fill(&prompt_context)
            .map_err(|e| LlmError::InvalidResponse {
                message: format!("Failed to fill task refinement template: {}", e),
            })?;

        Ok(vec![
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
        ])
    }

    fn create_execution_analysis_messages(
        &self,
        task: &Task,
        execution_result: &crate::llm::TaskExecutionResult,
        expected_outcome: &str,
    ) -> Result<Vec<Message>, LlmError> {
        let template = PromptTemplates::execution_analysis();
        let prompt_context = PromptContext::new()
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

        Ok(vec![
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
        ])
    }
    
    fn create_content_generation_messages(
        &self,
        prompt: &str,
        context: &str,
        _config: Option<&GenerationConfig>,
    ) -> Result<Vec<Message>, LlmError> {
        // For simple content generation, just create user message with context
        let content = format!("Context:\n{}\n\nRequest:\n{}", context, prompt);
        
        Ok(vec![
            Message {
                role: MessageRole::User,
                content,
                tool_calls: None,
                tool_call_id: None,
            },
        ])
    }
}

/// Utility functions for common template operations
pub struct TemplateUtils;

impl TemplateUtils {
    /// Create standard prompt context with common variables
    pub fn create_base_context() -> PromptContext {
        PromptContext::new()
            .with_variable("working_directory", std::env::current_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "unknown".to_string()))
            .with_variable("timestamp", chrono::Utc::now().to_rfc3339())
    }
    
    /// Fill template and handle common errors
    pub fn safe_fill_template(
        template: &PromptTemplate,
        context: &PromptContext,
        operation: &str,
    ) -> Result<(String, String), LlmError> {
        template.fill(context)
            .map_err(|e| LlmError::InvalidResponse {
                message: format!("Failed to fill {} template: {}", operation, e),
            })
    }
    
    /// Create messages from filled template
    pub fn create_messages_from_template(
        system_message: String,
        user_message: String,
    ) -> Vec<Message> {
        vec![
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
        ]
    }
    
    /// Get template by name with error handling
    pub fn get_template_safe(name: &str) -> Result<PromptTemplate, LlmError> {
        PromptTemplates::get_template(name)
            .ok_or_else(|| LlmError::InvalidResponse {
                message: format!("Unknown template: {}", name),
            })
    }
}