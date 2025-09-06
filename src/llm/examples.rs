//! Usage examples for the LLM integration layer
//! 
//! This module contains comprehensive examples demonstrating how to use
//! the LLM providers, prompt templates, and utility functions.

use super::*;
use crate::planning::{Plan, Task, TaskType};
use async_trait::async_trait;
use serde_json;
use std::collections::HashMap;


#[cfg(test)]
mod examples {
    use super::super::*;
    use crate::llm::*;
    
    /// Example: Creating and using an OpenRouter provider
    #[tokio::test]
    #[ignore] // Requires API key
    async fn example_openrouter_basic_usage() -> Result<(), Box<dyn std::error::Error>> {
        // Create provider with API key
        let provider = openrouter::OpenRouterProvider::new("your-api-key".to_string());
        
        // List available models
        let models = provider.list_models().await?;
        println!("Available models: {}", models.len());
        
        // Create a simple message
        let messages = vec![Message {
            role: MessageRole::User,
            content: "Hello, how are you?".to_string(),
            tool_calls: None,
            tool_call_id: None,
        }];
        
        // Generate response (commented out to avoid actual API call)
        // let response = provider.generate(&messages, "gpt-3.5-turbo", None, None).await?;
        // println!("Response: {:?}", response.content);
        
        Ok(())
    }
    
    /// Example: Using prompt templates for structured interactions
    #[test]
    fn example_prompt_templates() -> Result<(), Box<dyn std::error::Error>> {
        // Create a context for plan generation
        let context = PromptContext::new()
            .with_variable("context", "Working on a Rust web application")
            .with_variable("request", "Add authentication middleware")
            .with_variable("working_directory", "/tmp/test-project")
            .with_variable("project_type", "Rust web application")
            .with_variable("current_state", "Basic web server with routes implemented");
            
        // Get the plan generation template
        let template = PromptTemplates::plan_generation();
        
        // Fill the template
        let (system_message, user_message) = template.fill(&context)?;
        
        // Verify the messages were created
        assert!(system_message.contains("task planning AI"));
        assert!(user_message.contains("authentication middleware"));
        
        println!("Template filled successfully!");
        Ok(())
    }
    
    /// Example: Token counting and cost estimation
    #[test]
    fn example_token_counting_and_costs() -> Result<(), Box<dyn std::error::Error>> {
        let text = "This is a sample text that we want to count tokens for.";
        
        // Basic token estimation
        let token_count = TokenCounter::estimate_tokens(text);
        println!("Estimated tokens: {}", token_count);
        
        // Advanced token estimation
        let advanced_count = TokenCounter::estimate_tokens_advanced(text);
        println!("Advanced token count: {}", advanced_count);
        
        // Code-specific estimation
        let code = r#"
            fn main() {
                println!("Hello, world!");
            }
        "#;
        let code_tokens = TokenCounter::estimate_code_tokens(code);
        println!("Code tokens: {}", code_tokens);
        
        // Cost estimation
        let cost_estimator = CostEstimator::new();
        let cost_breakdown = cost_estimator.estimate_cost(
            "openai/gpt-3.5-turbo",
            text,
            Some(50), // Estimated completion tokens
        );
        
        println!("Estimated cost: {:?}", cost_breakdown.total_cost);
        
        Ok(())
    }
    
    /// Example: Usage tracking
    #[test]
    fn example_usage_tracking() -> Result<(), Box<dyn std::error::Error>> {
        let mut tracker = UsageTracker::new();
        
        // Simulate recording some API usage
        let cost_breakdown = CostBreakdown {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
            prompt_cost: Some(0.05),
            completion_cost: Some(0.075),
            total_cost: Some(0.125),
            currency: Some("USD".to_string()),
        };
        
        tracker.record_usage("gpt-3.5-turbo", &cost_breakdown);
        
        // Check totals
        assert_eq!(tracker.total_requests, 1);
        assert_eq!(tracker.total_prompt_tokens, 100);
        
        println!("Usage tracking example completed!");
        Ok(())
    }
    
    /// Example: Working with different prompt templates
    #[test]
    fn example_all_prompt_templates() -> Result<(), Box<dyn std::error::Error>> {
        let templates = PromptTemplates::list_templates();
        println!("Available templates: {:?}", templates);
        
        for template_name in templates {
            if let Some(template) = PromptTemplates::get_template(template_name) {
                println!("Template '{}' has {} variables", template_name, template.variables.len());
                
                // Create minimal context for testing
                let mut context = PromptContext::new();
                for variable in &template.variables {
                    context.set_variable(variable, format!("sample_{}", variable));
                }
                
                // Test template filling
                match template.fill(&context) {
                    Ok((system, user)) => {
                        println!("✓ Template '{}' filled successfully", template_name);
                        assert!(!system.is_empty());
                        assert!(!user.is_empty());
                    }
                    Err(e) => {
                        println!("✗ Template '{}' failed: {}", template_name, e);
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Example: Provider factory usage
    #[test]
    fn example_provider_factory() -> Result<(), Box<dyn std::error::Error>> {
        let providers = LlmProviderFactory::list_providers();
        println!("Available providers: {:?}", providers);
        
        // Test provider creation (would need real API keys to work)
        let mut config = std::collections::HashMap::new();
        config.insert("api_key".to_string(), "test-key".to_string());
        
        for provider_name in providers {
            match LlmProviderFactory::create_provider(provider_name, config.clone()) {
                Ok(provider) => {
                    println!("✓ Created provider: {}", provider.provider_name());
                }
                Err(e) => {
                    println!("Provider creation for {} would fail without real API key: {}", provider_name, e);
                }
            }
        }
        
        Ok(())
    }
}

/// Example workflows showing common patterns
pub mod workflows {
    use super::*;
    
    /// Workflow: Generate a plan using templates and provider
    pub async fn plan_generation_workflow(
        provider: &dyn LlmProvider,
        user_request: &str,
        project_context: &str,
        model: &str,
    ) -> Result<crate::planning::Plan, LlmError> {
        // Use the generate_plan method which internally uses templates
        provider.generate_plan(user_request, project_context, model).await
    }
    
    /// Workflow: Content generation with cost tracking
    pub async fn tracked_content_generation(
        provider: &dyn LlmProvider,
        prompt: &str,
        context: &str,
        model: &str,
        tracker: &mut UsageTracker,
        estimator: &CostEstimator,
    ) -> Result<String, LlmError> {
        // Estimate cost before making the call
        let estimated_cost = estimator.estimate_cost(model, &format!("{}\n{}", context, prompt), Some(200));
        println!("Estimated cost: ${:.4}", estimated_cost.total_cost.unwrap_or(0.0));
        
        // Generate content
        let response = provider.generate_content(prompt, context, model, None).await?;
        
        // If we had token usage from the response, we would record it:
        // tracker.record_usage(model, &actual_cost_breakdown);
        
        Ok(response)
    }
    
    /// Workflow: Multi-step plan execution with refinement
    pub async fn plan_execution_workflow(
        provider: &dyn LlmProvider,
        plan: &crate::planning::Plan,
        model: &str,
    ) -> Result<Vec<String>, LlmError> {
        let mut results = Vec::new();
        
        for task in &plan.tasks {
            // Use task refinement template to get concrete instructions
            let template = PromptTemplates::task_refinement();
            let context = PromptContext::new()
                .with_variable("plan_description", &plan.description)
                .with_variable("task_id", &task.id)
                .with_variable("task_type", &format!("{:?}", task.task_type))
                .with_variable("task_description", &task.description)
                .with_variable("task_parameters", &serde_json::to_string(&task.parameters).unwrap_or_default())
                .with_variable("global_context", "Sample global context")
                .with_variable("plan_context", "Sample plan context");
            
            let (system_message, user_message) = template.fill(&context)
                .map_err(|e| LlmError::InvalidResponse {
                    message: format!("Failed to fill template: {}", e),
                })?;
            
            let messages = vec![
                Message {
                    role: MessageRole::System,
                    content: system_message,
                    tool_calls: None,
                    tool_call_id: None,
                },
                Message {
                    role: MessageRole::User,
                    content: user_message,
                    tool_calls: None,
                    tool_call_id: None,
                },
            ];
            
            let response = provider.generate(&messages, model, None, None).await?;
            if let Some(content) = response.content {
                results.push(content);
            }
        }
        
        Ok(results)
    }
}