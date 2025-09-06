//! HTTP client configuration and management

use crate::llm::LlmError;
use reqwest::Client;
use std::time::Duration;
use super::{DEFAULT_TIMEOUT, DEFAULT_RETRY_ATTEMPTS, DEFAULT_RETRY_DELAY};

/// Configuration for HTTP client
#[derive(Debug, Clone)]
pub struct HttpClientConfig {
    pub timeout: Duration,
    pub retry_attempts: usize,
    pub retry_delay: Duration,
    pub user_agent: Option<String>,
    pub max_redirects: Option<usize>,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            retry_attempts: DEFAULT_RETRY_ATTEMPTS,
            retry_delay: DEFAULT_RETRY_DELAY,
            user_agent: Some("KAI-X/1.0".to_string()),
            max_redirects: Some(10),
        }
    }
}

/// Builder for HTTP client configuration
pub struct HttpClientBuilder {
    config: HttpClientConfig,
}

impl HttpClientBuilder {
    pub fn new() -> Self {
        Self {
            config: HttpClientConfig::default(),
        }
    }

    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = timeout;
        self
    }

    pub fn retry_attempts(mut self, attempts: usize) -> Self {
        self.config.retry_attempts = attempts;
        self
    }

    pub fn retry_delay(mut self, delay: Duration) -> Self {
        self.config.retry_delay = delay;
        self
    }

    pub fn user_agent(mut self, user_agent: String) -> Self {
        self.config.user_agent = Some(user_agent);
        self
    }

    pub fn max_redirects(mut self, redirects: usize) -> Self {
        self.config.max_redirects = Some(redirects);
        self
    }

    pub fn build(self) -> HttpClientConfig {
        self.config
    }
}

impl Default for HttpClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Wrapper around reqwest::Client with shared configuration
pub struct HttpClient {
    client: Client,
    config: HttpClientConfig,
}

impl HttpClient {
    /// Create a new HTTP client with the given configuration
    pub fn new(config: HttpClientConfig) -> Result<Self, LlmError> {
        let mut builder = Client::builder()
            .timeout(config.timeout);

        if let Some(ref user_agent) = config.user_agent {
            builder = builder.user_agent(user_agent);
        }

        if let Some(redirects) = config.max_redirects {
            builder = builder.redirect(reqwest::redirect::Policy::limited(redirects));
        }

        let client = builder.build()
            .map_err(|e| LlmError::Network(e))?;

        Ok(Self { client, config })
    }

    /// Create a new HTTP client with default configuration
    pub fn with_defaults() -> Result<Self, LlmError> {
        Self::new(HttpClientConfig::default())
    }

    /// Get the underlying reqwest client
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Get the client configuration
    pub fn config(&self) -> &HttpClientConfig {
        &self.config
    }

    /// Get retry configuration
    pub fn retry_config(&self) -> super::retry::RetryConfig {
        super::retry::RetryConfig {
            max_attempts: self.config.retry_attempts,
            base_delay: self.config.retry_delay,
            max_delay: self.config.retry_delay * 10, // Cap max delay at 10x base
            exponential_backoff: true,
        }
    }
}