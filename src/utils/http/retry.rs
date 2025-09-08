//! Retry logic for HTTP operations

use crate::llm::LlmError;
use std::time::Duration;

/// Configuration for retry behavior
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_attempts: usize,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub exponential_backoff: bool,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(1000),
            max_delay: Duration::from_secs(240),
            exponential_backoff: true,
        }
    }
}

/// Trait for operations that can be retried
pub trait RetryableOperation<T> {
    async fn execute(&self) -> Result<T, LlmError>;
    
    fn is_retryable(&self, error: &LlmError) -> bool {
        match error {
            LlmError::RateLimit { .. } => true,
            LlmError::Network(_) => true,
            LlmError::RequestFailed { status, .. } => *status >= 500,
            _ => false,
        }
    }
    
    fn should_retry(&self, error: &LlmError, attempt: usize, max_attempts: usize) -> bool {
        attempt < max_attempts && self.is_retryable(error)
    }
}

/// Execute an operation with retry logic
pub async fn execute_with_retry<F, Fut, T>(
    operation: F,
    config: &RetryConfig,
) -> Result<T, LlmError>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, LlmError>> + Send,
{
    let mut last_error = None;
    
    for attempt in 0..=config.max_attempts {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) => {
                last_error = Some(e);
                
                if attempt < config.max_attempts {
                    if let Some(ref error) = last_error {
                        if is_retryable_error(error) {
                            let delay = calculate_delay(error, attempt, config);
                            tokio::time::sleep(delay).await;
                        } else {
                            // Non-retryable error, break immediately
                            break;
                        }
                    }
                }
            }
        }
    }
    
    Err(last_error.unwrap_or_else(|| LlmError::Unknown {
        message: "Retry operation failed with no error".to_string(),
    }))
}

/// Check if an error is retryable
fn is_retryable_error(error: &LlmError) -> bool {
    match error {
        LlmError::RateLimit { .. } => true,
        LlmError::Network(_) => true,
        LlmError::RequestFailed { status, .. } => *status >= 500,
        _ => false,
    }
}

/// Calculate delay for retry attempt
fn calculate_delay(error: &LlmError, attempt: usize, config: &RetryConfig) -> Duration {
    match error {
        LlmError::RateLimit { retry_after } => {
            // Use retry_after if provided, otherwise use exponential backoff
            if let Some(retry_after) = retry_after {
                Duration::from_secs(*retry_after)
            } else {
                calculate_exponential_delay(attempt, config)
            }
        }
        _ => {
            if config.exponential_backoff {
                calculate_exponential_delay(attempt, config)
            } else {
                config.base_delay
            }
        }
    }
}

/// Calculate exponential backoff delay
fn calculate_exponential_delay(attempt: usize, config: &RetryConfig) -> Duration {
    let exponential_delay = config.base_delay * (2_u32.pow(attempt as u32));
    std::cmp::min(exponential_delay, config.max_delay)
}

/// Retry-enabled wrapper for async operations
pub struct RetryWrapper<F> {
    operation: F,
    config: RetryConfig,
}

impl<F> RetryWrapper<F> {
    pub fn new(operation: F, config: RetryConfig) -> Self {
        Self { operation, config }
    }
    
    pub fn with_defaults(operation: F) -> Self {
        Self::new(operation, RetryConfig::default())
    }
}

impl<F, Fut, T> RetryWrapper<F>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T, LlmError>> + Send,
{
    pub async fn execute(self) -> Result<T, LlmError> {
        execute_with_retry(self.operation, &self.config).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[tokio::test]
    async fn test_retry_success_after_failure() {
        let attempt_count = Arc::new(AtomicUsize::new(0));
        let attempt_count_clone = attempt_count.clone();
        
        let operation = move || {
            let count = attempt_count_clone.clone();
            async move {
                let current = count.fetch_add(1, Ordering::SeqCst);
                if current < 2 {
                    Err(LlmError::RequestFailed {
                        status: 500,
                        message: "test network error".to_string(),
                    })
                } else {
                    Ok("success".to_string())
                }
            }
        };
        
        let config = RetryConfig {
            max_attempts: 3,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(240),
            exponential_backoff: false,
        };
        
        let result = execute_with_retry(operation, &config).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");
        assert_eq!(attempt_count.load(Ordering::SeqCst), 3);
    }
    
    #[tokio::test]
    async fn test_retry_gives_up_after_max_attempts() {
        let attempt_count = Arc::new(AtomicUsize::new(0));
        let attempt_count_clone = attempt_count.clone();
        
        let operation = move || {
            let count = attempt_count_clone.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Err(LlmError::RequestFailed {
                    status: 503,
                    message: "test network error".to_string(),
                })
            }
        };
        
        let config = RetryConfig {
            max_attempts: 2,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(240),
            exponential_backoff: false,
        };
        
        let result: Result<String, LlmError> = execute_with_retry(operation, &config).await;
        assert!(result.is_err());
        assert_eq!(attempt_count.load(Ordering::SeqCst), 3); // 2 max attempts + initial attempt
    }
    
    #[tokio::test]
    async fn test_non_retryable_error_stops_immediately() {
        let attempt_count = Arc::new(AtomicUsize::new(0));
        let attempt_count_clone = attempt_count.clone();
        
        let operation = move || {
            let count = attempt_count_clone.clone();
            async move {
                count.fetch_add(1, Ordering::SeqCst);
                Err(LlmError::Authentication {
                    message: "invalid key".to_string(),
                })
            }
        };
        
        let config = RetryConfig {
            max_attempts: 3,
            base_delay: Duration::from_millis(1),
            max_delay: Duration::from_millis(240),
            exponential_backoff: false,
        };
        
        let result: Result<String, LlmError> = execute_with_retry(operation, &config).await;
        assert!(result.is_err());
        assert_eq!(attempt_count.load(Ordering::SeqCst), 1); // Should stop after first attempt
    }
}