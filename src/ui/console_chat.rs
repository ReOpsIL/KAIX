//! Simple console-based chat interface without frames

use crate::llm::LlmProvider;
use crate::planning::{Plan, TaskStatus, PlanStatus};
use crate::Result;
use std::io::{self, Write};
use std::sync::Arc;
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
    messages: Vec<ChatMessage>,
}

impl ConsoleChat {
    pub fn new(llm_provider: Arc<dyn LlmProvider>) -> Self {
        Self {
            llm_provider,
            messages: Vec::new(),
        }
    }
    
    pub async fn run(&mut self) -> Result<()> {
        // Simple welcome
        println!("{}", "🤖 KAI-X AI Assistant".bright_green().bold());
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
            
            // Generate response
            match self.generate_response(input).await {
                Ok(plan) => {
                    self.display_plan(&plan);
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
    
    async fn generate_response(&self, input: &str) -> Result<Plan> {
        // Build basic context
        let global_context = self.build_context().await?;
        
        // Generate plan using existing LLM provider
        let plan = self.llm_provider
            .generate_plan(input, &global_context, "anthropic/claude-3-haiku")
            .await?;
        
        Ok(plan)
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