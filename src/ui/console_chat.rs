//! Simple console-based chat interface without frames

use crate::llm::LlmProvider;
use crate::planning::{Plan, TaskStatus, PlanStatus};
use crate::execution::{ExecutionEngine, PromptPriority};
use crate::Result;
use std::io::{self, Write};
use std::sync::Arc;
use std::path::PathBuf;
use tokio::sync::RwLock;
use colored::*;

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

impl std::fmt::Display for MessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageRole::User => write!(f, "You"),
            MessageRole::Assistant => write!(f, "KAI-X"),
            MessageRole::System => write!(f, "System"),
        }
    }
}

pub struct ConsoleChat {
    llm_provider: Arc<dyn LlmProvider>,
    execution_engine: Arc<RwLock<ExecutionEngine>>,
    working_directory: PathBuf,
    messages: Vec<ChatMessage>,
}

impl ConsoleChat {
    pub fn new(
        llm_provider: Arc<dyn LlmProvider>,
        execution_engine: Arc<RwLock<ExecutionEngine>>,
        working_directory: PathBuf,
    ) -> Self {
        Self {
            llm_provider,
            execution_engine,
            working_directory,
            messages: Vec::new(),
        }
    }
    
    pub async fn run(&mut self) -> Result<()> {
        // Simple welcome with working directory display
        println!("{}", "🤖 KAI-X AI Assistant".bright_green().bold());
        println!("{} {}", "📁 Working directory:".bright_blue(), self.working_directory.display().to_string().bright_yellow());
        println!("{}", "Type your requests below. Use 'exit' to quit, 'clear' to clear chat.".dimmed());
        println!();
        
        loop {
            // Simple prompt
            print!("{} ", "›".bright_blue().bold());
            io::stdout().flush()?;
            
            // Read input
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();
            
            if input.is_empty() { 
                continue; 
            }
            
            // Handle commands
            match input.to_lowercase().as_str() {
                "exit" | "quit" => {
                    println!("{}", "Goodbye!".bright_yellow());
                    break;
                }
                "clear" => {
                    self.clear_screen();
                    self.messages.clear();
                    continue;
                }
                "help" => {
                    self.show_help();
                    continue;
                }
                _ => {}
            }
            
            // Add user message
            self.add_message(MessageRole::User, input.to_string());
            
            // Show AI is thinking
            print!("{} ", "🤔".bright_yellow());
            io::stdout().flush()?;
            
            // Generate and execute response
            match self.generate_and_execute_plan(input).await {
                Ok(plan) => {
                    self.add_message(MessageRole::Assistant, plan.description.clone());
                }
                Err(e) => {
                    let error_msg = format!("Error: {}", e);
                    println!("{}", error_msg.bright_red());
                    self.add_message(MessageRole::System, error_msg);
                }
            }
            
            println!(); // Add spacing
        }
        
        Ok(())
    }
    
    async fn generate_and_execute_plan(&self, input: &str) -> Result<Plan> {
        // Start by submitting the user prompt to the execution engine
        let prompt_id = {
            let engine = self.execution_engine.read().await;
            engine.submit_user_prompt(input.to_string(), PromptPriority::Normal).await
        };
        
        println!("🔄 Plan queued with ID: {}", prompt_id);
        println!("⏳ Waiting for plan generation...");
        
        // Get the current plan (the execution engine will generate it)
        // We'll poll for a plan to appear
        let mut attempts = 0;
        let max_attempts = 100; // 10 seconds with 100ms intervals (increased timeout)
        
        while attempts < max_attempts {
            let plan = {
                let engine = self.execution_engine.read().await;
                engine.get_current_plan().await
            };
            if let Some(plan) = plan {
                println!("✅ Plan generated: {}", plan.description);
                self.display_plan_with_execution_status(&plan).await;
                return Ok(plan);
            }
            
            // Show progress every second
            if attempts % 10 == 0 && attempts > 0 {
                println!("⏳ Still waiting... ({}/{}s)", attempts / 10, max_attempts / 10);
            }
            
            attempts += 1;
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        
        println!("❌ Timeout waiting for plan generation after {}s", max_attempts / 10);
        Err(crate::utils::errors::KaiError::execution("Timeout waiting for plan generation".to_string()))
    }
    
    async fn build_context(&self) -> Result<String> {
        // Simple context - you can expand this
        let working_dir = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| ".".to_string());
        
        Ok(format!("Working directory: {}", working_dir))
    }
    
    fn add_message(&mut self, role: MessageRole, content: String) {
        self.messages.push(ChatMessage {
            role,
            content,
            timestamp: chrono::Utc::now(),
        });
    }
    
    async fn display_plan_with_execution_status(&self, plan: &Plan) {
        println!(); // Space before plan
        
        // Plan description
        println!("{}", plan.description.bright_cyan());
        
        // Monitor task execution in real-time
        let mut _last_task_count = 0;
        let mut completed_tasks = std::collections::HashSet::new();
        
        // Initial display of tasks
        for (i, task) in plan.tasks.iter().enumerate() {
            let status_symbol = self.get_task_symbol(&task.status);
            let task_color = self.get_task_color(&task.status);
            
            println!("{}  {}. {}", 
                status_symbol,
                (i + 1).to_string().bright_yellow(),
                task.description.color(task_color)
            );
        }
        
        // Monitor execution progress
        let mut monitoring_attempts = 0;
        let max_monitoring_time = 300; // 30 seconds of monitoring
        
        while monitoring_attempts < max_monitoring_time {
            let current_plan = {
                let engine = self.execution_engine.read().await;
                engine.get_current_plan().await
            };
            if let Some(current_plan) = current_plan {
                let mut all_completed = true;
                let mut _has_changes = false;
                
                for (i, task) in current_plan.tasks.iter().enumerate() {
                    if task.status != TaskStatus::Completed && task.status != TaskStatus::Failed {
                        all_completed = false;
                    }
                    
                    // Check if this task status changed
                    let task_key = format!("{}:{}", i, task.id);
                    if !completed_tasks.contains(&task_key) {
                        if task.status == TaskStatus::Completed || task.status == TaskStatus::Failed {
                            completed_tasks.insert(task_key);
                            _has_changes = true;
                            
                            let status_symbol = self.get_task_symbol(&task.status);
                            let task_color = self.get_task_color(&task.status);
                            
                            // Update this specific line
                            print!("\r{} {}. {}", 
                                status_symbol,
                                (i + 1).to_string().bright_yellow(),
                                task.description.color(task_color)
                            );
                            println!(); // Move to next line
                        }
                    }
                }
                
                if all_completed {
                    break;
                }
            }
            
            monitoring_attempts += 1;
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        
        // Final plan status
        let final_plan = {
            let engine = self.execution_engine.read().await;
            engine.get_current_plan().await
        };
        if let Some(current_plan) = final_plan {
            if current_plan.status != PlanStatus::Ready {
                let status_color = self.get_plan_status_color(&current_plan.status);
                println!("\n{} {}", "Status:".dimmed(), format!("{:?}", current_plan.status).color(status_color));
            }
        }
    }
    
    fn display_plan(&self, plan: &Plan) {
        println!(); // Space before plan
        
        // Plan description
        println!("{}", plan.description.bright_cyan());
        
        // Tasks
        for (i, task) in plan.tasks.iter().enumerate() {
            let status_symbol = self.get_task_symbol(&task.status);
            let task_color = self.get_task_color(&task.status);
            
            println!("{}  {}. {}", 
                status_symbol,
                (i + 1).to_string().bright_yellow(),
                task.description.color(task_color)
            );
        }
        
        // Plan status
        if plan.status != PlanStatus::Ready {
            let status_color = self.get_plan_status_color(&plan.status);
            println!("\n{} {}", "Status:".dimmed(), format!("{:?}", plan.status).color(status_color));
        }
    }
    
    fn get_task_symbol(&self, status: &TaskStatus) -> &str {
        match status {
            TaskStatus::Pending => "⏸",
            TaskStatus::Ready => "▶",
            TaskStatus::InProgress => "⏳",
            TaskStatus::Completed => "✅",
            TaskStatus::Failed => "❌",
            TaskStatus::Skipped => "⏭",
        }
    }
    
    fn get_task_color(&self, status: &TaskStatus) -> colored::Color {
        match status {
            TaskStatus::Pending => colored::Color::White,
            TaskStatus::Ready => colored::Color::Yellow,
            TaskStatus::InProgress => colored::Color::Blue,
            TaskStatus::Completed => colored::Color::Green,
            TaskStatus::Failed => colored::Color::Red,
            TaskStatus::Skipped => colored::Color::BrightBlack,
        }
    }
    
    fn get_plan_status_color(&self, status: &PlanStatus) -> colored::Color {
        match status {
            PlanStatus::Ready => colored::Color::Yellow,
            PlanStatus::Executing => colored::Color::Blue,
            PlanStatus::Paused => colored::Color::Magenta,
            PlanStatus::Completed => colored::Color::Green,
            PlanStatus::Failed => colored::Color::Red,
            PlanStatus::Cancelled => colored::Color::BrightBlack,
        }
    }
    
    fn clear_screen(&self) {
        // Simple clear - works on most terminals
        print!("\x1B[2J\x1B[H");
        io::stdout().flush().unwrap();
        println!("{}", "🤖 KAI-X AI Assistant".bright_green().bold());
        println!("{}", "Type your requests below. Use 'exit' to quit, 'clear' to clear chat.".dimmed());
        println!();
    }
    
    fn show_help(&self) {
        println!();
        println!("{}", "Available commands:".bright_cyan().bold());
        println!("  {} - Exit the application", "exit/quit".bright_yellow());
        println!("  {} - Clear the chat history", "clear".bright_yellow());
        println!("  {} - Show this help message", "help".bright_yellow());
        println!();
        println!("{}", "Just type your request to get started!".dimmed());
        println!();
    }
}