//! HTTP header utilities for LLM providers

use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION, CONTENT_TYPE, USER_AGENT};
use std::collections::HashMap;

/// Common HTTP headers used across providers
pub struct CommonHeaders;

impl CommonHeaders {
    /// Content-Type: application/json
    pub fn json_content_type() -> (HeaderName, HeaderValue) {
        (CONTENT_TYPE, HeaderValue::from_static("application/json"))
    }
    
    /// User-Agent header with application name
    pub fn user_agent(app_name: &str) -> (HeaderName, HeaderValue) {
        let value = HeaderValue::from_str(app_name)
            .unwrap_or_else(|_| HeaderValue::from_static("KAI-X/1.0"));
        (USER_AGENT, value)
    }
    
    /// Authorization header with Bearer token
    pub fn bearer_auth(token: &str) -> Result<(HeaderName, HeaderValue), reqwest::header::InvalidHeaderValue> {
        let value = HeaderValue::from_str(&format!("Bearer {}", token))?;
        Ok((AUTHORIZATION, value))
    }
}

/// Builder for HTTP headers with provider-specific customizations
pub struct HeaderBuilder {
    headers: HeaderMap,
}

impl HeaderBuilder {
    /// Create a new header builder
    pub fn new() -> Self {
        Self {
            headers: HeaderMap::new(),
        }
    }
    
    /// Add content type JSON
    pub fn json_content_type(mut self) -> Self {
        let (name, value) = CommonHeaders::json_content_type();
        self.headers.insert(name, value);
        self
    }
    
    /// Add user agent header
    pub fn user_agent(mut self, app_name: &str) -> Self {
        let (name, value) = CommonHeaders::user_agent(app_name);
        self.headers.insert(name, value);
        self
    }
    
    /// Add Bearer authorization
    pub fn bearer_auth(mut self, token: &str) -> Result<Self, reqwest::header::InvalidHeaderValue> {
        let (name, value) = CommonHeaders::bearer_auth(token)?;
        self.headers.insert(name, value);
        Ok(self)
    }
    
    /// Add custom header
    pub fn header<K, V>(mut self, key: K, value: V) -> Result<Self, Box<dyn std::error::Error>>
    where
        HeaderName: TryFrom<K>,
        <HeaderName as TryFrom<K>>::Error: std::error::Error + 'static,
        HeaderValue: TryFrom<V>,
        <HeaderValue as TryFrom<V>>::Error: std::error::Error + 'static,
    {
        let name = HeaderName::try_from(key)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
        let value = HeaderValue::try_from(value)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
        self.headers.insert(name, value);
        Ok(self)
    }
    
    /// Add multiple headers from a HashMap
    pub fn headers(mut self, headers: HashMap<String, String>) -> Result<Self, Box<dyn std::error::Error>> {
        for (key, value) in headers {
            let name = HeaderName::try_from(key)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
            let header_value = HeaderValue::try_from(value)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
            self.headers.insert(name, header_value);
        }
        Ok(self)
    }
    
    /// Build the header map
    pub fn build(self) -> HeaderMap {
        self.headers
    }
}

impl Default for HeaderBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Provider-specific header builders
pub trait ProviderHeaders {
    /// Create headers for this provider
    fn create_headers(&self, api_key: &str) -> HeaderMap;
}

/// OpenRouter-specific header configuration
pub struct OpenRouterHeaders {
    pub referer: String,
    pub title: String,
}

impl OpenRouterHeaders {
    pub fn new() -> Self {
        Self {
            referer: "https://github.com/your-org/KAI-X".to_string(),
            title: "KAI-X".to_string(),
        }
    }
    
    pub fn with_referer(mut self, referer: String) -> Self {
        self.referer = referer;
        self
    }
    
    pub fn with_title(mut self, title: String) -> Self {
        self.title = title;
        self
    }
}

impl ProviderHeaders for OpenRouterHeaders {
    fn create_headers(&self, api_key: &str) -> HeaderMap {
        HeaderBuilder::new()
            .json_content_type()
            .user_agent("KAI-X/1.0")
            .bearer_auth(api_key)
            .unwrap_or_else(|_| HeaderBuilder::new().json_content_type().user_agent("KAI-X/1.0"))
            .header("HTTP-Referer", &self.referer)
            .unwrap_or_else(|_| HeaderBuilder::new().json_content_type().user_agent("KAI-X/1.0"))
            .header("X-Title", &self.title)
            .unwrap_or_else(|_| HeaderBuilder::new().json_content_type().user_agent("KAI-X/1.0"))
            .build()
    }
}

/// Gemini-specific header configuration (simpler than OpenRouter)
pub struct GeminiHeaders;

impl ProviderHeaders for GeminiHeaders {
    fn create_headers(&self, _api_key: &str) -> HeaderMap {
        // Gemini uses API key in URL, not headers
        HeaderBuilder::new()
            .json_content_type()
            .user_agent("KAI-X/1.0")
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_common_headers() {
        let (name, value) = CommonHeaders::json_content_type();
        assert_eq!(name, CONTENT_TYPE);
        assert_eq!(value, "application/json");
        
        let (name, value) = CommonHeaders::user_agent("TestApp/1.0");
        assert_eq!(name, USER_AGENT);
        assert_eq!(value, "TestApp/1.0");
        
        let (name, value) = CommonHeaders::bearer_auth("test-token").unwrap();
        assert_eq!(name, AUTHORIZATION);
        assert_eq!(value, "Bearer test-token");
    }
    
    #[test]
    fn test_header_builder() {
        let headers = HeaderBuilder::new()
            .json_content_type()
            .user_agent("TestApp/1.0")
            .bearer_auth("test-token")
            .unwrap()
            .header("Custom-Header", "custom-value")
            .unwrap()
            .build();
        
        assert_eq!(headers.get(CONTENT_TYPE).unwrap(), "application/json");
        assert_eq!(headers.get(USER_AGENT).unwrap(), "TestApp/1.0");
        assert_eq!(headers.get(AUTHORIZATION).unwrap(), "Bearer test-token");
        assert_eq!(headers.get("Custom-Header").unwrap(), "custom-value");
    }
    
    #[test]
    fn test_openrouter_headers() {
        let openrouter = OpenRouterHeaders::new()
            .with_referer("https://example.com".to_string())
            .with_title("MyApp".to_string());
        
        let headers = openrouter.create_headers("test-api-key");
        
        assert_eq!(headers.get(CONTENT_TYPE).unwrap(), "application/json");
        assert_eq!(headers.get(AUTHORIZATION).unwrap(), "Bearer test-api-key");
        assert_eq!(headers.get("HTTP-Referer").unwrap(), "https://example.com");
        assert_eq!(headers.get("X-Title").unwrap(), "MyApp");
    }
    
    #[test]
    fn test_gemini_headers() {
        let gemini = GeminiHeaders;
        let headers = gemini.create_headers("test-api-key");
        
        assert_eq!(headers.get(CONTENT_TYPE).unwrap(), "application/json");
        assert_eq!(headers.get(USER_AGENT).unwrap(), "KAI-X/1.0");
        // Gemini doesn't use Authorization header
        assert!(headers.get(AUTHORIZATION).is_none());
    }
}