//! KAI-X - A sophisticated Rust-based AI coding assistant CLI
//!
//! This library provides the core functionality for an AI-powered coding assistant
//! that can interpret user prompts, generate execution plans, and manage project context.

pub mod config;
pub mod context;
pub mod execution;
pub mod llm;
pub mod planning;
pub mod ui;
pub mod utils;

// Re-export commonly used types and traits
pub use config::{Config, ConfigManager, ProviderConfig};
pub use context::{ContextManager, GlobalContext, PlanContext, ContextConfig};
pub use execution::{ExecutionEngine, TaskExecutor, ExecutionConfig};
pub use llm::{LlmProvider, LlmProviderFactory, LlmError, Message, ToolDefinition};
pub use planning::{Plan, Task, TaskStatus, TaskResult, TaskType};
pub use ui::{UiManager, UiEvent};
pub use utils::errors::{KaiError, ConfigError};

/// The main result type used throughout the application
pub type Result<T> = std::result::Result<T, KaiError>;

/// Application version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Application name
pub const APP_NAME: &str = "KAI-X";