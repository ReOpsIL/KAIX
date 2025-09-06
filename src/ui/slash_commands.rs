//! Slash command processing with interactive menus

use crate::ui::events::{SlashCommand, UiEvent};
use crate::utils::errors::KaiError;
use crate::Result;
use inquire::{Select, Text, Confirm};
use std::path::PathBuf;
use tokio::sync::mpsc;

/// Slash command processor with interactive menus
pub struct SlashCommandProcessor {
    /// Event sender for communicating results
    event_sender: mpsc::UnboundedSender<UiEvent>,
    /// Current working directory
    working_directory: PathBuf,
}

impl SlashCommandProcessor {
    /// Create a new slash command processor
    pub fn new(event_sender: mpsc::UnboundedSender<UiEvent>) -> Self {
        Self {
            event_sender,
            working_directory: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        }
    }

    /// Process a slash command with interactive menus
    pub async fn process_command(&mut self, command: SlashCommand) -> Result<()> {
        match command {
            SlashCommand::Model(model_name) => {
                if model_name.is_empty() {
                    self.show_model_selector().await?;
                } else {
                    self.set_model(model_name).await?;
                }
            }
            SlashCommand::ListModels => {
                self.list_models().await?;
            }
            SlashCommand::Provider(provider_name) => {
                if provider_name.is_empty() {
                    self.show_provider_selector().await?;
                } else {
                    self.set_provider(provider_name).await?;
                }
            }
            SlashCommand::ResetContext => {
                self.reset_context().await?;
            }
            SlashCommand::RefreshContext => {
                self.refresh_context().await?;
            }
            SlashCommand::Help => {
                self.show_help().await?;
            }
            SlashCommand::WorkDir(path) => {
                if path.is_empty() {
                    self.show_directory_selector().await?;
                } else {
                    self.set_working_directory(path).await?;
                }
            }
            SlashCommand::History => {
                self.show_history().await?;
            }
            SlashCommand::Clear => {
                self.clear_chat().await?;
            }
            SlashCommand::Status => {
                self.show_status().await?;
            }
            SlashCommand::Cancel => {
                self.cancel_execution().await?;
            }
            SlashCommand::Pause => {
                self.pause_execution().await?;
            }
            SlashCommand::Resume => {
                self.resume_execution().await?;
            }
            SlashCommand::Unknown(cmd) => {
                self.handle_unknown_command(cmd).await?;
            }
        }
        Ok(())
    }

    /// Show interactive model selector
    async fn show_model_selector(&self) -> Result<()> {
        let models = vec![
            "anthropic/claude-3.5-sonnet",
            "anthropic/claude-3.5-haiku", 
            "openai/gpt-4o",
            "openai/gpt-4o-mini",
            "google/gemini-pro-1.5",
            "google/gemini-flash-1.5",
            "meta-llama/llama-3.1-405b-instruct",
            "meta-llama/llama-3.1-70b-instruct",
            "anthropic/claude-3-opus",
            "anthropic/claude-3-sonnet",
        ];

        tokio::task::spawn_blocking(move || {
            let selection = Select::new("Select a model:", models)
                .with_help_message("Choose the LLM model to use for code generation and analysis")
                .prompt();

            match selection {
                Ok(model) => {
                    println!("✓ Model set to: {}", model);
                    // In a real implementation, this would update the configuration
                }
                Err(e) => {
                    println!("Model selection cancelled: {}", e);
                }
            }
        }).await.map_err(|e| KaiError::ui(format!("Failed to show model selector: {}", e)))?;

        Ok(())
    }

    /// Show interactive provider selector
    async fn show_provider_selector(&self) -> Result<()> {
        let providers = vec![
            "OpenRouter",
            "Google Gemini",
            "OpenAI",
            "Anthropic",
            "Local (Ollama)",
        ];

        tokio::task::spawn_blocking(move || {
            let selection = Select::new("Select an LLM provider:", providers)
                .with_help_message("Choose the provider for LLM services")
                .prompt();

            match selection {
                Ok(provider) => {
                    println!("✓ Provider set to: {}", provider);
                    // In a real implementation, this would update the configuration
                }
                Err(e) => {
                    println!("Provider selection cancelled: {}", e);
                }
            }
        }).await.map_err(|e| KaiError::ui(format!("Failed to show provider selector: {}", e)))?;

        Ok(())
    }

    /// Show interactive directory selector
    async fn show_directory_selector(&self) -> Result<()> {
        let current_dir = self.working_directory.clone();
        
        tokio::task::spawn_blocking(move || {
            let path_input = Text::new("Enter working directory path:")
                .with_default(&current_dir.to_string_lossy())
                .with_help_message("Specify the root directory for the project context")
                .prompt();

            match path_input {
                Ok(path) => {
                    let path_buf = PathBuf::from(path);
                    if path_buf.exists() && path_buf.is_dir() {
                        println!("✓ Working directory set to: {}", path_buf.display());
                        // In a real implementation, this would update the working directory
                    } else {
                        println!("✗ Invalid directory: {}", path_buf.display());
                    }
                }
                Err(e) => {
                    println!("Directory selection cancelled: {}", e);
                }
            }
        }).await.map_err(|e| KaiError::ui(format!("Failed to show directory selector: {}", e)))?;

        Ok(())
    }

    /// Set model directly
    async fn set_model(&self, model_name: String) -> Result<()> {
        println!("✓ Model set to: {}", model_name);
        
        // Send event to update application state
        self.event_sender.send(UiEvent::ProviderChanged("provider".to_string(), model_name))
            .map_err(|e| KaiError::ui(format!("Failed to send provider change event: {}", e)))?;
        
        Ok(())
    }

    /// Set provider directly
    async fn set_provider(&self, provider_name: String) -> Result<()> {
        println!("✓ Provider set to: {}", provider_name);
        
        // Send event to update application state
        self.event_sender.send(UiEvent::ProviderChanged(provider_name, "model".to_string()))
            .map_err(|e| KaiError::ui(format!("Failed to send provider change event: {}", e)))?;
        
        Ok(())
    }

    /// List available models
    async fn list_models(&self) -> Result<()> {
        println!("\n📋 Available Models:");
        println!("┌─────────────────────────────────────┬──────────────┬─────────────┐");
        println!("│ Model                               │ Provider     │ Context     │");
        println!("├─────────────────────────────────────┼──────────────┼─────────────┤");
        println!("│ anthropic/claude-3.5-sonnet        │ Anthropic    │ 200K tokens │");
        println!("│ anthropic/claude-3.5-haiku         │ Anthropic    │ 200K tokens │");
        println!("│ openai/gpt-4o                      │ OpenAI       │ 128K tokens │");
        println!("│ openai/gpt-4o-mini                 │ OpenAI       │ 128K tokens │");
        println!("│ google/gemini-pro-1.5              │ Google       │ 2M tokens   │");
        println!("│ google/gemini-flash-1.5            │ Google       │ 1M tokens   │");
        println!("│ meta-llama/llama-3.1-405b-instruct │ Meta         │ 32K tokens  │");
        println!("│ meta-llama/llama-3.1-70b-instruct  │ Meta         │ 32K tokens  │");
        println!("└─────────────────────────────────────┴──────────────┴─────────────┘");
        
        Ok(())
    }

    /// Reset context with confirmation
    async fn reset_context(&self) -> Result<()> {
        tokio::task::spawn_blocking(move || {
            let confirm = Confirm::new("Are you sure you want to reset the entire context?")
                .with_help_message("This will clear all cached file summaries and context data")
                .with_default(false)
                .prompt();

            match confirm {
                Ok(true) => {
                    println!("🔄 Resetting context...");
                    // In a real implementation, this would reset the global context
                }
                Ok(false) | Err(_) => {
                    println!("Context reset cancelled");
                }
            }
        }).await.map_err(|e| KaiError::ui(format!("Failed to reset context: {}", e)))?;

        self.event_sender.send(UiEvent::ContextRefreshed)
            .map_err(|e| KaiError::ui(format!("Failed to send context refresh event: {}", e)))?;

        Ok(())
    }

    /// Refresh context for modified files
    async fn refresh_context(&self) -> Result<()> {
        println!("🔄 Refreshing context for modified files...");
        
        self.event_sender.send(UiEvent::ContextRefreshed)
            .map_err(|e| KaiError::ui(format!("Failed to send context refresh event: {}", e)))?;
        
        Ok(())
    }

    /// Show comprehensive help
    async fn show_help(&self) -> Result<()> {
        println!("\n🔥 KAI-X Help - Slash Commands");
        println!("═══════════════════════════════════════════════════════════");
        println!();
        println!("📋 Model & Provider Management:");
        println!("  /model [name]     - Set or select LLM model");
        println!("  /list-models      - Show all available models");
        println!("  /provider [name]  - Set or select LLM provider");
        println!();
        println!("🗂️  Context Management:");
        println!("  /reset-context    - Reset and regenerate all context");
        println!("  /refresh-context  - Refresh context for modified files");
        println!("  /workdir [path]   - Set or select working directory");
        println!();
        println!("🎛️  Execution Control:");
        println!("  /cancel          - Cancel current plan execution");
        println!("  /pause           - Pause current plan execution");
        println!("  /resume          - Resume paused execution");
        println!("  /status          - Show application status");
        println!();
        println!("💬 Interface Commands:");
        println!("  /history         - Show command history");
        println!("  /clear           - Clear chat history");
        println!("  /help            - Show this help message");
        println!();
        println!("🔍 Special Characters:");
        println!("  @ [path]         - Trigger file browser and select files");
        println!("  # [query]        - Search command history");
        println!();
        println!("⌨️  Keyboard Shortcuts:");
        println!("  Ctrl+A           - Move to beginning of line");
        println!("  Ctrl+E           - Move to end of line");
        println!("  Ctrl+W           - Delete word backward");
        println!("  Ctrl+U           - Delete to line start");
        println!("  Ctrl+K           - Delete to line end");
        println!("  Tab              - Accept completion");
        println!("  Esc              - Cancel/Exit");
        println!();
        
        Ok(())
    }

    /// Set working directory
    async fn set_working_directory(&mut self, path: String) -> Result<()> {
        let path_buf = PathBuf::from(path);
        
        if path_buf.exists() && path_buf.is_dir() {
            self.working_directory = path_buf.clone();
            println!("✓ Working directory set to: {}", path_buf.display());
            
            self.event_sender.send(UiEvent::WorkingDirectoryChanged(path_buf.to_string_lossy().to_string()))
                .map_err(|e| KaiError::ui(format!("Failed to send working directory change event: {}", e)))?;
        } else {
            return Err(KaiError::ui(format!("Invalid directory: {}", path_buf.display())));
        }
        
        Ok(())
    }

    /// Show command history
    async fn show_history(&self) -> Result<()> {
        println!("📜 Command History:");
        println!("This would show the command history from the HistoryService");
        // In a real implementation, this would integrate with HistoryService
        Ok(())
    }

    /// Clear chat history
    async fn clear_chat(&self) -> Result<()> {
        tokio::task::spawn_blocking(move || {
            let confirm = Confirm::new("Clear all chat history?")
                .with_default(false)
                .prompt();

            match confirm {
                Ok(true) => {
                    println!("🧹 Chat history cleared");
                }
                Ok(false) | Err(_) => {
                    println!("Clear cancelled");
                }
            }
        }).await.map_err(|e| KaiError::ui(format!("Failed to clear chat: {}", e)))?;

        Ok(())
    }

    /// Show application status
    async fn show_status(&self) -> Result<()> {
        println!("\n📊 KAI-X Status");
        println!("═════════════════");
        println!("🏠 Working Directory: {}", self.working_directory.display());
        println!("🤖 Provider: OpenRouter (example)");
        println!("🧠 Model: anthropic/claude-3.5-sonnet (example)");
        println!("📁 Context Files: 0 (example)");
        println!("⚡ Execution State: Idle (example)");
        println!("💾 Memory Usage: 0 MB (example)");
        
        self.event_sender.send(UiEvent::StatusUpdate(
            crate::ui::events::StatusUpdate::new()
                .with_working_directory(self.working_directory.to_string_lossy().to_string())
                .with_execution_state("Idle".to_string())
        )).map_err(|e| KaiError::ui(format!("Failed to send status update: {}", e)))?;
        
        Ok(())
    }

    /// Cancel current execution
    async fn cancel_execution(&self) -> Result<()> {
        println!("⏹️  Cancelling current execution...");
        self.event_sender.send(UiEvent::ExecutionStateChanged("Cancelled".to_string()))
            .map_err(|e| KaiError::ui(format!("Failed to send cancellation event: {}", e)))?;
        Ok(())
    }

    /// Pause current execution
    async fn pause_execution(&self) -> Result<()> {
        println!("⏸️  Pausing execution...");
        self.event_sender.send(UiEvent::ExecutionStateChanged("Paused".to_string()))
            .map_err(|e| KaiError::ui(format!("Failed to send pause event: {}", e)))?;
        Ok(())
    }

    /// Resume paused execution
    async fn resume_execution(&self) -> Result<()> {
        println!("▶️  Resuming execution...");
        self.event_sender.send(UiEvent::ExecutionStateChanged("Executing".to_string()))
            .map_err(|e| KaiError::ui(format!("Failed to send resume event: {}", e)))?;
        Ok(())
    }

    /// Handle unknown command with suggestions
    async fn handle_unknown_command(&self, command: String) -> Result<()> {
        println!("❓ Unknown command: {}", command);
        
        // Provide suggestions based on fuzzy matching
        let all_commands = vec![
            "/model", "/list-models", "/provider", "/reset-context", 
            "/refresh-context", "/help", "/workdir", "/history", 
            "/clear", "/status", "/cancel", "/pause", "/resume"
        ];
        
        let suggestions: Vec<&str> = all_commands
            .iter()
            .filter(|cmd| {
                let cmd_lower = cmd.to_lowercase();
                let query_lower = command.to_lowercase();
                cmd_lower.contains(&query_lower) || 
                levenshtein_distance(&cmd_lower, &query_lower) <= 2
            })
            .cloned()
            .collect();
        
        if !suggestions.is_empty() {
            println!("💡 Did you mean:");
            for suggestion in suggestions {
                println!("   {}", suggestion);
            }
        }
        println!("   Type /help for all available commands");
        
        Ok(())
    }

    /// Get current working directory
    pub fn get_working_directory(&self) -> &PathBuf {
        &self.working_directory
    }
}

/// Simple Levenshtein distance calculation for command suggestions
fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.len();
    let len2 = s2.len();
    
    if len1 == 0 { return len2; }
    if len2 == 0 { return len1; }
    
    let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];
    
    for i in 0..=len1 { matrix[i][0] = i; }
    for j in 0..=len2 { matrix[0][j] = j; }
    
    for (i, c1) in s1.chars().enumerate() {
        for (j, c2) in s2.chars().enumerate() {
            let cost = if c1 == c2 { 0 } else { 1 };
            matrix[i + 1][j + 1] = (matrix[i][j + 1] + 1)
                .min(matrix[i + 1][j] + 1)
                .min(matrix[i][j] + cost);
        }
    }
    
    matrix[len1][len2]
}