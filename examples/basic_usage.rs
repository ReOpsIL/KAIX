//! Basic usage examples for KAI-X
//! 
//! This file demonstrates how to use the core components of KAI-X
//! in a programmatic way.

use kai_x::{
    config::{Config, ConfigManager, ProviderConfig},
    context::{ContextConfig, ContextManager},
    execution::{ExecutionEngine, ExecutionConfig},
    llm::{LlmProviderFactory, Message, MessageRole},
    planning::{Plan, Task, TaskType},
    Result,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[tokio::main]
async fn main() -> Result<()> {
    // Example 1: Basic configuration setup
    configuration_example().await?;
    
    // Example 2: Creating and managing plans
    planning_example().await?;
    
    // Example 3: Context management
    context_example().await?;
    
    println!("All examples completed successfully!");
    Ok(())
}

/// Example of setting up and using configuration
async fn configuration_example() -> Result<()> {
    println!("=== Configuration Example ===");
    
    // Create a configuration manager
    let mut config_manager = ConfigManager::new()?;
    
    // Configure a provider
    let mut provider_config = ProviderConfig::default();
    provider_config.api_key = Some("your-api-key-here".to_string());
    provider_config.base_url = Some("https://openrouter.ai/api/v1".to_string());
    
    config_manager.set_provider_config("openrouter".to_string(), provider_config)?;
    config_manager.set_active_provider("openrouter".to_string())?;
    config_manager.set_active_model("anthropic/claude-3-haiku".to_string())?;
    
    // Set working directory
    config_manager.set_working_directory(std::env::current_dir()?)?;
    
    let config = config_manager.config();
    println!("Active provider: {}", config.active_provider);
    println!("Active model: {}", config.active_model);
    
    Ok(())
}

/// Example of creating and working with plans and tasks
async fn planning_example() -> Result<()> {
    println!("\n=== Planning Example ===");
    
    // Create a new plan
    let mut plan = Plan::new("Example development workflow");
    
    // Create tasks with dependencies
    let read_task = Task::new(
        "read_main", 
        "Read the main.rs file", 
        TaskType::ReadFile
    ).with_parameter("path", "src/main.rs");
    
    let analyze_task = Task::new(
        "analyze_main",
        "Analyze the main.rs file structure",
        TaskType::AnalyzeCode
    )
    .with_parameter("path", "src/main.rs")
    .with_dependency("read_main");
    
    let backup_task = Task::new(
        "backup_main",
        "Create a backup of main.rs",
        TaskType::ExecuteCommand
    )
    .with_parameter("command", "cp src/main.rs src/main.rs.bak")
    .with_dependency("analyze_main");
    
    // Add tasks to plan
    plan.add_task(read_task);
    plan.add_task(analyze_task);
    plan.add_task(backup_task);
    
    println!("Created plan: {}", plan.description);
    println!("Tasks:");
    for (i, task) in plan.tasks.iter().enumerate() {
        println!("  {}. {} ({})", i + 1, task.description, task.id);
        if !task.dependencies.is_empty() {
            println!("     Dependencies: {:?}", task.dependencies);
        }
    }
    
    // Get ready tasks (those with satisfied dependencies)
    let ready_tasks = plan.get_ready_tasks();
    println!("Ready to execute: {} tasks", ready_tasks.len());
    
    Ok(())
}

/// Example of context management
async fn context_example() -> Result<()> {
    println!("\n=== Context Example ===");
    
    // This example shows how context management would work
    // Note: In a real scenario, you'd need a valid LLM provider
    
    println!("Context management requires a configured LLM provider");
    println!("The context manager would:");
    println!("  1. Scan the working directory for files");
    println!("  2. Generate summaries using the LLM");
    println!("  3. Track file modifications");
    println!("  4. Provide context for plan generation");
    
    // Example configuration for context
    let context_config = ContextConfig {
        max_file_size: 1024 * 1024, // 1MB
        exclude_patterns: vec![
            "*.log".to_string(),
            "target/**".to_string(),
            ".git/**".to_string(),
        ],
        priority_extensions: vec![
            "rs".to_string(),
            "toml".to_string(),
            "md".to_string(),
        ],
        max_depth: Some(10),
        follow_symlinks: false,
    };
    
    println!("Context config:");
    println!("  Max file size: {} bytes", context_config.max_file_size);
    println!("  Priority extensions: {:?}", context_config.priority_extensions);
    println!("  Exclude patterns: {:?}", context_config.exclude_patterns);
    
    Ok(())
}

/// Example of how you might create and use an LLM provider
/// Note: This is a conceptual example - you'd need valid API keys
async fn llm_provider_example() -> Result<()> {
    println!("\n=== LLM Provider Example ===");
    
    // Create provider settings
    let mut settings = HashMap::new();
    settings.insert("api_key".to_string(), "your-api-key".to_string());
    
    // This would fail without a real API key, so we just show the structure
    println!("To create an LLM provider:");
    println!("  1. Set up API credentials");
    println!("  2. Choose a provider (openrouter, gemini, etc.)");
    println!("  3. Select an appropriate model");
    
    // Example of what the code would look like:
    /*
    let provider = LlmProviderFactory::create_provider("openrouter", settings)?;
    
    let models = provider.list_models().await?;
    println!("Available models: {}", models.len());
    
    let messages = vec![Message {
        role: MessageRole::User,
        content: "Hello, can you help me code?".to_string(),
        tool_calls: None,
        tool_call_id: None,
    }];
    
    let response = provider.generate(
        &messages,
        "anthropic/claude-3-haiku",
        None,
        None,
    ).await?;
    
    println!("Response: {:?}", response.content);
    */
    
    Ok(())
}

/// Example showing the full system integration
async fn full_system_example() -> Result<()> {
    println!("\n=== Full System Integration Example ===");
    
    // This shows how all components work together
    println!("In a complete KAI-X setup:");
    println!("  1. ConfigManager loads user preferences and API keys");
    println!("  2. ContextManager scans and summarizes the project");
    println!("  3. LLM Provider generates plans based on user prompts");
    println!("  4. ExecutionEngine executes tasks with the agentic loop");
    println!("  5. UI Manager provides interactive interface");
    
    // The integration would look like this:
    /*
    let config_manager = ConfigManager::new()?;
    let config = config_manager.config();
    
    // Create LLM provider
    let provider_config = config.get_active_provider_config()
        .ok_or_else(|| KaiError::not_found("No active provider"))?;
    
    let mut settings = HashMap::new();
    if let Some(api_key) = &provider_config.api_key {
        settings.insert("api_key".to_string(), api_key.clone());
    }
    
    let llm_provider = Arc::new(LlmProviderFactory::create_provider(
        &config.active_provider,
        settings,
    )?);
    
    // Create context manager
    let working_dir = config.working_directory.clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap());
    
    let context_manager = Arc::new(RwLock::new(ContextManager::new(
        working_dir,
        llm_provider.clone(),
        config.active_model.clone(),
        Some(config.context.clone().into()),
    )));
    
    // Create execution engine
    let execution_engine = Arc::new(RwLock::new(ExecutionEngine::new(
        context_manager.clone(),
        llm_provider.clone(),
        config.active_model.clone(),
        Some(config.execution.clone().into()),
    )));
    
    println!("All systems initialized and ready!");
    */
    
    Ok(())
}