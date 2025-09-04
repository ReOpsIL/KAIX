//! Context management for maintaining project state and plan execution context

use crate::utils::errors::KaiError;
use crate::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use chrono::{DateTime, Utc};

pub mod manager;
pub mod global;
pub mod plan;

pub use manager::ContextManager;
pub use global::{GlobalContext, FileContext};
pub use plan::PlanContext;

/// Represents the context for a specific file in the project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextEntry {
    /// File path relative to the working directory
    pub path: PathBuf,
    /// Generated summary of the file content
    pub summary: String,
    /// File metadata
    pub metadata: FileMetadata,
    /// When this context entry was last updated
    pub updated_at: DateTime<Utc>,
}

/// Metadata about a file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    /// File size in bytes
    pub size: u64,
    /// Last modification time
    pub modified_at: DateTime<Utc>,
    /// File extension (if any)
    pub extension: Option<String>,
    /// Detected programming language
    pub language: Option<String>,
    /// Whether this is a binary file
    pub is_binary: bool,
}

/// Configuration for context generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    /// Maximum size in bytes for files to include in context
    pub max_file_size: u64,
    /// File patterns to exclude from context (in addition to .gitignore)
    pub exclude_patterns: Vec<String>,
    /// File extensions to prioritize in context generation
    pub priority_extensions: Vec<String>,
    /// Maximum depth for directory traversal
    pub max_depth: Option<usize>,
    /// Whether to follow symbolic links
    pub follow_symlinks: bool,
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_file_size: 1024 * 1024, // 1MB
            exclude_patterns: vec![
                "*.log".to_string(),
                "*.tmp".to_string(),
                "target/**".to_string(),
                "node_modules/**".to_string(),
                ".git/**".to_string(),
            ],
            priority_extensions: vec![
                "rs".to_string(),
                "js".to_string(),
                "ts".to_string(),
                "py".to_string(),
                "java".to_string(),
                "cpp".to_string(),
                "c".to_string(),
                "go".to_string(),
                "rb".to_string(),
            ],
            max_depth: Some(10),
            follow_symlinks: false,
        }
    }
}

impl ContextEntry {
    /// Create a new context entry
    pub fn new(path: PathBuf, summary: String, metadata: FileMetadata) -> Self {
        Self {
            path,
            summary,
            metadata,
            updated_at: Utc::now(),
        }
    }

    /// Check if this context entry is outdated compared to the actual file
    pub fn is_outdated(&self, file_path: &Path) -> Result<bool> {
        if !file_path.exists() {
            return Ok(true);
        }

        let file_metadata = std::fs::metadata(file_path)
            .map_err(|e| KaiError::file_system(file_path, e))?;

        let modified_time = file_metadata.modified()
            .map_err(|e| KaiError::file_system(file_path, e))?;

        let modified_datetime: DateTime<Utc> = modified_time.into();

        Ok(modified_datetime > self.metadata.modified_at)
    }
}

impl FileMetadata {
    /// Create metadata from a file path
    pub fn from_path(path: &Path) -> Result<Self> {
        let metadata = std::fs::metadata(path)
            .map_err(|e| KaiError::file_system(path, e))?;

        let modified_time = metadata.modified()
            .map_err(|e| KaiError::file_system(path, e))?;

        let extension = path.extension()
            .and_then(|ext| ext.to_str())
            .map(|s| s.to_string());

        let language = detect_language(&extension);
        let is_binary = is_binary_file(path)?;

        Ok(Self {
            size: metadata.len(),
            modified_at: modified_time.into(),
            extension,
            language,
            is_binary,
        })
    }
}

/// Detect programming language from file extension
fn detect_language(extension: &Option<String>) -> Option<String> {
    extension.as_ref().and_then(|ext| {
        match ext.to_lowercase().as_str() {
            "rs" => Some("rust".to_string()),
            "js" => Some("javascript".to_string()),
            "ts" => Some("typescript".to_string()),
            "py" => Some("python".to_string()),
            "java" => Some("java".to_string()),
            "cpp" | "cc" | "cxx" => Some("cpp".to_string()),
            "c" => Some("c".to_string()),
            "h" | "hpp" => Some("c_header".to_string()),
            "go" => Some("go".to_string()),
            "rb" => Some("ruby".to_string()),
            "php" => Some("php".to_string()),
            "cs" => Some("csharp".to_string()),
            "swift" => Some("swift".to_string()),
            "kt" => Some("kotlin".to_string()),
            "scala" => Some("scala".to_string()),
            "clj" | "cljs" => Some("clojure".to_string()),
            "hs" => Some("haskell".to_string()),
            "ml" => Some("ocaml".to_string()),
            "elm" => Some("elm".to_string()),
            "dart" => Some("dart".to_string()),
            "r" => Some("r".to_string()),
            "jl" => Some("julia".to_string()),
            "lua" => Some("lua".to_string()),
            "sh" | "bash" | "zsh" => Some("shell".to_string()),
            "sql" => Some("sql".to_string()),
            "html" => Some("html".to_string()),
            "css" => Some("css".to_string()),
            "xml" => Some("xml".to_string()),
            "json" => Some("json".to_string()),
            "yaml" | "yml" => Some("yaml".to_string()),
            "toml" => Some("toml".to_string()),
            "md" => Some("markdown".to_string()),
            _ => None,
        }
    })
}

/// Check if a file is binary
fn is_binary_file(path: &Path) -> Result<bool> {
    let mut file = std::fs::File::open(path)
        .map_err(|e| KaiError::file_system(path, e))?;

    let mut buffer = [0; 1024];
    let bytes_read = std::io::Read::read(&mut file, &mut buffer)
        .map_err(|e| KaiError::file_system(path, e))?;

    // Check for null bytes (common in binary files)
    Ok(buffer[..bytes_read].contains(&0))
}