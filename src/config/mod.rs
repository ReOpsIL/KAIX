//! Configuration management for KAI-X

use crate::utils::errors::{ConfigError, KaiError};
use crate::Result;
use dirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Main application configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Currently active LLM provider
    pub active_provider: String,
    /// Currently active model
    pub active_model: String,
    /// Working directory for the current session
    pub working_directory: Option<PathBuf>,
    /// Provider configurations
    pub providers: HashMap<String, ProviderConfig>,
    /// UI preferences
    pub ui: UiConfig,
    /// Context generation settings
    pub context: ContextConfig,
    /// Execution settings
    pub execution: ExecutionConfig,
    /// Logging configuration
    pub logging: LoggingConfig,
}

/// Configuration for an LLM provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// API key for the provider (stored securely)
    pub api_key: Option<String>,
    /// Base URL for the provider API
    pub base_url: Option<String>,
    /// Default model to use with this provider
    pub default_model: Option<String>,
    /// Provider-specific settings
    pub settings: HashMap<String, serde_json::Value>,
}

/// UI configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiConfig {
    /// Theme preference (light, dark, auto)
    pub theme: String,
    /// Number of lines to show in history
    pub history_limit: usize,
    /// Whether to show task progress indicators
    pub show_progress: bool,
    /// Whether to auto-complete file paths
    pub auto_complete_paths: bool,
    /// Key bindings preference (vim, emacs, default)
    pub key_bindings: String,
}

/// Context generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextConfig {
    /// Maximum file size to include in context (bytes)
    pub max_file_size: u64,
    /// Maximum total context size (characters)
    pub max_context_size: usize,
    /// File extensions to prioritize
    pub priority_extensions: Vec<String>,
    /// Patterns to exclude from context
    pub exclude_patterns: Vec<String>,
    /// Whether to generate detailed summaries
    pub detailed_summaries: bool,
}

/// Task execution configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    /// Maximum number of concurrent tasks
    pub max_concurrent_tasks: usize,
    /// Default timeout for tasks (seconds)
    pub default_timeout_seconds: u64,
    /// Whether to automatically retry failed tasks
    pub auto_retry: bool,
    /// Maximum number of retries
    pub max_retries: usize,
    /// Whether to pause on errors
    pub pause_on_error: bool,
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level (trace, debug, info, warn, error)
    pub level: String,
    /// Whether to log to file
    pub log_to_file: bool,
    /// Log file path
    pub log_file: Option<PathBuf>,
    /// Whether to include timestamps
    pub include_timestamps: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            active_provider: "openrouter".to_string(),
            active_model: "anthropic/claude-3-haiku".to_string(),
            working_directory: None,
            providers: HashMap::new(),
            ui: UiConfig::default(),
            context: ContextConfig::default(),
            execution: ExecutionConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            api_key: None,
            base_url: None,
            default_model: None,
            settings: HashMap::new(),
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "auto".to_string(),
            history_limit: 1000,
            show_progress: true,
            auto_complete_paths: true,
            key_bindings: "default".to_string(),
        }
    }
}

impl Default for ContextConfig {
    fn default() -> Self {
        Self {
            max_file_size: 1024 * 1024, // 1MB
            max_context_size: 100_000, // 100k characters
            priority_extensions: vec![
                "rs".to_string(),
                "js".to_string(),
                "ts".to_string(),
                "py".to_string(),
                "java".to_string(),
                "go".to_string(),
                "cpp".to_string(),
                "c".to_string(),
            ],
            exclude_patterns: vec![
                "*.log".to_string(),
                "*.tmp".to_string(),
                "target/**".to_string(),
                "node_modules/**".to_string(),
                ".git/**".to_string(),
                "*.exe".to_string(),
                "*.bin".to_string(),
                "*.so".to_string(),
                "*.dll".to_string(),
            ],
            detailed_summaries: true,
        }
    }
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: 4,
            default_timeout_seconds: 300, // 5 minutes
            auto_retry: false,
            max_retries: 3,
            pause_on_error: true,
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            log_to_file: false,
            log_file: None,
            include_timestamps: true,
        }
    }
}

/// Configuration manager for loading, saving, and managing application configuration
pub struct ConfigManager {
    config: Config,
    config_path: PathBuf,
}

impl ConfigManager {
    /// Create a new configuration manager
    pub fn new() -> Result<Self> {
        let config_path = Self::get_config_path()?;
        let config = if config_path.exists() {
            Self::load_config(&config_path)?
        } else {
            let default_config = Config::default();
            Self::save_config(&config_path, &default_config)?;
            default_config
        };

        Ok(Self { config, config_path })
    }

    /// Get the current configuration
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Get a mutable reference to the configuration
    pub fn config_mut(&mut self) -> &mut Config {
        &mut self.config
    }

    /// Save the current configuration to disk
    pub fn save(&self) -> Result<()> {
        Self::save_config(&self.config_path, &self.config)
    }

    /// Reload configuration from disk
    pub fn reload(&mut self) -> Result<()> {
        if self.config_path.exists() {
            self.config = Self::load_config(&self.config_path)?;
        }
        Ok(())
    }

    /// Update the active provider
    pub fn set_active_provider(&mut self, provider: String) -> Result<()> {
        self.config.active_provider = provider;
        self.save()
    }

    /// Update the active model
    pub fn set_active_model(&mut self, model: String) -> Result<()> {
        self.config.active_model = model;
        self.save()
    }

    /// Set the working directory
    pub fn set_working_directory(&mut self, path: PathBuf) -> Result<()> {
        self.config.working_directory = Some(path);
        self.save()
    }

    /// Add or update a provider configuration
    pub fn set_provider_config(&mut self, name: String, provider_config: ProviderConfig) -> Result<()> {
        self.config.providers.insert(name, provider_config);
        self.save()
    }

    /// Get a provider configuration
    pub fn get_provider_config(&self, name: &str) -> Option<&ProviderConfig> {
        self.config.providers.get(name)
    }

    /// Remove a provider configuration
    pub fn remove_provider(&mut self, name: &str) -> Result<()> {
        self.config.providers.remove(name);
        self.save()
    }

    /// Get the configuration file path
    fn get_config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| ConfigError::FileNotFound { 
                path: PathBuf::from("config directory")
            })?;
        
        let app_config_dir = config_dir.join("kai-x");
        if !app_config_dir.exists() {
            fs::create_dir_all(&app_config_dir)
                .map_err(|e| ConfigError::WriteError { source: e })?;
        }
        
        Ok(app_config_dir.join("config.toml"))
    }

    /// Load configuration from file
    fn load_config(path: &Path) -> Result<Config> {
        let content = fs::read_to_string(path)
            .map_err(|e| ConfigError::ReadError { source: e })?;
        
        toml::from_str(&content)
            .map_err(|e| ConfigError::ParseError { source: e })
            .map_err(Into::into)
    }

    /// Save configuration to file
    fn save_config(path: &Path, config: &Config) -> Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)
                    .map_err(|e| ConfigError::WriteError { source: e })?;
            }
        }

        let content = toml::to_string_pretty(config)
            .map_err(|e| ConfigError::SerializeError { source: e })?;
        
        fs::write(path, content)
            .map_err(|e| ConfigError::WriteError { source: e })?;
        
        Ok(())
    }
}

impl Config {
    /// Get the active provider configuration
    pub fn get_active_provider_config(&self) -> Option<&ProviderConfig> {
        self.providers.get(&self.active_provider)
    }

    /// Check if the configuration is valid
    pub fn validate(&self) -> Result<()> {
        // Validate active provider exists
        if !self.providers.contains_key(&self.active_provider) {
            return Err(KaiError::validation(
                "active_provider",
                format!("Provider '{}' not found in configuration", self.active_provider)
            ));
        }

        // Validate active provider has API key
        if let Some(provider_config) = self.get_active_provider_config() {
            if provider_config.api_key.is_none() {
                return Err(KaiError::validation(
                    "provider.api_key",
                    format!("No API key configured for provider '{}'", self.active_provider)
                ));
            }
        }

        // Validate working directory exists if set
        if let Some(ref workdir) = self.working_directory {
            if !workdir.exists() {
                return Err(KaiError::validation(
                    "working_directory",
                    format!("Working directory does not exist: {}", workdir.display())
                ));
            }
        }

        Ok(())
    }
}