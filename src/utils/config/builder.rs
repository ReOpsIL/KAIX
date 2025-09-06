//! Configuration builder patterns

use crate::config::{Config, ProviderConfig};
use std::collections::HashMap;
use std::path::PathBuf;

/// Builder for application configuration
pub struct ConfigBuilder {
    config: Config,
}

impl ConfigBuilder {
    /// Create a new config builder with defaults
    pub fn new() -> Self {
        Self {
            config: Config::default(),
        }
    }
    
    /// Create builder from existing config
    pub fn from_config(config: Config) -> Self {
        Self { config }
    }
    
    /// Set active provider
    pub fn active_provider(mut self, provider: String) -> Self {
        self.config.active_provider = provider;
        self
    }
    
    /// Set active model
    pub fn active_model(mut self, model: String) -> Self {
        self.config.active_model = model;
        self
    }
    
    /// Set working directory
    pub fn working_directory(mut self, path: PathBuf) -> Self {
        self.config.working_directory = Some(path);
        self
    }
    
    /// Add provider configuration
    pub fn add_provider(mut self, name: String, provider_config: ProviderConfig) -> Self {
        self.config.providers.insert(name, provider_config);
        self
    }
    
    /// Add provider with builder
    pub fn add_provider_with_builder<F>(self, name: String, builder_fn: F) -> Self
    where
        F: FnOnce(ProviderConfigBuilder) -> ProviderConfigBuilder,
    {
        let provider_config = builder_fn(ProviderConfigBuilder::new_for_provider(&name)).build();
        self.add_provider(name, provider_config)
    }
    
    /// Build the configuration
    pub fn build(self) -> Config {
        self.config
    }
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for provider configuration
pub struct ProviderConfigBuilder {
    config: ProviderConfig,
}

impl ProviderConfigBuilder {
    /// Create new provider config builder
    pub fn new() -> Self {
        Self {
            config: ProviderConfig::default(),
        }
    }
    
    /// Create builder for specific provider with defaults
    pub fn new_for_provider(provider_name: &str) -> Self {
        Self {
            config: ProviderConfig::new_for_provider(provider_name),
        }
    }
    
    /// Set API key
    pub fn api_key(mut self, api_key: String) -> Self {
        self.config.api_key = Some(api_key);
        self
    }
    
    /// Set base URL
    pub fn base_url(mut self, base_url: String) -> Self {
        self.config.base_url = Some(base_url);
        self
    }
    
    /// Set default model
    pub fn default_model(mut self, model: String) -> Self {
        self.config.default_model = Some(model);
        self
    }
    
    /// Add custom setting
    pub fn setting(mut self, key: String, value: serde_json::Value) -> Self {
        self.config.settings.insert(key, value);
        self
    }
    
    /// Add multiple settings
    pub fn settings(mut self, settings: HashMap<String, serde_json::Value>) -> Self {
        self.config.settings.extend(settings);
        self
    }
    
    /// Build the provider configuration
    pub fn build(self) -> ProviderConfig {
        self.config
    }
}

impl Default for ProviderConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for provider-specific settings used by LLM factory
pub struct ProviderSettingsBuilder {
    settings: HashMap<String, String>,
}

impl ProviderSettingsBuilder {
    /// Create new settings builder
    pub fn new() -> Self {
        Self {
            settings: HashMap::new(),
        }
    }
    
    /// Add API key setting
    pub fn api_key(mut self, api_key: String) -> Self {
        self.settings.insert("api_key".to_string(), api_key);
        self
    }
    
    /// Add base URL setting
    pub fn base_url(mut self, base_url: String) -> Self {
        self.settings.insert("base_url".to_string(), base_url);
        self
    }
    
    /// Add custom setting
    pub fn setting(mut self, key: String, value: String) -> Self {
        self.settings.insert(key, value);
        self
    }
    
    /// Add multiple settings
    pub fn settings(mut self, settings: HashMap<String, String>) -> Self {
        self.settings.extend(settings);
        self
    }
    
    /// Build from provider config and environment
    pub fn from_provider_config(
        provider_name: &str,
        provider_config: Option<&ProviderConfig>,
    ) -> Self {
        let mut builder = Self::new();
        
        // Add API key with environment precedence
        if let Some(api_key) = super::ApiKeyResolver::resolve_api_key(
            provider_name,
            provider_config.and_then(|p| p.api_key.as_deref())
        ) {
            builder = builder.api_key(api_key);
        }
        
        // Add base URL
        if let Some(base_url) = ProviderConfig::get_base_url_for_provider(provider_name) {
            builder = builder.base_url(base_url);
        }
        
        // Add custom settings from provider config
        if let Some(config) = provider_config {
            for (key, value) in &config.settings {
                if let Some(string_value) = value.as_str() {
                    builder = builder.setting(key.clone(), string_value.to_string());
                }
            }
        }
        
        builder
    }
    
    /// Build the settings HashMap
    pub fn build(self) -> HashMap<String, String> {
        self.settings
    }
}

impl Default for ProviderSettingsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    #[test]
    fn test_config_builder() {
        let config = ConfigBuilder::new()
            .active_provider("openrouter".to_string())
            .active_model("claude-3-haiku".to_string())
            .working_directory(PathBuf::from("/test"))
            .add_provider_with_builder("openrouter".to_string(), |builder| {
                builder
                    .api_key("test-key".to_string())
                    .default_model("claude-3-haiku".to_string())
            })
            .build();

        assert_eq!(config.active_provider, "openrouter");
        assert_eq!(config.active_model, "claude-3-haiku");
        assert_eq!(config.working_directory, Some(PathBuf::from("/test")));
        assert!(config.providers.contains_key("openrouter"));
        
        let provider = &config.providers["openrouter"];
        assert_eq!(provider.api_key, Some("test-key".to_string()));
        assert_eq!(provider.default_model, Some("claude-3-haiku".to_string()));
    }

    #[test]
    fn test_provider_config_builder() {
        let config = ProviderConfigBuilder::new_for_provider("gemini")
            .api_key("test-key".to_string())
            .setting("custom".to_string(), json!("value"))
            .build();

        assert_eq!(config.api_key, Some("test-key".to_string()));
        assert_eq!(config.base_url, Some("https://generativelanguage.googleapis.com/v1beta".to_string()));
        assert_eq!(config.settings.get("custom"), Some(&json!("value")));
    }

    #[test]
    fn test_provider_settings_builder() {
        std::env::remove_var("TEST_PROVIDER_API_KEY");
        
        let provider_config = ProviderConfig {
            api_key: Some("config-key".to_string()),
            base_url: Some("https://test.com".to_string()),
            default_model: Some("test-model".to_string()),
            settings: {
                let mut settings = HashMap::new();
                settings.insert("custom".to_string(), json!("value"));
                settings
            },
        };

        let settings = ProviderSettingsBuilder::from_provider_config("test_provider", Some(&provider_config))
            .setting("extra".to_string(), "extra-value".to_string())
            .build();

        assert_eq!(settings.get("api_key"), Some(&"config-key".to_string()));
        assert_eq!(settings.get("extra"), Some(&"extra-value".to_string()));
        assert_eq!(settings.get("custom"), Some(&"value".to_string()));
    }

    #[test]
    fn test_provider_settings_builder_env_precedence() {
        std::env::set_var("TEST_PROVIDER_API_KEY", "env-key");
        
        let provider_config = ProviderConfig {
            api_key: Some("config-key".to_string()),
            base_url: None,
            default_model: None,
            settings: HashMap::new(),
        };

        let settings = ProviderSettingsBuilder::from_provider_config("test_provider", Some(&provider_config))
            .build();

        assert_eq!(settings.get("api_key"), Some(&"env-key".to_string()));
        
        std::env::remove_var("TEST_PROVIDER_API_KEY");
    }
}