//! Configuration access patterns and utilities

use crate::config::{Config, ProviderConfig};
use std::collections::HashMap;

/// Trait for unified configuration access
pub trait ConfigAccess {
    /// Get configuration reference
    fn config(&self) -> &Config;
    
    /// Get API key for provider with environment precedence
    fn get_api_key(&self, provider_name: &str) -> Option<String> {
        super::ApiKeyResolver::resolve_api_key(
            provider_name,
            self.config().providers.get(provider_name)
                .and_then(|p| p.api_key.as_deref())
        )
    }
    
    /// Get base URL for provider
    fn get_base_url(&self, provider_name: &str) -> Option<String> {
        ProviderConfig::get_base_url_for_provider(provider_name)
    }
    
    /// Get provider settings for LLM factory
    fn get_provider_settings(&self, provider_name: &str) -> HashMap<String, String> {
        let mut settings = HashMap::new();
        
        if let Some(api_key) = self.get_api_key(provider_name) {
            settings.insert("api_key".to_string(), api_key);
        }
        
        if let Some(base_url) = self.get_base_url(provider_name) {
            settings.insert("base_url".to_string(), base_url);
        }
        
        // Add custom settings from provider config
        if let Some(provider_config) = self.config().providers.get(provider_name) {
            for (key, value) in &provider_config.settings {
                if let Some(string_value) = value.as_str() {
                    settings.insert(key.clone(), string_value.to_string());
                }
            }
        }
        
        settings
    }
    
    /// Check if provider is ready (has API key)
    fn is_provider_ready(&self, provider_name: &str) -> bool {
        self.get_api_key(provider_name).is_some()
    }
    
    /// Get active provider settings
    fn get_active_provider_settings(&self) -> HashMap<String, String> {
        let active_provider = &self.config().active_provider;
        self.get_provider_settings(active_provider)
    }
    
    /// Validate active provider configuration
    fn validate_active_provider(&self) -> Result<(), crate::utils::errors::KaiError> {
        let config = self.config();
        
        if config.active_provider.is_empty() {
            return Err(crate::utils::errors::KaiError::not_found(
                "No active provider configured"
            ));
        }
        
        if !self.is_provider_ready(&config.active_provider) {
            let env_key = super::ApiKeyResolver::env_key_name(&config.active_provider);
            return Err(crate::utils::errors::KaiError::validation(
                "provider.api_key",
                format!("No API key found for provider '{}'. Set {} environment variable or configure api_key in config.", 
                    config.active_provider, env_key)
            ));
        }
        
        Ok(())
    }
}

/// API key resolution utilities
pub struct ApiKeyResolver;

impl ApiKeyResolver {
    /// Resolve API key with environment variable precedence
    pub fn resolve(provider_name: &str, config_api_key: Option<&str>) -> Option<String> {
        super::ApiKeyResolver::resolve_api_key(provider_name, config_api_key)
    }
    
    /// Get environment variable name for provider
    pub fn env_var_name(provider_name: &str) -> String {
        super::ApiKeyResolver::env_key_name(provider_name)
    }
    
    /// Check if API key is available
    pub fn is_available(provider_name: &str, config_api_key: Option<&str>) -> bool {
        Self::resolve(provider_name, config_api_key).is_some()
    }
    
    /// Get status of API key sources
    pub fn get_key_status(provider_name: &str, config_api_key: Option<&str>) -> ApiKeyStatus {
        let env_var_name = Self::env_var_name(provider_name);
        let has_env_key = std::env::var(&env_var_name).is_ok();
        let has_config_key = config_api_key.is_some();
        
        match (has_env_key, has_config_key) {
            (true, _) => ApiKeyStatus::Environment(env_var_name),
            (false, true) => ApiKeyStatus::Config,
            (false, false) => ApiKeyStatus::Missing(env_var_name),
        }
    }
}

/// Status of API key availability
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiKeyStatus {
    /// Key available from environment variable
    Environment(String),
    /// Key available from config file
    Config,
    /// Key missing, shows expected env var name
    Missing(String),
}

impl ApiKeyStatus {
    /// Check if API key is available
    pub fn is_available(&self) -> bool {
        !matches!(self, ApiKeyStatus::Missing(_))
    }
    
    /// Get display string for status
    pub fn display(&self) -> &'static str {
        match self {
            ApiKeyStatus::Environment(_) => "🌍 Environment",
            ApiKeyStatus::Config => "📁 Config",
            ApiKeyStatus::Missing(_) => "❌ Missing",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, ProviderConfig};
    use std::collections::HashMap;

    struct TestConfigAccess {
        config: Config,
    }

    impl ConfigAccess for TestConfigAccess {
        fn config(&self) -> &Config {
            &self.config
        }
    }

    #[test]
    fn test_config_access_api_key_resolution() {
        let mut providers = HashMap::new();
        providers.insert("test".to_string(), ProviderConfig {
            api_key: Some("config-key".to_string()),
            base_url: None,
            default_model: None,
            settings: HashMap::new(),
        });

        let config = Config {
            active_provider: "test".to_string(),
            active_model: "test-model".to_string(),
            working_directory: None,
            providers,
            ui: Default::default(),
            context: Default::default(),
            execution: Default::default(),
            logging: Default::default(),
        };

        let accessor = TestConfigAccess { config };

        // Test config fallback
        std::env::remove_var("TEST_API_KEY");
        assert_eq!(accessor.get_api_key("test"), Some("config-key".to_string()));

        // Test environment precedence
        std::env::set_var("TEST_API_KEY", "env-key");
        assert_eq!(accessor.get_api_key("test"), Some("env-key".to_string()));

        std::env::remove_var("TEST_API_KEY");
    }

    #[test]
    fn test_api_key_status() {
        std::env::remove_var("TEST_API_KEY");
        
        // Test missing key
        let status = ApiKeyResolver::get_key_status("test", None);
        assert_eq!(status, ApiKeyStatus::Missing("TEST_API_KEY".to_string()));
        assert!(!status.is_available());
        
        // Test config key
        let status = ApiKeyResolver::get_key_status("test", Some("config-key"));
        assert_eq!(status, ApiKeyStatus::Config);
        assert!(status.is_available());
        
        // Test environment key
        std::env::set_var("TEST_API_KEY", "env-key");
        let status = ApiKeyResolver::get_key_status("test", Some("config-key"));
        assert_eq!(status, ApiKeyStatus::Environment("TEST_API_KEY".to_string()));
        assert!(status.is_available());
        
        std::env::remove_var("TEST_API_KEY");
    }
}