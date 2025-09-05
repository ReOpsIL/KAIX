//! UI event types for communication between components

use crate::planning::{Plan, Task, TaskResult};

/// Main UI event types
#[derive(Debug, Clone)]
pub enum UiEvent {
    /// User submitted a prompt
    SubmitPrompt(String),
    /// User wants to quit the application
    Quit,
    /// Plan was updated
    PlanUpdated(Plan),
    /// Task was completed
    TaskCompleted(TaskCompleted),
    /// Task started execution
    TaskStarted(TaskStarted),
    /// Task failed
    TaskFailed(TaskFailed),
    /// Execution state changed
    ExecutionStateChanged(String),
    /// Error occurred
    Error(String),
    /// Status update
    StatusUpdate(StatusUpdate),
    /// Context was refreshed
    ContextRefreshed,
    /// Provider changed
    ProviderChanged(String, String), // provider, model
    /// Working directory changed
    WorkingDirectoryChanged(String),
    /// Slash command executed
    SlashCommand(SlashCommand),
}

/// Task completion event
#[derive(Debug, Clone)]
pub struct TaskCompleted {
    pub task_id: String,
    pub task_description: String,
    pub result: TaskResult,
}

/// Task start event
#[derive(Debug, Clone)]
pub struct TaskStarted {
    pub task_id: String,
    pub task_description: String,
}

/// Task failure event
#[derive(Debug, Clone)]
pub struct TaskFailed {
    pub task_id: String,
    pub task_description: String,
    pub error: String,
}

/// Status update event
#[derive(Debug, Clone)]
pub struct StatusUpdate {
    pub execution_state: Option<String>,
    pub active_provider: Option<String>,
    pub active_model: Option<String>,
    pub working_directory: Option<String>,
    pub context_files: Option<usize>,
    pub memory_usage: Option<u64>,
}

/// Slash command variants
#[derive(Debug, Clone)]
pub enum SlashCommand {
    Model(String),
    ListModels,
    Provider(String),
    ResetContext,
    RefreshContext,
    Help,
    WorkDir(String),
    History,
    Clear,
    Status,
    Cancel,
    Pause,
    Resume,
    Unknown(String),
}

/// Key event wrapper
#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

/// Key codes
#[derive(Debug, Clone)]
pub enum KeyCode {
    Backspace,
    Enter,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Tab,
    BackTab,
    Delete,
    Insert,
    F(u8),
    Char(char),
    Null,
    Esc,
}

/// Key modifiers
#[derive(Debug, Clone)]
pub struct KeyModifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
}

/// Input event for handling user input
#[derive(Debug, Clone)]
pub enum InputEvent {
    Key(KeyEvent),
    Paste(String),
    Resize(u16, u16), // width, height
}

impl From<crossterm::event::KeyEvent> for KeyEvent {
    fn from(key_event: crossterm::event::KeyEvent) -> Self {
        Self {
            code: key_event.code.into(),
            modifiers: key_event.modifiers.into(),
        }
    }
}

impl From<crossterm::event::KeyCode> for KeyCode {
    fn from(key_code: crossterm::event::KeyCode) -> Self {
        match key_code {
            crossterm::event::KeyCode::Backspace => KeyCode::Backspace,
            crossterm::event::KeyCode::Enter => KeyCode::Enter,
            crossterm::event::KeyCode::Left => KeyCode::Left,
            crossterm::event::KeyCode::Right => KeyCode::Right,
            crossterm::event::KeyCode::Up => KeyCode::Up,
            crossterm::event::KeyCode::Down => KeyCode::Down,
            crossterm::event::KeyCode::Home => KeyCode::Home,
            crossterm::event::KeyCode::End => KeyCode::End,
            crossterm::event::KeyCode::PageUp => KeyCode::PageUp,
            crossterm::event::KeyCode::PageDown => KeyCode::PageDown,
            crossterm::event::KeyCode::Tab => KeyCode::Tab,
            crossterm::event::KeyCode::BackTab => KeyCode::BackTab,
            crossterm::event::KeyCode::Delete => KeyCode::Delete,
            crossterm::event::KeyCode::Insert => KeyCode::Insert,
            crossterm::event::KeyCode::F(n) => KeyCode::F(n),
            crossterm::event::KeyCode::Char(c) => KeyCode::Char(c),
            crossterm::event::KeyCode::Null => KeyCode::Null,
            crossterm::event::KeyCode::Esc => KeyCode::Esc,
            _ => KeyCode::Esc, // Default case for unhandled keys
        }
    }
}

impl From<crossterm::event::KeyModifiers> for KeyModifiers {
    fn from(modifiers: crossterm::event::KeyModifiers) -> Self {
        Self {
            shift: modifiers.contains(crossterm::event::KeyModifiers::SHIFT),
            control: modifiers.contains(crossterm::event::KeyModifiers::CONTROL),
            alt: modifiers.contains(crossterm::event::KeyModifiers::ALT),
        }
    }
}

impl SlashCommand {
    /// Parse a slash command from input string
    pub fn parse(input: &str) -> Self {
        if !input.starts_with('/') {
            return SlashCommand::Unknown(input.to_string());
        }

        let parts: Vec<&str> = input[1..].split_whitespace().collect();
        if parts.is_empty() {
            return SlashCommand::Unknown(input.to_string());
        }

        match parts[0] {
            "model" => {
                if parts.len() > 1 {
                    SlashCommand::Model(parts[1..].join(" "))
                } else {
                    SlashCommand::Unknown(input.to_string())
                }
            }
            "list-models" => SlashCommand::ListModels,
            "provider" => {
                if parts.len() > 1 {
                    SlashCommand::Provider(parts[1].to_string())
                } else {
                    SlashCommand::Unknown(input.to_string())
                }
            }
            "reset-context" => SlashCommand::ResetContext,
            "refresh-context" => SlashCommand::RefreshContext,
            "help" => SlashCommand::Help,
            "workdir" => {
                if parts.len() > 1 {
                    SlashCommand::WorkDir(parts[1..].join(" "))
                } else {
                    SlashCommand::Unknown(input.to_string())
                }
            }
            "history" => SlashCommand::History,
            "clear" => SlashCommand::Clear,
            "status" => SlashCommand::Status,
            "cancel" => SlashCommand::Cancel,
            "pause" => SlashCommand::Pause,
            "resume" => SlashCommand::Resume,
            _ => SlashCommand::Unknown(input.to_string()),
        }
    }

    /// Get the command description for help
    pub fn description(&self) -> &'static str {
        match self {
            SlashCommand::Model(_) => "Set the active LLM model",
            SlashCommand::ListModels => "List all available models",
            SlashCommand::Provider(_) => "Switch LLM provider",
            SlashCommand::ResetContext => "Reset and regenerate context",
            SlashCommand::RefreshContext => "Refresh context for modified files",
            SlashCommand::Help => "Show this help message",
            SlashCommand::WorkDir(_) => "Set working directory",
            SlashCommand::History => "Show command history",
            SlashCommand::Clear => "Clear the chat",
            SlashCommand::Status => "Show application status",
            SlashCommand::Cancel => "Cancel current execution",
            SlashCommand::Pause => "Pause current execution",
            SlashCommand::Resume => "Resume paused execution",
            SlashCommand::Unknown(_) => "Unknown command",
        }
    }
}

impl StatusUpdate {
    /// Create a new empty status update
    pub fn new() -> Self {
        Self {
            execution_state: None,
            active_provider: None,
            active_model: None,
            working_directory: None,
            context_files: None,
            memory_usage: None,
        }
    }

    /// Set execution state
    pub fn with_execution_state(mut self, state: String) -> Self {
        self.execution_state = Some(state);
        self
    }

    /// Set active provider
    pub fn with_provider(mut self, provider: String) -> Self {
        self.active_provider = Some(provider);
        self
    }

    /// Set active model
    pub fn with_model(mut self, model: String) -> Self {
        self.active_model = Some(model);
        self
    }

    /// Set working directory
    pub fn with_working_directory(mut self, workdir: String) -> Self {
        self.working_directory = Some(workdir);
        self
    }

    /// Set context files count
    pub fn with_context_files(mut self, count: usize) -> Self {
        self.context_files = Some(count);
        self
    }

    /// Set memory usage
    pub fn with_memory_usage(mut self, usage: u64) -> Self {
        self.memory_usage = Some(usage);
        self
    }
}

impl Default for StatusUpdate {
    fn default() -> Self {
        Self::new()
    }
}