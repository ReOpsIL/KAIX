//! Shared configuration utilities and patterns
//! 
//! This module provides common configuration access patterns used across the application,
//! eliminating duplication in API key retrieval, provider setup, and configuration management.

use crate::config::{Config, ConfigManager, ProviderConfig};
use crate::Result;
use std::collections::HashMap;

pub mod access;
pub mod builder;
pub mod provider;

pub use access::ConfigAccess;
pub use builder::{ConfigBuilder, ProviderConfigBuilder};
pub use provider::{ProviderSettings, ProviderSettingsBuilder};

/// Trait for unified configuration access patterns
pub trait ConfigAccessPattern {
    /// Get API key for a provider (environment variable takes precedence)
    fn get_provider_api_key(&self, provider_name: &str) -> Option<String>;
    
    /// Get base URL for a provider (hardcoded based on provider type)
    fn get_provider_base_url(&self, provider_name: &str) -> Option<String>;
    
    /// Get provider settings as HashMap for LLM factory
    fn get_provider_settings(&self, provider_name: &str) -> HashMap<String, String>;
    
    /// Check if provider is configured and has API key
    fn is_provider_ready(&self, provider_name: &str) -> bool;
}

/// Standard implementation of configuration access patterns
impl ConfigAccessPattern for Config {
    fn get_provider_api_key(&self, provider_name: &str) -> Option<String> {
        self.get_api_key_for_provider(provider_name)
    }
    
    fn get_provider_base_url(&self, provider_name: &str) -> Option<String> {
        ProviderConfig::get_base_url_for_provider(provider_name)
    }
    
    fn get_provider_settings(&self, provider_name: &str) -> HashMap<String, String> {
        let mut settings = HashMap::new();
        
        // Add API key if available
        if let Some(api_key) = self.get_provider_api_key(provider_name) {
            settings.insert("api_key".to_string(), api_key);
        }
        
        // Add base URL if available
        if let Some(base_url) = self.get_provider_base_url(provider_name) {
            settings.insert("base_url".to_string(), base_url);
        }
        
        // Add provider-specific settings from config
        if let Some(provider_config) = self.providers.get(provider_name) {
            for (key, value) in &provider_config.settings {
                if let Some(string_value) = value.as_str() {
                    settings.insert(key.clone(), string_value.to_string());
                }
            }
        }
        
        settings
    }
    
    fn is_provider_ready(&self, provider_name: &str) -> bool {
        self.get_provider_api_key(provider_name).is_some()
    }
}

/// Extended configuration access for ConfigManager
pub trait ConfigManagerExtensions {
    /// Get active provider settings ready for LLM factory
    fn get_active_provider_settings(&self) -> HashMap<String, String>;
    
    /// Validate active provider is ready for use
    fn validate_active_provider(&self) -> Result<()>;
    
    /// Get provider configuration with fallbacks
    fn get_provider_config_or_default(&self, provider_name: &str) -> ProviderConfig;
}

impl ConfigManagerExtensions for ConfigManager {
    fn get_active_provider_settings(&self) -> HashMap<String, String> {
        let config = self.config();
        config.get_provider_settings(&config.active_provider)
    }
    
    fn validate_active_provider(&self) -> Result<()> {
        let config = self.config();
        
        if config.active_provider.is_empty() {
            return Err(crate::utils::errors::KaiError::not_found(
                "No active provider configured"
            ));
        }
        
        if !config.is_provider_ready(&config.active_provider) {
            let env_key = format!("{}_API_KEY", config.active_provider.to_uppercase());
            return Err(crate::utils::errors::KaiError::validation(
                "provider.api_key",
                format!("No API key found for provider '{}'. Set {} environment variable or configure api_key in config.", 
                    config.active_provider, env_key)
            ));
        }
        
        Ok(())
    }
    
    fn get_provider_config_or_default(&self, provider_name: &str) -> ProviderConfig {
        self.get_provider_config(provider_name)
            .unwrap_or_else(|| ProviderConfig::new_for_provider(provider_name))
    }
}

/// Centralized API key resolution logic
pub struct ApiKeyResolver;

impl ApiKeyResolver {
    /// Get API key for provider with environment variable precedence
    pub fn resolve_api_key(provider_name: &str, config_api_key: Option<&str>) -> Option<String> {
        // Try environment variable first
        let env_key = format!("{}_API_KEY", provider_name.to_uppercase());
        if let Ok(api_key) = std::env::var(&env_key) {
            if !api_key.trim().is_empty() {
                return Some(api_key);
            }
        }
        
        // Fallback to config file
        config_api_key.map(|s| s.to_string())
    }
    
    /// Get environment variable name for provider
    pub fn env_key_name(provider_name: &str) -> String {
        format!("{}_API_KEY", provider_name.to_uppercase())
    }
    
    /// Check if provider has API key available (either env var or config)
    pub fn has_api_key(provider_name: &str, config_api_key: Option<&str>) -> bool {
        Self::resolve_api_key(provider_name, config_api_key).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_api_key_resolver_env_precedence() {
        // Set environment variable
        std::env::set_var("TEST_PROVIDER_API_KEY", "env-key");
        
        let result = ApiKeyResolver::resolve_api_key("test_provider", Some("config-key"));
        assert_eq!(result, Some("env-key".to_string()));
        
        // Clean up
        std::env::remove_var("TEST_PROVIDER_API_KEY");
    }
    
    #[test]
    fn test_api_key_resolver_config_fallback() {
        // Ensure no env var
        std::env::remove_var("TEST_PROVIDER_API_KEY");
        
        let result = ApiKeyResolver::resolve_api_key("test_provider", Some("config-key"));
        assert_eq!(result, Some("config-key".to_string()));
    }
    
    #[test]
    fn test_api_key_resolver_none_available() {
        // Ensure no env var
        std::env::remove_var("TEST_PROVIDER_API_KEY");
        
        let result = ApiKeyResolver::resolve_api_key("test_provider", None);
        assert_eq!(result, None);
    }
    
    #[test]
    fn test_env_key_name_generation() {
        assert_eq!(ApiKeyResolver::env_key_name("openrouter"), "OPENROUTER_API_KEY");
        assert_eq!(ApiKeyResolver::env_key_name("gemini"), "GEMINI_API_KEY");
        assert_eq!(ApiKeyResolver::env_key_name("test_provider"), "TEST_PROVIDER_API_KEY");
    }
}