//! Task executor for individual task execution with security sandboxing

use super::ExecutionConfig;
use crate::llm::{TaskExecutionResult, GenerationConfig};
use crate::planning::{Task, TaskType};
use crate::utils::errors::KaiError;
use crate::llm::LlmProvider;
use crate::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::fs;
use tokio::process::Command;
use tracing::{debug, error, info, warn};

/// Executes individual tasks with security sandboxing and enhanced capabilities
pub struct TaskExecutor {
    /// Configuration for execution behavior
    config: ExecutionConfig,
    /// Working directory for all operations (security boundary)
    working_dir: PathBuf,
    /// LLM provider for content generation and analysis
    llm_provider: Arc<dyn LlmProvider>,
    /// Current model to use for LLM tasks
    model: String,
    /// Security audit logger
    audit_log: Vec<SecurityAuditEntry>,
    /// Resource usage tracker
    resource_tracker: ResourceTracker,
}

impl TaskExecutor {
    /// Create a new task executor with security sandboxing
    pub fn new(
        config: ExecutionConfig,
        working_dir: PathBuf,
        llm_provider: Arc<dyn LlmProvider>,
        model: String,
    ) -> Self {
        info!("Creating TaskExecutor with working_dir: {:?}", working_dir);
        
        Self {
            config,
            working_dir,
            llm_provider,
            model,
            audit_log: Vec::new(),
            resource_tracker: ResourceTracker::new(),
        }
    }
    
    /// Get resource usage statistics
    pub fn get_resource_stats(&self) -> ResourceStats {
        self.resource_tracker.get_stats()
    }
    
    /// Get security audit log
    pub fn get_audit_log(&self) -> &[SecurityAuditEntry] {
        &self.audit_log
    }
    
    /// Helper function to create successful TaskExecutionResult
    fn success_result(
        output: Option<serde_json::Value>,
        stdout: Option<String>,
        execution_time_ms: u64,
    ) -> TaskExecutionResult {
        TaskExecutionResult {
            success: true,
            output,
            error: None,
            stdout,
            stderr: None,
            exit_code: None,
            execution_time_ms,
            metadata: HashMap::new(),
        }
    }
    
    /// Helper function to create failed TaskExecutionResult
    fn failure_result(
        error_msg: String,
        stderr: Option<String>,
        exit_code: Option<i32>,
    ) -> TaskExecutionResult {
        TaskExecutionResult {
            success: false,
            output: None,
            error: Some(error_msg),
            stdout: None,
            stderr,
            exit_code,
            execution_time_ms: 0,
            metadata: HashMap::new(),
        }
    }
    
    /// Update working directory (requires validation)
    pub fn update_working_dir(&mut self, new_working_dir: PathBuf) -> Result<()> {
        if !new_working_dir.exists() {
            return Err(KaiError::execution(format!(
                "Working directory does not exist: {}",
                new_working_dir.display()
            )));
        }
        
        if !new_working_dir.is_dir() {
            return Err(KaiError::execution(format!(
                "Working directory is not a directory: {}",
                new_working_dir.display()
            )));
        }
        
        info!("Updating working directory from {:?} to {:?}", self.working_dir, new_working_dir);
        self.working_dir = new_working_dir;
        Ok(())
    }

    /// Execute a task with the given refined instruction and context
    pub async fn execute_task(
        &mut self,
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

    /// Execute a read file task with security validation
    async fn execute_read_file(&mut self, task: &Task) -> Result<TaskExecutionResult> {
        let start_time = SystemTime::now();
        let path = task.parameters.get("path")
            .and_then(|p| p.as_str())
            .ok_or_else(|| KaiError::task(&task.id, "Missing 'path' parameter"))?;

        // Validate and sanitize path
        let sanitized_path = self.validate_and_sanitize_path(path, &task.id)?;
        
        // Log security audit
        self.log_security_audit(SecurityAuditEntry {
            task_id: task.id.clone(),
            operation: "read_file".to_string(),
            path: sanitized_path.clone(),
            timestamp: SystemTime::now(),
            allowed: true,
            reason: None,
        });

        debug!("Reading file: {:?}", sanitized_path);
        
        match fs::read_to_string(&sanitized_path).await {
            Ok(content) => {
                let size = content.len();
                let execution_time = start_time.elapsed().unwrap_or(Duration::ZERO).as_millis() as u64;
                info!("Successfully read file {:?} ({} bytes) in {}ms", sanitized_path, size, execution_time);
                
                Ok(TaskExecutionResult {
                    success: true,
                    output: Some(serde_json::json!({
                        "path": path,
                        "content": content,
                        "size": size,
                        "sanitized_path": sanitized_path.to_string_lossy()
                    })),
                    error: None,
                    stdout: None,
                    stderr: None,
                    exit_code: None,
                    execution_time_ms: execution_time,
                    metadata: HashMap::new(),
                })
            }
            Err(e) => {
                let execution_time = start_time.elapsed().unwrap_or(Duration::ZERO).as_millis() as u64;
                error!("Failed to read file {:?}: {}", sanitized_path, e);
                
                Ok(TaskExecutionResult {
                    success: false,
                    output: None,
                    error: Some(format!("Failed to read file '{}': {}", path, e)),
                    stdout: None,
                    stderr: None,
                    exit_code: None,
                    execution_time_ms: execution_time,
                    metadata: HashMap::new(),
                })
            }
        }
    }

    /// Execute a write file task with security validation and backup
    async fn execute_write_file(&mut self, task: &Task, refined_instruction: &str) -> Result<TaskExecutionResult> {
        let path = task.parameters.get("path")
            .and_then(|p| p.as_str())
            .ok_or_else(|| KaiError::task(&task.id, "Missing 'path' parameter"))?;

        // Validate and sanitize path
        let sanitized_path = self.validate_and_sanitize_path(path, &task.id)?;
        
        // Extract content from parameters or refined instruction
        let content = if let Some(content) = task.parameters.get("content").and_then(|c| c.as_str()) {
            content.to_string()
        } else {
            refined_instruction.to_string()
        };

        // Check if this is a destructive operation
        let is_destructive = sanitized_path.exists();
        let existing_content = if is_destructive {
            fs::read_to_string(&sanitized_path).await.ok()
        } else {
            None
        };

        // Log security audit
        self.log_security_audit(SecurityAuditEntry {
            task_id: task.id.clone(),
            operation: "write_file".to_string(),
            path: sanitized_path.clone(),
            timestamp: SystemTime::now(),
            allowed: true,
            reason: if is_destructive { Some("Overwriting existing file".to_string()) } else { None },
        });

        debug!("Writing file: {:?} ({} bytes)", sanitized_path, content.len());

        // Ensure parent directory exists within working directory
        if let Some(parent) = sanitized_path.parent() {
            if !parent.exists() {
                if let Err(e) = fs::create_dir_all(parent).await {
                    error!("Failed to create directory {:?}: {}", parent, e);
                    return Ok(Self::failure_result(
                        format!("Failed to create directory '{}': {}", parent.display(), e),
                        None,
                        None,
                    ));
                }
            }
        }

        match fs::write(&sanitized_path, &content).await {
            Ok(_) => {
                info!("Successfully wrote file {:?} ({} bytes)", sanitized_path, content.len());
                
                Ok(Self::success_result(
                    Some(serde_json::json!({
                        "path": path,
                        "sanitized_path": sanitized_path.to_string_lossy(),
                        "bytes_written": content.len(),
                        "was_destructive": is_destructive,
                        "previous_content_length": existing_content.as_ref().map(|c| c.len())
                    })),
                    None,
                    0,
                ))
            }
            Err(e) => {
                error!("Failed to write file {:?}: {}", sanitized_path, e);
                
                Ok(Self::failure_result(
                    format!("Failed to write file '{}': {}", path, e),
                    None,
                    None,
                ))
            }
        }
    }

    /// Execute a command task with security restrictions and monitoring
    async fn execute_command(&mut self, task: &Task, refined_instruction: &str) -> Result<TaskExecutionResult> {
        let command_str = if let Some(cmd) = task.parameters.get("command").and_then(|c| c.as_str()) {
            cmd.to_string()
        } else {
            refined_instruction.to_string()
        };

        // Validate command for security
        if let Err(security_error) = self.validate_command_security(&command_str, &task.id) {
            return Ok(Self::failure_result(
                security_error.to_string(),
                None,
                Some(-1),
            ));
        }

        // Use working directory by default, validate any custom working dir
        let working_dir = if let Some(dir) = task.parameters.get("working_dir").and_then(|d| d.as_str()) {
            self.validate_and_sanitize_path(dir, &task.id)?
        } else {
            self.working_dir.clone()
        };

        // Log security audit
        self.log_security_audit(SecurityAuditEntry {
            task_id: task.id.clone(),
            operation: "execute_command".to_string(),
            path: working_dir.clone(),
            timestamp: SystemTime::now(),
            allowed: true,
            reason: Some(format!("Command: {}", command_str)),
        });

        // Parse command and arguments with better handling
        let parts: Vec<&str> = command_str.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(TaskExecutionResult {
                success: false,
                output: None,
                error: Some("Empty command".to_string()),
                stdout: None,
                stderr: None,
                exit_code: None,
                execution_time_ms: 0,
                metadata: HashMap::new(),
            });
        }

        info!("Executing command: {} in {:?}", command_str, working_dir);
        let start_time = SystemTime::now();
        
        let mut cmd = Command::new(parts[0]);
        if parts.len() > 1 {
            cmd.args(&parts[1..]);
        }

        cmd.current_dir(&working_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true); // Ensure cleanup if cancelled

        match cmd.spawn() {
            Ok(mut child) => {
                let output = tokio::time::timeout(
                    Duration::from_secs(self.config.default_timeout_seconds),
                    child.wait_with_output()
                ).await;

                let execution_time = start_time.elapsed().unwrap_or(Duration::ZERO).as_millis() as u64;
                
                match output {
                    Ok(Ok(output)) => {
                        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                        let success = output.status.success();
                        let exit_code = output.status.code();

                        if success {
                            info!("Command completed successfully in {}ms", execution_time);
                        } else {
                            warn!("Command failed with exit code {:?} in {}ms", exit_code, execution_time);
                        }

                        Ok(TaskExecutionResult {
                            success,
                            output: Some(serde_json::json!({
                                "command": command_str,
                                "working_dir": working_dir.to_string_lossy(),
                                "stdout": stdout,
                                "stderr": stderr,
                                "exit_code": exit_code,
                                "execution_time_ms": execution_time
                            })),
                            error: if success { None } else { Some(stderr.clone()) },
                            stdout: Some(stdout),
                            stderr: Some(stderr),
                            exit_code,
                            execution_time_ms: execution_time,
                            metadata: HashMap::new(),
                        })
                    }
                    Ok(Err(e)) => {
                        error!("Command execution error: {}", e);
                        Ok(TaskExecutionResult {
                            success: false,
                            output: None,
                            error: Some(format!("Failed to execute command: {}", e)),
                            stdout: None,
                            stderr: None,
                            exit_code: None,
                            execution_time_ms: 0,
                            metadata: HashMap::new(),
                        })
                    }
                    Err(_) => {
                        warn!("Command timed out after {}s", self.config.default_timeout_seconds);
                        // Note: Process cleanup is handled automatically by kill_on_drop(true)
                        
                        Ok(TaskExecutionResult {
                            success: false,
                            output: None,
                            error: Some(format!("Command timed out after {}s", self.config.default_timeout_seconds)),
                            stdout: None,
                            stderr: None,
                            exit_code: Some(-124), // Timeout exit code
                            execution_time_ms: self.config.default_timeout_seconds as u64 * 1000,
                            metadata: HashMap::new(),
                        })
                    }
                }
            }
            Err(e) => {
                error!("Failed to spawn command '{}': {}", command_str, e);
                Ok(TaskExecutionResult {
                    success: false,
                    output: None,
                    error: Some(format!("Failed to spawn command: {}", e)),
                    stdout: None,
                    stderr: None,
                    exit_code: None,
                    execution_time_ms: 0,
                    metadata: HashMap::new(),
                })
            }
        }
    }

    /// Execute a content generation task using LLM provider
    async fn execute_generate_content(
        &mut self,
        task: &Task,
        refined_instruction: &str,
        context: &str,
    ) -> Result<TaskExecutionResult> {
        let prompt = task.parameters.get("prompt")
            .and_then(|p| p.as_str())
            .unwrap_or(refined_instruction);

        let config = task.parameters.get("max_tokens")
            .and_then(|t| t.as_u64())
            .map(|tokens| GenerationConfig {
                max_tokens: Some(tokens as u32),
                ..Default::default()
            });

        debug!("Generating content with prompt length: {}, context length: {}", prompt.len(), context.len());
        let start_time = SystemTime::now();
        
        match self.llm_provider.generate_content(prompt, context, &self.model, config.as_ref()).await {
            Ok(generated_content) => {
                let execution_time = start_time.elapsed().unwrap_or(Duration::ZERO).as_millis() as u64;
                info!("Successfully generated content ({} chars) in {}ms", generated_content.len(), execution_time);
                
                Ok(TaskExecutionResult {
                    success: true,
                    output: Some(serde_json::json!({
                        "generated_content": generated_content,
                        "prompt": prompt,
                        "context_used": !context.is_empty(),
                        "context_length": context.len(),
                        "content_length": generated_content.len(),
                        "model": self.model,
                        "execution_time_ms": execution_time
                    })),
                    error: None,
                    stdout: Some(generated_content),
                    stderr: None,
                    exit_code: None,
                    execution_time_ms: execution_time,
                    metadata: HashMap::new(),
                })
            }
            Err(e) => {
                let execution_time = start_time.elapsed().unwrap_or(Duration::ZERO).as_millis() as u64;
                error!("Failed to generate content: {}", e);
                Ok(TaskExecutionResult {
                    success: false,
                    output: None,
                    error: Some(format!("Failed to generate content: {}", e)),
                    stdout: None,
                    stderr: None,
                    exit_code: None,
                    execution_time_ms: execution_time,
                    metadata: HashMap::new(),
                })
            }
        }
    }

    /// Execute a code analysis task using LLM provider for deep analysis
    async fn execute_analyze_code(&mut self, task: &Task, context: &str) -> Result<TaskExecutionResult> {
        let path = task.parameters.get("path")
            .and_then(|p| p.as_str())
            .ok_or_else(|| KaiError::task(&task.id, "Missing 'path' parameter"))?;

        // Validate and sanitize path
        let sanitized_path = self.validate_and_sanitize_path(path, &task.id)?;
        
        debug!("Analyzing code file: {:?}", sanitized_path);
        let start_time = SystemTime::now();
        
        match fs::read_to_string(&sanitized_path).await {
            Ok(content) => {
                // Basic static analysis
                let lines = content.lines().count();
                let chars = content.len();
                let has_tests = content.contains("test") || content.contains("Test") || content.contains("#[test]");
                let has_comments = content.contains("//") || content.contains("/*") || content.contains("#");
                let language = detect_language_from_extension(path);
                let complexity_estimate = estimate_complexity(&content, language.as_deref());
                
                // Use LLM for deeper analysis if requested
                let llm_analysis = if task.parameters.get("deep_analysis").and_then(|v| v.as_bool()).unwrap_or(false) {
                    let analysis_prompt = format!(
                        "Analyze this {} code for quality, potential issues, and improvements:\n\n{}\n\nContext:\n{}",
                        language.as_deref().unwrap_or("unknown"),
                        content,
                        context
                    );
                    
                    let analysis_config = GenerationConfig {
                        max_tokens: Some(1000),
                        ..Default::default()
                    };
                    match self.llm_provider.generate_content(&analysis_prompt, "", &self.model, Some(&analysis_config)).await {
                        Ok(analysis) => Some(analysis),
                        Err(e) => {
                            warn!("LLM analysis failed: {}", e);
                            None
                        }
                    }
                } else {
                    None
                };
                
                let execution_time = start_time.elapsed().unwrap_or(Duration::ZERO).as_millis() as u64;
                info!("Analyzed code file {:?} ({} lines) in {}ms", sanitized_path, lines, execution_time);

                Ok(Self::success_result(
                    Some(serde_json::json!({
                        "path": path,
                        "sanitized_path": sanitized_path.to_string_lossy(),
                        "lines": lines,
                        "characters": chars,
                        "has_tests": has_tests,
                        "has_comments": has_comments,
                        "language": language,
                        "complexity_estimate": complexity_estimate,
                        "llm_analysis": llm_analysis,
                        "execution_time_ms": execution_time
                    })),
                    None,
                    execution_time,
                ))
            }
            Err(e) => {
                error!("Failed to analyze file {:?}: {}", sanitized_path, e);
                Ok(TaskExecutionResult {
                    success: false,
                    output: None,
                    error: Some(format!("Failed to analyze file '{}': {}", path, e)),
                    stdout: None,
                    stderr: None,
                    exit_code: None,
                    execution_time_ms: 0,
                    metadata: HashMap::new(),
                })
            }
        }
    }

    /// Execute a list files task with enhanced metadata
    async fn execute_list_files(&mut self, task: &Task) -> Result<TaskExecutionResult> {
        let path = task.parameters.get("path")
            .and_then(|p| p.as_str())
            .unwrap_or(".");

        let recursive = task.parameters.get("recursive")
            .and_then(|r| r.as_bool())
            .unwrap_or(false);

        let include_hidden = task.parameters.get("include_hidden")
            .and_then(|h| h.as_bool())
            .unwrap_or(false);

        // Validate and sanitize path
        let sanitized_path = self.validate_and_sanitize_path(path, &task.id)?;
        
        debug!("Listing files in {:?} (recursive: {}, include_hidden: {})", sanitized_path, recursive, include_hidden);
        let start_time = SystemTime::now();

        match self.list_directory_contents(&sanitized_path, recursive).await {
            Ok(files) => {
                let execution_time = start_time.elapsed().unwrap_or(Duration::ZERO).as_millis() as u64;
                let file_count = files.len();
                let total_size: u64 = files.iter()
                    .filter_map(|f| f.size)
                    .sum();
                
                info!("Listed {} files in {:?} in {}ms", file_count, sanitized_path, execution_time);
                
                Ok(TaskExecutionResult {
                    success: true,
                    output: Some(serde_json::json!({
                        "path": path,
                        "sanitized_path": sanitized_path.to_string_lossy(),
                        "files": files,
                        "count": file_count,
                        "total_size_bytes": total_size,
                        "recursive": recursive,
                        "include_hidden": include_hidden,
                        "execution_time_ms": execution_time
                    })),
                    error: None,
                    stdout: None,
                    stderr: None,
                    exit_code: None,
                    execution_time_ms: 0,
                    metadata: HashMap::new(),
                })
            }
            Err(e) => {
                error!("Failed to list files in {:?}: {}", sanitized_path, e);
                Ok(TaskExecutionResult {
                    success: false,
                    output: None,
                    error: Some(format!("Failed to list files in '{}': {}", path, e)),
                    stdout: None,
                    stderr: None,
                    exit_code: None,
                    execution_time_ms: 0,
                    metadata: HashMap::new(),
                })
            }
        }
    }

    /// Execute a create directory task with security validation
    async fn execute_create_directory(&mut self, task: &Task) -> Result<TaskExecutionResult> {
        let path = task.parameters.get("path")
            .and_then(|p| p.as_str())
            .ok_or_else(|| KaiError::task(&task.id, "Missing 'path' parameter"))?;

        // Validate and sanitize path (but allow non-existent paths for creation)
        let requested_path = PathBuf::from(path);
        let target_path = if requested_path.is_absolute() {
            requested_path
        } else {
            self.working_dir.join(&requested_path)
        };
        
        // Ensure target is within working directory
        if !target_path.starts_with(&self.working_dir) {
            self.log_security_audit(SecurityAuditEntry {
                task_id: task.id.clone(),
                operation: "create_directory".to_string(),
                path: target_path.clone(),
                timestamp: SystemTime::now(),
                allowed: false,
                reason: Some("Path outside working directory".to_string()),
            });
            
            return Ok(TaskExecutionResult {
                success: false,
                output: None,
                error: Some(format!(
                    "Cannot create directory outside working directory: {}",
                    target_path.display()
                )),
                stdout: None,
                stderr: None,
                exit_code: None,
                execution_time_ms: 0,
                metadata: HashMap::new(),
            });
        }
        
        // Log security audit
        self.log_security_audit(SecurityAuditEntry {
            task_id: task.id.clone(),
            operation: "create_directory".to_string(),
            path: target_path.clone(),
            timestamp: SystemTime::now(),
            allowed: true,
            reason: None,
        });
        
        debug!("Creating directory: {:?}", target_path);
        
        match fs::create_dir_all(&target_path).await {
            Ok(_) => {
                info!("Successfully created directory: {:?}", target_path);
                Ok(TaskExecutionResult {
                    success: true,
                    output: Some(serde_json::json!({
                        "path": path,
                        "sanitized_path": target_path.to_string_lossy(),
                        "created": true
                    })),
                    error: None,
                    stdout: None,
                    stderr: None,
                    exit_code: None,
                    execution_time_ms: 0,
                    metadata: HashMap::new(),
                })
            }
            Err(e) => {
                error!("Failed to create directory {:?}: {}", target_path, e);
                Ok(TaskExecutionResult {
                    success: false,
                    output: None,
                    error: Some(format!("Failed to create directory '{}': {}", path, e)),
                    stdout: None,
                    stderr: None,
                    exit_code: None,
                    execution_time_ms: 0,
                    metadata: HashMap::new(),
                })
            }
        }
    }

    /// Execute a delete task with safety checks and confirmation
    async fn execute_delete(&mut self, task: &Task) -> Result<TaskExecutionResult> {
        let path = task.parameters.get("path")
            .and_then(|p| p.as_str())
            .ok_or_else(|| KaiError::task(&task.id, "Missing 'path' parameter"))?;

        // Validate and sanitize path
        let sanitized_path = self.validate_and_sanitize_path(path, &task.id)?;
        
        // Safety check - require explicit confirmation for destructive operations
        let force_delete = task.parameters.get("force")
            .and_then(|f| f.as_bool())
            .unwrap_or(false);
        
        if !force_delete {
            return Ok(TaskExecutionResult {
                success: false,
                output: None,
                error: Some("Delete operation requires explicit 'force: true' parameter for safety".to_string()),
                stdout: None,
                stderr: None,
                exit_code: None,
                execution_time_ms: 0,
                metadata: HashMap::new(),
            });
        }
        
        // Additional safety checks for critical paths
        let path_str = sanitized_path.to_string_lossy();
        if path_str.contains("src") || path_str.contains(".git") || sanitized_path == self.working_dir {
            warn!("Attempted to delete critical path: {:?}", sanitized_path);
            self.log_security_audit(SecurityAuditEntry {
                task_id: task.id.clone(),
                operation: "delete".to_string(),
                path: sanitized_path.clone(),
                timestamp: SystemTime::now(),
                allowed: false,
                reason: Some("Critical path protection".to_string()),
            });
            
            return Ok(TaskExecutionResult {
                success: false,
                output: None,
                error: Some(format!("Cannot delete critical path: {}", path)),
                stdout: None,
                stderr: None,
                exit_code: None,
                execution_time_ms: 0,
                metadata: HashMap::new(),
            });
        }
        
        // Log security audit
        self.log_security_audit(SecurityAuditEntry {
            task_id: task.id.clone(),
            operation: "delete".to_string(),
            path: sanitized_path.clone(),
            timestamp: SystemTime::now(),
            allowed: true,
            reason: Some("Force delete requested".to_string()),
        });
        
        debug!("Deleting path: {:?}", sanitized_path);
        
        if !sanitized_path.exists() {
            return Ok(TaskExecutionResult {
                success: false,
                output: None,
                error: Some(format!("Path does not exist: {}", path)),
                stdout: None,
                stderr: None,
                exit_code: None,
                execution_time_ms: 0,
                metadata: HashMap::new(),
            });
        }

        let is_file = sanitized_path.is_file();
        let is_dir = sanitized_path.is_dir();
        
        let result = if is_file {
            fs::remove_file(&sanitized_path).await
        } else if is_dir {
            fs::remove_dir_all(&sanitized_path).await
        } else {
            return Ok(TaskExecutionResult {
                success: false,
                output: None,
                error: Some(format!("Path is neither file nor directory: {}", path)),
                stdout: None,
                stderr: None,
                exit_code: None,
                execution_time_ms: 0,
                metadata: HashMap::new(),
            });
        };

        match result {
            Ok(_) => {
                info!("Successfully deleted {:?}", sanitized_path);
                Ok(TaskExecutionResult {
                    success: true,
                    output: Some(serde_json::json!({
                        "path": path,
                        "sanitized_path": sanitized_path.to_string_lossy(),
                        "deleted": true,
                        "was_file": is_file,
                        "was_directory": is_dir
                    })),
                    error: None,
                    stdout: None,
                    stderr: None,
                    exit_code: None,
                    execution_time_ms: 0,
                    metadata: HashMap::new(),
                })
            }
            Err(e) => {
                error!("Failed to delete {:?}: {}", sanitized_path, e);
                Ok(TaskExecutionResult {
                    success: false,
                    output: None,
                    error: Some(format!("Failed to delete '{}': {}", path, e)),
                    stdout: None,
                    stderr: None,
                    exit_code: None,
                    execution_time_ms: 0,
                    metadata: HashMap::new(),
                })
            }
        }
    }

    /// List directory contents with security filtering
    async fn list_directory_contents(&mut self, path: &PathBuf, recursive: bool) -> Result<Vec<FileInfo>> {
        let mut files = Vec::new();
        let mut entries = fs::read_dir(path).await
            .map_err(|e| KaiError::execution(format!("Failed to read directory: {}", e)))?;

        while let Some(entry) = entries.next_entry().await
            .map_err(|e| KaiError::execution(format!("Failed to read directory entry: {}", e)))? {
            
            let entry_path = entry.path();
            
            // Skip hidden files and directories unless explicitly requested
            if entry_path.file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.starts_with('.'))
                .unwrap_or(false) {
                continue;
            }
            
            if entry_path.is_file() {
                if let Ok(metadata) = entry.metadata().await {
                    let file_info = FileInfo {
                        path: entry_path.to_string_lossy().to_string(),
                        is_file: true,
                        is_directory: false,
                        size: Some(metadata.len()),
                        modified: metadata.modified().ok()
                            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                            .map(|duration| duration.as_secs()),
                        permissions: get_file_permissions(&metadata),
                    };
                    files.push(file_info);
                }
            } else if entry_path.is_dir() {
                let dir_info = FileInfo {
                    path: entry_path.to_string_lossy().to_string(),
                    is_file: false,
                    is_directory: true,
                    size: None,
                    modified: None,
                    permissions: None,
                };
                files.push(dir_info.clone());
                
                if recursive {
                    let sub_files = Box::pin(self.list_directory_contents(&entry_path, recursive)).await?;
                    files.extend(sub_files);
                }
            }
        }

        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(files)
    }

    /// Validate and sanitize file paths within working directory
    fn validate_and_sanitize_path(&mut self, path: &str, task_id: &str) -> Result<PathBuf> {
        let requested_path = PathBuf::from(path);
        
        // Convert to absolute path based on working directory
        let absolute_path = if requested_path.is_absolute() {
            requested_path
        } else {
            self.working_dir.join(&requested_path)
        };
        
        // Canonicalize to resolve any .. or . components and symlinks
        let canonical_path = absolute_path.canonicalize()
            .unwrap_or_else(|_| absolute_path); // Use original if canonicalize fails
        
        // Ensure the path is within the working directory
        if !canonical_path.starts_with(&self.working_dir) {
            self.log_security_audit(SecurityAuditEntry {
                task_id: task_id.to_string(),
                operation: "path_validation".to_string(),
                path: canonical_path.clone(),
                timestamp: SystemTime::now(),
                allowed: false,
                reason: Some("Path outside working directory".to_string()),
            });
            
            return Err(KaiError::security(format!(
                "Path '{}' is outside the working directory '{}'",
                canonical_path.display(),
                self.working_dir.display()
            )));
        }
        
        Ok(canonical_path)
    }

    /// Validate command for security risks
    fn validate_command_security(&mut self, command: &str, task_id: &str) -> std::result::Result<(), KaiError> {
        let forbidden_commands = [
            "rm", "rmdir", "del", "format", "fdisk", "mkfs",
            "sudo", "su", "chmod", "chown", "passwd",
            "curl", "wget", "nc", "telnet", "ssh", "ftp",
            "python -c", "perl -e", "ruby -e", "node -e",
            "eval", "exec", "system",
        ];
        
        let dangerous_patterns = [
            "&&", ";", "|", ">>", ">", "<", "$(", "`",
            "rm -rf", "dd if=", ":(){ :|:& };:", "fork()",
        ];
        
        let command_lower = command.to_lowercase();
        
        // Check for forbidden commands
        for forbidden in &forbidden_commands {
            if command_lower.contains(forbidden) {
                self.log_security_audit(SecurityAuditEntry {
                    task_id: task_id.to_string(),
                    operation: "command_validation".to_string(),
                    path: PathBuf::from(command),
                    timestamp: SystemTime::now(),
                    allowed: false,
                    reason: Some(format!("Forbidden command: {}", forbidden)),
                });
                
                return Err(KaiError::security(format!("Forbidden command detected: {}", forbidden)));
            }
        }
        
        // Check for dangerous patterns
        for pattern in &dangerous_patterns {
            if command.contains(pattern) {
                self.log_security_audit(SecurityAuditEntry {
                    task_id: task_id.to_string(),
                    operation: "command_validation".to_string(),
                    path: PathBuf::from(command),
                    timestamp: SystemTime::now(),
                    allowed: false,
                    reason: Some(format!("Dangerous pattern: {}", pattern)),
                });
                
                return Err(KaiError::security(format!("Dangerous pattern detected: {}", pattern)));
            }
        }
        
        Ok(())
    }

    /// Log security audit entry (simplified implementation)
    fn log_security_audit(&mut self, entry: SecurityAuditEntry) {
        // Store the audit entry
        self.audit_log.push(entry.clone());
        if !entry.allowed {
            error!(
                "SECURITY AUDIT: Task {} attempted {} on {:?} - DENIED: {}",
                entry.task_id,
                entry.operation,
                entry.path,
                entry.reason.as_deref().unwrap_or("No reason provided")
            );
        } else {
            debug!(
                "SECURITY AUDIT: Task {} performed {} on {:?} - ALLOWED",
                entry.task_id,
                entry.operation,
                entry.path
            );
        }
        
        // Update resource tracker
        if entry.allowed {
            self.resource_tracker.record_operation(&entry.operation);
        }
        
        // In a real implementation, this would be stored in a persistent audit log
        // For now, we just use the logging framework
    }
}

/// Security audit entry for logging operations
#[derive(Debug, Clone)]
struct SecurityAuditEntry {
    task_id: String,
    operation: String,
    path: PathBuf,
    timestamp: SystemTime,
    allowed: bool,
    reason: Option<String>,
}

/// File information with metadata
#[derive(Debug, Clone, serde::Serialize)]
struct FileInfo {
    path: String,
    is_file: bool,
    is_directory: bool,
    size: Option<u64>,
    modified: Option<u64>, // Unix timestamp
    permissions: Option<String>,
}

/// Resource usage tracker
#[derive(Debug, Clone)]
struct ResourceTracker {
    task_count: u64,
    total_execution_time: Duration,
    memory_usage: u64,
}

impl ResourceTracker {
    fn new() -> Self {
        Self {
            task_count: 0,
            total_execution_time: Duration::ZERO,
            memory_usage: 0,
        }
    }
    
    fn record_task_execution(&mut self, execution_time: Duration) {
        self.task_count += 1;
        self.total_execution_time += execution_time;
    }
    
    fn record_operation(&mut self, _operation: &str) {
        // Record operation for tracking
        // Could be enhanced to track specific operation types
    }
    
    fn get_stats(&self) -> ResourceStats {
        ResourceStats {
            task_count: self.task_count,
            total_execution_time_ms: self.total_execution_time.as_millis() as u64,
            average_execution_time_ms: if self.task_count > 0 {
                (self.total_execution_time.as_millis() as u64) / self.task_count
            } else {
                0
            },
            memory_usage_bytes: self.memory_usage,
        }
    }
}

/// Resource usage statistics
#[derive(Debug, Clone, serde::Serialize)]
pub struct ResourceStats {
    pub task_count: u64,
    pub total_execution_time_ms: u64,
    pub average_execution_time_ms: u64,
    pub memory_usage_bytes: u64,
}

/// Get file permissions as a string
fn get_file_permissions(metadata: &std::fs::Metadata) -> Option<String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = metadata.permissions();
        Some(format!("{:o}", permissions.mode() & 0o777))
    }
    #[cfg(not(unix))]
    {
        Some(if metadata.permissions().readonly() {
            "readonly".to_string()
        } else {
            "readwrite".to_string()
        })
    }
}

/// Estimate code complexity (simplified)
fn estimate_complexity(content: &str, language: Option<&str>) -> u32 {
    let mut complexity = 1; // Base complexity
    
    // Count control flow statements
    let control_flow_patterns = match language {
        Some("rust") => vec!["if ", "else", "match ", "for ", "while ", "loop"],
        Some("javascript") | Some("typescript") => vec!["if (", "else", "switch", "for (", "while (", "catch"],
        Some("python") => vec!["if ", "elif", "else:", "for ", "while ", "except:"],
        _ => vec!["if", "else", "for", "while", "switch", "case"],
    };
    
    for pattern in control_flow_patterns {
        complexity += content.matches(pattern).count() as u32;
    }
    
    // Add complexity for functions/methods
    match language {
        Some("rust") => complexity += content.matches("fn ").count() as u32,
        Some("javascript") | Some("typescript") => {
            complexity += content.matches("function ").count() as u32;
            complexity += content.matches(" => ").count() as u32;
        }
        Some("python") => complexity += content.matches("def ").count() as u32,
        _ => {}
    }
    
    complexity
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
                "rb" => Some("ruby".to_string()),
                "php" => Some("php".to_string()),
                "cs" => Some("csharp".to_string()),
                "kt" => Some("kotlin".to_string()),
                "swift" => Some("swift".to_string()),
                "scala" => Some("scala".to_string()),
                "clj" | "cljs" => Some("clojure".to_string()),
                _ => None,
            }
        })
}