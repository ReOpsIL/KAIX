//! Error types used throughout the application

use std::path::PathBuf;
use thiserror::Error;

/// Main error type for the KAI-X application
#[derive(Error, Debug)]
pub enum KaiError {
    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("LLM provider error: {0}")]
    Llm(#[from] crate::llm::LlmError),

    #[error("Planning error: {message}")]
    Planning { message: String },

    #[error("Context error: {message}")]
    Context { message: String },

    #[error("Execution error: {message}")]
    Execution { message: String },

    #[error("UI error: {message}")]
    Ui { message: String },

    #[error("Task error: {task_id}: {message}")]
    Task { task_id: String, message: String },

    #[error("Provider error: {provider}: {message}")]
    Provider { provider: String, message: String },

    #[error("Authentication error: {message}")]
    Authentication { message: String },

    #[error("Validation error: {field}: {message}")]
    Validation { field: String, message: String },

    #[error("File system error: {path}: {source}")]
    FileSystem {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Path error: invalid path {path}")]
    InvalidPath { path: String },

    #[error("Permission denied: {resource}")]
    PermissionDenied { resource: String },

    #[error("Resource not found: {resource}")]
    NotFound { resource: String },

    #[error("Resource already exists: {resource}")]
    AlreadyExists { resource: String },

    #[error("Timeout error: operation timed out after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },

    #[error("Cancelled: {operation}")]
    Cancelled { operation: String },

    #[error("JSON serialization/deserialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("YAML serialization/deserialization error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("TOML serialization/deserialization error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("HTTP request error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("UUID error: {0}")]
    Uuid(#[from] uuid::Error),

    #[error("Glob pattern error: {0}")]
    GlobPattern(#[from] glob::PatternError),

    #[error("Unknown error: {message}")]
    Unknown { message: String },
}

/// Configuration-specific errors
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Missing configuration key: {key}")]
    MissingKey { key: String },

    #[error("Invalid configuration value for {key}: {value}")]
    InvalidValue { key: String, value: String },

    #[error("Configuration file not found: {path}")]
    FileNotFound { path: PathBuf },

    #[error("Failed to read configuration: {source}")]
    ReadError { 
        #[source]
        source: std::io::Error 
    },

    #[error("Failed to write configuration: {source}")]
    WriteError { 
        #[source]
        source: std::io::Error 
    },

    #[error("Failed to parse configuration: {source}")]
    ParseError { 
        #[source]
        source: toml::de::Error 
    },

    #[error("Failed to serialize configuration: {source}")]
    SerializeError { 
        #[source]
        source: toml::ser::Error 
    },
}

impl KaiError {
    /// Create a new planning error
    pub fn planning<S: Into<String>>(message: S) -> Self {
        Self::Planning {
            message: message.into(),
        }
    }

    /// Create a new context error
    pub fn context<S: Into<String>>(message: S) -> Self {
        Self::Context {
            message: message.into(),
        }
    }

    /// Create a new execution error
    pub fn execution<S: Into<String>>(message: S) -> Self {
        Self::Execution {
            message: message.into(),
        }
    }

    /// Create a new UI error
    pub fn ui<S: Into<String>>(message: S) -> Self {
        Self::Ui {
            message: message.into(),
        }
    }

    /// Create a new task error
    pub fn task<S1: Into<String>, S2: Into<String>>(task_id: S1, message: S2) -> Self {
        Self::Task {
            task_id: task_id.into(),
            message: message.into(),
        }
    }

    /// Create a new provider error
    pub fn provider<S1: Into<String>, S2: Into<String>>(provider: S1, message: S2) -> Self {
        Self::Provider {
            provider: provider.into(),
            message: message.into(),
        }
    }

    /// Create a new authentication error
    pub fn authentication<S: Into<String>>(message: S) -> Self {
        Self::Authentication {
            message: message.into(),
        }
    }

    /// Create a new validation error
    pub fn validation<S1: Into<String>, S2: Into<String>>(field: S1, message: S2) -> Self {
        Self::Validation {
            field: field.into(),
            message: message.into(),
        }
    }

    /// Create a new file system error
    pub fn file_system<P: Into<PathBuf>>(path: P, source: std::io::Error) -> Self {
        Self::FileSystem {
            path: path.into(),
            source,
        }
    }

    /// Create a new invalid path error
    pub fn invalid_path<S: Into<String>>(path: S) -> Self {
        Self::InvalidPath {
            path: path.into(),
        }
    }

    /// Create a new permission denied error
    pub fn permission_denied<S: Into<String>>(resource: S) -> Self {
        Self::PermissionDenied {
            resource: resource.into(),
        }
    }

    /// Create a new not found error
    pub fn not_found<S: Into<String>>(resource: S) -> Self {
        Self::NotFound {
            resource: resource.into(),
        }
    }

    /// Create a new already exists error
    pub fn already_exists<S: Into<String>>(resource: S) -> Self {
        Self::AlreadyExists {
            resource: resource.into(),
        }
    }

    /// Create a new timeout error
    pub fn timeout(timeout_ms: u64) -> Self {
        Self::Timeout { timeout_ms }
    }

    /// Create a new cancelled error
    pub fn cancelled<S: Into<String>>(operation: S) -> Self {
        Self::Cancelled {
            operation: operation.into(),
        }
    }

    /// Create a new unknown error
    pub fn unknown<S: Into<String>>(message: S) -> Self {
        Self::Unknown {
            message: message.into(),
        }
    }

    /// Check if this error is recoverable
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::Timeout { .. } | Self::Http(_) => true,
            Self::Authentication { .. } | Self::PermissionDenied { .. } => false,
            Self::Validation { .. } | Self::InvalidPath { .. } => false,
            _ => true,
        }
    }

    /// Get error category for logging/metrics
    pub fn category(&self) -> &'static str {
        match self {
            Self::Config(_) => "config",
            Self::Llm(_) => "llm",
            Self::Planning { .. } => "planning",
            Self::Context { .. } => "context",
            Self::Execution { .. } => "execution",
            Self::Ui { .. } => "ui",
            Self::Task { .. } => "task",
            Self::Provider { .. } => "provider",
            Self::Authentication { .. } => "auth",
            Self::Validation { .. } => "validation",
            Self::FileSystem { .. } => "filesystem",
            Self::InvalidPath { .. } => "path",
            Self::PermissionDenied { .. } => "permission",
            Self::NotFound { .. } => "notfound",
            Self::AlreadyExists { .. } => "exists",
            Self::Timeout { .. } => "timeout",
            Self::Cancelled { .. } => "cancelled",
            Self::Json(_) => "json",
            Self::Yaml(_) => "yaml",
            Self::Toml(_) => "toml",
            Self::Http(_) => "http",
            Self::Regex(_) => "regex",
            Self::Io(_) => "io",
            Self::Uuid(_) => "uuid",
            Self::GlobPattern(_) => "glob",
            Self::Unknown { .. } => "unknown",
        }
    }
}