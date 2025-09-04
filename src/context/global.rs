//! Global context management for project-wide state

use super::{ContextConfig, ContextEntry, FileMetadata};
use crate::llm::LlmProvider;
use crate::utils::errors::KaiError;
use crate::utils::fs::{discover_files, should_ignore_file, resolve_path_pattern, expand_glob_pattern};
use crate::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc};
use tokio::fs;
use ignore::{Walk, WalkBuilder};
use serde::{Serialize, Deserialize};

/// Global context that maintains a summary of the entire project
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Cache of discovered files to avoid repeated scanning
    #[serde(skip)]
    file_discovery_cache: Option<(DateTime<Utc>, Vec<PathBuf>)>,
    /// Statistics about the context
    stats: GlobalContextStats,
}

/// Statistics about the global context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalContextStats {
    /// Total number of files tracked
    pub total_files: usize,
    /// Number of files successfully processed
    pub processed_files: usize,
    /// Number of files skipped (binary, too large, etc.)
    pub skipped_files: usize,
    /// Total size of processed files in bytes
    pub total_size_bytes: u64,
    /// Languages detected in the project
    pub languages: HashSet<String>,
    /// Directories with the most files
    pub top_directories: Vec<(PathBuf, usize)>,
    /// Last scan duration in milliseconds
    pub last_scan_duration_ms: u64,
}

impl Default for GlobalContextStats {
    fn default() -> Self {
        Self {
            total_files: 0,
            processed_files: 0,
            skipped_files: 0,
            total_size_bytes: 0,
            languages: HashSet::new(),
            top_directories: Vec::new(),
            last_scan_duration_ms: 0,
        }
    }
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
            file_discovery_cache: None,
            stats: GlobalContextStats::default(),
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

    /// Get context for files matching a pattern with intelligent content handling
    pub async fn get_file_context(&self, pattern: &str) -> Result<String> {
        let files = self.resolve_file_pattern(pattern).await?;
        
        if files.is_empty() {
            return Ok(format!("No files found matching pattern: '{}'", pattern));
        }
        
        tracing::debug!("Getting context for {} files matching '{}'", files.len(), pattern);
        
        let mut context_parts = Vec::new();
        let mut total_content_size = 0;
        const MAX_TOTAL_SIZE: usize = 50000;  // Limit total content size
        
        for (index, file_path) in files.iter().enumerate() {
            let relative_path = file_path.strip_prefix(&self.working_directory)
                .map_err(|_| KaiError::context("Failed to create relative path".to_string()))?;
            
            let context_entry = if let Some(context) = self.file_contexts.get(relative_path) {
                // Use existing summary
                format!("File: {}\nSummary: {}\n", relative_path.display(), context.summary)
            } else {
                // Generate content on-the-fly
                match self.get_file_content_for_context(file_path, total_content_size < MAX_TOTAL_SIZE).await {
                    Ok(content) => {
                        total_content_size += content.len();
                        format!("File: {}\nContent: {}\n", relative_path.display(), content)
                    }
                    Err(e) => {
                        tracing::warn!("Failed to read file {}: {}", file_path.display(), e);
                        format!("File: {}\nError: Unable to read file\n", relative_path.display())
                    }
                }
            };
            
            context_parts.push(context_entry);
            
            // Limit number of files to prevent overwhelming the context
            if index >= 20 {
                context_parts.push(format!("... and {} more files (truncated for brevity)", 
                                         files.len() - index - 1));
                break;
            }
            
            // Stop if we've reached the content size limit
            if total_content_size >= MAX_TOTAL_SIZE {
                context_parts.push("... (content truncated to prevent overwhelming context)".to_string());
                break;
            }
        }
        
        let header = format!("Found {} file(s) matching pattern '{}':\n\n", files.len(), pattern);
        Ok(format!("{}{}", header, context_parts.join("\n")))
    }
    
    /// Get file content for context, with size and type awareness
    async fn get_file_content_for_context(&self, file_path: &Path, include_full_content: bool) -> Result<String> {
        // Check if file should be included
        if !self.should_include_file(file_path).await? {
            return Ok("[File excluded from context]".to_string());
        }
        
        let content = fs::read_to_string(file_path).await
            .map_err(|e| KaiError::file_system(file_path, e))?;
        
        if !include_full_content || content.len() > 5000 {
            // Return truncated content with summary
            let truncated = if content.len() > 1000 {
                format!("{}\n\n... (truncated, {} total characters)", 
                       &content[..1000], content.len())
            } else {
                content
            };
            
            Ok(truncated)
        } else {
            Ok(content)
        }
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

    /// Discover all relevant files in the project with intelligent caching and filtering
    async fn discover_project_files(&mut self) -> Result<Vec<PathBuf>> {
        let start_time = std::time::Instant::now();
        
        // Check if we can use cached results
        if let Some((cache_time, cached_files)) = &self.file_discovery_cache {
            let cache_age = Utc::now().signed_duration_since(*cache_time);
            if cache_age.num_minutes() < 5 {  // Cache valid for 5 minutes
                tracing::debug!("Using cached file discovery results ({} files)", cached_files.len());
                return Ok(cached_files.clone());
            }
        }
        
        tracing::info!("Discovering files in {}", self.working_directory.display());
        
        // Use the enhanced file discovery with git-aware filtering
        let files = self.discover_files_with_advanced_filtering().await?;
        
        // Update statistics
        let scan_duration = start_time.elapsed().as_millis() as u64;
        self.stats.last_scan_duration_ms = scan_duration;
        self.stats.total_files = files.len();
        
        // Cache the results
        self.file_discovery_cache = Some((Utc::now(), files.clone()));
        
        tracing::info!("Discovered {} files in {}ms", files.len(), scan_duration);
        Ok(files)
    }
    
    /// Advanced file discovery with comprehensive filtering
    async fn discover_files_with_advanced_filtering(&mut self) -> Result<Vec<PathBuf>> {
        let mut builder = WalkBuilder::new(&self.working_directory);
        
        // Configure the walker with git-aware filtering
        builder
            .git_ignore(true)
            .git_global(true)
            .git_exclude(true)
            .add_custom_ignore_filename(".aiignore")
            .hidden(false)  // Include hidden files but respect .gitignore
            .follow_links(self.config.follow_symlinks);
            
        if let Some(depth) = self.config.max_depth {
            builder.max_depth(Some(depth));
        }
        
        let walker = builder.build();
        let mut files = Vec::new();
        let mut directory_counts: HashMap<PathBuf, usize> = HashMap::new();
        let mut processed_files = 0;
        let mut skipped_files = 0;
        let mut total_size = 0u64;
        let mut languages = HashSet::new();
        
        for result in walker {
            match result {
                Ok(entry) => {
                    if entry.file_type().map_or(false, |ft| ft.is_file()) {
                        let path = entry.path().to_path_buf();
                        
                        // Apply additional filtering
                        if self.should_include_file(&path).await? {
                            files.push(path.clone());
                            processed_files += 1;
                            
                            // Update statistics
                            if let Ok(metadata) = fs::metadata(&path).await {
                                total_size += metadata.len();
                            }
                            
                            // Track directory counts
                            if let Some(parent) = path.parent() {
                                let relative_parent = parent.strip_prefix(&self.working_directory)
                                    .unwrap_or(parent).to_path_buf();
                                *directory_counts.entry(relative_parent).or_insert(0) += 1;
                            }
                            
                            // Track languages
                            if let Some(ext) = path.extension() {
                                if let Some(ext_str) = ext.to_str() {
                                    if let Some(lang) = self.detect_language_from_extension(ext_str) {
                                        languages.insert(lang);
                                    }
                                }
                            }
                        } else {
                            skipped_files += 1;
                        }
                    }
                }
                Err(err) => {
                    tracing::warn!("Failed to process file entry: {}", err);
                }
            }
        }
        
        // Update statistics
        self.stats.processed_files = processed_files;
        self.stats.skipped_files = skipped_files;
        self.stats.total_size_bytes = total_size;
        self.stats.languages = languages;
        
        // Get top directories by file count
        let mut dir_vec: Vec<_> = directory_counts.into_iter().collect();
        dir_vec.sort_by(|a, b| b.1.cmp(&a.1));
        self.stats.top_directories = dir_vec.into_iter().take(10).collect();
        
        // Prioritize files by importance (source code first)
        files.sort_by(|a, b| self.get_file_priority(a).cmp(&self.get_file_priority(b)));
        
        Ok(files)
    }
    
    /// Check if a file should be included in the context
    async fn should_include_file(&self, path: &Path) -> Result<bool> {
        // Check file size limit
        if let Ok(metadata) = fs::metadata(path).await {
            if metadata.len() > self.config.max_file_size {
                tracing::debug!("Skipping large file: {} ({} bytes)", 
                               path.display(), metadata.len());
                return Ok(false);
            }
        }
        
        // Check exclude patterns
        let path_str = path.to_string_lossy();
        for pattern in &self.config.exclude_patterns {
            if glob::Pattern::new(pattern)
                .map_err(|e| KaiError::context(format!("Invalid glob pattern '{}': {}", pattern, e)))?
                .matches(&path_str) {
                tracing::debug!("Skipping file matching pattern '{}': {}", pattern, path.display());
                return Ok(false);
            }
        }
        
        // Check if it's a binary file (for text files only)
        if self.is_likely_binary_file(path).await? {
            tracing::debug!("Skipping binary file: {}", path.display());
            return Ok(false);
        }
        
        // Check for common unimportant files
        if self.is_unimportant_file(path) {
            tracing::debug!("Skipping unimportant file: {}", path.display());
            return Ok(false);
        }
        
        Ok(true)
    }
    
    /// Detect if a file is likely binary by examining its content
    async fn is_likely_binary_file(&self, path: &Path) -> Result<bool> {
        match fs::File::open(path).await {
            Ok(mut file) => {
                let mut buffer = [0; 512];
                match tokio::io::AsyncReadExt::read(&mut file, &mut buffer).await {
                    Ok(bytes_read) if bytes_read > 0 => {
                        // Check for null bytes (common in binary files)
                        let null_byte_ratio = buffer[..bytes_read].iter().filter(|&&b| b == 0).count() as f64 
                                             / bytes_read as f64;
                        Ok(null_byte_ratio > 0.01) // If more than 1% null bytes, likely binary
                    }
                    _ => Ok(false), // If we can't read, assume text
                }
            }
            Err(_) => Ok(false), // If we can't open, don't exclude
        }
    }
    
    /// Check if a file is generally unimportant for code context
    fn is_unimportant_file(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy().to_lowercase();
        let name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        // Check for common unimportant files
        let unimportant_patterns = [
            ".ds_store", "thumbs.db", "desktop.ini",
            "package-lock.json", "yarn.lock", "pnpm-lock.yaml",
            "*.min.js", "*.min.css", "*.bundle.js",
            "changelog", "license", "copying", "authors",
            ".env.example", ".env.template",
        ];
        
        for pattern in &unimportant_patterns {
            if name.contains(pattern) || path_str.contains(pattern) {
                return true;
            }
        }
        
        // Check for generated/build directories in path
        let build_dirs = ["target/", "build/", "dist/", "out/", ".next/", ".nuxt/"];
        for dir in &build_dirs {
            if path_str.contains(dir) {
                return true;
            }
        }
        
        false
    }
    
    /// Get priority score for file sorting (lower number = higher priority)
    fn get_file_priority(&self, path: &Path) -> u32 {
        let extension = path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_lowercase();
        
        // Source code files get highest priority
        if self.config.priority_extensions.contains(&extension) {
            match extension.as_str() {
                "rs" => 1,  // Rust files first
                "js" | "ts" | "jsx" | "tsx" => 2,  // JavaScript/TypeScript
                "py" => 3,  // Python
                "java" | "kt" | "scala" => 4,  // JVM languages
                "cpp" | "c" | "h" | "hpp" => 5,  // C/C++
                "go" => 6,  // Go
                _ => 10,  // Other priority languages
            }
        } else {
            match extension.as_str() {
                "toml" | "yaml" | "yml" | "json" => 20,  // Config files
                "md" => 25,  // Documentation
                "txt" => 30,  // Text files
                "" => 40,  // Files without extension
                _ => 50,  // Everything else
            }
        }
    }
    
    /// Detect language from file extension
    fn detect_language_from_extension(&self, extension: &str) -> Option<String> {
        match extension.to_lowercase().as_str() {
            "rs" => Some("Rust".to_string()),
            "js" | "jsx" => Some("JavaScript".to_string()),
            "ts" | "tsx" => Some("TypeScript".to_string()),
            "py" => Some("Python".to_string()),
            "java" => Some("Java".to_string()),
            "kt" => Some("Kotlin".to_string()),
            "scala" => Some("Scala".to_string()),
            "cpp" | "cc" | "cxx" => Some("C++".to_string()),
            "c" => Some("C".to_string()),
            "h" | "hpp" => Some("C/C++ Header".to_string()),
            "go" => Some("Go".to_string()),
            "rb" => Some("Ruby".to_string()),
            "php" => Some("PHP".to_string()),
            "cs" => Some("C#".to_string()),
            "swift" => Some("Swift".to_string()),
            "clj" | "cljs" => Some("Clojure".to_string()),
            "hs" => Some("Haskell".to_string()),
            "ml" => Some("OCaml".to_string()),
            "elm" => Some("Elm".to_string()),
            "dart" => Some("Dart".to_string()),
            "r" => Some("R".to_string()),
            "jl" => Some("Julia".to_string()),
            "lua" => Some("Lua".to_string()),
            "sh" | "bash" | "zsh" => Some("Shell".to_string()),
            "sql" => Some("SQL".to_string()),
            "html" => Some("HTML".to_string()),
            "css" => Some("CSS".to_string()),
            "xml" => Some("XML".to_string()),
            "json" => Some("JSON".to_string()),
            "yaml" | "yml" => Some("YAML".to_string()),
            "toml" => Some("TOML".to_string()),
            "md" => Some("Markdown".to_string()),
            _ => None,
        }
    }

    /// Resolve a file pattern to actual file paths with intelligent pattern matching
    async fn resolve_file_pattern(&self, pattern: &str) -> Result<Vec<PathBuf>> {
        tracing::debug!("Resolving file pattern: '{}'", pattern);
        
        // Normalize the pattern
        let normalized_pattern = pattern.trim();
        if normalized_pattern.is_empty() {
            return Ok(vec![]);
        }
        
        // Handle absolute vs relative paths
        let base_path = if normalized_pattern.starts_with('/') {
            PathBuf::from("/")
        } else {
            self.working_directory.clone()
        };
        
        let pattern_path = base_path.join(normalized_pattern);
        
        // If it's a direct file path, return it
        if pattern_path.is_file() {
            tracing::debug!("Pattern resolved to single file: {}", pattern_path.display());
            return Ok(vec![pattern_path]);
        }
        
        // If it's a directory, return all files in it recursively
        if pattern_path.is_dir() {
            tracing::debug!("Pattern resolved to directory: {}", pattern_path.display());
            return self.get_files_in_directory_filtered(&pattern_path).await;
        }
        
        // Handle special pattern types
        if normalized_pattern.contains('*') || normalized_pattern.contains('?') {
            // Direct glob pattern
            return self.glob_files_advanced(normalized_pattern).await;
        } else if normalized_pattern.contains('/') {
            // Path with directories
            return self.resolve_path_with_directories(normalized_pattern).await;
        } else {
            // Simple string - search by filename or extension
            return self.search_by_name_or_extension(normalized_pattern).await;
        }
    }
    
    /// Get all files in a directory recursively with filtering
    async fn get_files_in_directory_filtered(&self, dir_path: &Path) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();
        let mut builder = WalkBuilder::new(dir_path);
        
        builder
            .git_ignore(true)
            .add_custom_ignore_filename(".aiignore")
            .hidden(false)
            .follow_links(self.config.follow_symlinks);
            
        if let Some(depth) = self.config.max_depth {
            builder.max_depth(Some(depth));
        }
        
        let walker = builder.build();
        
        for result in walker {
            match result {
                Ok(entry) => {
                    if entry.file_type().map_or(false, |ft| ft.is_file()) {
                        let path = entry.path().to_path_buf();
                        if self.should_include_file(&path).await? {
                            files.push(path);
                        }
                    }
                }
                Err(err) => {
                    tracing::warn!("Failed to process directory entry in {}: {}", 
                                 dir_path.display(), err);
                }
            }
        }
        
        // Sort files by priority
        files.sort_by(|a, b| self.get_file_priority(a).cmp(&self.get_file_priority(b)));
        
        tracing::debug!("Found {} files in directory {}", files.len(), dir_path.display());
        Ok(files)
    }
    
    /// Advanced glob pattern matching
    async fn glob_files_advanced(&self, pattern: &str) -> Result<Vec<PathBuf>> {
        let full_pattern = if pattern.starts_with('/') {
            pattern.to_string()
        } else {
            self.working_directory.join(pattern).to_string_lossy().to_string()
        };
        
        let mut files = Vec::new();
        
        match glob::glob(&full_pattern) {
            Ok(paths) => {
                for entry in paths {
                    match entry {
                        Ok(path) => {
                            if path.is_file() && self.should_include_file(&path).await? {
                                files.push(path);
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Glob error for pattern '{}': {}", pattern, e);
                        }
                    }
                }
            }
            Err(e) => {
                return Err(KaiError::context(format!("Invalid glob pattern '{}': {}", pattern, e)));
            }
        }
        
        // Sort by priority
        files.sort_by(|a, b| self.get_file_priority(a).cmp(&self.get_file_priority(b)));
        
        tracing::debug!("Glob pattern '{}' matched {} files", pattern, files.len());
        Ok(files)
    }
    
    /// Resolve patterns with directory components
    async fn resolve_path_with_directories(&self, pattern: &str) -> Result<Vec<PathBuf>> {
        let pattern_path = self.working_directory.join(pattern);
        
        // Check if the pattern has wildcards in directory names
        let path_components: Vec<&str> = pattern.split('/').collect();
        let has_wildcards = path_components.iter().any(|comp| comp.contains('*') || comp.contains('?'));
        
        if has_wildcards {
            // Use glob for complex patterns
            return self.glob_files_advanced(pattern).await;
        }
        
        // Check if it's a partial path that we can expand
        if let Some(parent) = pattern_path.parent() {
            if parent.is_dir() {
                // Look for files matching the pattern in the parent directory
                let filename = pattern_path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("");
                
                if filename.is_empty() {
                    return self.get_files_in_directory_filtered(parent).await;
                }
                
                // Search for files containing the filename
                let search_pattern = format!("{}/*{}*", parent.display(), filename);
                return self.glob_files_advanced(&search_pattern).await;
            }
        }
        
        Ok(vec![])
    }
    
    /// Search files by name or extension
    async fn search_by_name_or_extension(&self, search_term: &str) -> Result<Vec<PathBuf>> {
        let mut matching_files = Vec::new();
        
        // Check if it's an extension search
        let is_extension = search_term.starts_with('.') || 
                          (search_term.len() <= 5 && search_term.chars().all(|c| c.is_alphanumeric()));
        
        if is_extension {
            // Search by file extension
            let ext = if search_term.starts_with('.') {
                &search_term[1..]
            } else {
                search_term
            };
            
            let pattern = format!("**/*.{}", ext);
            return self.glob_files_advanced(&pattern).await;
        }
        
        // Search by filename containing the term
        let patterns = vec![
            format!("**/*{}*", search_term),
            format!("**/*{}*.*", search_term),
            format!("**/{}*", search_term),
        ];
        
        for pattern in patterns {
            let mut files = self.glob_files_advanced(&pattern).await?;
            matching_files.append(&mut files);
        }
        
        // Remove duplicates and sort
        matching_files.sort();
        matching_files.dedup();
        
        // Sort by relevance (exact matches first, then by priority)
        matching_files.sort_by(|a, b| {
            let a_name = a.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let b_name = b.file_name().and_then(|n| n.to_str()).unwrap_or("");
            
            let a_exact = a_name.to_lowercase().contains(&search_term.to_lowercase());
            let b_exact = b_name.to_lowercase().contains(&search_term.to_lowercase());
            
            match (a_exact, b_exact) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => self.get_file_priority(a).cmp(&self.get_file_priority(b))
            }
        });
        
        tracing::debug!("Name/extension search for '{}' found {} files", search_term, matching_files.len());
        Ok(matching_files)
    }


    /// Generate a summary for a file using the LLM with intelligent chunking
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
        
        let language = self.detect_language_from_extension(file_extension)
            .unwrap_or_else(|| "text".to_string());
        
        // If content is small enough, summarize directly
        if content.len() <= 8000 {  // Roughly 2000 tokens
            return self.generate_direct_summary(content, file_path, &language, llm_provider, model).await;
        }
        
        // For large files, use chunked summarization
        self.generate_chunked_summary(content, file_path, &language, llm_provider, model).await
    }
    
    /// Generate summary for small files directly
    async fn generate_direct_summary(
        &self,
        content: &str,
        file_path: &Path,
        language: &str,
        llm_provider: &dyn LlmProvider,
        model: &str,
    ) -> Result<String> {
        let context = self.build_file_context_prompt(file_path, language);
        let prompt = format!(
            "{}\n\n"
            "Please provide a concise but comprehensive summary of this {} file. Include:\n"
            "• Primary purpose and role in the project\n"
            "• Key functions, classes, types, or components defined\n"
            "• Important dependencies and relationships\n"
            "• Notable patterns, algorithms, or architectural decisions\n"
            "• Public interfaces or APIs exposed\n"
            "• Any configuration, constants, or important data structures\n\n"
            "Keep the summary focused and under 300 words. Format as structured text.\n\n"
            "File content:\n```{}\n{}\n```",
            context,
            language.to_lowercase(),
            language.to_lowercase(),
            content
        );
        
        llm_provider.generate_content(&prompt, "", model, None).await
            .map_err(|e| KaiError::context(format!("Failed to generate file summary for {}: {}", 
                                                   file_path.display(), e)))
    }
    
    /// Generate summary for large files using chunking
    async fn generate_chunked_summary(
        &self,
        content: &str,
        file_path: &Path,
        language: &str,
        llm_provider: &dyn LlmProvider,
        model: &str,
    ) -> Result<String> {
        tracing::info!("Using chunked summarization for large file: {} ({} chars)", 
                      file_path.display(), content.len());
        
        // Split content into logical chunks
        let chunks = self.split_content_into_chunks(content, language);
        
        if chunks.is_empty() {
            return Ok("Empty file or failed to chunk content.".to_string());
        }
        
        // Summarize each chunk
        let mut chunk_summaries = Vec::new();
        for (i, chunk) in chunks.iter().enumerate() {
            let chunk_prompt = format!(
                "Summarize this portion ({}/{}) of a {} file. Focus on key definitions, "
                "logic, and important details. Keep summary concise (under 100 words):\n\n"
                "```{}\n{}\n```",
                i + 1,
                chunks.len(),
                language,
                language.to_lowercase(),
                chunk
            );
            
            match llm_provider.generate_content(&chunk_prompt, "", model, None).await {
                Ok(summary) => chunk_summaries.push(format!("Part {}: {}", i + 1, summary)),
                Err(e) => {
                    tracing::warn!("Failed to summarize chunk {} of {}: {}", i + 1, file_path.display(), e);
                    chunk_summaries.push(format!("Part {}: [Summary failed]", i + 1));
                }
            }
        }
        
        // Combine chunk summaries into final summary
        let context = self.build_file_context_prompt(file_path, language);
        let combined_summaries = chunk_summaries.join("\n\n");
        
        let final_prompt = format!(
            "{}\n\n"
            "Based on these partial summaries of a {} file, create a comprehensive overview that:\n"
            "• Describes the overall purpose and architecture\n"
            "• Highlights the most important functions, classes, and components\n"
            "• Identifies key relationships and dependencies\n"
            "• Notes significant patterns or design decisions\n\n"
            "Partial summaries:\n{}\n\n"
            "Provide a unified summary under 400 words:",
            context,
            language,
            combined_summaries
        );
        
        llm_provider.generate_content(&final_prompt, "", model, None).await
            .map_err(|e| KaiError::context(format!("Failed to generate final summary for {}: {}", 
                                                   file_path.display(), e)))
    }
    
    /// Build context prompt for file summarization
    fn build_file_context_prompt(&self, file_path: &Path, language: &str) -> String {
        let relative_path = file_path.strip_prefix(&self.working_directory)
            .unwrap_or(file_path);
        
        let directory_context = if let Some(parent) = relative_path.parent() {
            format!("Located in: {}", parent.display())
        } else {
            "Located in project root".to_string()
        };
        
        format!(
            "File: {}\n"
            "Language: {}\n"
            "{}\n"
            "Project: {} ({})",
            relative_path.display(),
            language,
            directory_context,
            self.working_directory.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown"),
            self.working_directory.display()
        )
    }
    
    /// Split content into logical chunks for processing
    fn split_content_into_chunks(&self, content: &str, language: &str) -> Vec<String> {
        const MAX_CHUNK_SIZE: usize = 6000;  // ~1500 tokens
        const MIN_CHUNK_SIZE: usize = 1000;  // Don't create tiny chunks
        
        if content.len() <= MAX_CHUNK_SIZE {
            return vec![content.to_string()];
        }
        
        let mut chunks = Vec::new();
        let lines: Vec<&str> = content.lines().collect();
        
        match language.to_lowercase().as_str() {
            "rust" | "java" | "javascript" | "typescript" | "c++" | "c" | "go" => {
                self.split_by_logical_blocks(&lines, MAX_CHUNK_SIZE, MIN_CHUNK_SIZE)
            }
            "python" => {
                self.split_python_by_functions(&lines, MAX_CHUNK_SIZE, MIN_CHUNK_SIZE)
            }
            "markdown" | "text" => {
                self.split_by_sections(&lines, MAX_CHUNK_SIZE, MIN_CHUNK_SIZE)
            }
            _ => {
                self.split_by_line_count(&lines, MAX_CHUNK_SIZE)
            }
        }
    }
    
    /// Split content by logical code blocks (functions, classes, etc.)
    fn split_by_logical_blocks(&self, lines: &[&str], max_size: usize, min_size: usize) -> Vec<String> {
        let mut chunks = Vec::new();
        let mut current_chunk = Vec::new();
        let mut current_size = 0;
        let mut brace_depth = 0;
        
        for line in lines {
            let line_size = line.len() + 1; // +1 for newline
            
            // Track brace depth to identify logical boundaries
            for ch in line.chars() {
                match ch {
                    '{' => brace_depth += 1,
                    '}' => brace_depth = brace_depth.saturating_sub(1),
                    _ => {}
                }
            }
            
            current_chunk.push(*line);
            current_size += line_size;
            
            // Split at logical boundaries when size limit is approached
            if current_size >= max_size && brace_depth == 0 && current_size >= min_size {
                if !current_chunk.is_empty() {
                    chunks.push(current_chunk.join("\n"));
                    current_chunk.clear();
                    current_size = 0;
                }
            }
        }
        
        // Add remaining content
        if !current_chunk.is_empty() {
            chunks.push(current_chunk.join("\n"));
        }
        
        chunks
    }
    
    /// Split Python code by functions and classes
    fn split_python_by_functions(&self, lines: &[&str], max_size: usize, min_size: usize) -> Vec<String> {
        let mut chunks = Vec::new();
        let mut current_chunk = Vec::new();
        let mut current_size = 0;
        
        for line in lines {
            let line_size = line.len() + 1;
            let trimmed = line.trim();
            
            // Check for function or class definition
            let is_boundary = trimmed.starts_with("def ") || 
                             trimmed.starts_with("class ") || 
                             trimmed.starts_with("async def ");
            
            if is_boundary && current_size >= min_size && current_size > max_size / 2 {
                if !current_chunk.is_empty() {
                    chunks.push(current_chunk.join("\n"));
                    current_chunk.clear();
                    current_size = 0;
                }
            }
            
            current_chunk.push(*line);
            current_size += line_size;
            
            if current_size >= max_size {
                if !current_chunk.is_empty() {
                    chunks.push(current_chunk.join("\n"));
                    current_chunk.clear();
                    current_size = 0;
                }
            }
        }
        
        if !current_chunk.is_empty() {
            chunks.push(current_chunk.join("\n"));
        }
        
        chunks
    }
    
    /// Split by sections (for markdown and text files)
    fn split_by_sections(&self, lines: &[&str], max_size: usize, _min_size: usize) -> Vec<String> {
        let mut chunks = Vec::new();
        let mut current_chunk = Vec::new();
        let mut current_size = 0;
        
        for line in lines {
            let line_size = line.len() + 1;
            let is_header = line.starts_with('#') || line.starts_with("=") || line.starts_with("-");
            
            if is_header && current_size >= max_size / 2 && !current_chunk.is_empty() {
                chunks.push(current_chunk.join("\n"));
                current_chunk.clear();
                current_size = 0;
            }
            
            current_chunk.push(*line);
            current_size += line_size;
            
            if current_size >= max_size {
                if !current_chunk.is_empty() {
                    chunks.push(current_chunk.join("\n"));
                    current_chunk.clear();
                    current_size = 0;
                }
            }
        }
        
        if !current_chunk.is_empty() {
            chunks.push(current_chunk.join("\n"));
        }
        
        chunks
    }
    
    /// Simple split by line count as fallback
    fn split_by_line_count(&self, lines: &[&str], max_size: usize) -> Vec<String> {
        let mut chunks = Vec::new();
        let mut current_chunk = Vec::new();
        let mut current_size = 0;
        
        for line in lines {
            let line_size = line.len() + 1;
            
            current_chunk.push(*line);
            current_size += line_size;
            
            if current_size >= max_size {
                if !current_chunk.is_empty() {
                    chunks.push(current_chunk.join("\n"));
                    current_chunk.clear();
                    current_size = 0;
                }
            }
        }
        
        if !current_chunk.is_empty() {
            chunks.push(current_chunk.join("\n"));
        }
        
        chunks
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