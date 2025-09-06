//! Template context and message builders

use crate::llm::{Message, MessageRole, LlmError};
use crate::llm::prompts::{PromptContext, PromptTemplate};
use std::collections::HashMap;

/// Builder for creating prompt contexts with fluent API
pub struct TemplateContextBuilder {
    context: PromptContext,
}

impl TemplateContextBuilder {
    /// Create a new context builder
    pub fn new() -> Self {
        Self {
            context: PromptContext::new(),
        }
    }
    
    /// Create builder with base context (working directory, timestamp)
    pub fn with_base_context() -> Self {
        let mut builder = Self::new();
        
        // Add working directory
        if let Ok(cwd) = std::env::current_dir() {
            builder = builder.var("working_directory", cwd.display().to_string());
        }
        
        // Add timestamp (using system time as chrono might not be available)
        builder = builder.var("timestamp", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs().to_string())
            .unwrap_or_else(|_| "unknown".to_string()));
        
        builder
    }
    
    /// Add a variable to the context
    pub fn var<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.context.set_variable(key, value);
        self
    }
    
    /// Add multiple variables from a HashMap
    pub fn vars(mut self, vars: HashMap<String, String>) -> Self {
        for (key, value) in vars {
            self.context.set_variable(key, value);
        }
        self
    }
    
    /// Add variables conditionally
    pub fn var_if<K, V>(self, condition: bool, key: K, value: V) -> Self
    where
        K: Into<String>,
        V: Into<String>,
    {
        if condition {
            self.var(key, value)
        } else {
            self
        }
    }
    
    /// Add optional variable (only if Some)
    pub fn var_option<K, V>(self, key: K, value: Option<V>) -> Self
    where
        K: Into<String>,
        V: Into<String>,
    {
        if let Some(v) = value {
            self.var(key, v)
        } else {
            self
        }
    }
    
    /// Add JSON serialized data
    pub fn json_var<K, V>(self, key: K, value: &V) -> Self
    where
        K: Into<String>,
        V: serde::Serialize,
    {
        match serde_json::to_string_pretty(value) {
            Ok(json_str) => self.var(key, json_str),
            Err(_) => self.var(key, "null"),
        }
    }
    
    /// Build the context
    pub fn build(self) -> PromptContext {
        self.context
    }
    
    /// Fill template with this context
    pub fn fill_template(self, template: &PromptTemplate) -> Result<(String, String), String> {
        template.fill(&self.context)
    }
}

impl Default for TemplateContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for creating LLM messages
pub struct MessageBuilder {
    messages: Vec<Message>,
}

impl MessageBuilder {
    /// Create a new message builder
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }
    
    /// Add a system message
    pub fn system<S: Into<String>>(mut self, content: S) -> Self {
        self.messages.push(Message {
            role: MessageRole::System,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        });
        self
    }
    
    /// Add a user message
    pub fn user<S: Into<String>>(mut self, content: S) -> Self {
        self.messages.push(Message {
            role: MessageRole::User,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        });
        self
    }
    
    /// Add an assistant message
    pub fn assistant<S: Into<String>>(mut self, content: S) -> Self {
        self.messages.push(Message {
            role: MessageRole::Assistant,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        });
        self
    }
    
    /// Add messages from filled template
    pub fn from_template(mut self, system: String, user: String) -> Self {
        self.messages.push(Message {
            role: MessageRole::System,
            content: system,
            tool_calls: None,
            tool_call_id: None,
        });
        self.messages.push(Message {
            role: MessageRole::User,
            content: user,
            tool_calls: None,
            tool_call_id: None,
        });
        self
    }
    
    /// Build the message list
    pub fn build(self) -> Vec<Message> {
        self.messages
    }
}

impl Default for MessageBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Combined builder for creating template contexts and LLM messages
pub struct LlmMessageBuilder {
    context_builder: TemplateContextBuilder,
}

impl LlmMessageBuilder {
    /// Create a new LLM message builder
    pub fn new() -> Self {
        Self {
            context_builder: TemplateContextBuilder::new(),
        }
    }
    
    /// Create builder with base context
    pub fn with_base_context() -> Self {
        Self {
            context_builder: TemplateContextBuilder::with_base_context(),
        }
    }
    
    /// Add a variable to the context
    pub fn var<K, V>(mut self, key: K, value: V) -> Self
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.context_builder = self.context_builder.var(key, value);
        self
    }
    
    /// Add multiple variables
    pub fn vars(mut self, vars: HashMap<String, String>) -> Self {
        self.context_builder = self.context_builder.vars(vars);
        self
    }
    
    /// Add JSON variable
    pub fn json_var<K, V>(mut self, key: K, value: &V) -> Self
    where
        K: Into<String>,
        V: serde::Serialize,
    {
        self.context_builder = self.context_builder.json_var(key, value);
        self
    }
    
    /// Fill template and create messages
    pub fn fill_and_create_messages(self, template: &PromptTemplate) -> Result<Vec<Message>, LlmError> {
        let context = self.context_builder.build();
        let (system_message, user_message) = template.fill(&context)
            .map_err(|e| LlmError::InvalidResponse {
                message: format!("Failed to fill template: {}", e),
            })?;
        
        Ok(MessageBuilder::new()
            .from_template(system_message, user_message)
            .build())
    }
}

impl Default for LlmMessageBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Specialized builders for common use cases
pub struct PlanGenerationMessageBuilder;

impl PlanGenerationMessageBuilder {
    pub fn create(prompt: &str, context: &str) -> Result<Vec<Message>, LlmError> {
        LlmMessageBuilder::with_base_context()
            .var("context", context)
            .var("request", prompt)
            .var("project_type", "rust") // Default, can be overridden
            .var("current_state", "unknown") // Default, can be overridden
            .fill_and_create_messages(&crate::llm::prompts::PromptTemplates::plan_generation())
    }
}

pub struct TaskRefinementMessageBuilder;

impl TaskRefinementMessageBuilder {
    pub fn create(
        task: &crate::planning::Task,
        plan_description: &str,
        global_context: &str,
        plan_context: &str,
        dependency_outputs: &HashMap<String, serde_json::Value>,
    ) -> Result<Vec<Message>, LlmError> {
        LlmMessageBuilder::with_base_context()
            .var("plan_description", plan_description)
            .var("task_id", &task.id)
            .var("task_type", &format!("{:?}", task.task_type))
            .var("task_description", &task.description)
            .json_var("task_parameters", &task.parameters)
            .var("global_context", global_context)
            .var("plan_context", plan_context)
            .json_var("dependency_outputs", dependency_outputs)
            .fill_and_create_messages(&crate::llm::prompts::PromptTemplates::task_refinement())
    }
}

pub struct ExecutionAnalysisMessageBuilder;

impl ExecutionAnalysisMessageBuilder {
    pub fn create(
        task: &crate::planning::Task,
        execution_result: &crate::llm::TaskExecutionResult,
        expected_outcome: &str,
    ) -> Result<Vec<Message>, LlmError> {
        LlmMessageBuilder::with_base_context()
            .var("task_id", &task.id)
            .var("task_type", &format!("{:?}", task.task_type))
            .var("task_description", &task.description)
            .var("expected_outcome", expected_outcome)
            .json_var("task_parameters", &task.parameters)
            .var("success", &execution_result.success.to_string())
            .var("exit_code", &execution_result.exit_code.map(|c| c.to_string()).unwrap_or_else(|| "N/A".to_string()))
            .var("execution_time_ms", &execution_result.execution_time_ms.to_string())
            .var("stdout", execution_result.stdout.as_deref().unwrap_or(""))
            .var("stderr", execution_result.stderr.as_deref().unwrap_or(""))
            .json_var("output_data", &execution_result.output)
            .var("error_message", execution_result.error.as_deref().unwrap_or(""))
            .var("plan_description", "") // Usually not available at analysis time
            .var("plan_context", "") // Usually not available at analysis time
            .json_var("task_dependencies", &task.dependencies)
            .fill_and_create_messages(&crate::llm::prompts::PromptTemplates::execution_analysis())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_context_builder() {
        let context = TemplateContextBuilder::new()
            .var("key1", "value1")
            .var("key2", "value2")
            .build();
        
        assert_eq!(context.get_variable("key1"), Some(&"value1".to_string()));
        assert_eq!(context.get_variable("key2"), Some(&"value2".to_string()));
    }

    #[test]
    fn test_message_builder() {
        let messages = MessageBuilder::new()
            .system("System message")
            .user("User message")
            .build();
        
        assert_eq!(messages.len(), 2);
        assert!(matches!(messages[0].role, MessageRole::System));
        assert!(matches!(messages[1].role, MessageRole::User));
        assert_eq!(messages[0].content, "System message");
        assert_eq!(messages[1].content, "User message");
    }
}