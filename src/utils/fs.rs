//! File system utilities with git-aware filtering

use crate::context::ContextConfig;
use crate::utils::errors::KaiError;
use crate::Result;
use ignore::{Walk, WalkBuilder};
use std::path::{Path, PathBuf};
use tokio::fs;
use std::collections::HashSet;

/// File discovery configuration
#[derive(Debug, Clone)]
pub struct FileDiscoveryConfig {
    /// Whether to respect .gitignore files
    pub respect_gitignore: bool,
    /// Whether to respect .aiignore files
    pub respect_aiignore: bool,
    /// Maximum depth to traverse
    pub max_depth: Option<usize>,
    /// Additional patterns to ignore
    pub ignore_patterns: Vec<String>,
    /// File extensions to prioritize
    pub priority_extensions: Vec<String>,
    /// Maximum file size to consider
    pub max_file_size: u64,
    /// Whether to follow symbolic links
    pub follow_symlinks: bool,
}

impl Default for FileDiscoveryConfig {
    fn default() -> Self {
        Self {
            respect_gitignore: true,
            respect_aiignore: true,
            max_depth: Some(10),
            ignore_patterns: vec![
                "target/".to_string(),
                "node_modules/".to_string(),
                ".git/".to_string(),
                "*.log".to_string(),
                "*.tmp".to_string(),
                ".DS_Store".to_string(),
                "*.pyc".to_string(),
                "__pycache__/".to_string(),
                ".env".to_string(),
                ".env.*".to_string(),
                "dist/".to_string(),
                "build/".to_string(),
            ],
            priority_extensions: vec![
                "rs".to_string(), "js".to_string(), "ts".to_string(), "py".to_string(),
                "java".to_string(), "cpp".to_string(), "c".to_string(), "go".to_string(),
                "rb".to_string(), "php".to_string(), "cs".to_string(), "swift".to_string(),
                "kt".to_string(), "scala".to_string(), "clj".to_string(), "hs".to_string(),
                "ml".to_string(), "elm".to_string(), "dart".to_string(), "r".to_string(),
                "jl".to_string(), "lua".to_string(), "sh".to_string(), "bash".to_string(),
                "zsh".to_string(), "sql".to_string(), "html".to_string(), "css".to_string(),
                "xml".to_string(), "json".to_string(), "yaml".to_string(), "yml".to_string(),
                "toml".to_string(), "md".to_string(), "txt".to_string(),
            ],
            max_file_size: 1024 * 1024, // 1MB
            follow_symlinks: false,
        }
    }
}

impl From<&ContextConfig> for FileDiscoveryConfig {
    fn from(context_config: &ContextConfig) -> Self {
        Self {
            respect_gitignore: true,
            respect_aiignore: true,
            max_depth: context_config.max_depth,
            ignore_patterns: context_config.exclude_patterns.clone(),
            priority_extensions: context_config.priority_extensions.clone(),
            max_file_size: context_config.max_file_size,
            follow_symlinks: context_config.follow_symlinks,
        }
    }
}

/// Check if a path is a valid file
pub fn is_valid_file<P: AsRef<Path>>(path: P) -> bool {
    let path = path.as_ref();
    path.exists() && path.is_file()
}

/// Check if a path is a valid directory
pub fn is_valid_directory<P: AsRef<Path>>(path: P) -> bool {
    let path = path.as_ref();
    path.exists() && path.is_dir()
}

/// Expand glob pattern to files
pub fn expand_glob_pattern<P: AsRef<Path>>(
    base_path: P,
    pattern: &str,
) -> Result<Vec<PathBuf>, KaiError> {
    let base = base_path.as_ref();
    let full_pattern = base.join(pattern);
    
    let mut files = Vec::new();
    for entry in glob::glob(full_pattern.to_str().unwrap())? {
        match entry {
            Ok(path) => {
                if path.is_file() {
                    files.push(path);
                }
            }
            Err(e) => {
                tracing::warn!("Glob error: {}", e);
            }
        }
    }
    
    Ok(files)
}

/// Resolve a path string to concrete file paths
/// Handles single files, directories (expanded to recursive glob), and glob patterns
pub fn resolve_path_pattern<P: AsRef<Path>>(
    base_path: P,
    pattern: &str,
) -> Result<Vec<PathBuf>, KaiError> {
    let base = base_path.as_ref();
    let target_path = base.join(pattern);

    if target_path.is_file() {
        // Single file
        Ok(vec![target_path])
    } else if target_path.is_dir() {
        // Directory - expand to recursive glob
        let recursive_pattern = format!("{}/**/*", pattern);
        expand_glob_pattern(base, &recursive_pattern)
    } else if pattern.contains('*') || pattern.contains('?') {
        // Glob pattern
        expand_glob_pattern(base, pattern)
    } else {
        // Path doesn't exist
        Ok(vec![])
    }
}

/// Discover files using ContextConfig
pub async fn discover_files<P: AsRef<Path>>(
    root: P, 
    config: &ContextConfig
) -> Result<Vec<PathBuf>> {
    let mut builder = WalkBuilder::new(&root);
    
    builder
        .git_ignore(true)
        .add_custom_ignore_filename(".aiignore");

    if let Some(depth) = config.max_depth {
        builder.max_depth(Some(depth));
    }

    let walker = builder.build();
    let mut files = Vec::new();

    for result in walker {
        match result {
            Ok(entry) => {
                if entry.file_type().map_or(false, |ft| ft.is_file()) {
                    let path = entry.path().to_path_buf();
                    if !should_ignore_file(&path, config).await? {
                        files.push(path);
                    }
                }
            }
            Err(err) => {
                tracing::warn!("Failed to process file entry: {}", err);
            }
        }
    }

    Ok(files)
}

/// Check if a file should be ignored based on context configuration
pub async fn should_ignore_file<P: AsRef<Path>>(
    path: P,
    config: &ContextConfig,
) -> Result<bool> {
    let path = path.as_ref();
    
    // Check file size
    if let Ok(metadata) = std::fs::metadata(path) {
        if metadata.len() > config.max_file_size {
            return Ok(true);
        }
    }

    // Check exclude patterns
    let path_str = path.to_string_lossy();
    for pattern in &config.exclude_patterns {
        if path_str.contains(pattern) || glob::Pattern::new(pattern)
            .map_err(|e| KaiError::context(format!("Invalid glob pattern: {}", e)))?
            .matches(&path_str) {
            return Ok(true);
        }
    }

    Ok(false)
}