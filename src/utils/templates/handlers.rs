//! Template handlers for different LLM operations

use crate::llm::{LlmError, Message, TaskExecutionResult, TaskAnalysis};
use crate::planning::Task;
use std::collections::HashMap;
use super::builders::{PlanGenerationMessageBuilder, TaskRefinementMessageBuilder, ExecutionAnalysisMessageBuilder};

/// Trait for handling template operations across LLM providers
pub trait TemplateHandler {
    /// Create messages for plan generation
    fn plan_generation_messages(
        &self,
        prompt: &str,
        context: &str,
    ) -> Result<Vec<Message>, LlmError>;
    
    /// Create messages for task refinement
    fn task_refinement_messages(
        &self,
        task: &Task,
        plan_description: &str,
        global_context: &str,
        plan_context: &str,
        dependency_outputs: &HashMap<String, serde_json::Value>,
    ) -> Result<Vec<Message>, LlmError>;
    
    /// Create messages for execution analysis
    fn execution_analysis_messages(
        &self,
        task: &Task,
        execution_result: &TaskExecutionResult,
        expected_outcome: &str,
    ) -> Result<Vec<Message>, LlmError>;
    
    /// Parse task analysis from response content
    fn parse_task_analysis(&self, content: &str) -> Result<TaskAnalysis, LlmError>;
}

/// Standard implementation of template handling
pub struct StandardTemplateHandler;

impl TemplateHandler for StandardTemplateHandler {
    fn plan_generation_messages(
        &self,
        prompt: &str,
        context: &str,
    ) -> Result<Vec<Message>, LlmError> {
        PlanGenerationMessageBuilder::create(prompt, context)
    }
    
    fn task_refinement_messages(
        &self,
        task: &Task,
        plan_description: &str,
        global_context: &str,
        plan_context: &str,
        dependency_outputs: &HashMap<String, serde_json::Value>,
    ) -> Result<Vec<Message>, LlmError> {
        TaskRefinementMessageBuilder::create(
            task,
            plan_description,
            global_context,
            plan_context,
            dependency_outputs,
        )
    }
    
    fn execution_analysis_messages(
        &self,
        task: &Task,
        execution_result: &TaskExecutionResult,
        expected_outcome: &str,
    ) -> Result<Vec<Message>, LlmError> {
        ExecutionAnalysisMessageBuilder::create(task, execution_result, expected_outcome)
    }
    
    fn parse_task_analysis(&self, content: &str) -> Result<TaskAnalysis, LlmError> {
        // Parse the JSON response into TaskAnalysis
        let analysis_json: serde_json::Value = serde_json::from_str(content)
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
                .filter_map(|v| v.as_str().map(|s| std::path::PathBuf::from(s)))
                .collect::<Vec<std::path::PathBuf>>());
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

/// Template handler factory
pub struct TemplateHandlerFactory;

impl TemplateHandlerFactory {
    /// Create a standard template handler
    pub fn standard() -> StandardTemplateHandler {
        StandardTemplateHandler
    }
    
    /// Create template handler for specific provider
    pub fn for_provider(provider_name: &str) -> Box<dyn TemplateHandler> {
        match provider_name.to_lowercase().as_str() {
            "openrouter" | "gemini" | _ => Box::new(StandardTemplateHandler),
        }
    }
}

/// Utility functions for common template operations
pub struct TemplateOperations;

impl TemplateOperations {
    /// Create and fill plan generation messages
    pub fn create_plan_messages(prompt: &str, context: &str) -> Result<Vec<Message>, LlmError> {
        let handler = StandardTemplateHandler;
        handler.plan_generation_messages(prompt, context)
    }
    
    /// Create and fill task refinement messages
    pub fn create_task_refinement_messages(
        task: &Task,
        plan_description: &str,
        global_context: &str,
        plan_context: &str,
        dependency_outputs: &HashMap<String, serde_json::Value>,
    ) -> Result<Vec<Message>, LlmError> {
        let handler = StandardTemplateHandler;
        handler.task_refinement_messages(
            task,
            plan_description,
            global_context,
            plan_context,
            dependency_outputs,
        )
    }
    
    /// Create and fill execution analysis messages
    pub fn create_execution_analysis_messages(
        task: &Task,
        execution_result: &TaskExecutionResult,
        expected_outcome: &str,
    ) -> Result<Vec<Message>, LlmError> {
        let handler = StandardTemplateHandler;
        handler.execution_analysis_messages(task, execution_result, expected_outcome)
    }
    
    /// Parse task analysis from JSON content
    pub fn parse_task_analysis(content: &str) -> Result<TaskAnalysis, LlmError> {
        let handler = StandardTemplateHandler;
        handler.parse_task_analysis(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planning::{Task, TaskType};
    use serde_json::json;

    #[test]
    fn test_template_handler_factory() {
        let handler = TemplateHandlerFactory::for_provider("openrouter");
        // Test that we can create handlers without panicking
        assert!(!std::ptr::addr_of!(*handler).is_null());
        
        let handler2 = TemplateHandlerFactory::standard();
        // Ensure we can create standard handlers
        assert!(!std::ptr::addr_of!(handler2).is_null());
    }

    #[test]
    fn test_parse_task_analysis_success() {
        let json_content = json!({
            "success": true,
            "summary": "Task completed successfully",
            "details": "All operations completed without errors",
            "extracted_data": {
                "files_created": ["test.rs"]
            },
            "next_steps": ["Run tests"],
            "context_updates": {
                "status": "completed"
            },
            "modified_files": ["/path/to/test.rs"],
            "metadata": {
                "execution_time": "100ms"
            }
        });

        let handler = StandardTemplateHandler;
        let result = handler.parse_task_analysis(&json_content.to_string());
        
        assert!(result.is_ok());
        let analysis = result.unwrap();
        assert!(analysis.success);
        assert_eq!(analysis.summary, "Task completed successfully");
        assert!(analysis.next_steps.is_some());
    }

    #[test]
    fn test_parse_task_analysis_failure() {
        let json_content = json!({
            "success": false,
            "summary": "Task failed",
            "details": "Error occurred during execution",
            "error": "File not found"
        });

        let handler = StandardTemplateHandler;
        let result = handler.parse_task_analysis(&json_content.to_string());
        
        assert!(result.is_ok());
        let analysis = result.unwrap();
        assert!(!analysis.success);
        assert_eq!(analysis.error, Some("File not found".to_string()));
    }

    #[test]
    fn test_parse_invalid_json() {
        let invalid_json = "not valid json";
        
        let handler = StandardTemplateHandler;
        let result = handler.parse_task_analysis(invalid_json);
        
        assert!(result.is_err());
        match result.unwrap_err() {
            LlmError::InvalidResponse { message } => {
                assert!(message.contains("Failed to parse task analysis JSON"));
            }
            _ => panic!("Expected InvalidResponse error"),
        }
    }
}