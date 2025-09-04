//! Global context management for project-wide state

use super::{ContextConfig, ContextEntry, FileMetadata};
use crate::llm::LlmProvider;
use crate::utils::errors::KaiError;
use crate::utils::fs::{discover_files, should_ignore_file};
use crate::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc};

/// Global context that maintains a summary of the entire project
#[derive(Debug, Clone)]
pub struct GlobalContext {
    /// Working directory being tracked
    working_directory: PathBuf,
    /// Map of file paths to their context entries
    file_contexts: HashMap<PathBuf, ContextEntry>,
    /// Overall project summary
    project_summary: Option<String>,
    /// Configuration for context generation
    config: ContextConfig,
    /// When the global context was last updated
    last_updated: DateTime<Utc>,
}

impl GlobalContext {
    /// Create a new global context
    pub fn new(working_directory: PathBuf, config: ContextConfig) -> Self {
        Self {
            working_directory,
            file_contexts: HashMap::new(),
            project_summary: None,
            config,
            last_updated: Utc::now(),
        }
    }

    /// Update the configuration
    pub fn update_config(&mut self, config: ContextConfig) {
        self.config = config;
        self.last_updated = Utc::now();
    }

    /// Refresh context by checking for modified files
    pub async fn refresh(&mut self, llm_provider: &dyn LlmProvider, model: &str) -> Result<()> {
        let current_files = self.discover_project_files().await?;
        let mut updated_files = Vec::new();

        // Check for new or modified files
        for file_path in &current_files {
            let relative_path = file_path.strip_prefix(&self.working_directory)
                .map_err(|_| KaiError::context("Failed to create relative path".to_string()))?
                .to_path_buf();

            let should_update = if let Some(existing_context) = self.file_contexts.get(&relative_path) {
                existing_context.is_outdated(file_path)?
            } else {
                true // New file
            };

            if should_update {
                self.update_file_context(&relative_path, llm_provider, model).await?;
                updated_files.push(relative_path);
            }
        }

        // Remove contexts for files that no longer exist
        let existing_files: Vec<PathBuf> = self.file_contexts.keys().cloned().collect();
        for relative_path in existing_files {
            let full_path = self.working_directory.join(&relative_path);
            if !full_path.exists() {
                self.file_contexts.remove(&relative_path);
                updated_files.push(relative_path);
            }
        }

        if !updated_files.is_empty() {
            self.regenerate_project_summary(llm_provider, model).await?;
        }

        self.last_updated = Utc::now();
        Ok(())
    }

    /// Force complete regeneration of all context
    pub async fn regenerate(&mut self, llm_provider: &dyn LlmProvider, model: &str) -> Result<()> {
        self.file_contexts.clear();
        
        let files = self.discover_project_files().await?;
        
        for file_path in &files {
            let relative_path = file_path.strip_prefix(&self.working_directory)
                .map_err(|_| KaiError::context("Failed to create relative path".to_string()))?
                .to_path_buf();
            
            self.update_file_context(&relative_path, llm_provider, model).await?;
        }

        self.regenerate_project_summary(llm_provider, model).await?;
        self.last_updated = Utc::now();
        Ok(())
    }

    /// Update context for a specific file
    pub async fn update_file_context(
        &mut self,
        relative_path: &Path,
        llm_provider: &dyn LlmProvider,
        model: &str,
    ) -> Result<()> {
        let full_path = self.working_directory.join(relative_path);
        
        if !full_path.exists() {
            self.file_contexts.remove(relative_path);
            return Ok(());
        }

        let metadata = FileMetadata::from_path(&full_path)?;
        
        // Skip binary files
        if metadata.is_binary {
            return Ok(());
        }

        // Skip files that are too large
        if metadata.size > self.config.max_file_size {
            return Ok(());
        }

        let content = std::fs::read_to_string(&full_path)
            .map_err(|e| KaiError::file_system(&full_path, e))?;

        let summary = self.generate_file_summary(&content, relative_path, llm_provider, model).await?;
        
        let context_entry = ContextEntry::new(relative_path.to_path_buf(), summary, metadata);
        self.file_contexts.insert(relative_path.to_path_buf(), context_entry);
        
        Ok(())
    }

    /// Get context for files matching a pattern
    pub async fn get_file_context(&self, pattern: &str) -> Result<String> {
        let files = self.resolve_file_pattern(pattern).await?;
        let mut context_parts = Vec::new();

        for file_path in files {
            let relative_path = file_path.strip_prefix(&self.working_directory)
                .map_err(|_| KaiError::context("Failed to create relative path".to_string()))?;

            if let Some(context) = self.file_contexts.get(relative_path) {
                context_parts.push(format!("// {}\n{}", file_path.display(), context.summary));
            } else {
                // If no context exists, read the file directly
                if file_path.is_file() {
                    let content = std::fs::read_to_string(&file_path)
                        .map_err(|e| KaiError::file_system(&file_path, e))?;
                    context_parts.push(format!("// {}\n{}", file_path.display(), content));
                }
            }
        }

        Ok(context_parts.join("\n\n"))
    }

    /// Get a summary of the entire project
    pub fn get_summary(&self) -> String {
        let mut summary = String::new();
        
        if let Some(project_summary) = &self.project_summary {
            summary.push_str("Project Overview:\n");
            summary.push_str(project_summary);
            summary.push_str("\n\n");
        }

        summary.push_str("File Structure:\n");
        let mut sorted_files: Vec<_> = self.file_contexts.keys().collect();
        sorted_files.sort();

        for relative_path in sorted_files {
            if let Some(context) = self.file_contexts.get(relative_path) {
                summary.push_str(&format!("- {}: {}\n", 
                    relative_path.display(), 
                    context.summary.lines().next().unwrap_or("No summary available")));
            }
        }

        summary
    }

    /// Check if any tracked files have been modified
    pub async fn has_modifications(&self) -> Result<bool> {
        for (relative_path, context) in &self.file_contexts {
            let full_path = self.working_directory.join(relative_path);
            if context.is_outdated(&full_path)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Get statistics about the global context
    pub async fn get_stats(&self) -> Result<super::manager::ContextStats> {
        let total_files = self.file_contexts.len();
        let mut outdated_files = 0;
        let mut total_size_bytes = 0;
        let mut languages = std::collections::HashSet::new();

        for (relative_path, context) in &self.file_contexts {
            let full_path = self.working_directory.join(relative_path);
            if context.is_outdated(&full_path)? {
                outdated_files += 1;
            }
            
            total_size_bytes += context.metadata.size;
            
            if let Some(language) = &context.metadata.language {
                languages.insert(language.clone());
            }
        }

        Ok(super::manager::ContextStats {
            total_files,
            outdated_files,
            total_size_bytes,
            languages: languages.into_iter().collect(),
            last_updated: self.last_updated,
        })
    }

    /// Discover all relevant files in the project
    async fn discover_project_files(&self) -> Result<Vec<PathBuf>> {
        discover_files(&self.working_directory, &self.config).await
    }

    /// Resolve a file pattern to actual file paths
    async fn resolve_file_pattern(&self, pattern: &str) -> Result<Vec<PathBuf>> {
        // If it's a direct file path, return it
        let pattern_path = self.working_directory.join(pattern);
        if pattern_path.is_file() {
            return Ok(vec![pattern_path]);
        }

        // If it's a directory, return all files in it
        if pattern_path.is_dir() {
            return self.get_files_in_directory(&pattern_path).await;
        }

        // Try glob pattern matching
        let glob_pattern = if pattern.contains('*') || pattern.contains('?') {
            pattern.to_string()
        } else {
            format!("**/*{}*", pattern)
        };

        self.glob_files(&glob_pattern).await
    }

    /// Get all files in a directory recursively
    async fn get_files_in_directory(&self, dir_path: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        let mut entries = tokio::fs::read_dir(dir_path).await
            .map_err(|e| KaiError::file_system(dir_path, e))?;

        while let Some(entry) = entries.next_entry().await
            .map_err(|e| KaiError::file_system(dir_path, e))? {
            
            let path = entry.path();
            
            if should_ignore_file(&path, &self.config).await? {
                continue;
            }

            if path.is_file() {
                files.push(path);
            } else if path.is_dir() {
                files.extend(self.get_files_in_directory(&path).await?);
            }
        }

        Ok(files)
    }

    /// Find files using glob patterns
    async fn glob_files(&self, pattern: &str) -> Result<Vec<PathBuf>> {
        let glob_pattern = self.working_directory.join(pattern);
        let pattern_str = glob_pattern.to_str()
            .ok_or_else(|| KaiError::context("Invalid glob pattern".to_string()))?;

        let mut files = Vec::new();
        for entry in glob::glob(pattern_str)
            .map_err(|e| KaiError::context(format!("Invalid glob pattern: {}", e)))? {
            
            let path = entry
                .map_err(|e| KaiError::context(format!("Glob error: {}", e)))?;
            
            if path.is_file() && !should_ignore_file(&path, &self.config).await? {
                files.push(path);
            }
        }

        Ok(files)
    }

    /// Generate a summary for a file using the LLM
    async fn generate_file_summary(
        &self,
        content: &str,
        file_path: &Path,
        llm_provider: &dyn LlmProvider,
        model: &str,
    ) -> Result<String> {
        let file_extension = file_path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("unknown");

        let prompt = format!(
            "Please provide a concise summary of this {} file. Focus on:\n\
            - Main purpose and functionality\n\
            - Key classes, functions, or components\n\
            - Important dependencies or relationships\n\
            - Any notable patterns or architectural decisions\n\
            \nKeep the summary under 200 words.\n\nFile content:\n{}",
            file_extension,
            content
        );

        llm_provider.generate_content(&prompt, "", model, None).await
            .map_err(|e| KaiError::context(format!("Failed to generate file summary: {}", e)))
    }

    /// Regenerate the overall project summary
    async fn regenerate_project_summary(
        &mut self,
        llm_provider: &dyn LlmProvider,
        model: &str,
    ) -> Result<()> {
        if self.file_contexts.is_empty() {
            self.project_summary = None;
            return Ok(());
        }

        let mut file_summaries = Vec::new();
        let mut sorted_files: Vec<_> = self.file_contexts.iter().collect();
        sorted_files.sort_by_key(|(path, _)| path.as_os_str());

        for (path, context) in sorted_files {
            file_summaries.push(format!("{}: {}", path.display(), context.summary));
        }

        let combined_summaries = file_summaries.join("\n\n");
        
        let prompt = format!(
            "Based on the following file summaries from a software project, \
            provide a high-level overview of the project including:\n\
            - Overall purpose and domain\n\
            - Main architectural patterns\n\
            - Key technologies and frameworks used\n\
            - Project structure and organization\n\
            \nKeep the overview concise but comprehensive (under 300 words).\n\n\
            File summaries:\n{}",
            combined_summaries
        );

        let summary = llm_provider.generate_content(&prompt, "", model, None).await
            .map_err(|e| KaiError::context(format!("Failed to generate project summary: {}", e)))?;

        self.project_summary = Some(summary);
        Ok(())
    }
}

/// Alias for backward compatibility
pub type FileContext = ContextEntry;