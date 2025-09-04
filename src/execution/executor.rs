//! Task executor for individual task execution

use super::{ExecutionConfig, TaskExecutionResult};
use crate::planning::{Task, TaskType};
use crate::utils::errors::KaiError;
use crate::Result;
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;

/// Executes individual tasks
pub struct TaskExecutor {
    config: ExecutionConfig,
}

impl TaskExecutor {
    /// Create a new task executor
    pub fn new(config: ExecutionConfig) -> Self {
        Self { config }
    }

    /// Execute a task with the given refined instruction and context
    pub async fn execute_task(
        &self,
        task: &Task,
        refined_instruction: &str,
        context: &str,
    ) -> Result<TaskExecutionResult> {
        match task.task_type {
            TaskType::ReadFile => self.execute_read_file(task).await,
            TaskType::WriteFile => self.execute_write_file(task, refined_instruction).await,
            TaskType::ExecuteCommand => self.execute_command(task, refined_instruction).await,
            TaskType::GenerateContent => self.execute_generate_content(task, refined_instruction, context).await,
            TaskType::AnalyzeCode => self.execute_analyze_code(task, context).await,
            TaskType::ListFiles => self.execute_list_files(task).await,
            TaskType::CreateDirectory => self.execute_create_directory(task).await,
            TaskType::Delete => self.execute_delete(task).await,
        }
    }

    /// Execute a read file task
    async fn execute_read_file(&self, task: &Task) -> Result<TaskExecutionResult> {
        let path = task.parameters.get("path")
            .and_then(|p| p.as_str())
            .ok_or_else(|| KaiError::task(&task.id, "Missing 'path' parameter"))?;

        match fs::read_to_string(path).await {
            Ok(content) => Ok(TaskExecutionResult {
                success: true,
                output: Some(serde_json::json!({
                    "path": path,
                    "content": content,
                    "size": content.len()
                })),
                error: None,
                stdout: None,
                stderr: None,
                exit_code: None,
            }),
            Err(e) => Ok(TaskExecutionResult {
                success: false,
                output: None,
                error: Some(format!("Failed to read file '{}': {}", path, e)),
                stdout: None,
                stderr: None,
                exit_code: None,
            }),
        }
    }

    /// Execute a write file task
    async fn execute_write_file(&self, task: &Task, refined_instruction: &str) -> Result<TaskExecutionResult> {
        let path = task.parameters.get("path")
            .and_then(|p| p.as_str())
            .ok_or_else(|| KaiError::task(&task.id, "Missing 'path' parameter"))?;

        // Extract content from the refined instruction
        // In a real implementation, this would be more sophisticated
        let content = if let Some(content) = task.parameters.get("content").and_then(|c| c.as_str()) {
            content.to_string()
        } else {
            refined_instruction.to_string()
        };

        // Ensure parent directory exists
        if let Some(parent) = Path::new(path).parent() {
            if !parent.exists() {
                if let Err(e) = fs::create_dir_all(parent).await {
                    return Ok(TaskExecutionResult {
                        success: false,
                        output: None,
                        error: Some(format!("Failed to create directory '{}': {}", parent.display(), e)),
                        stdout: None,
                        stderr: None,
                        exit_code: None,
                    });
                }
            }
        }

        match fs::write(path, &content).await {
            Ok(_) => Ok(TaskExecutionResult {
                success: true,
                output: Some(serde_json::json!({
                    "path": path,
                    "bytes_written": content.len()
                })),
                error: None,
                stdout: None,
                stderr: None,
                exit_code: None,
            }),
            Err(e) => Ok(TaskExecutionResult {
                success: false,
                output: None,
                error: Some(format!("Failed to write file '{}': {}", path, e)),
                stdout: None,
                stderr: None,
                exit_code: None,
            }),
        }
    }

    /// Execute a command task
    async fn execute_command(&self, task: &Task, refined_instruction: &str) -> Result<TaskExecutionResult> {
        let command_str = if let Some(cmd) = task.parameters.get("command").and_then(|c| c.as_str()) {
            cmd.to_string()
        } else {
            refined_instruction.to_string()
        };

        let working_dir = task.parameters.get("working_dir")
            .and_then(|d| d.as_str())
            .unwrap_or(".");

        // Parse command and arguments (simplified)
        let parts: Vec<&str> = command_str.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(TaskExecutionResult {
                success: false,
                output: None,
                error: Some("Empty command".to_string()),
                stdout: None,
                stderr: None,
                exit_code: None,
            });
        }

        let mut cmd = Command::new(parts[0]);
        if parts.len() > 1 {
            cmd.args(&parts[1..]);
        }

        cmd.current_dir(working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        match cmd.spawn() {
            Ok(mut child) => {
                let output = child.wait_with_output().await
                    .map_err(|e| KaiError::task(&task.id, format!("Failed to execute command: {}", e)))?;

                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let success = output.status.success();

                Ok(TaskExecutionResult {
                    success,
                    output: Some(serde_json::json!({
                        "command": command_str,
                        "stdout": stdout,
                        "stderr": stderr,
                        "exit_code": output.status.code()
                    })),
                    error: if success { None } else { Some(stderr.clone()) },
                    stdout: Some(stdout),
                    stderr: Some(stderr),
                    exit_code: output.status.code(),
                })
            }
            Err(e) => Ok(TaskExecutionResult {
                success: false,
                output: None,
                error: Some(format!("Failed to spawn command: {}", e)),
                stdout: None,
                stderr: None,
                exit_code: None,
            }),
        }
    }

    /// Execute a content generation task
    async fn execute_generate_content(
        &self,
        task: &Task,
        refined_instruction: &str,
        context: &str,
    ) -> Result<TaskExecutionResult> {
        // This would typically involve calling the LLM again
        // For now, just return the refined instruction as content
        Ok(TaskExecutionResult {
            success: true,
            output: Some(serde_json::json!({
                "generated_content": refined_instruction,
                "context_used": !context.is_empty()
            })),
            error: None,
            stdout: None,
            stderr: None,
            exit_code: None,
        })
    }

    /// Execute a code analysis task
    async fn execute_analyze_code(&self, task: &Task, context: &str) -> Result<TaskExecutionResult> {
        let path = task.parameters.get("path")
            .and_then(|p| p.as_str())
            .ok_or_else(|| KaiError::task(&task.id, "Missing 'path' parameter"))?;

        match fs::read_to_string(path).await {
            Ok(content) => {
                // Basic analysis - in a real implementation, this would be more sophisticated
                let lines = content.lines().count();
                let chars = content.len();
                let has_tests = content.contains("test") || content.contains("Test");
                let has_comments = content.contains("//") || content.contains("/*");

                Ok(TaskExecutionResult {
                    success: true,
                    output: Some(serde_json::json!({
                        "path": path,
                        "lines": lines,
                        "characters": chars,
                        "has_tests": has_tests,
                        "has_comments": has_comments,
                        "language": detect_language_from_extension(path)
                    })),
                    error: None,
                    stdout: None,
                    stderr: None,
                    exit_code: None,
                })
            }
            Err(e) => Ok(TaskExecutionResult {
                success: false,
                output: None,
                error: Some(format!("Failed to analyze file '{}': {}", path, e)),
                stdout: None,
                stderr: None,
                exit_code: None,
            }),
        }
    }

    /// Execute a list files task
    async fn execute_list_files(&self, task: &Task) -> Result<TaskExecutionResult> {
        let path = task.parameters.get("path")
            .and_then(|p| p.as_str())
            .unwrap_or(".");

        let recursive = task.parameters.get("recursive")
            .and_then(|r| r.as_bool())
            .unwrap_or(false);

        match self.list_directory_contents(path, recursive).await {
            Ok(files) => Ok(TaskExecutionResult {
                success: true,
                output: Some(serde_json::json!({
                    "path": path,
                    "files": files,
                    "count": files.len()
                })),
                error: None,
                stdout: None,
                stderr: None,
                exit_code: None,
            }),
            Err(e) => Ok(TaskExecutionResult {
                success: false,
                output: None,
                error: Some(format!("Failed to list files in '{}': {}", path, e)),
                stdout: None,
                stderr: None,
                exit_code: None,
            }),
        }
    }

    /// Execute a create directory task
    async fn execute_create_directory(&self, task: &Task) -> Result<TaskExecutionResult> {
        let path = task.parameters.get("path")
            .and_then(|p| p.as_str())
            .ok_or_else(|| KaiError::task(&task.id, "Missing 'path' parameter"))?;

        match fs::create_dir_all(path).await {
            Ok(_) => Ok(TaskExecutionResult {
                success: true,
                output: Some(serde_json::json!({
                    "path": path,
                    "created": true
                })),
                error: None,
                stdout: None,
                stderr: None,
                exit_code: None,
            }),
            Err(e) => Ok(TaskExecutionResult {
                success: false,
                output: None,
                error: Some(format!("Failed to create directory '{}': {}", path, e)),
                stdout: None,
                stderr: None,
                exit_code: None,
            }),
        }
    }

    /// Execute a delete task
    async fn execute_delete(&self, task: &Task) -> Result<TaskExecutionResult> {
        let path = task.parameters.get("path")
            .and_then(|p| p.as_str())
            .ok_or_else(|| KaiError::task(&task.id, "Missing 'path' parameter"))?;

        let path_obj = Path::new(path);
        let result = if path_obj.is_file() {
            fs::remove_file(path).await
        } else if path_obj.is_dir() {
            fs::remove_dir_all(path).await
        } else {
            return Ok(TaskExecutionResult {
                success: false,
                output: None,
                error: Some(format!("Path does not exist: {}", path)),
                stdout: None,
                stderr: None,
                exit_code: None,
            });
        };

        match result {
            Ok(_) => Ok(TaskExecutionResult {
                success: true,
                output: Some(serde_json::json!({
                    "path": path,
                    "deleted": true
                })),
                error: None,
                stdout: None,
                stderr: None,
                exit_code: None,
            }),
            Err(e) => Ok(TaskExecutionResult {
                success: false,
                output: None,
                error: Some(format!("Failed to delete '{}': {}", path, e)),
                stdout: None,
                stderr: None,
                exit_code: None,
            }),
        }
    }

    /// List directory contents
    async fn list_directory_contents(&self, path: &str, recursive: bool) -> Result<Vec<String>> {
        let mut files = Vec::new();
        let mut entries = fs::read_dir(path).await
            .map_err(|e| KaiError::execution(format!("Failed to read directory: {}", e)))?;

        while let Some(entry) = entries.next_entry().await
            .map_err(|e| KaiError::execution(format!("Failed to read directory entry: {}", e)))? {
            
            let path = entry.path();
            let path_str = path.to_string_lossy().to_string();
            
            if path.is_file() {
                files.push(path_str);
            } else if path.is_dir() && recursive {
                let sub_files = self.list_directory_contents(&path_str, recursive).await?;
                files.extend(sub_files);
            }
        }

        files.sort();
        Ok(files)
    }
}

/// Detect programming language from file extension
fn detect_language_from_extension(path: &str) -> Option<String> {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .and_then(|ext| {
            match ext.to_lowercase().as_str() {
                "rs" => Some("rust".to_string()),
                "js" => Some("javascript".to_string()),
                "ts" => Some("typescript".to_string()),
                "py" => Some("python".to_string()),
                "java" => Some("java".to_string()),
                "go" => Some("go".to_string()),
                "cpp" | "cc" | "cxx" => Some("cpp".to_string()),
                "c" => Some("c".to_string()),
                "h" | "hpp" => Some("c_header".to_string()),
                _ => None,
            }
        })
}