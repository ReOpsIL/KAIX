//! Provider-specific configuration utilities

use crate::config::ProviderConfig;
use std::collections::HashMap;

/// Provider settings for LLM factory
pub type ProviderSettings = HashMap<String, String>;

/// Builder for provider settings
pub struct ProviderSettingsBuilder {
    settings: ProviderSettings,
}

impl ProviderSettingsBuilder {
    /// Create new builder
    pub fn new() -> Self {
        Self {
            settings: HashMap::new(),
        }
    }
    
    /// Add API key
    pub fn with_api_key(mut self, api_key: String) -> Self {
        self.settings.insert("api_key".to_string(), api_key);
        self
    }
    
    /// Add base URL
    pub fn with_base_url(mut self, base_url: String) -> Self {
        self.settings.insert("base_url".to_string(), base_url);
        self
    }
    
    /// Add custom setting
    pub fn with_setting(mut self, key: String, value: String) -> Self {
        self.settings.insert(key, value);
        self
    }
    
    /// Add multiple settings
    pub fn with_settings(mut self, settings: HashMap<String, String>) -> Self {
        self.settings.extend(settings);
        self
    }
    
    /// Build the settings
    pub fn build(self) -> ProviderSettings {
        self.settings
    }
}

impl Default for ProviderSettingsBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Utility functions for provider configuration
pub struct ProviderUtils;

impl ProviderUtils {
    /// Create provider settings from config and environment
    pub fn create_settings(
        provider_name: &str,
        provider_config: Option<&ProviderConfig>,
    ) -> ProviderSettings {
        let mut settings = HashMap::new();
        
        // Add API key with environment precedence
        if let Some(api_key) = super::ApiKeyResolver::resolve_api_key(
            provider_name,
            provider_config.and_then(|p| p.api_key.as_deref())
        ) {
            settings.insert("api_key".to_string(), api_key);
        }
        
        // Add base URL
        if let Some(base_url) = ProviderConfig::get_base_url_for_provider(provider_name) {
            settings.insert("base_url".to_string(), base_url);
        }
        
        // Add custom settings from provider config
        if let Some(config) = provider_config {
            for (key, value) in &config.settings {
                if let Some(string_value) = value.as_str() {
                    settings.insert(key.clone(), string_value.to_string());
                }
            }
        }
        
        settings
    }
    
    /// Validate provider settings have required keys
    pub fn validate_settings(
        provider_name: &str,
        settings: &ProviderSettings,
    ) -> Result<(), crate::utils::errors::KaiError> {
        match provider_name.to_lowercase().as_str() {
            "openrouter" | "gemini" => {
                if !settings.contains_key("api_key") {
                    let env_key = super::ApiKeyResolver::env_key_name(provider_name);
                    return Err(crate::utils::errors::KaiError::validation(
                        "provider.api_key",
                        format!("API key required for {}. Set {} environment variable or configure in settings.", 
                            provider_name, env_key)
                    ));
                }
            }
            _ => {
                // For unknown providers, just require API key
                if !settings.contains_key("api_key") {
                    return Err(crate::utils::errors::KaiError::validation(
                        "provider.api_key",
                        "API key required for provider"
                    ));
                }
            }
        }
        
        Ok(())
    }
    
    /// Get expected base URL for provider
    pub fn get_expected_base_url(provider_name: &str) -> Option<String> {
        ProviderConfig::get_base_url_for_provider(provider_name)
    }
    
    /// Get default model for provider
    pub fn get_default_model(provider_name: &str) -> Option<String> {
        match provider_name.to_lowercase().as_str() {
            "openrouter" => Some("google/gemini-2.5-pro".to_string()),
            "gemini" => Some("gemini-pro".to_string()),
            "openai" => Some("gpt-3.5-turbo".to_string()),
            "anthropic" => Some("claude-3-haiku-20240307".to_string()),
            _ => None,
        }
    }
    
    /// Check if provider is recognized
    pub fn is_known_provider(provider_name: &str) -> bool {
        matches!(
            provider_name.to_lowercase().as_str(),
            "openrouter" | "gemini" | "openai" | "anthropic"
        )
    }
}

/// Provider metadata and capabilities
#[derive(Debug, Clone)]
pub struct ProviderMetadata {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub base_url: Option<String>,
    pub default_model: Option<String>,
    pub supports_streaming: bool,
    pub supports_tools: bool,
    pub requires_api_key: bool,
}

impl ProviderMetadata {
    /// Get metadata for known providers
    pub fn for_provider(provider_name: &str) -> Option<Self> {
        match provider_name.to_lowercase().as_str() {
            "openrouter" => Some(Self {
                name: "openrouter".to_string(),
                display_name: "OpenRouter".to_string(),
                description: "Access to multiple LLM providers through OpenRouter".to_string(),
                base_url: Some("https://openrouter.ai/api/v1".to_string()),
                default_model: Some("google/gemini-2.5-pro".to_string()),
                supports_streaming: true,
                supports_tools: true,
                requires_api_key: true,
            }),
            "gemini" => Some(Self {
                name: "gemini".to_string(),
                display_name: "Google Gemini".to_string(),
                description: "Google's Gemini language models".to_string(),
                base_url: Some("https://generativelanguage.googleapis.com/v1beta".to_string()),
                default_model: Some("gemini-pro".to_string()),
                supports_streaming: true,
                supports_tools: true,
                requires_api_key: true,
            }),
            "openai" => Some(Self {
                name: "openai".to_string(),
                display_name: "OpenAI".to_string(),
                description: "OpenAI's GPT models".to_string(),
                base_url: Some("https://api.openai.com/v1".to_string()),
                default_model: Some("gpt-3.5-turbo".to_string()),
                supports_streaming: true,
                supports_tools: true,
                requires_api_key: true,
            }),
            "anthropic" => Some(Self {
                name: "anthropic".to_string(),
                display_name: "Anthropic".to_string(),
                description: "Anthropic's Claude models".to_string(),
                base_url: Some("https://api.anthropic.com".to_string()),
                default_model: Some("claude-3-haiku-20240307".to_string()),
                supports_streaming: true,
                supports_tools: true,
                requires_api_key: true,
            }),
            _ => None,
        }
    }
    
    /// Get all known providers
    pub fn all_providers() -> Vec<Self> {
        vec!["openrouter", "gemini", "openai", "anthropic"]
            .into_iter()
            .filter_map(Self::for_provider)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_provider_settings_builder() {
        let settings = ProviderSettingsBuilder::new()
            .with_api_key("test-key".to_string())
            .with_base_url("https://test.com".to_string())
            .with_setting("custom".to_string(), "value".to_string())
            .build();

        assert_eq!(settings.get("api_key"), Some(&"test-key".to_string()));
        assert_eq!(settings.get("base_url"), Some(&"https://test.com".to_string()));
        assert_eq!(settings.get("custom"), Some(&"value".to_string()));
    }

    #[test]
    fn test_provider_utils_create_settings() {
        std::env::remove_var("TEST_API_KEY");
        
        let provider_config = ProviderConfig {
            api_key: Some("config-key".to_string()),
            base_url: None,
            default_model: None,
            settings: {
                let mut settings = HashMap::new();
                settings.insert("custom".to_string(), json!("value"));
                settings
            },
        };

        let settings = ProviderUtils::create_settings("test", Some(&provider_config));
        
        assert_eq!(settings.get("api_key"), Some(&"config-key".to_string()));
        assert_eq!(settings.get("custom"), Some(&"value".to_string()));
    }

    #[test]
    fn test_provider_utils_validation() {
        let mut settings = HashMap::new();
        settings.insert("base_url".to_string(), "https://test.com".to_string());
        
        // Should fail without API key
        let result = ProviderUtils::validate_settings("openrouter", &settings);
        assert!(result.is_err());
        
        // Should succeed with API key
        settings.insert("api_key".to_string(), "test-key".to_string());
        let result = ProviderUtils::validate_settings("openrouter", &settings);
        assert!(result.is_ok());
    }

    #[test]
    fn test_provider_metadata() {
        let metadata = ProviderMetadata::for_provider("openrouter").unwrap();
        assert_eq!(metadata.name, "openrouter");
        assert_eq!(metadata.display_name, "OpenRouter");
        assert!(metadata.supports_tools);
        assert!(metadata.requires_api_key);

        let all_providers = ProviderMetadata::all_providers();
        assert_eq!(all_providers.len(), 4);
    }

    #[test]
    fn test_provider_utils_known_providers() {
        assert!(ProviderUtils::is_known_provider("openrouter"));
        assert!(ProviderUtils::is_known_provider("gemini"));
        assert!(!ProviderUtils::is_known_provider("unknown"));
    }
}