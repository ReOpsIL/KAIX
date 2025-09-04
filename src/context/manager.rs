//! Context manager for coordinating global and plan contexts

use super::{ContextConfig, GlobalContext, PlanContext};
use crate::llm::LlmProvider;
use crate::utils::errors::KaiError;
use crate::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Manages both global project context and temporary plan contexts
pub struct ContextManager {
    /// Working directory for the project
    working_directory: PathBuf,
    /// Global context for the entire project
    global_context: Arc<RwLock<GlobalContext>>,
    /// Configuration for context generation
    config: ContextConfig,
    /// LLM provider for generating context summaries
    llm_provider: Arc<dyn LlmProvider>,
    /// Current model to use for context generation
    model: String,
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
            config,
            llm_provider,
            model,
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

    /// Create a new plan context
    pub fn create_plan_context(&self, plan_id: String) -> PlanContext {
        PlanContext::new(plan_id)
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
}

/// Statistics about the current context
#[derive(Debug, Clone)]
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