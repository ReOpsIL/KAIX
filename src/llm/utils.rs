//! Utility functions for LLM operations including token counting and cost estimation

use super::{LlmError, ModelInfo, TokenUsage};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Token counting utility for estimating token usage
pub struct TokenCounter;

impl TokenCounter {
    /// Rough estimation of tokens in text using simple heuristics
    /// This is an approximation - actual token counts vary by model and tokenizer
    pub fn estimate_tokens(text: &str) -> u32 {
        if text.is_empty() {
            return 0;
        }

        // Basic heuristic: ~4 characters per token for English text
        // This varies significantly by language and content type
        let char_count = text.chars().count() as f64;
        let word_count = text.split_whitespace().count() as f64;
        
        // Use a weighted average: prioritize words but consider character density
        let estimated_tokens = (word_count * 1.3) + (char_count * 0.25);
        
        // Add some overhead for special tokens, formatting, etc.
        (estimated_tokens * 1.1).ceil() as u32
    }

    /// More sophisticated token estimation using character patterns
    pub fn estimate_tokens_advanced(text: &str) -> u32 {
        if text.is_empty() {
            return 0;
        }

        let mut token_count = 0u32;
        let mut current_word = String::new();
        
        for ch in text.chars() {
            match ch {
                // Whitespace splits tokens
                ' ' | '\t' | '\n' | '\r' => {
                    if !current_word.is_empty() {
                        token_count += Self::estimate_word_tokens(&current_word);
                        current_word.clear();
                    }
                    // Some whitespace might be tokens themselves (especially in code)
                    if ch == '\n' {
                        token_count += 1;
                    }
                }
                // Punctuation often creates separate tokens
                '.' | ',' | '!' | '?' | ';' | ':' | '(' | ')' | '[' | ']' | '{' | '}' | '"' | '\'' => {
                    if !current_word.is_empty() {
                        token_count += Self::estimate_word_tokens(&current_word);
                        current_word.clear();
                    }
                    token_count += 1;
                }
                // Regular characters accumulate into words
                _ => {
                    current_word.push(ch);
                }
            }
        }
        
        // Handle the last word
        if !current_word.is_empty() {
            token_count += Self::estimate_word_tokens(&current_word);
        }

        token_count
    }

    /// Estimate tokens for a single word
    fn estimate_word_tokens(word: &str) -> u32 {
        let len = word.chars().count();
        
        // Very short words are usually 1 token
        if len <= 3 {
            1
        }
        // Medium words might be 1-2 tokens
        else if len <= 8 {
            if word.chars().all(|c| c.is_ascii_alphabetic()) {
                1 // Common English words
            } else {
                2 // Mixed characters, numbers, etc.
            }
        }
        // Long words often get split into multiple tokens
        else {
            ((len as f64) / 4.0).ceil() as u32
        }
    }

    /// Estimate tokens for structured data (JSON, code, etc.)
    pub fn estimate_structured_tokens(text: &str, multiplier: f64) -> u32 {
        let base_estimate = Self::estimate_tokens_advanced(text);
        (base_estimate as f64 * multiplier).ceil() as u32
    }

    /// Estimate tokens for code content
    pub fn estimate_code_tokens(code: &str) -> u32 {
        // Code typically has more tokens per character due to syntax
        Self::estimate_structured_tokens(code, 1.4)
    }

    /// Estimate tokens for JSON content
    pub fn estimate_json_tokens(json: &str) -> u32 {
        // JSON has overhead from structure but is often more compressed
        Self::estimate_structured_tokens(json, 1.2)
    }
}

/// Cost estimation utility for calculating API costs
#[derive(Debug, Clone)]
pub struct CostEstimator {
    model_pricing: HashMap<String, ModelPricing>,
}

/// Detailed pricing information for a model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPricing {
    pub prompt_cost_per_million: Option<f64>,
    pub completion_cost_per_million: Option<f64>,
    pub currency: String,
}

/// Cost breakdown for a request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostBreakdown {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    pub prompt_cost: Option<f64>,
    pub completion_cost: Option<f64>,
    pub total_cost: Option<f64>,
    pub currency: Option<String>,
}

impl CostEstimator {
    /// Create a new cost estimator with default pricing data
    pub fn new() -> Self {
        let mut model_pricing = HashMap::new();
        
        // OpenRouter pricing (approximate, as of 2024)
        model_pricing.insert("openai/gpt-3.5-turbo".to_string(), ModelPricing {
            prompt_cost_per_million: Some(0.5),
            completion_cost_per_million: Some(1.5),
            currency: "USD".to_string(),
        });
        
        model_pricing.insert("openai/gpt-4".to_string(), ModelPricing {
            prompt_cost_per_million: Some(30.0),
            completion_cost_per_million: Some(60.0),
            currency: "USD".to_string(),
        });

        model_pricing.insert("openai/gpt-4-turbo".to_string(), ModelPricing {
            prompt_cost_per_million: Some(10.0),
            completion_cost_per_million: Some(30.0),
            currency: "USD".to_string(),
        });

        model_pricing.insert("anthropic/claude-3-sonnet".to_string(), ModelPricing {
            prompt_cost_per_million: Some(3.0),
            completion_cost_per_million: Some(15.0),
            currency: "USD".to_string(),
        });

        model_pricing.insert("anthropic/claude-3-opus".to_string(), ModelPricing {
            prompt_cost_per_million: Some(15.0),
            completion_cost_per_million: Some(75.0),
            currency: "USD".to_string(),
        });

        // Google Gemini pricing (approximate)
        model_pricing.insert("gemini-pro".to_string(), ModelPricing {
            prompt_cost_per_million: Some(0.5),
            completion_cost_per_million: Some(1.5),
            currency: "USD".to_string(),
        });

        model_pricing.insert("gemini-pro-1.5".to_string(), ModelPricing {
            prompt_cost_per_million: Some(3.5),
            completion_cost_per_million: Some(10.5),
            currency: "USD".to_string(),
        });

        Self { model_pricing }
    }

    /// Add or update pricing for a model
    pub fn set_model_pricing(&mut self, model_id: &str, pricing: ModelPricing) {
        self.model_pricing.insert(model_id.to_string(), pricing);
    }

    /// Get pricing for a model
    pub fn get_model_pricing(&self, model_id: &str) -> Option<&ModelPricing> {
        self.model_pricing.get(model_id)
    }

    /// Calculate cost for a given token usage
    pub fn calculate_cost(&self, model_id: &str, usage: &TokenUsage) -> CostBreakdown {
        let pricing = self.model_pricing.get(model_id);
        
        let (prompt_cost, completion_cost, total_cost, currency) = if let Some(pricing) = pricing {
            let prompt_cost = pricing.prompt_cost_per_million
                .map(|rate| (usage.prompt_tokens as f64 / 1_000_000.0) * rate);
            
            let completion_cost = pricing.completion_cost_per_million
                .map(|rate| (usage.completion_tokens as f64 / 1_000_000.0) * rate);
            
            let total_cost = match (prompt_cost, completion_cost) {
                (Some(p), Some(c)) => Some(p + c),
                (Some(p), None) => Some(p),
                (None, Some(c)) => Some(c),
                (None, None) => None,
            };
            
            (prompt_cost, completion_cost, total_cost, Some(pricing.currency.clone()))
        } else {
            (None, None, None, None)
        };
        
        CostBreakdown {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
            prompt_cost,
            completion_cost,
            total_cost,
            currency,
        }
    }

    /// Estimate cost for a prompt before sending to API
    pub fn estimate_cost(&self, model_id: &str, prompt_text: &str, estimated_completion_tokens: Option<u32>) -> CostBreakdown {
        let prompt_tokens = TokenCounter::estimate_tokens(prompt_text);
        let completion_tokens = estimated_completion_tokens.unwrap_or(150); // Default estimate
        let total_tokens = prompt_tokens + completion_tokens;
        
        let usage = TokenUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens,
        };
        
        self.calculate_cost(model_id, &usage)
    }

    /// Get all available models with pricing
    pub fn list_models_with_pricing(&self) -> Vec<(String, &ModelPricing)> {
        self.model_pricing
            .iter()
            .map(|(model, pricing)| (model.clone(), pricing))
            .collect()
    }
}

/// Usage tracking utility for monitoring API usage over time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageTracker {
    pub total_requests: u64,
    pub total_prompt_tokens: u64,
    pub total_completion_tokens: u64,
    pub total_cost: f64,
    pub model_usage: HashMap<String, ModelUsage>,
    pub daily_usage: HashMap<String, DailyUsage>, // Date -> usage
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelUsage {
    pub requests: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_cost: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyUsage {
    pub requests: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_cost: f64,
}

impl UsageTracker {
    pub fn new() -> Self {
        Self {
            total_requests: 0,
            total_prompt_tokens: 0,
            total_completion_tokens: 0,
            total_cost: 0.0,
            model_usage: HashMap::new(),
            daily_usage: HashMap::new(),
        }
    }

    /// Record a new API request
    pub fn record_usage(&mut self, model_id: &str, cost_breakdown: &CostBreakdown) {
        let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
        
        // Update totals
        self.total_requests += 1;
        self.total_prompt_tokens += cost_breakdown.prompt_tokens as u64;
        self.total_completion_tokens += cost_breakdown.completion_tokens as u64;
        
        if let Some(cost) = cost_breakdown.total_cost {
            self.total_cost += cost;
        }

        // Update model-specific usage
        let model_usage = self.model_usage.entry(model_id.to_string()).or_insert(ModelUsage {
            requests: 0,
            prompt_tokens: 0,
            completion_tokens: 0,
            total_cost: 0.0,
        });
        
        model_usage.requests += 1;
        model_usage.prompt_tokens += cost_breakdown.prompt_tokens as u64;
        model_usage.completion_tokens += cost_breakdown.completion_tokens as u64;
        
        if let Some(cost) = cost_breakdown.total_cost {
            model_usage.total_cost += cost;
        }

        // Update daily usage
        let daily_usage = self.daily_usage.entry(today).or_insert(DailyUsage {
            requests: 0,
            prompt_tokens: 0,
            completion_tokens: 0,
            total_cost: 0.0,
        });
        
        daily_usage.requests += 1;
        daily_usage.prompt_tokens += cost_breakdown.prompt_tokens as u64;
        daily_usage.completion_tokens += cost_breakdown.completion_tokens as u64;
        
        if let Some(cost) = cost_breakdown.total_cost {
            daily_usage.total_cost += cost;
        }
    }

    /// Get usage for a specific model
    pub fn get_model_usage(&self, model_id: &str) -> Option<&ModelUsage> {
        self.model_usage.get(model_id)
    }

    /// Get usage for a specific date
    pub fn get_daily_usage(&self, date: &str) -> Option<&DailyUsage> {
        self.daily_usage.get(date)
    }

    /// Get usage for the current month
    pub fn get_monthly_usage(&self) -> DailyUsage {
        let current_month = chrono::Utc::now().format("%Y-%m").to_string();
        
        self.daily_usage
            .iter()
            .filter(|(date, _)| date.starts_with(&current_month))
            .fold(DailyUsage {
                requests: 0,
                prompt_tokens: 0,
                completion_tokens: 0,
                total_cost: 0.0,
            }, |mut acc, (_, usage)| {
                acc.requests += usage.requests;
                acc.prompt_tokens += usage.prompt_tokens;
                acc.completion_tokens += usage.completion_tokens;
                acc.total_cost += usage.total_cost;
                acc
            })
    }
}

impl Default for CostEstimator {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for UsageTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_estimation() {
        assert_eq!(TokenCounter::estimate_tokens(""), 0);
        assert!(TokenCounter::estimate_tokens("hello world") > 0);
        assert!(TokenCounter::estimate_tokens("hello world") < 10);
        
        let long_text = "This is a much longer text that should result in more tokens being estimated by the token counter utility.";
        assert!(TokenCounter::estimate_tokens(long_text) > TokenCounter::estimate_tokens("short"));
    }

    #[test]
    fn test_advanced_token_estimation() {
        let simple_text = "hello world";
        let code_text = "fn main() { println!(\"Hello, world!\"); }";
        
        assert!(TokenCounter::estimate_tokens_advanced(code_text) > TokenCounter::estimate_tokens_advanced(simple_text));
        assert!(TokenCounter::estimate_code_tokens(code_text) > TokenCounter::estimate_tokens(code_text));
    }

    #[test]
    fn test_cost_estimation() {
        let estimator = CostEstimator::new();
        let usage = TokenUsage {
            prompt_tokens: 1000,
            completion_tokens: 500,
            total_tokens: 1500,
        };
        
        let cost = estimator.calculate_cost("openai/gpt-3.5-turbo", &usage);
        assert!(cost.total_cost.is_some());
        assert!(cost.total_cost.unwrap() > 0.0);
    }

    #[test]
    fn test_usage_tracking() {
        let mut tracker = UsageTracker::new();
        let cost_breakdown = CostBreakdown {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
            prompt_cost: Some(0.05),
            completion_cost: Some(0.075),
            total_cost: Some(0.125),
            currency: Some("USD".to_string()),
        };
        
        tracker.record_usage("test-model", &cost_breakdown);
        
        assert_eq!(tracker.total_requests, 1);
        assert_eq!(tracker.total_prompt_tokens, 100);
        assert_eq!(tracker.total_completion_tokens, 50);
        assert!((tracker.total_cost - 0.125).abs() < 0.001);
        
        let model_usage = tracker.get_model_usage("test-model").unwrap();
        assert_eq!(model_usage.requests, 1);
    }
}