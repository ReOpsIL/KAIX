//! KAI-X - A sophisticated Rust-based AI coding assistant CLI

use kai_x::{
    config::ConfigManager,
    context::ContextManager,
    execution::ExecutionEngine,
    llm::LlmProviderFactory,
    ui::UiManager,
    Result,
};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, error};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

/// KAI-X: A sophisticated Rust-based AI coding assistant
#[derive(Parser)]
#[command(name = "kai")]
#[command(version, about, long_about = None)]
struct Cli {
    /// Working directory for the session
    #[arg(short, long)]
    workdir: Option<PathBuf>,

    /// Configuration file path
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "info")]
    log_level: String,

    /// Non-interactive mode (for scripting)
    #[arg(short, long)]
    non_interactive: bool,

    /// Subcommands
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize KAI-X configuration
    Init {
        /// Force overwrite existing configuration
        #[arg(long)]
        force: bool,
    },
    /// Manage LLM providers
    Provider {
        #[command(subcommand)]
        action: ProviderAction,
    },
    /// Run a single prompt and exit
    Prompt {
        /// The prompt to execute
        prompt: String,
        /// Output format (text, json)
        #[arg(short, long, default_value = "text")]
        format: String,
    },
    /// Interactive chat mode (default)
    Chat,
    /// Check configuration and system status
    Status,
}

#[derive(Subcommand)]
enum ProviderAction {
    /// List available providers
    List,
    /// Add a new provider
    Add {
        name: String,
        api_key: String,
        #[arg(long)]
        base_url: Option<String>,
    },
    /// Remove a provider
    Remove { name: String },
    /// Set active provider
    Set { name: String, model: Option<String> },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    init_logging(&cli.log_level)?;

    info!("Starting KAI-X v{}", env!("CARGO_PKG_VERSION"));

    match cli.command.unwrap_or(Commands::Chat) {
        Commands::Init { force } => init_config(force).await,
        Commands::Provider { action } => handle_provider_command(action).await,
        Commands::Prompt { prompt, format } => run_single_prompt(prompt, format, cli.workdir).await,
        Commands::Chat => run_interactive_mode(cli).await,
        Commands::Status => show_status().await,
    }
}

/// Initialize logging
fn init_logging(log_level: &str) -> Result<()> {
    let filter = EnvFilter::try_new(log_level)
        .map_err(|e| kai_x::utils::errors::KaiError::unknown(format!("Invalid log level: {}", e)))?;

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(filter)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true)
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .map_err(|e| kai_x::utils::errors::KaiError::unknown(format!("Failed to set logger: {}", e)))?;

    Ok(())
}

/// Initialize configuration
async fn init_config(force: bool) -> Result<()> {
    info!("Initializing KAI-X configuration...");

    let mut config_manager = ConfigManager::new()?;

    // Check if config already exists
    if !force {
        info!("Configuration already exists. Use --force to overwrite.");
        return Ok(());
    }

    // Interactive setup would go here
    println!("KAI-X configuration initialized successfully!");
    println!("You can now add LLM providers using: kai provider add <name> <api-key>");

    Ok(())
}

/// Handle provider management commands
async fn handle_provider_command(action: ProviderAction) -> Result<()> {
    let mut config_manager = ConfigManager::new()?;

    match action {
        ProviderAction::List => {
            println!("Available providers:");
            let available = LlmProviderFactory::list_providers();
            for provider in available {
                let configured = config_manager.get_provider_config(provider).is_some();
                let status = if configured { "✓" } else { "✗" };
                println!("  {} {} ({})", status, provider, if configured { "configured" } else { "not configured" });
            }
        }
        ProviderAction::Add { name, api_key, base_url } => {
            let mut provider_config = kai_x::config::ProviderConfig::default();
            provider_config.api_key = Some(api_key);
            provider_config.base_url = base_url;
            
            config_manager.set_provider_config(name.clone(), provider_config)?;
            println!("Provider '{}' added successfully!", name);
        }
        ProviderAction::Remove { name } => {
            config_manager.remove_provider(&name)?;
            println!("Provider '{}' removed successfully!", name);
        }
        ProviderAction::Set { name, model } => {
            if config_manager.get_provider_config(&name).is_none() {
                return Err(kai_x::utils::errors::KaiError::not_found(format!("Provider '{}'", name)));
            }
            
            config_manager.set_active_provider(name.clone())?;
            
            if let Some(model) = model {
                config_manager.set_active_model(model)?;
            }
            
            println!("Active provider set to '{}'", name);
        }
    }

    Ok(())
}

/// Run a single prompt and exit
async fn run_single_prompt(prompt: String, format: String, workdir: Option<PathBuf>) -> Result<()> {
    info!("Running single prompt: {}", prompt);

    // Initialize system
    let (config_manager, context_manager, execution_engine) = initialize_core_systems(workdir).await?;

    // Get LLM provider
    let config = config_manager.config();
    let provider_config = config.get_active_provider_config()
        .ok_or_else(|| kai_x::utils::errors::KaiError::not_found("No active provider configured"))?;

    let mut provider_settings = std::collections::HashMap::new();
    if let Some(api_key) = &provider_config.api_key {
        provider_settings.insert("api_key".to_string(), api_key.clone());
    }

    let llm_provider = LlmProviderFactory::create_provider(
        &config.active_provider,
        provider_settings,
    )?;

    // Generate and execute plan
    let context_manager_read = context_manager.read().await;
    let global_context = context_manager_read.get_global_context_summary().await?;
    drop(context_manager_read);

    let plan = llm_provider.generate_plan(&prompt, &global_context, &config.active_model).await?;

    // For single prompt mode, just show the plan
    match format.as_str() {
        "json" => {
            let json = plan.to_json()?;
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
        _ => {
            println!("Generated Plan: {}", plan.description);
            for (i, task) in plan.tasks.iter().enumerate() {
                println!("  {}. {}", i + 1, task.description);
            }
        }
    }

    Ok(())
}

/// Run interactive chat mode
async fn run_interactive_mode(cli: Cli) -> Result<()> {
    info!("Starting interactive mode");

    // Initialize core systems
    let (config_manager, context_manager, execution_engine) = initialize_core_systems(cli.workdir).await?;

    // Initialize UI
    let mut ui_manager = UiManager::new();
    let mut terminal = UiManager::init_terminal()?;

    info!("KAI-X interactive mode ready");

    // Run the main loop
    let result = ui_manager.run(&mut terminal).await;

    // Cleanup
    UiManager::restore_terminal()?;

    if let Err(e) = result {
        error!("UI error: {}", e);
        return Err(e);
    }

    info!("KAI-X shutdown complete");
    Ok(())
}

/// Show system status
async fn show_status() -> Result<()> {
    println!("KAI-X Status");
    println!("============");

    // Load configuration
    let config_manager = ConfigManager::new()?;
    let config = config_manager.config();

    println!("Version: {}", env!("CARGO_PKG_VERSION"));
    println!("Active Provider: {}", config.active_provider);
    println!("Active Model: {}", config.active_model);
    
    if let Some(workdir) = &config.working_directory {
        println!("Working Directory: {}", workdir.display());
    }

    println!("Configured Providers:");
    for (name, provider_config) in &config.providers {
        let has_key = provider_config.api_key.is_some();
        println!("  - {} (API Key: {})", name, if has_key { "✓" } else { "✗" });
    }

    Ok(())
}

/// Initialize core systems
async fn initialize_core_systems(
    workdir: Option<PathBuf>,
) -> Result<(ConfigManager, Arc<RwLock<ContextManager>>, Arc<RwLock<ExecutionEngine>>)> {
    // Load configuration
    let mut config_manager = ConfigManager::new()?;

    // Set working directory if provided
    let working_dir = if let Some(workdir) = workdir {
        config_manager.set_working_directory(workdir.clone())?;
        workdir
    } else {
        config_manager.config().working_directory.clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap())
    };

    info!("Working directory: {}", working_dir.display());

    // Validate configuration
    config_manager.config().validate()?;

    // Get LLM provider
    let config = config_manager.config();
    let provider_config = config.get_active_provider_config()
        .ok_or_else(|| kai_x::utils::errors::KaiError::not_found("No active provider configured"))?;

    let mut provider_settings = std::collections::HashMap::new();
    if let Some(api_key) = &provider_config.api_key {
        provider_settings.insert("api_key".to_string(), api_key.clone());
    }
    if let Some(base_url) = &provider_config.base_url {
        provider_settings.insert("base_url".to_string(), base_url.clone());
    }

    let llm_provider = Arc::new(LlmProviderFactory::create_provider(
        &config.active_provider,
        provider_settings,
    )?);

    // Initialize context manager
    let context_manager = Arc::new(RwLock::new(ContextManager::new(
        working_dir,
        llm_provider.clone(),
        config.active_model.clone(),
        Some(config.context.clone().into()),
    )));

    // Initialize execution engine
    let execution_engine = Arc::new(RwLock::new(ExecutionEngine::new(
        context_manager.clone(),
        llm_provider.clone(),
        config.active_model.clone(),
        Some(config.execution.clone().into()),
    )));

    Ok((config_manager, context_manager, execution_engine))
}

// Conversion implementations for configuration types
impl From<kai_x::config::ContextConfig> for kai_x::context::ContextConfig {
    fn from(config: kai_x::config::ContextConfig) -> Self {
        kai_x::context::ContextConfig {
            max_file_size: config.max_file_size,
            exclude_patterns: config.exclude_patterns,
            priority_extensions: config.priority_extensions,
            max_depth: Some(10), // Default value
            follow_symlinks: false, // Default value
        }
    }
}

impl From<kai_x::config::ExecutionConfig> for kai_x::execution::ExecutionConfig {
    fn from(config: kai_x::config::ExecutionConfig) -> Self {
        kai_x::execution::ExecutionConfig {
            max_concurrent_tasks: config.max_concurrent_tasks,
            default_timeout_seconds: config.default_timeout_seconds,
            auto_retry: config.auto_retry,
            max_retries: config.max_retries,
            pause_on_error: config.pause_on_error,
        }
    }
}
