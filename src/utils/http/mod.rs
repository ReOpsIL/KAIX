//! Shared HTTP utilities for LLM providers
//! 
//! This module provides common HTTP client functionality used across different LLM providers,
//! eliminating code duplication in client setup, retry logic, and error handling.

use crate::llm::LlmError;
use reqwest::Client;
use std::time::Duration;

pub mod client;
pub mod retry;
pub mod headers;

pub use client::{HttpClient, HttpClientConfig, HttpClientBuilder};
pub use retry::{RetryConfig, RetryableOperation, execute_with_retry};
pub use headers::{HeaderBuilder, CommonHeaders};

/// Default timeout for HTTP requests (2 minutes)
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(120);

/// Default number of retry attempts
pub const DEFAULT_RETRY_ATTEMPTS: usize = 3;

/// Default delay between retries
pub const DEFAULT_RETRY_DELAY: Duration = Duration::from_millis(1000);

/// Common HTTP client configuration used across providers
#[derive(Debug, Clone)]
pub struct SharedHttpConfig {
    pub timeout: Duration,
    pub retry_attempts: usize,
    pub retry_delay: Duration,
    pub user_agent: Option<String>,
}

impl Default for SharedHttpConfig {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            retry_attempts: DEFAULT_RETRY_ATTEMPTS,
            retry_delay: DEFAULT_RETRY_DELAY,
            user_agent: Some("KAI-X/1.0".to_string()),
        }
    }
}

/// Create a configured reqwest client with standard settings
pub fn create_client(config: &SharedHttpConfig) -> Result<Client, LlmError> {
    let mut builder = Client::builder()
        .timeout(config.timeout);

    if let Some(ref user_agent) = config.user_agent {
        builder = builder.user_agent(user_agent);
    }

    builder.build()
        .map_err(|e| LlmError::Network(e))
}

/// Parse standard HTTP error responses
pub fn parse_http_error(status: u16, body: &str, model_name: Option<&str>) -> LlmError {
    match status {
        429 => {
            // Try to extract retry-after from the response
            let retry_after = extract_retry_after(body);
            LlmError::RateLimit { retry_after }
        }
        401 | 403 => LlmError::Authentication {
            message: "Invalid API key or insufficient permissions".to_string(),
        },
        400 => {
            if body.contains("model") && (body.contains("not found") || body.contains("invalid") || body.contains("does not exist")) {
                LlmError::InvalidModel {
                    model: model_name.unwrap_or("unknown").to_string(),
                }
            } else {
                LlmError::RequestFailed {
                    status,
                    message: body.to_string(),
                }
            }
        }
        _ => LlmError::RequestFailed {
            status,
            message: body.to_string(),
        },
    }
}

/// Extract retry-after value from error response
fn extract_retry_after(body: &str) -> Option<u64> {
    // Try to parse retry_after from JSON response
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
        json.get("retry_after")
            .and_then(|v| v.as_u64())
            .or_else(|| {
                json.get("error")
                    .and_then(|e| e.get("retry_after"))
                    .and_then(|v| v.as_u64())
            })
    } else {
        None
    }
}