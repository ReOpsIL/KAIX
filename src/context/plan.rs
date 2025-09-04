//! Plan context for managing temporary state during plan execution

use crate::planning::{Task, TaskResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// Temporary context that exists only during plan execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanContext {
    /// Unique identifier for the plan this context belongs to
    pub plan_id: String,
    /// Map of task IDs to their execution results
    pub task_results: HashMap<String, TaskResult>,
    /// Map of variable names to their values (for inter-task communication)
    pub variables: HashMap<String, serde_json::Value>,
    /// Accumulated outputs from tasks (for building final results)
    pub outputs: Vec<PlanOutput>,
    /// When this context was created
    pub created_at: DateTime<Utc>,
    /// When this context was last updated
    pub updated_at: DateTime<Utc>,
}

/// Represents an output entry in the plan context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanOutput {
    /// ID of the task that generated this output
    pub task_id: String,
    /// Human-readable description of the output
    pub description: String,
    /// The actual output data
    pub data: serde_json::Value,
    /// Type of output (file_content, command_output, analysis_result, etc.)
    pub output_type: String,
    /// When this output was generated
    pub timestamp: DateTime<Utc>,
}

impl PlanContext {
    /// Create a new plan context
    pub fn new(plan_id: String) -> Self {
        let now = Utc::now();
        Self {
            plan_id,
            task_results: HashMap::new(),
            variables: HashMap::new(),
            outputs: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Add a task result to the context
    pub fn add_task_result(&mut self, task_id: String, result: TaskResult) {
        self.task_results.insert(task_id, result);
        self.updated_at = Utc::now();
    }

    /// Get a task result by ID
    pub fn get_task_result(&self, task_id: &str) -> Option<&TaskResult> {
        self.task_results.get(task_id)
    }

    /// Set a variable value
    pub fn set_variable<K, V>(&mut self, key: K, value: V) 
    where 
        K: Into<String>,
        V: serde::Serialize,
    {
        if let Ok(json_value) = serde_json::to_value(value) {
            self.variables.insert(key.into(), json_value);
            self.updated_at = Utc::now();
        }
    }

    /// Get a variable value
    pub fn get_variable(&self, key: &str) -> Option<&serde_json::Value> {
        self.variables.get(key)
    }

    /// Get a variable as a specific type
    pub fn get_variable_as<T>(&self, key: &str) -> Result<Option<T>, serde_json::Error>
    where
        T: serde::de::DeserializeOwned,
    {
        match self.variables.get(key) {
            Some(value) => Ok(Some(serde_json::from_value(value.clone())?)),
            None => Ok(None),
        }
    }

    /// Add an output entry
    pub fn add_output(&mut self, task_id: String, description: String, data: serde_json::Value, output_type: String) {
        let output = PlanOutput {
            task_id,
            description,
            data,
            output_type,
            timestamp: Utc::now(),
        };
        
        self.outputs.push(output);
        self.updated_at = Utc::now();
    }

    /// Get all outputs for a specific task
    pub fn get_task_outputs(&self, task_id: &str) -> Vec<&PlanOutput> {
        self.outputs
            .iter()
            .filter(|output| output.task_id == task_id)
            .collect()
    }

    /// Get outputs by type
    pub fn get_outputs_by_type(&self, output_type: &str) -> Vec<&PlanOutput> {
        self.outputs
            .iter()
            .filter(|output| output.output_type == output_type)
            .collect()
    }

    /// Get all outputs
    pub fn get_all_outputs(&self) -> &[PlanOutput] {
        &self.outputs
    }

    /// Clear all context data
    pub fn clear(&mut self) {
        self.task_results.clear();
        self.variables.clear();
        self.outputs.clear();
        self.updated_at = Utc::now();
    }

    /// Get a summary of the context for use in LLM prompts
    pub fn get_summary(&self) -> String {
        let mut summary = String::new();
        
        // Add task results summary
        if !self.task_results.is_empty() {
            summary.push_str("Task Results:\n");
            for (task_id, result) in &self.task_results {
                let status = if result.success { "SUCCESS" } else { "FAILED" };
                summary.push_str(&format!("- {}: {} ({}ms)\n", task_id, status, result.execution_time_ms));
                
                if let Some(output) = &result.output {
                    if let Ok(output_str) = serde_json::to_string_pretty(output) {
                        let truncated = if output_str.len() > 200 {
                            format!("{}...", &output_str[..200])
                        } else {
                            output_str
                        };
                        summary.push_str(&format!("  Output: {}\n", truncated));
                    }
                }
                
                if let Some(error) = &result.error {
                    summary.push_str(&format!("  Error: {}\n", error));
                }
            }
            summary.push('\n');
        }

        // Add variables summary
        if !self.variables.is_empty() {
            summary.push_str("Variables:\n");
            for (key, value) in &self.variables {
                if let Ok(value_str) = serde_json::to_string_pretty(value) {
                    let truncated = if value_str.len() > 100 {
                        format!("{}...", &value_str[..100])
                    } else {
                        value_str
                    };
                    summary.push_str(&format!("- {}: {}\n", key, truncated));
                }
            }
            summary.push('\n');
        }

        // Add outputs summary
        if !self.outputs.is_empty() {
            summary.push_str("Generated Outputs:\n");
            for output in &self.outputs {
                summary.push_str(&format!("- {} ({}): {}\n", 
                    output.task_id, 
                    output.output_type, 
                    output.description));
            }
        }

        if summary.is_empty() {
            summary.push_str("No context data available yet.");
        }

        summary
    }

    /// Get the dependency chain for completed tasks
    pub fn get_completed_task_chain(&self) -> Vec<String> {
        self.task_results
            .iter()
            .filter(|(_, result)| result.success)
            .map(|(task_id, _)| task_id.clone())
            .collect()
    }

    /// Check if a task has completed successfully
    pub fn is_task_completed(&self, task_id: &str) -> bool {
        self.task_results
            .get(task_id)
            .map(|result| result.success)
            .unwrap_or(false)
    }

    /// Get failed tasks
    pub fn get_failed_tasks(&self) -> Vec<(String, String)> {
        self.task_results
            .iter()
            .filter(|(_, result)| !result.success)
            .map(|(task_id, result)| {
                (task_id.clone(), result.error.clone().unwrap_or_default())
            })
            .collect()
    }

    /// Export context as JSON for serialization
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Import context from JSON
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}