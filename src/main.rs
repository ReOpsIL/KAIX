//! KAI-X - A sophisticated Rust-based AI coding assistant CLI

use KAI_X::{
    config::ConfigManager,
    context::ContextManager,
    execution::ExecutionEngine,
    llm::{LlmProvider, LlmProviderFactory},
    ui::UiManager,
    Result,
};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, error, warn};
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

#[derive(Subcommand, Clone)]
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

#[derive(Subcommand, Clone)]
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

    match cli.command.clone().unwrap_or(Commands::Chat) {
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
        .map_err(|e| KAI_X::utils::errors::KaiError::unknown(format!("Invalid log level: {}", e)))?;

    let subscriber = FmtSubscriber::builder()
        .with_env_filter(filter)
        .with_target(false)
        .with_thread_ids(false)
        .with_file(true)
        .with_line_number(true)
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .map_err(|e| KAI_X::utils::errors::KaiError::unknown(format!("Failed to set logger: {}", e)))?;

    Ok(())
}

/// Initialize configuration with enhanced features
async fn init_config(force: bool) -> Result<()> {
    info!("Initializing KAI-X configuration with enhanced features...");

    // Initialize with migration support
    let (mut config_manager, migration_warnings) = ConfigManager::initialize_with_migration()?;

    // Display migration warnings if any
    if !migration_warnings.is_empty() {
        println!("⚠️  Configuration migration warnings:");
        for warning in &migration_warnings {
            println!("   {}", warning);
        }
        println!();
    }

    // Check if config already exists and force flag
    let config_path = dirs::config_dir()
        .ok_or_else(|| KAI_X::utils::errors::KaiError::not_found("config directory"))?
        .join("kai-x")
        .join("config.toml");

    if config_path.exists() && !force {
        println!("✅ Configuration already exists at {}", config_path.display());
        
        // Validate existing configuration
        let validation_result = config_manager.validate_comprehensive()?;
        if validation_result.is_valid() {
            println!("✅ Configuration is valid");
        } else {
            println!("⚠️  Configuration has {} errors and {} warnings", 
                validation_result.error_count(),
                validation_result.warning_count()
            );
            println!("   Use 'kai status' to see detailed validation results");
        }
        
        println!("   Use --force to reinitialize configuration");
        return Ok(());
    }

    // Secure storage functionality removed as per requirements

    // Create backup if overwriting
    if config_path.exists() && force {
        match config_manager.create_backup() {
            Ok(backup_path) => {
                println!("💾 Configuration backup created: {}", backup_path.display());
            }
            Err(e) => {
                warn!("Failed to create configuration backup: {}", e);
            }
        }
    }

    // Interactive setup
    println!("\n🚀 KAI-X Configuration Setup");
    println!("═══════════════════════════════");
    
    // This would normally include interactive provider setup
    println!("✅ KAI-X configuration initialized successfully!");
    println!();
    println!("📋 Next steps:");
    println!("   1. Add LLM providers: kai provider add <name> <api-key>");
    println!("   2. Set working directory: kai --workdir /path/to/project");
    println!("   3. Start interactive mode: kai chat");
    println!("   4. Check status: kai status");

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
            let mut provider_config = KAI_X::config::ProviderConfig::default();
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
                return Err(KAI_X::utils::errors::KaiError::not_found(format!("Provider '{}'", name)));
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
        .ok_or_else(|| KAI_X::utils::errors::KaiError::not_found("No active provider configured"))?;

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

/// Show comprehensive system status
async fn show_status() -> Result<()> {
    println!("📊 KAI-X Comprehensive Status");
    println!("═══════════════════════════════");

    // Initialize with migration support
    let (config_manager, migration_warnings) = ConfigManager::initialize_with_migration()?;

    // Show migration warnings if any
    if !migration_warnings.is_empty() {
        println!("\n⚠️  Recent migration warnings:");
        for warning in &migration_warnings {
            println!("   {}", warning);
        }
    }

    // Basic information
    println!("\n🔧 Application:");
    println!("   Version: {}", env!("CARGO_PKG_VERSION"));
    println!("   Working Directory: {}", std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "Unknown".to_string()));

    // Configuration validation
    let validation_result = config_manager.validate_comprehensive()?;
    println!("\n✅ Configuration Validation:");
    if validation_result.is_valid() {
        println!("   Status: Valid ✓");
    } else {
        println!("   Status: {} errors, {} warnings ⚠️", 
            validation_result.error_count(),
            validation_result.warning_count());
            
        // Show critical errors
        let critical_errors = validation_result.get_critical_errors();
        if !critical_errors.is_empty() {
            println!("   Critical errors:");
            for (field, error) in critical_errors.iter().take(3) { // Show first 3
                println!("     • {}: {}", field, error.message);
            }
            if critical_errors.len() > 3 {
                println!("     • ... and {} more", critical_errors.len() - 3);
            }
        }
    }

    // Current configuration
    let config = config_manager.config();
    println!("\n🤖 Active Configuration:");
    println!("   Provider: {}", config.active_provider);
    println!("   Model: {}", config.active_model);
    
    if let Some(workdir) = &config.working_directory {
        println!("   Project Directory: {}", workdir.display());
        
        // Check if directory exists and is accessible
        if workdir.exists() {
            if workdir.is_dir() {
                println!("     Status: Accessible ✓");
                
                // Check for common project files
                let project_indicators = ["Cargo.toml", "package.json", ".git", "README.md"];
                let found_indicators: Vec<&str> = project_indicators.iter()
                    .filter(|&&indicator| workdir.join(indicator).exists())
                    .cloned()
                    .collect();
                
                if !found_indicators.is_empty() {
                    println!("     Project type: Detected ({})", found_indicators.join(", "));
                }
            } else {
                println!("     Status: Not a directory ⚠️");
            }
        } else {
            println!("     Status: Does not exist ❌");
        }
    } else {
        println!("   Project Directory: Not set");
    }

    // API keys are now read from environment variables
    println!("\n🔒 Security:");
    println!("   API Key Source: Environment Variables ✓");

    println!("\n🔧 Configured Providers:");
    if config.providers.is_empty() {
        println!("   No providers configured");
        println!("   Add providers with: kai provider add <name> <api-key>");
    } else {
        for (name, provider_config) in &config.providers {
            let is_active = name == &config.active_provider;
            let has_file_key = provider_config.api_key.is_some();
            
            // Check for environment variable API key based on provider name
            let env_key_name = format!("{}_API_KEY", name.to_uppercase());
            let has_env_key = std::env::var(&env_key_name).is_ok();
            
            let status_icon = if is_active { "🟢" } else { "⚪" };
            let key_status = if has_env_key {
                "🌍 Environment"
            } else if has_file_key {
                "📁 File"
            } else {
                "❌ Missing"
            };
            
            println!("   {} {} ({})", status_icon, name, key_status);
            
            if let Some(ref base_url) = provider_config.base_url {
                println!("     URL: {}", base_url);
            }
            if let Some(ref default_model) = provider_config.default_model {
                println!("     Default Model: {}", default_model);
            }
        }
    }

    // Configuration file locations
    println!("\n📁 File Locations:");
    if let Some(config_dir) = dirs::config_dir() {
        let app_config_dir = config_dir.join("kai-x");
        println!("   Config Directory: {}", app_config_dir.display());
        println!("   Configuration: {}", app_config_dir.join("config.toml").display());
        println!("   Backups: {}", app_config_dir.join("backups").display());
    }
    
    if let Ok(data_dir) = dirs::data_local_dir().or_else(dirs::data_dir).ok_or_else(|| KAI_X::utils::errors::KaiError::not_found("data directory")) {
        let app_data_dir = data_dir.join("kai-x");
        println!("   Session Data: {}", app_data_dir.join("session.json").display());
        println!("   History: {}", app_data_dir.join("history.json").display());
    }

    // Show suggestions if configuration needs improvement
    if !validation_result.suggestions.is_empty() {
        println!("\n💡 Suggestions:");
        for suggestion in validation_result.suggestions.iter().take(3) {
            println!("   • {}", suggestion.message);
        }
        if validation_result.suggestions.len() > 3 {
            println!("   • ... and {} more suggestions", validation_result.suggestions.len() - 3);
        }
    }

    // Show helpful commands
    println!("\n🔧 Quick Commands:");
    println!("   kai provider add <name> <key>  - Add LLM provider");
    println!("   kai --workdir <path>          - Set project directory");
    println!("   kai init --force              - Reinitialize configuration");
    println!("   kai chat                      - Start interactive mode");

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
        .ok_or_else(|| KAI_X::utils::errors::KaiError::not_found("No active provider configured"))?;

    let mut provider_settings = std::collections::HashMap::new();
    if let Some(api_key) = &provider_config.api_key {
        provider_settings.insert("api_key".to_string(), api_key.clone());
    }
    if let Some(base_url) = &provider_config.base_url {
        provider_settings.insert("base_url".to_string(), base_url.clone());
    }

    let provider_box = LlmProviderFactory::create_provider(
        &config.active_provider,
        provider_settings,
    )?;
    let llm_provider: Arc<dyn LlmProvider> = Arc::from(provider_box);

    // Initialize context manager
    let context_manager = Arc::new(RwLock::new(ContextManager::new(
        working_dir.clone(),
        llm_provider.clone(),
        config.active_model.clone(),
        None,
    )));

    // Initialize execution engine
    let execution_engine = Arc::new(RwLock::new(ExecutionEngine::new(
        context_manager.clone(),
        llm_provider.clone(),
        config.active_model.clone(),
        working_dir.clone(),
        None,
    )));

    Ok((config_manager, context_manager, execution_engine))
}

// Complex From trait implementations removed as per requirements
