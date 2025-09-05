//! Configuration management for KAI-X

use crate::utils::errors::{ConfigError, KaiError};
use crate::Result;
use dirs;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::SystemTime;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

// Sub-modules for enhanced configuration functionality
pub mod enhanced;
pub mod secure_storage;
pub mod validation;
pub mod migration;
pub mod slash_integration;

// Re-export key types from sub-modules
pub use enhanced::{
    EnhancedConfigManager, SessionState, HistoryEntry, HistoryEntryType,
    UiState, LayoutState, PlanRecoveryData, SessionStats
};
pub use secure_storage::{
    SecureStorageConfig, FileEncryptionConfig
};
pub use validation::{
    KaiConfigValidator, ValidationResult, ValidationError, ValidationWarning,
    ValidationSuggestion, ErrorSeverity, SuggestionPriority, ConfigValidator
};
pub use migration::{
    ConfigMigrator, Migration, MigrationPlan, MigrationStep, MigrationLogEntry
};
pub use slash_integration::EnhancedSlashCommandProcessor;

/// Configuration change events
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
    /// Configuration validated
    ValidationCompleted {
        success: bool,
        errors: Vec<String>,
    },
    /// Configuration backup created
    BackupCreated {
        backup_path: PathBuf,
        timestamp: SystemTime,
    },
    /// Configuration migrated
    ConfigMigrated {
        from_version: String,
        to_version: String,
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
            active_provider: "".to_string(),
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
    config: Arc<RwLock<Config>>,
    config_path: PathBuf,
    session_state: Arc<RwLock<SessionState>>,
    session_path: PathBuf,
    backup_dir: PathBuf,
    event_sender: Option<mpsc::UnboundedSender<ConfigEvent>>,
    /// Configuration version for migration support
    config_version: String,
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

        let session_path = Self::get_session_path()?;
        let session_state = Arc::new(RwLock::new(SessionState::default()));
        let backup_dir = Self::get_backup_dir()?;

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            config_path,
            session_state,
            session_path,
            backup_dir,
            event_sender: None,
            config_version: "1.0.0".to_string(),
        })
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
        Ok(f(&*config))
    }

    /// Save the current configuration to disk
    pub fn save(&self) -> Result<()> {
        let config = self.config.read().map_err(|e| KaiError::unknown(format!("Failed to read config: {}", e)))?;
        Self::save_config(&self.config_path, &*config)
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

    /// Get the session file path
    fn get_session_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| ConfigError::FileNotFound { 
                path: PathBuf::from("config directory")
            })?;
        
        let app_config_dir = config_dir.join("kai-x");
        if !app_config_dir.exists() {
            fs::create_dir_all(&app_config_dir)
                .map_err(|e| ConfigError::WriteError { source: e })?;
        }
        
        Ok(app_config_dir.join("session.json"))
    }

    /// Get the backup directory path
    fn get_backup_dir() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| ConfigError::FileNotFound { 
                path: PathBuf::from("config directory")
            })?;
        
        let app_config_dir = config_dir.join("kai-x");
        let backup_dir = app_config_dir.join("backups");
        
        if !backup_dir.exists() {
            fs::create_dir_all(&backup_dir)
                .map_err(|e| ConfigError::WriteError { source: e })?;
        }
        
        Ok(backup_dir)
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
    
    /// Create enhanced configuration manager with full functionality
    pub fn create_enhanced(event_sender: mpsc::UnboundedSender<ConfigEvent>) -> Result<(Self, EnhancedConfigManager)> {
        let config_manager = Self::new()?; // Note: we'd need to add with_events method to base ConfigManager
        // Convert ConfigEvent to enhanced::ConfigEvent
        let (enhanced_sender, mut enhanced_receiver) = mpsc::unbounded_channel::<enhanced::ConfigEvent>();
        let session_manager = EnhancedConfigManager::with_events(enhanced_sender)?;
        
        Ok((config_manager, session_manager))
    }
    
    /// Initialize configuration system with migration support
    pub fn initialize_with_migration() -> Result<(Self, Vec<String>)> {
        let mut migrator = ConfigMigrator::new();
        let config_path = Self::get_config_path()?;
        let backup_dir = dirs::config_dir()
            .ok_or_else(|| ConfigError::FileNotFound { 
                path: PathBuf::from("config directory")
            })?
            .join("kai-x")
            .join("backups");
        
        let mut warnings = Vec::new();
        
        // Check if migration is needed
        if migrator.needs_migration(&config_path)? {
            info!("Configuration migration required");
            
            // Perform migration
            migrator.migrate_config_file(&config_path, Some(&backup_dir))?;
            
            // Collect migration warnings
            for entry in migrator.get_migration_history() {
                if entry.successful {
                    warnings.extend(entry.notes.iter().cloned());
                }
            }
            
            info!("Configuration migration completed successfully");
        }
        
        // Load migrated configuration
        let config_manager = Self::new()?;
        
        Ok((config_manager, warnings))
    }
    
    /// Validate current configuration comprehensively
    pub fn validate_comprehensive(&self) -> Result<ValidationResult> {
        let validator = KaiConfigValidator::new();
        let config = self.config.read().map_err(|e| KaiError::unknown(format!("Failed to read config: {}", e)))?;
        Ok(validator.validate_config(&*config))
    }
    
    
    /// Create configuration backup
    pub fn create_backup(&self) -> Result<PathBuf> {
        let backup_dir = dirs::config_dir()
            .ok_or_else(|| ConfigError::FileNotFound { 
                path: PathBuf::from("config directory")
            })?
            .join("kai-x")
            .join("backups");
            
        if !backup_dir.exists() {
            fs::create_dir_all(&backup_dir)
                .map_err(|e| ConfigError::WriteError { source: e })?;
        }
        
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|e| KaiError::unknown(format!("Time error: {}", e)))?
            .as_secs();
        
        let backup_path = backup_dir.join(format!("config_backup_{}.toml", timestamp));
        let config = self.config.read().map_err(|e| KaiError::unknown(format!("Failed to read config: {}", e)))?;
        Self::save_config(&backup_path, &*config)?;
        
        info!("Configuration backup created at {}", backup_path.display());
        Ok(backup_path)
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