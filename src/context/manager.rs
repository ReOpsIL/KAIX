//! Context manager for coordinating global and plan contexts

use super::{ContextConfig, GlobalContext, PlanContext};
use super::global::{ContextMemoryConfig, ContextMemoryStats};
use crate::llm::LlmProvider;
use crate::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

/// Manages both global project context and temporary plan contexts with health monitoring
pub struct ContextManager {
    /// Working directory for the project
    working_directory: PathBuf,
    /// Global context for the entire project
    global_context: Arc<RwLock<GlobalContext>>,
    /// Active plan contexts
    plan_contexts: HashMap<String, Arc<RwLock<PlanContext>>>,
    /// Configuration for context generation
    config: ContextConfig,
    /// LLM provider for generating context summaries
    llm_provider: Arc<dyn LlmProvider>,
    /// Current model to use for context generation
    model: String,
    /// Health monitoring configuration
    health_config: ContextHealthConfig,
    /// Last health check results
    last_health_check: Option<ContextHealthReport>,
    /// Manager creation time
    created_at: DateTime<Utc>,
}

impl ContextManager {
    /// Create a new context manager
    pub fn new(
        working_directory: PathBuf,
        llm_provider: Arc<dyn LlmProvider>,
        model: String,
        config: Option<ContextConfig>,
    ) -> Self {
        let config = config.unwrap_or_default();
        let global_context = Arc::new(RwLock::new(GlobalContext::new(
            working_directory.clone(),
            config.clone(),
        )));

        Self {
            working_directory,
            global_context,
            plan_contexts: HashMap::new(),
            config,
            llm_provider,
            model,
            health_config: ContextHealthConfig::default(),
            last_health_check: None,
            created_at: Utc::now(),
        }
    }
    
    /// Create a new context manager with custom health configuration
    pub fn with_health_config(
        working_directory: PathBuf,
        llm_provider: Arc<dyn LlmProvider>,
        model: String,
        config: Option<ContextConfig>,
        health_config: ContextHealthConfig,
    ) -> Self {
        let config = config.unwrap_or_default();
        let global_context = Arc::new(RwLock::new(GlobalContext::new(
            working_directory.clone(),
            config.clone(),
        )));

        Self {
            working_directory,
            global_context,
            plan_contexts: HashMap::new(),
            config,
            llm_provider,
            model,
            health_config,
            last_health_check: None,
            created_at: Utc::now(),
        }
    }
    
    /// Create a new context manager with memory configuration
    pub fn with_memory_config(
        working_directory: PathBuf,
        llm_provider: Arc<dyn LlmProvider>,
        model: String,
        config: Option<ContextConfig>,
        memory_config: ContextMemoryConfig,
    ) -> Self {
        let config = config.unwrap_or_default();
        let global_context = Arc::new(RwLock::new(GlobalContext::with_memory_config(
            working_directory.clone(),
            config.clone(),
            memory_config,
        )));

        Self {
            working_directory,
            global_context,
            plan_contexts: HashMap::new(),
            config,
            llm_provider,
            model,
            health_config: ContextHealthConfig::default(),
            last_health_check: None,
            created_at: Utc::now(),
        }
    }

    /// Get the working directory
    pub fn working_directory(&self) -> &Path {
        &self.working_directory
    }

    /// Set a new working directory and invalidate global context
    pub async fn set_working_directory(&mut self, path: PathBuf) -> Result<()> {
        self.working_directory = path.clone();
        
        let mut global_context = self.global_context.write().await;
        *global_context = GlobalContext::new(path, self.config.clone());
        
        Ok(())
    }

    /// Get a reference to the global context
    pub fn global_context(&self) -> Arc<RwLock<GlobalContext>> {
        Arc::clone(&self.global_context)
    }

    /// Create a new plan context and register it
    pub fn create_plan_context(&mut self, plan_id: String) -> Arc<RwLock<PlanContext>> {
        let plan_context = Arc::new(RwLock::new(PlanContext::new(plan_id.clone())));
        self.plan_contexts.insert(plan_id, Arc::clone(&plan_context));
        plan_context
    }
    
    /// Create plan context with tasks for dependency tracking
    pub fn create_plan_context_with_tasks(
        &mut self, 
        plan_id: String, 
        tasks: &[crate::planning::Task]
    ) -> Arc<RwLock<PlanContext>> {
        let plan_context = Arc::new(RwLock::new(
            PlanContext::new(plan_id.clone()).with_dependency_graph(tasks)
        ));
        self.plan_contexts.insert(plan_id, Arc::clone(&plan_context));
        plan_context
    }
    
    /// Get an existing plan context
    pub fn get_plan_context(&self, plan_id: &str) -> Option<Arc<RwLock<PlanContext>>> {
        self.plan_contexts.get(plan_id).cloned()
    }
    
    /// Remove a plan context
    pub fn remove_plan_context(&mut self, plan_id: &str) -> Option<Arc<RwLock<PlanContext>>> {
        self.plan_contexts.remove(plan_id)
    }
    
    /// List all active plan contexts
    pub fn list_plan_contexts(&self) -> Vec<String> {
        self.plan_contexts.keys().cloned().collect()
    }

    /// Update the global context by scanning for file changes
    pub async fn refresh_global_context(&self) -> Result<()> {
        let mut global_context = self.global_context.write().await;
        global_context.refresh(&*self.llm_provider, &self.model).await
    }

    /// Force regeneration of the entire global context
    pub async fn regenerate_global_context(&self) -> Result<()> {
        let mut global_context = self.global_context.write().await;
        global_context.regenerate(&*self.llm_provider, &self.model).await
    }

    /// Update global context for specific files that were modified
    pub async fn update_global_context_for_files(&self, file_paths: &[PathBuf]) -> Result<()> {
        let mut global_context = self.global_context.write().await;
        
        for file_path in file_paths {
            if file_path.starts_with(&self.working_directory) {
                global_context.update_file_context(
                    file_path,
                    &*self.llm_provider,
                    &self.model,
                ).await?;
            }
        }
        
        Ok(())
    }

    /// Get context for specific files or directories (for @ commands)
    pub async fn get_file_context(&self, pattern: &str) -> Result<String> {
        let global_context = self.global_context.read().await;
        global_context.get_file_context(pattern).await
    }

    /// Get a summary of the global context suitable for LLM prompts
    pub async fn get_global_context_summary(&self) -> Result<String> {
        let global_context = self.global_context.read().await;
        Ok(global_context.get_summary())
    }

    /// Update the context configuration
    pub async fn update_config(&mut self, config: ContextConfig) -> Result<()> {
        self.config = config.clone();
        let mut global_context = self.global_context.write().await;
        global_context.update_config(config);
        Ok(())
    }

    /// Get the current context configuration
    pub fn get_config(&self) -> &ContextConfig {
        &self.config
    }
    
    /// Update the LLM model used for context generation
    pub fn set_model(&mut self, model: String) {
        self.model = model;
    }
    
    /// Get the current LLM model
    pub fn get_model(&self) -> &str {
        &self.model
    }

    /// Check if any files in the working directory have been modified
    pub async fn has_modifications(&self) -> Result<bool> {
        let global_context = self.global_context.read().await;
        global_context.has_modifications().await
    }

    /// Get statistics about the global context
    pub async fn get_context_stats(&self) -> Result<ContextStats> {
        let global_context = self.global_context.read().await;
        global_context.get_stats().await
    }
    
    /// Get comprehensive manager statistics
    pub async fn get_manager_stats(&self) -> Result<ContextManagerStats> {
        let global_context = self.global_context.read().await;
        let global_stats = global_context.get_stats().await?;
        let memory_stats = global_context.get_memory_stats();
        
        let plan_context_count = self.plan_contexts.len();
        let mut total_plan_outputs = 0;
        let mut total_plan_memory = 0;
        
        for plan_context in self.plan_contexts.values() {
            let ctx = plan_context.read().await;
            let plan_memory = ctx.get_memory_stats();
            total_plan_outputs += plan_memory.total_outputs;
            total_plan_memory += plan_memory.total_estimated_memory_bytes;
        }
        
        Ok(ContextManagerStats {
            global_context_stats: global_stats,
            memory_stats,
            active_plan_contexts: plan_context_count,
            total_plan_outputs,
            total_plan_memory_bytes: total_plan_memory,
            uptime_seconds: Utc::now().signed_duration_since(self.created_at).num_seconds() as u64,
        })
    }
    
    /// Perform comprehensive health check
    pub async fn health_check(&mut self) -> Result<ContextHealthReport> {
        let mut report = ContextHealthReport::new();
        
        // Check global context health
        let mut global_context = self.global_context.write().await;
        
        // 1. Check memory usage
        let memory_stats = global_context.get_memory_stats();
        if memory_stats.memory_usage_percentage > self.health_config.memory_warning_threshold {
            report.warnings.push(ContextWarning {
                category: "memory".to_string(),
                message: format!("Memory usage at {}% (limit: {} bytes)", 
                               memory_stats.memory_usage_percentage, 
                               memory_stats.memory_limit_bytes),
                severity: if memory_stats.memory_usage_percentage > self.health_config.memory_critical_threshold {
                    WarningSeverity::Critical
                } else {
                    WarningSeverity::Warning
                },
            });
        }
        
        // 2. Check for file modifications
        let mod_check = global_context.check_modifications_detailed().await?;
        if !mod_check.modified_files.is_empty() || !mod_check.new_files.is_empty() {
            report.warnings.push(ContextWarning {
                category: "staleness".to_string(),
                message: format!("Context may be stale: {} modified files, {} new files", 
                               mod_check.modified_files.len(), mod_check.new_files.len()),
                severity: WarningSeverity::Info,
            });
            
            report.stale_files_count = mod_check.modified_files.len() + mod_check.new_files.len();
        }
        
        // 3. Check context age
        let stats = global_context.get_stats().await?;
        let context_age = Utc::now().signed_duration_since(stats.last_updated).num_hours();
        if context_age > self.health_config.max_context_age_hours as i64 {
            report.warnings.push(ContextWarning {
                category: "age".to_string(),
                message: format!("Context is {} hours old (max: {})", 
                               context_age, self.health_config.max_context_age_hours),
                severity: WarningSeverity::Warning,
            });
        }
        
        // 4. Clean up caches if needed
        global_context.cleanup_caches();
        
        // 5. Check plan contexts health
        let mut unhealthy_plans = Vec::new();
        for (plan_id, plan_context_arc) in &self.plan_contexts {
            let plan_context = plan_context_arc.read().await;
            let plan_memory = plan_context.get_memory_stats();
            
            if plan_memory.total_estimated_memory_bytes > self.health_config.max_plan_memory_bytes {
                unhealthy_plans.push(plan_id.clone());
            }
        }
        
        if !unhealthy_plans.is_empty() {
            report.warnings.push(ContextWarning {
                category: "plan_memory".to_string(),
                message: format!("{} plan contexts exceed memory limits", unhealthy_plans.len()),
                severity: WarningSeverity::Warning,
            });
        }
        
        // 6. Overall health assessment
        let critical_warnings = report.warnings.iter().filter(|w| matches!(w.severity, WarningSeverity::Critical)).count();
        let warnings = report.warnings.iter().filter(|w| matches!(w.severity, WarningSeverity::Warning)).count();
        
        report.overall_health = if critical_warnings > 0 {
            OverallHealth::Critical
        } else if warnings > 0 {
            OverallHealth::Warning
        } else {
            OverallHealth::Healthy
        };
        
        report.completed_at = Utc::now();
        self.last_health_check = Some(report.clone());
        
        tracing::info!(
            "Health check completed: {} ({} warnings, {} critical)", 
            report.overall_health, warnings, critical_warnings
        );
        
        Ok(report)
    }
    
    /// Perform maintenance tasks
    pub async fn maintenance(&mut self) -> Result<MaintenanceReport> {
        let mut report = MaintenanceReport::new();
        
        // 1. Clean up old plan contexts
        let cutoff_time = Utc::now() - chrono::Duration::hours(self.health_config.plan_cleanup_age_hours as i64);
        let mut removed_plans = Vec::new();
        
        self.plan_contexts.retain(|plan_id, plan_arc| {
            // We can't await here, so we'll use try_read to check if we can access the plan
            if let Ok(plan) = plan_arc.try_read() {
                if plan.created_at < cutoff_time {
                    removed_plans.push(plan_id.clone());
                    false
                } else {
                    true
                }
            } else {
                true // Keep if we can't check
            }
        });
        
        report.cleaned_plan_contexts = removed_plans.len();
        
        // 2. Update global context if needed
        let health_check = self.health_check().await?;
        if health_check.stale_files_count > 0 {
            let mut global_context = self.global_context.write().await;
            let update_result = global_context.update_modified_files(&*self.llm_provider, &self.model).await?;
            report.updated_files = update_result.total_changes;
        }
        
        // 3. Clean up plan context memory
        for plan_context_arc in self.plan_contexts.values() {
            let mut plan_context = plan_context_arc.write().await;
            let old_outputs = plan_context.get_memory_stats().total_outputs;
            plan_context.cleanup_old_outputs(
                self.health_config.max_plan_outputs,
                self.health_config.plan_cleanup_age_hours as i64,
            );
            let new_outputs = plan_context.get_memory_stats().total_outputs;
            report.cleaned_plan_outputs += old_outputs.saturating_sub(new_outputs);
        }
        
        report.completed_at = Utc::now();
        
        tracing::info!(
            "Maintenance completed: {} files updated, {} plan contexts cleaned, {} outputs cleaned",
            report.updated_files,
            report.cleaned_plan_contexts,
            report.cleaned_plan_outputs
        );
        
        Ok(report)
    }
    
    /// Validate context consistency
    pub async fn validate_consistency(&self) -> Result<ValidationReport> {
        let mut report = ValidationReport::new();
        
        let global_context = self.global_context.read().await;
        
        // 1. Check if all tracked files still exist
        let mod_check = global_context.check_modifications_detailed().await?;
        report.missing_files = mod_check.deleted_files;
        report.outdated_files = mod_check.modified_files.len();
        
        // 2. Validate plan context dependencies
        for (plan_id, plan_context_arc) in &self.plan_contexts {
            let plan_context = plan_context_arc.read().await;
            
            // Check for circular dependencies
            if let Some(circular_deps) = self.detect_circular_dependencies(&plan_context) {
                report.validation_errors.push(ValidationError {
                    error_type: "circular_dependency".to_string(),
                    message: format!("Circular dependency detected in plan {}: {:?}", plan_id, circular_deps),
                });
            }
        }
        
        // 3. Check memory consistency
        let memory_stats = global_context.get_memory_stats();
        let stats = global_context.get_stats().await?;
        if memory_stats.total_memory_bytes == 0 && stats.total_files > 0 {
            report.validation_errors.push(ValidationError {
                error_type: "memory_inconsistency".to_string(),
                message: "Memory usage is zero but file contexts exist".to_string(),
            });
        }
        
        report.is_valid = report.validation_errors.is_empty();
        report.completed_at = Utc::now();
        
        Ok(report)
    }
    
    /// Detect circular dependencies in plan context
    fn detect_circular_dependencies(&self, plan_context: &PlanContext) -> Option<Vec<String>> {
        use std::collections::{HashSet, VecDeque};
        
        for (task_id, _dependencies) in &plan_context.dependency_graph {
            let mut visited = HashSet::new();
            let mut stack = VecDeque::new();
            stack.push_back(task_id.clone());
            
            while let Some(current_task) = stack.pop_back() {
                if visited.contains(&current_task) {
                    // Found a cycle
                    return Some(visited.into_iter().collect());
                }
                
                visited.insert(current_task.clone());
                
                if let Some(deps) = plan_context.dependency_graph.get(&current_task) {
                    for dep in deps {
                        if !visited.contains(dep) {
                            stack.push_back(dep.clone());
                        }
                    }
                }
            }
        }
        
        None
    }
    
    /// Get the last health check report
    pub fn get_last_health_check(&self) -> Option<&ContextHealthReport> {
        self.last_health_check.as_ref()
    }
}

/// Statistics about the current context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextStats {
    /// Total number of files tracked
    pub total_files: usize,
    /// Number of outdated context entries
    pub outdated_files: usize,
    /// Total size of all tracked files in bytes
    pub total_size_bytes: u64,
    /// Number of different programming languages detected
    pub languages: Vec<String>,
    /// Last update timestamp
    pub last_updated: chrono::DateTime<chrono::Utc>,
}

/// Comprehensive context manager statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextManagerStats {
    /// Global context statistics
    pub global_context_stats: ContextStats,
    /// Memory usage statistics
    pub memory_stats: ContextMemoryStats,
    /// Number of active plan contexts
    pub active_plan_contexts: usize,
    /// Total outputs across all plan contexts
    pub total_plan_outputs: usize,
    /// Total memory used by plan contexts
    pub total_plan_memory_bytes: usize,
    /// Manager uptime in seconds
    pub uptime_seconds: u64,
}

/// Health monitoring configuration
#[derive(Debug, Clone)]
pub struct ContextHealthConfig {
    /// Memory usage warning threshold (percentage)
    pub memory_warning_threshold: u8,
    /// Memory usage critical threshold (percentage)
    pub memory_critical_threshold: u8,
    /// Maximum context age before warning (hours)
    pub max_context_age_hours: u32,
    /// Maximum memory per plan context (bytes)
    pub max_plan_memory_bytes: usize,
    /// Age threshold for cleaning up old plan contexts (hours)
    pub plan_cleanup_age_hours: u32,
    /// Maximum outputs per plan context before cleanup
    pub max_plan_outputs: usize,
}

impl Default for ContextHealthConfig {
    fn default() -> Self {
        Self {
            memory_warning_threshold: 80,
            memory_critical_threshold: 95,
            max_context_age_hours: 24,
            max_plan_memory_bytes: 50 * 1024 * 1024, // 50MB per plan
            plan_cleanup_age_hours: 48,
            max_plan_outputs: 100,
        }
    }
}

/// Health check report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextHealthReport {
    /// Overall health status
    pub overall_health: OverallHealth,
    /// List of warnings and issues
    pub warnings: Vec<ContextWarning>,
    /// Number of stale files detected
    pub stale_files_count: usize,
    /// When the health check was completed
    pub completed_at: DateTime<Utc>,
}

impl ContextHealthReport {
    fn new() -> Self {
        Self {
            overall_health: OverallHealth::Healthy,
            warnings: Vec::new(),
            stale_files_count: 0,
            completed_at: Utc::now(),
        }
    }
}

/// Overall health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OverallHealth {
    Healthy,
    Warning,
    Critical,
}

impl std::fmt::Display for OverallHealth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OverallHealth::Healthy => write!(f, "Healthy"),
            OverallHealth::Warning => write!(f, "Warning"),
            OverallHealth::Critical => write!(f, "Critical"),
        }
    }
}

/// Context warning or issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextWarning {
    /// Category of the warning
    pub category: String,
    /// Warning message
    pub message: String,
    /// Severity level
    pub severity: WarningSeverity,
}

/// Warning severity levels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WarningSeverity {
    Info,
    Warning,
    Critical,
}

/// Maintenance report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaintenanceReport {
    /// Number of files updated
    pub updated_files: usize,
    /// Number of plan contexts cleaned up
    pub cleaned_plan_contexts: usize,
    /// Number of plan outputs cleaned up
    pub cleaned_plan_outputs: usize,
    /// When maintenance was completed
    pub completed_at: DateTime<Utc>,
}

impl MaintenanceReport {
    fn new() -> Self {
        Self {
            updated_files: 0,
            cleaned_plan_contexts: 0,
            cleaned_plan_outputs: 0,
            completed_at: Utc::now(),
        }
    }
}

/// Validation report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    /// Whether the context is valid
    pub is_valid: bool,
    /// List of validation errors
    pub validation_errors: Vec<ValidationError>,
    /// Files that no longer exist
    pub missing_files: Vec<PathBuf>,
    /// Number of outdated files
    pub outdated_files: usize,
    /// When validation was completed
    pub completed_at: DateTime<Utc>,
}

impl ValidationReport {
    fn new() -> Self {
        Self {
            is_valid: true,
            validation_errors: Vec::new(),
            missing_files: Vec::new(),
            outdated_files: 0,
            completed_at: Utc::now(),
        }
    }
}

/// Validation error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// Type of validation error
    pub error_type: String,
    /// Error message
    pub message: String,
}