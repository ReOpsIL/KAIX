//! Usage examples for the LLM integration layer
//! 
//! This module contains comprehensive examples demonstrating how to use
//! the LLM providers, prompt templates, and utility functions.

use super::*;
use crate::planning::{Plan, Task, TaskType};
use async_trait::async_trait;
use serde_json;
use std::collections::HashMap;

/// Mock LLM provider for testing and examples
pub struct MockLlmProvider {
    responses: std::sync::Mutex<std::collections::VecDeque<String>>,
}

impl MockLlmProvider {
    /// Create a new mock provider with default responses
    pub fn new() -> Self {
        let mut responses = std::collections::VecDeque::new();
        responses.push_back("This is a mock response from the LLM.".to_string());
        
        Self {
            responses: std::sync::Mutex::new(responses),
        }
    }
    
    /// Create a mock provider with custom responses
    pub fn with_responses(responses: Vec<String>) -> Self {
        let mut response_queue = std::collections::VecDeque::new();
        for response in responses {
            response_queue.push_back(response);
        }
        
        Self {
            responses: std::sync::Mutex::new(response_queue),
        }
    }
    
    /// Add a response to the queue
    pub fn add_response(&self, response: String) {
        self.responses.lock().unwrap().push_back(response);
    }
    
    /// Get the next response (for internal use)
    fn get_next_response(&self) -> String {
        let mut responses = self.responses.lock().unwrap();
        responses.pop_front().unwrap_or_else(|| {
            "Default mock response - no more responses queued.".to_string()
        })
    }
}

#[async_trait]
impl LlmProvider for MockLlmProvider {
    fn provider_name(&self) -> &str {
        "mock"
    }
    
    async fn list_models(&self) -> Result<Vec<ModelInfo>, LlmError> {
        Ok(vec![
            ModelInfo {
                id: "mock-model".to_string(),
                name: "Mock Model".to_string(),
                description: Some("A mock model for testing".to_string()),
                context_length: Some(4096),
                max_output_tokens: Some(1024),
                pricing: Some(ModelPricing {
                    prompt: Some(0.0),
                    completion: Some(0.0),
                }),
            }
        ])
    }
    
    async fn generate(
        &self,
        _messages: &[Message],
        _model: &str,
        _tools: Option<&[ToolDefinition]>,
        _config: Option<&GenerationConfig>,
    ) -> Result<LlmResponse, LlmError> {
        let content = self.get_next_response();
        
        Ok(LlmResponse {
            content: Some(content),
            tool_calls: None,
            finish_reason: "completed".to_string(),
            usage: Some(TokenUsage {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
            }),
        })
    }
    
    async fn generate_plan(
        &self,
        _prompt: &str,
        _context: &str,
        _model: &str,
    ) -> Result<crate::planning::Plan, LlmError> {
        let mut plan = Plan::new("Mock generated plan");
        
        let task = Task::new("mock_task_1", "Mock task description", TaskType::ReadFile)
            .with_parameter("path", "src/main.rs");
        
        plan.add_task(task);
        
        Ok(plan)
    }
    
    async fn generate_content(
        &self,
        _prompt: &str,
        _context: &str,
        _model: &str,
        _config: Option<&GenerationConfig>,
    ) -> Result<String, LlmError> {
        Ok(self.get_next_response())
    }
    
    async fn validate_model(&self, model: &str) -> Result<ModelInfo, LlmError> {
        if model == "mock-model" {
            Ok(ModelInfo {
                id: "mock-model".to_string(),
                name: "Mock Model".to_string(),
                description: Some("A mock model for testing".to_string()),
                context_length: Some(4096),
                max_output_tokens: Some(1024),
                pricing: Some(ModelPricing {
                    prompt: Some(0.0),
                    completion: Some(0.0),
                }),
            })
        } else {
            Err(LlmError::InvalidModel {
                model: model.to_string(),
            })
        }
    }
    
    async fn refine_task_for_execution(
        &self,
        task: &crate::planning::Task,
        _global_context: &str,
        _plan_context: &str,
        _dependency_outputs: &[crate::planning::TaskResult],
        _model: &str,
    ) -> Result<String, LlmError> {
        Ok(format!("Mock refined instruction for task: {}", task.description))
    }
    
    async fn analyze_task_result(
        &self,
        raw_result: &TaskExecutionResult,
        _task_objective: &str,
        _model: &str,
    ) -> Result<TaskAnalysis, LlmError> {
        Ok(TaskAnalysis {
            success_assessment: raw_result.exit_code.unwrap_or(0) == 0,
            extracted_information: HashMap::new(),
            error_diagnosis: raw_result.stderr.as_ref().filter(|s| !s.is_empty()).cloned(),
            follow_up_actions: vec![],
        })
    }
}

impl Default for MockLlmProvider {
    fn default() -> Self {
        Self::new()
    }
}

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
            .with_variable("request", "Add authentication middleware");
            
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