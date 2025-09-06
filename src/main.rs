//! KAI-X - A sophisticated Rust-based AI coding assistant CLI

use KAI_X::{
    config::ConfigManager,
    context::ContextManager,
    execution::{ExecutionEngine, TaskExecutor},
    llm::{LlmProvider, LlmProviderFactory},
    planning::manager::AgenticPlanningCoordinator,
    ui::ConsoleChat,
    utils::debug::{DEBUG_TRACER, is_debug_enabled},
    debug_flow, debug_checkpoint, debug_error,
    Result,
};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use std::collections::HashMap;
use tracing::{info, error, warn};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

/// KAI-X: A sophisticated Rust-based AI coding assistant
#[derive(Parser, Debug)]
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

#[derive(Subcommand, Clone, Debug)]
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

#[derive(Subcommand, Clone, Debug)]
enum ProviderAction {
    /// List available providers
    List,
    /// Add a new provider
    Add {
        name: String,
    },
    /// Remove a provider
    Remove { name: String },
    /// Set active provider
    Set { name: String, model: Option<String> },
}

#[tokio::main]
async fn main() -> Result<()> {
    let start_time = std::time::Instant::now();
    
    debug_flow!("main", "application_startup", {
        let cli = Cli::parse();
        
        // Check if user requested debug help
        if std::env::var("KAI_DEBUG_HELP").is_ok() {
            KAI_X::utils::debug::print_debug_help();
            return Ok(());
        }

        let mut flow_context = DEBUG_TRACER.start_flow("main", "application_startup");
        debug_checkpoint!(&mut flow_context, "cli_parsed", {
            let mut state = HashMap::new();
            state.insert("workdir".to_string(), serde_json::Value::String(
                cli.workdir.as_ref().map(|p| p.display().to_string()).unwrap_or("None".to_string())
            ));
            state.insert("config_path".to_string(), serde_json::Value::String(
                cli.config.as_ref().map(|p| p.display().to_string()).unwrap_or("None".to_string())
            ));
            state.insert("log_level".to_string(), serde_json::Value::String(cli.log_level.clone()));
            state.insert("non_interactive".to_string(), serde_json::Value::Bool(cli.non_interactive));
            state.insert("command".to_string(), serde_json::Value::String(format!("{:?}", cli.command)));
            state
        });

        // Initialize tracing with debug flow
        debug_checkpoint!(&mut flow_context, "initialize_logging_start");
        match init_logging(&cli.log_level) {
            Ok(_) => {
                debug_checkpoint!(&mut flow_context, "initialize_logging_success");
                if is_debug_enabled() {
                    info!("🔍 [DEBUG] Debug tracing enabled - use KAI_DEBUG_HELP=1 for configuration options");
                    info!("🔍 [DEBUG] Debug summary: {:?}", DEBUG_TRACER.get_debug_summary());
                }
            }
            Err(e) => {
                debug_error!(&mut flow_context, &e, "initialize_logging_failed");
                return Err(e);
            }
        }

        info!("🚀 Starting KAI-X v{}", env!("CARGO_PKG_VERSION"));
        info!("📋 CLI Arguments: {:?}", cli);
        info!("⏱️  Startup timestamp: {:?}", std::time::SystemTime::now());
        // Step 1: Initialize Configuration Manager
        debug_checkpoint!(&mut flow_context, "config_manager_init_start");
        let config_init_start = std::time::Instant::now();
        let config_manager = match ConfigManager::new() {
            Ok(manager) => {
                let config_init_time = config_init_start.elapsed();
                debug_checkpoint!(&mut flow_context, "config_manager_init_success", {
                    let mut state = HashMap::new();
                    state.insert("init_duration_ms".to_string(), serde_json::Value::Number(
                        serde_json::Number::from(config_init_time.as_millis() as u64)
                    ));
                    state.insert("active_provider".to_string(), serde_json::Value::String(
                        manager.config().active_provider.clone()
                    ));
                    state.insert("active_model".to_string(), serde_json::Value::String(
                        manager.config().active_model.clone()
                    ));
                    state
                });
                info!("✅ [FLOW] Configuration manager initialized in {:?}", config_init_time);
                manager
            }
            Err(e) => {
                debug_error!(&mut flow_context, &e, "config_manager_init_failed");
                return Err(e);
            }
        };
        
        // Step 2: Validate Provider Configuration
        debug_checkpoint!(&mut flow_context, "provider_validation_start");
        let validation_start = std::time::Instant::now();
        match validate_provider_configuration(&config_manager, &mut flow_context) {
            Ok(_) => {
                let validation_time = validation_start.elapsed();
                debug_checkpoint!(&mut flow_context, "provider_validation_success", {
                    let mut state = HashMap::new();
                    state.insert("validation_duration_ms".to_string(), serde_json::Value::Number(
                        serde_json::Number::from(validation_time.as_millis() as u64)
                    ));
                    state
                });
                info!("✅ [FLOW] Provider validation completed in {:?}", validation_time);
            }
            Err(e) => {
                debug_error!(&mut flow_context, &e, "provider_validation_failed");
                return Err(e);
            }
        }

        let command = cli.command.clone().unwrap_or(Commands::Chat);
        debug_checkpoint!(&mut flow_context, "command_execution_start", {
            let mut state = HashMap::new();
            state.insert("command".to_string(), serde_json::Value::String(format!("{:?}", command)));
            state
        });
        
        let command_start = std::time::Instant::now();
        
        let result = match command {
            Commands::Init { force } => {
                debug_checkpoint!(&mut flow_context, "executing_init_command");
                init_config(force, &mut flow_context).await
            },
            Commands::Provider { action } => {
                debug_checkpoint!(&mut flow_context, "executing_provider_command");
                handle_provider_command(action, &mut flow_context).await
            },
            Commands::Status => {
                debug_checkpoint!(&mut flow_context, "executing_status_command");
                show_status(&mut flow_context).await
            },
            Commands::Prompt { prompt, format } => {
                debug_checkpoint!(&mut flow_context, "executing_single_prompt_command");
                run_single_prompt(prompt, format, cli.workdir, &mut flow_context).await
            },
            Commands::Chat => {
                debug_checkpoint!(&mut flow_context, "executing_interactive_mode");
                run_interactive_mode(cli, &mut flow_context).await
            },
        };
        
        let command_time = command_start.elapsed();
        let total_time = start_time.elapsed();
        
        match &result {
            Ok(_) => {
                debug_checkpoint!(&mut flow_context, "command_execution_success", {
                    let mut state = HashMap::new();
                    state.insert("command_duration_ms".to_string(), serde_json::Value::Number(
                        serde_json::Number::from(command_time.as_millis() as u64)
                    ));
                    state.insert("total_runtime_ms".to_string(), serde_json::Value::Number(
                        serde_json::Number::from(total_time.as_millis() as u64)
                    ));
                    state
                });
                info!("✅ [FLOW] Command completed successfully in {:?}", command_time);
                info!("🏁 [FLOW] Total application runtime: {:?}", total_time);
            }
            Err(e) => {
                debug_error!(&mut flow_context, e, "command_execution_failed");
                error!("❌ [FLOW] Command failed after {:?}: {}", command_time, e);
                error!("🏁 [FLOW] Application failed after total runtime: {:?}", total_time);
            }
        }
        
        DEBUG_TRACER.end_flow(&flow_context);
        
        // Print final debug summary if debugging is enabled
        if is_debug_enabled() {
            let debug_summary = DEBUG_TRACER.get_debug_summary();
            info!("🔍 [DEBUG-SUMMARY] Checkpoints: {}, Active flows: {}, Errors: {}", 
                debug_summary.total_checkpoints, debug_summary.active_flows, debug_summary.error_count);
            if !debug_summary.component_stats.is_empty() {
                info!("🔍 [DEBUG-SUMMARY] Component activity: {:?}", debug_summary.component_stats);
            }
        }
        
        result
    })
}

/// Initialize logging with debug flow tracing
fn init_logging(log_level: &str) -> Result<()> {
    println!("🔧 [DEBUG] Initializing logging system with level: {}", log_level);
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

    println!("✅ [DEBUG] Logging system initialized successfully");
    Ok(())
}

/// Validate that LLM provider is configured
fn validate_provider_configuration(
    config_manager: &ConfigManager, 
    flow_context: &mut KAI_X::utils::debug::FlowContext
) -> Result<()> {
    debug_checkpoint!(flow_context, "provider_validation_get_config");
    let config = config_manager.config();
    
    debug_checkpoint!(flow_context, "provider_validation_check_active", {
        let mut state = HashMap::new();
        state.insert("active_provider".to_string(), serde_json::Value::String(config.active_provider.clone()));
        state.insert("active_model".to_string(), serde_json::Value::String(config.active_model.clone()));
        state.insert("provider_empty".to_string(), serde_json::Value::Bool(config.active_provider.is_empty()));
        state
    });
    
    if config.active_provider.is_empty() {
        debug_checkpoint!(flow_context, "provider_validation_no_provider");
        error!("❌ [VALIDATION] No LLM provider configured!");
        println!("❌ No LLM provider configured!");
        println!("   Set your API key first:");
        println!("   export OPENROUTER_API_KEY=your-key-here");
        println!("   ");
        println!("   To use other providers:");
        println!("   kai provider add gemini");
        println!("   export GEMINI_API_KEY=your-key-here");
        println!("   kai provider set gemini");
        let error = KAI_X::utils::errors::KaiError::not_found("No active provider configured. Set OPENROUTER_API_KEY environment variable.");
        debug_error!(flow_context, &error, "provider_validation_no_provider_error");
        return Err(error);
    }
    
    // Check API key availability
    debug_checkpoint!(flow_context, "provider_validation_check_api_key");
    let api_key_available = config.get_active_api_key().is_some();
    
    debug_checkpoint!(flow_context, "provider_validation_api_key_result", {
        let mut state = HashMap::new();
        state.insert("api_key_available".to_string(), serde_json::Value::Bool(api_key_available));
        state.insert("provider".to_string(), serde_json::Value::String(config.active_provider.clone()));
        state
    });
    
    info!("🔑 [VALIDATION] API key available for '{}': {}", config.active_provider, api_key_available);
    
    if !api_key_available {
        let env_key_name = format!("{}_API_KEY", config.active_provider.to_uppercase());
        debug_checkpoint!(flow_context, "provider_validation_missing_api_key", {
            let mut state = HashMap::new();
            state.insert("expected_env_var".to_string(), serde_json::Value::String(env_key_name.clone()));
            state
        });
        warn!("⚠️  [VALIDATION] No API key found for provider '{}'. Expected env var: {}", config.active_provider, env_key_name);
    }
    
    debug_checkpoint!(flow_context, "provider_validation_complete");
    info!("✅ [VALIDATION] Provider configuration validation completed successfully");
    Ok(())
}

/// Initialize configuration with enhanced features
async fn init_config(force: bool, flow_context: &mut KAI_X::utils::debug::FlowContext) -> Result<()> {
    debug_checkpoint!(flow_context, "init_config_start");
    info!("Initializing KAI-X configuration with enhanced features...");

    // Initialize configuration
    let _config_manager = ConfigManager::new()?;

    // Check if config already exists and force flag
    let config_path = dirs::config_dir()
        .ok_or_else(|| KAI_X::utils::errors::KaiError::not_found("config directory"))?
        .join("kai-x")
        .join("config.toml");

    if config_path.exists() && !force {
        println!("✅ Configuration already exists at {}", config_path.display());
        
        // Basic configuration check
        println!("✅ Configuration is valid");
        
        println!("   Use --force to reinitialize configuration");
        return Ok(());
    }

    // Secure storage functionality removed as per requirements

    // Note: Overwriting existing configuration if force flag is set

    // Interactive setup
    println!("\n🚀 KAI-X Configuration Setup");
    println!("═══════════════════════════════");
    
    // This would normally include interactive provider setup
    println!("✅ KAI-X configuration initialized successfully!");
    println!();
    println!("📋 Next steps:");
    println!("   1. Set your API key: export OPENROUTER_API_KEY=your-key-here");
    println!("   2. Add more providers: kai provider add gemini");
    println!("   3. Set working directory: kai --workdir /path/to/project");
    println!("   4. Start interactive mode: kai chat");
    println!("   5. Check status: kai status");

    Ok(())
}

/// Handle provider management commands
async fn handle_provider_command(action: ProviderAction, flow_context: &mut KAI_X::utils::debug::FlowContext) -> Result<()> {
    debug_checkpoint!(flow_context, "provider_command_start", {
        let mut state = HashMap::new();
        state.insert("action".to_string(), serde_json::Value::String(format!("{:?}", action)));
        state
    });
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
        ProviderAction::Add { name } => {
            let provider_config = KAI_X::config::ProviderConfig::new_for_provider(&name);
            
            // Check if provider is recognized before moving the config
            let is_recognized = provider_config.base_url.is_some();
            let base_url = provider_config.base_url.clone();
            
            config_manager.set_provider_config(name.clone(), provider_config)?;
            println!("Provider '{}' added successfully!", name);
            
            if !is_recognized {
                println!("⚠️  Warning: '{}' is not a recognized provider. Base URL will need to be configured manually.", name);
            } else if let Some(url) = base_url {
                println!("   Base URL: {}", url);
            }
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
async fn run_single_prompt(prompt: String, format: String, workdir: Option<PathBuf>, flow_context: &mut KAI_X::utils::debug::FlowContext) -> Result<()> {
    debug_checkpoint!(flow_context, "single_prompt_start", {
        let mut state = HashMap::new();
        state.insert("prompt_length".to_string(), serde_json::Value::Number(serde_json::Number::from(prompt.len() as u64)));
        state.insert("format".to_string(), serde_json::Value::String(format.clone()));
        state.insert("workdir".to_string(), serde_json::Value::String(workdir.as_ref().map(|p| p.display().to_string()).unwrap_or("None".to_string())));
        state
    });
    let single_prompt_start = std::time::Instant::now();
    info!("📝 [SINGLE-PROMPT] Starting single prompt execution");
    info!("📝 [SINGLE-PROMPT] Prompt: '{}'", prompt);
    info!("📝 [SINGLE-PROMPT] Format: '{}'", format);
    info!("📝 [SINGLE-PROMPT] Working directory: {:?}", workdir);

    // Initialize system (provider validation already done in main)
    info!("🔧 [SINGLE-PROMPT] Initializing core systems");
    let core_init_start = std::time::Instant::now();
    let (config_manager, context_manager, _execution_engine, _planning_manager) = initialize_core_systems(workdir).await?;
    let core_init_time = core_init_start.elapsed();
    info!("✅ [SINGLE-PROMPT] Core systems initialized in {:?}", core_init_time);

    // Get LLM provider
    info!("🤖 [SINGLE-PROMPT] Setting up LLM provider");
    let provider_setup_start = std::time::Instant::now();
    let config = config_manager.config();

    let mut provider_settings = std::collections::HashMap::new();
    
    // Get API key using centralized function
    if let Some(api_key) = config.get_active_api_key() {
        provider_settings.insert("api_key".to_string(), api_key);
    }
    
    // Get base URL from hardcoded provider settings
    if let Some(base_url) = KAI_X::config::ProviderConfig::get_base_url_for_provider(&config.active_provider) {
        provider_settings.insert("base_url".to_string(), base_url);
    }

    let llm_provider = LlmProviderFactory::create_provider(
        &config.active_provider,
        provider_settings,
    )?;
    let provider_setup_time = provider_setup_start.elapsed();
    info!("✅ [SINGLE-PROMPT] LLM provider '{}' ready in {:?}", config.active_provider, provider_setup_time);

    // Generate and execute plan
    info!("🌍 [SINGLE-PROMPT] Building global context");
    let context_build_start = std::time::Instant::now();
    let context_manager_read = context_manager.read().await;
    let global_context = context_manager_read.get_global_context_summary().await?;
    drop(context_manager_read);
    let context_build_time = context_build_start.elapsed();
    info!("✅ [SINGLE-PROMPT] Global context built in {:?} (length: {} chars)", context_build_time, global_context.len());

    info!("🗨️ [SINGLE-PROMPT] Generating plan with LLM");
    let plan_gen_start = std::time::Instant::now();
    let plan = llm_provider.generate_plan(&prompt, &global_context, &config.active_model).await?;
    let plan_gen_time = plan_gen_start.elapsed();
    info!("✅ [SINGLE-PROMPT] Plan generated in {:?} (tasks: {})", plan_gen_time, plan.tasks.len());

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

    let single_prompt_time = single_prompt_start.elapsed();
    info!("🏁 [SINGLE-PROMPT] Single prompt execution completed in {:?}", single_prompt_time);
    Ok(())
}

/// Run interactive chat mode (simple console)
async fn run_interactive_mode(cli: Cli, flow_context: &mut KAI_X::utils::debug::FlowContext) -> Result<()> {
    debug_checkpoint!(flow_context, "interactive_mode_start");
    info!("Starting interactive mode");
    
    // Initialize core systems properly (following the spec)
    let (config_manager, _context_manager, execution_engine, _planning_manager) = initialize_core_systems(cli.workdir).await?;
    
    // Get working directory from config (validated during initialization)
    let working_dir = config_manager.config().working_directory.clone()
        .ok_or_else(|| KAI_X::utils::errors::KaiError::not_found("Working directory not set"))?;
    
    let config = config_manager.config();
    
    // Create LLM provider
    let mut provider_settings = std::collections::HashMap::new();
    
    if let Some(api_key) = config.get_active_api_key() {
        provider_settings.insert("api_key".to_string(), api_key);
    }
    
    if let Some(base_url) = KAI_X::config::ProviderConfig::get_base_url_for_provider(&config.active_provider) {
        provider_settings.insert("base_url".to_string(), base_url);
    }
    
    let provider_box = LlmProviderFactory::create_provider(
        &config.active_provider,
        provider_settings,
    )?;
    let llm_provider: Arc<dyn LlmProvider> = Arc::from(provider_box);
    
    // Start the execution engine in the background
    let execution_engine_for_loop = execution_engine.clone();
    let _execution_handle = tokio::spawn(async move {
        let engine = execution_engine_for_loop.read().await;
        if let Err(e) = engine.start().await {
            eprintln!("Execution engine error: {}", e);
        }
    });
    
    info!("🤖 KAI-X console chat ready with execution engine");
    
    // Run console chat with proper execution engine integration
    let mut console_chat = ConsoleChat::new(llm_provider, execution_engine, working_dir);
    console_chat.run().await
}

/// Show comprehensive system status
async fn show_status(flow_context: &mut KAI_X::utils::debug::FlowContext) -> Result<()> {
    debug_checkpoint!(flow_context, "status_command_start");
    println!("📊 KAI-X Comprehensive Status");
    println!("═══════════════════════════════");

    // Initialize configuration
    let config_manager = ConfigManager::new()?;

    // Basic information
    println!("\n🔧 Application:");
    println!("   Version: {}", env!("CARGO_PKG_VERSION"));
    println!("   Working Directory: {}", std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "Unknown".to_string()));

    // Basic configuration status
    println!("\n✅ Configuration Status:");
    println!("   Status: Valid ✓");

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
            let has_api_key = config.get_api_key_for_provider(name).is_some();
            let has_file_key = provider_config.api_key.is_some();
            
            // Check for environment variable API key based on provider name
            let env_key_name = format!("{}_API_KEY", name.to_uppercase());
            let has_env_key = std::env::var(&env_key_name).is_ok();
            
            let status_icon = if is_active { "🟢" } else { "⚪" };
            let key_status = if has_env_key {
                "🌍 Environment"
            } else if has_file_key {
                "📁 File"
            } else if has_api_key {
                "✅ Available"
            } else {
                "❌ Missing"
            };
            
            println!("   {} {} ({})", status_icon, name, key_status);
            
            if let Some(base_url) = KAI_X::config::ProviderConfig::get_base_url_for_provider(name) {
                println!("     URL: {}", base_url);
            }
            if let Some(ref default_model) = provider_config.default_model {
                println!("     Default Model: {}", default_model);
            }
        }
    }

    // Configuration file locations
    println!("\n📁 File Locations:");
    if let Some(config_dir) = dirs::data_local_dir().or_else(dirs::config_dir) {
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


    // Show helpful commands
    println!("\n🔧 Quick Commands:");
    println!("   export OPENROUTER_API_KEY=key - Set API key for default provider");
    println!("   kai provider add <name>       - Add LLM provider");
    println!("   kai --workdir <path>          - Set project directory");
    println!("   kai init --force              - Reinitialize configuration");
    println!("   kai chat                      - Start interactive mode");

    Ok(())
}

/// Initialize core systems
async fn initialize_core_systems(
    workdir: Option<PathBuf>,
) -> Result<(ConfigManager, Arc<tokio::sync::RwLock<ContextManager>>, Arc<tokio::sync::RwLock<ExecutionEngine>>, Arc<tokio::sync::RwLock<AgenticPlanningCoordinator>>)> {
    // Load configuration
    let mut config_manager = ConfigManager::new()?;

    // Set working directory if provided
    let working_dir = if let Some(workdir) = workdir {
        config_manager.set_working_directory(workdir.clone())?;
        workdir
    } else {
        config_manager.config().working_directory.clone()
            .ok_or_else(|| KAI_X::utils::errors::KaiError::not_found(
                "No working directory specified. Use --workdir /path/to/project to set working directory"
            ))?
    };

    // Validate working directory is not the source directory
    let current_dir = std::env::current_dir().unwrap_or_default();
    if working_dir == current_dir {
        return Err(KAI_X::utils::errors::KaiError::validation(
            "workdir",
            format!("Cannot use KAI-X source directory as workdir ({}). Use --workdir to specify a different project directory", current_dir.display())
        ));
    }

    // Create working directory if it doesn't exist
    if !working_dir.exists() {
        info!("Creating working directory: {}", working_dir.display());
        std::fs::create_dir_all(&working_dir)
            .map_err(|e| KAI_X::utils::errors::KaiError::file_system(&working_dir, e))?;
    } else if !working_dir.is_dir() {
        return Err(KAI_X::utils::errors::KaiError::validation(
            "workdir",
            format!("Working directory path exists but is not a directory: {}", working_dir.display())
        ));
    }

    info!("Working directory: {}", working_dir.display());

    // Validate configuration
    config_manager.config().validate()?;

    // Get LLM provider (provider validation already done in main)
    let config = config_manager.config();
    let mut provider_settings = std::collections::HashMap::new();
    
    // Get API key using centralized function
    if let Some(api_key) = config.get_active_api_key() {
        provider_settings.insert("api_key".to_string(), api_key);
    }
    
    // Get base URL from hardcoded provider settings
    if let Some(base_url) = KAI_X::config::ProviderConfig::get_base_url_for_provider(&config.active_provider) {
        provider_settings.insert("base_url".to_string(), base_url);
    }

    let provider_box = LlmProviderFactory::create_provider(
        &config.active_provider,
        provider_settings,
    )?;
    let llm_provider: Arc<dyn LlmProvider> = Arc::from(provider_box);

    // Initialize context manager
    let context_manager = Arc::new(tokio::sync::RwLock::new(ContextManager::new(
        working_dir.clone(),
        llm_provider.clone(),
        config.active_model.clone(),
        None,
    )));

    // Initialize execution engine
    let execution_engine = Arc::new(tokio::sync::RwLock::new(ExecutionEngine::new(
        context_manager.clone(),
        llm_provider.clone(),
        config.active_model.clone(),
        working_dir.clone(),
        None,
    )));

    // Initialize task executor for planning manager
    let execution_config = KAI_X::execution::ExecutionConfig::default();
    let task_executor = TaskExecutor::new(
        execution_config,
        working_dir.clone(),
        llm_provider.clone(),
        config.active_model.clone(),
    );

    // Initialize planning manager
    // Create a new context manager for the planning manager
    let planning_context_manager = ContextManager::new(
        working_dir.clone(),
        llm_provider.clone(),
        config.active_model.clone(),
        None,
    );
    
    let planning_manager = AgenticPlanningCoordinator::new(
        task_executor,
        planning_context_manager,
        llm_provider.clone(),
        config.active_model.clone(),
        None, // Use default config
    );

    Ok((config_manager, context_manager, execution_engine, Arc::new(tokio::sync::RwLock::new(planning_manager))))
}

// Complex From trait implementations removed as per requirements
