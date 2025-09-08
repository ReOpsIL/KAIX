//! Configuration management for KAI-X

use crate::utils::errors::{ConfigError, KaiError};
use crate::utils::debug::DEBUG_TRACER;
use crate::{debug_checkpoint, debug_error};
use crate::Result;
use dirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

// Basic configuration functionality focused on spec requirements

/// Configuration change events (simplified for spec compliance)
#[derive(Debug, Clone)]
pub enum ConfigEvent {
    /// Provider configuration changed
    ProviderChanged {
        provider: String,
        model: Option<String>,
    },
    /// Working directory changed
    WorkingDirectoryChanged {
        old_path: Option<PathBuf>,
        new_path: PathBuf,
    },
}



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
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
        let mut providers = HashMap::new();
        
        // Add default OpenRouter provider configuration with hardcoded base URL
        providers.insert("openrouter".to_string(), ProviderConfig::new_for_provider("openrouter"));
        
        Self {
            active_provider: "openrouter".to_string(),
            active_model: "google/gemini-2.5-pro".to_string(),
            working_directory: None,
            providers,
            ui: UiConfig::default(),
            context: ContextConfig::default(),
            execution: ExecutionConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

impl ProviderConfig {
    /// Create a new provider configuration with hardcoded base URL based on provider name
    pub fn new_for_provider(provider_name: &str) -> Self {
        let (base_url, default_model) = match provider_name.to_lowercase().as_str() {
            "openrouter" => (
                Some("https://openrouter.ai/api/v1".to_string()),
                Some("google/gemini-2.5-pro".to_string()),
            ),
            "gemini" => (
                Some("https://generativelanguage.googleapis.com/v1beta".to_string()),
                Some("gemini-2.5-pro".to_string()),
            ),
            "openai" => (
                Some("https://api.openai.com/v1".to_string()),
                Some("gpt-3.5-turbo".to_string()),
            ),
            "anthropic" => (
                Some("https://api.anthropic.com".to_string()),
                Some("claude-3-haiku-20240307".to_string()),
            ),
            _ => (None, None), // Unknown provider
        };

        Self {
            api_key: None,
            base_url,
            default_model,
            settings: HashMap::new(),
        }
    }

    /// Get the base URL for this provider, hardcoded based on provider type
    pub fn get_base_url_for_provider(provider_name: &str) -> Option<String> {
        match provider_name.to_lowercase().as_str() {
            "openrouter" => Some("https://openrouter.ai/api/v1".to_string()),
            "gemini" => Some("https://generativelanguage.googleapis.com/v1beta".to_string()),
            "openai" => Some("https://api.openai.com/v1".to_string()),
            "anthropic" => Some("https://api.anthropic.com".to_string()),
            _ => None,
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
    config: Arc<RwLock<Config>>,
    config_path: PathBuf,
}

impl ConfigManager {
    /// Create a new configuration manager
    pub fn new() -> Result<Self> {
        let mut flow_context = DEBUG_TRACER.start_flow("config", "config_manager_new");
        
        debug_checkpoint!(&mut flow_context, "get_config_path_start");
        let config_path = match Self::get_config_path() {
            Ok(path) => {
                debug_checkpoint!(&mut flow_context, "get_config_path_success", {
                    let mut state = HashMap::new();
                    state.insert("config_path".to_string(), serde_json::Value::String(path.display().to_string()));
                    state
                });
                path
            }
            Err(e) => {
                debug_error!(&mut flow_context, &e, "get_config_path_failed");
                return Err(e);
            }
        };
        
        debug_checkpoint!(&mut flow_context, "check_config_exists", {
            let mut state = HashMap::new();
            state.insert("exists".to_string(), serde_json::Value::Bool(config_path.exists()));
            state
        });
        
        let config = if config_path.exists() {
            debug_checkpoint!(&mut flow_context, "load_existing_config");
            match Self::load_config(&config_path) {
                Ok(cfg) => {
                    debug_checkpoint!(&mut flow_context, "load_config_success", {
                        let mut state = HashMap::new();
                        state.insert("active_provider".to_string(), serde_json::Value::String(cfg.active_provider.clone()));
                        state.insert("provider_count".to_string(), serde_json::Value::Number(serde_json::Number::from(cfg.providers.len() as u64)));
                        state
                    });
                    cfg
                }
                Err(e) => {
                    debug_error!(&mut flow_context, &e, "load_config_failed");
                    return Err(e);
                }
            }
        } else {
            debug_checkpoint!(&mut flow_context, "create_default_config");
            let default_config = Config::default();
            match Self::save_config(&config_path, &default_config) {
                Ok(_) => {
                    debug_checkpoint!(&mut flow_context, "save_default_config_success");
                    default_config
                }
                Err(e) => {
                    debug_error!(&mut flow_context, &e, "save_default_config_failed");
                    return Err(e);
                }
            }
        };

        let result = Self {
            config: Arc::new(RwLock::new(config)),
            config_path,
        };
        
        debug_checkpoint!(&mut flow_context, "config_manager_created");
        DEBUG_TRACER.end_flow(&flow_context);
        
        Ok(result)
    }

    /// Get a cloned copy of the current configuration
    pub fn config(&self) -> Config {
        self.config.read().unwrap().clone()
    }

    /// Execute a closure with read access to the configuration
    pub fn with_config<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Config) -> T,
    {
        let config = self.config.read().map_err(|e| KaiError::unknown(format!("Failed to read config: {}", e)))?;
        Ok(f(&config))
    }

    /// Save the current configuration to disk
    pub fn save(&self) -> Result<()> {
        let config = self.config.read().map_err(|e| KaiError::unknown(format!("Failed to read config: {}", e)))?;
        Self::save_config(&self.config_path, &config)
    }

    /// Reload configuration from disk
    pub fn reload(&mut self) -> Result<()> {
        if self.config_path.exists() {
            let new_config = Self::load_config(&self.config_path)?;
            let mut config = self.config.write().map_err(|e| KaiError::unknown(format!("Failed to write config: {}", e)))?;
            *config = new_config;
        }
        Ok(())
    }

    /// Update the active provider
    pub fn set_active_provider(&mut self, provider: String) -> Result<()> {
        let mut config = self.config.write().map_err(|e| KaiError::unknown(format!("Failed to write config: {}", e)))?;
        config.active_provider = provider;
        drop(config);
        self.save()
    }

    /// Update the active model
    pub fn set_active_model(&mut self, model: String) -> Result<()> {
        let mut config = self.config.write().map_err(|e| KaiError::unknown(format!("Failed to write config: {}", e)))?;
        config.active_model = model;
        drop(config);
        self.save()
    }

    /// Set the working directory
    pub fn set_working_directory(&mut self, path: PathBuf) -> Result<()> {
        let mut config = self.config.write().map_err(|e| KaiError::unknown(format!("Failed to write config: {}", e)))?;
        config.working_directory = Some(path);
        drop(config);
        self.save()
    }

    /// Add or update a provider configuration
    pub fn set_provider_config(&mut self, name: String, provider_config: ProviderConfig) -> Result<()> {
        let mut config = self.config.write().map_err(|e| KaiError::unknown(format!("Failed to write config: {}", e)))?;
        config.providers.insert(name, provider_config);
        drop(config);
        self.save()
    }

    /// Get a provider configuration
    pub fn get_provider_config(&self, name: &str) -> Option<ProviderConfig> {
        let config = self.config.read().ok()?;
        config.providers.get(name).cloned()
    }

    /// Remove a provider configuration
    pub fn remove_provider(&mut self, name: &str) -> Result<()> {
        let mut config = self.config.write().map_err(|e| KaiError::unknown(format!("Failed to write config: {}", e)))?;
        config.providers.remove(name);
        drop(config);
        self.save()
    }

    /// Get the configuration file path
    fn get_config_path() -> Result<PathBuf> {
        // Use data_local_dir which gives ~/Library/Application Support on macOS
        // and ~/.local/share on Linux, which is more appropriate for app config
        let config_dir = dirs::data_local_dir()
            .or_else(dirs::config_dir)
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

    /// Get API key for a provider, checking environment variables first, then config
    pub fn get_api_key_for_provider(&self, provider_name: &str) -> Option<String> {
        // Try environment variable first
        let env_key = format!("{}_API_KEY", provider_name.to_uppercase());
        if let Ok(api_key) = std::env::var(&env_key) {
            if !api_key.trim().is_empty() {
                return Some(api_key);
            }
        }
        
        // Fallback to config file
        self.providers.get(provider_name)
            .and_then(|p| p.api_key.clone())
    }

    /// Get API key for the currently active provider
    pub fn get_active_api_key(&self) -> Option<String> {
        self.get_api_key_for_provider(&self.active_provider)
    }

    /// Check if the configuration is valid
    pub fn validate(&self) -> Result<()> {
        // Skip provider validation if no provider is configured
        if !self.active_provider.is_empty() {
            // Validate active provider exists
            if !self.providers.contains_key(&self.active_provider) {
                return Err(KaiError::validation(
                    "active_provider",
                    format!("Provider '{}' not found in configuration", self.active_provider)
                ));
            }

            // Validate active provider has API key (either in config or environment)
            if self.get_active_provider_config().is_some() {
                if self.get_active_api_key().is_none() {
                    let env_key_name = format!("{}_API_KEY", self.active_provider.to_uppercase());
                    return Err(KaiError::validation(
                        "provider.api_key",
                        format!("No API key found for provider '{}'. Set {} environment variable or configure api_key in config.", 
                            self.active_provider, env_key_name)
                    ));
                }
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