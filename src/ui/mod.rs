//! User interface services with independent stateful components

pub mod services;
pub mod components;
pub mod events;
pub mod slash_commands;
pub mod file_browser;
pub mod clipboard;

pub use services::{InputBufferService, HistoryService, CompletionService, EditingMode, TextSelection};
pub use components::{ChatComponent, PlanComponent, StatusComponent, ApplicationStatus};
pub use events::{UiEvent, KeyEvent, InputEvent, SlashCommand};
pub use slash_commands::SlashCommandProcessor;
pub use file_browser::{FileBrowserComponent, FileEntry};
pub use clipboard::{ClipboardManager, copy_to_clipboard, paste_from_clipboard, clipboard_has_content};

use crate::utils::errors::KaiError;
use crate::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};
use std::io;
use tokio::sync::mpsc;

/// Main UI manager that coordinates all UI services
pub struct UiManager {
    /// Input buffer service
    input_buffer: InputBufferService,
    /// History service
    history: HistoryService,
    /// Completion service
    completion: CompletionService,
    /// File browser component
    file_browser: FileBrowserComponent,
    /// Slash command processor
    slash_processor: SlashCommandProcessor,
    /// Chat component for conversation history
    chat_component: ChatComponent,
    /// Plan component for plan visualization
    plan_component: PlanComponent,
    /// Status component for application status
    status_component: StatusComponent,
    /// Event channel sender
    event_sender: mpsc::UnboundedSender<UiEvent>,
    /// Event channel receiver
    event_receiver: mpsc::UnboundedReceiver<UiEvent>,
    /// Current working directory
    working_directory: std::path::PathBuf,
    /// Clipboard manager for copy/paste operations
    clipboard: ClipboardManager,
}

impl UiManager {
    /// Create a new UI manager
    pub fn new() -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        let working_directory = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

        Self {
            input_buffer: InputBufferService::new(),
            history: HistoryService::new(),
            completion: CompletionService::new(),
            file_browser: FileBrowserComponent::new(working_directory.clone()),
            slash_processor: SlashCommandProcessor::new(event_sender.clone()),
            chat_component: ChatComponent::new(),
            plan_component: PlanComponent::new(),
            status_component: StatusComponent::new(),
            event_sender,
            event_receiver,
            working_directory,
            clipboard: ClipboardManager::new(),
        }
    }

    /// Initialize the terminal UI
    pub fn init_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
        enable_raw_mode().map_err(|e| KaiError::ui(format!("Failed to enable raw mode: {}", e)))?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .map_err(|e| KaiError::ui(format!("Failed to setup terminal: {}", e)))?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)
            .map_err(|e| KaiError::ui(format!("Failed to create terminal: {}", e)))?;
        Ok(terminal)
    }

    /// Restore the terminal to normal mode
    pub fn restore_terminal() -> Result<()> {
        disable_raw_mode().map_err(|e| KaiError::ui(format!("Failed to disable raw mode: {}", e)))?;
        execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)
            .map_err(|e| KaiError::ui(format!("Failed to restore terminal: {}", e)))?;
        Ok(())
    }

    /// Run the main UI event loop
    pub async fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            // Handle terminal events
            if event::poll(std::time::Duration::from_millis(100))
                .map_err(|e| KaiError::ui(format!("Failed to poll events: {}", e)))? {
                
                if let Ok(event) = event::read() {
                    match event {
                        Event::Key(key) => {
                            if let Some(ui_event) = self.handle_key_event(key).await? {
                                // Send event to application
                                if self.event_sender.send(ui_event).is_err() {
                                    break; // Channel closed
                                }
                            }
                        }
                        Event::Resize(_, _) => {
                            // Handle terminal resize
                        }
                        _ => {}
                    }
                }
            }

            // Handle UI events from the application
            while let Ok(ui_event) = self.event_receiver.try_recv() {
                self.handle_ui_event(ui_event).await?;
            }

            // Render the UI
            terminal.draw(|f| {
                self.render(f);
            }).map_err(|e| KaiError::ui(format!("Failed to draw UI: {}", e)))?;
        }

        Ok(())
    }

    /// Handle a key event
    async fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) -> Result<Option<UiEvent>> {
        // Handle file browser input first
        if self.file_browser.is_visible() {
            match key.code {
                KeyCode::Esc => {
                    self.file_browser.hide();
                    return Ok(None);
                }
                KeyCode::Up => {
                    self.file_browser.select_previous();
                    return Ok(None);
                }
                KeyCode::Down => {
                    self.file_browser.select_next();
                    return Ok(None);
                }
                KeyCode::Enter => {
                    if let Some(selected_file) = self.file_browser.get_selected_file() {
                        let file_path = format!("@{}", selected_file.to_string_lossy());
                        self.input_buffer.apply_completion(&file_path);
                        self.file_browser.hide();
                    }
                    return Ok(None);
                }
                _ => {}
            }
        }

        match key.code {
            KeyCode::Esc => {
                // Hide file browser if visible, otherwise exit
                if self.file_browser.is_visible() {
                    self.file_browser.hide();
                    return Ok(None);
                }
                return Ok(Some(UiEvent::Quit));
            }
            KeyCode::Enter => {
                // Submit current input
                let input = self.input_buffer.get_content();
                if !input.trim().is_empty() {
                    self.history.add_entry(input.clone()).await?;
                    
                    // Check if it's a slash command
                    if input.trim().starts_with('/') {
                        let command = SlashCommand::parse(&input.trim());
                        self.slash_processor.process_command(command).await?;
                        self.input_buffer.clear();
                        return Ok(None);
                    } else {
                        self.input_buffer.clear();
                        return Ok(Some(UiEvent::SubmitPrompt(input)));
                    }
                }
            }
            KeyCode::Tab => {
                // Handle completion
                if let Some(completion) = self.completion.get_active_completion() {
                    self.input_buffer.apply_completion(&completion);
                }
            }
            KeyCode::Up => {
                // Navigate history up
                if let Some(entry) = self.history.navigate_up() {
                    self.input_buffer.set_content(entry);
                }
            }
            KeyCode::Down => {
                // Navigate history down
                if let Some(entry) = self.history.navigate_down() {
                    self.input_buffer.set_content(entry);
                }
            }
            KeyCode::Char(c) => {
                // Handle character input
                self.input_buffer.insert_char(c);
                
                let content = self.input_buffer.get_content();
                let cursor_pos = self.input_buffer.cursor_position();
                
                // Check for special character triggers
                if content.contains('@') {
                    // Trigger file browser
                    let lines: Vec<&str> = content.lines().collect();
                    if cursor_pos.0 < lines.len() {
                        let current_line = lines[cursor_pos.0];
                        if let Some(at_pos) = current_line.rfind('@') {
                            let query = &current_line[at_pos..];
                            self.file_browser.trigger_browser(query).await?;
                        }
                    }
                }
                
                // Update completions
                self.completion.update_suggestions(&content, cursor_pos).await?;
            }
            KeyCode::Backspace => {
                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                    self.input_buffer.delete_word_backward();
                } else {
                    self.input_buffer.delete_backward();
                }
            }
            KeyCode::Delete => {
                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                    self.input_buffer.delete_word_forward();
                } else {
                    self.input_buffer.delete_forward();
                }
            }
            KeyCode::Left => {
                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                    self.input_buffer.move_cursor_word_backward();
                } else {
                    self.input_buffer.move_cursor_left();
                }
            }
            KeyCode::Right => {
                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                    self.input_buffer.move_cursor_word_forward();
                } else {
                    self.input_buffer.move_cursor_right();
                }
            }
            KeyCode::Home => {
                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                    self.input_buffer.move_cursor_to_document_start();
                } else {
                    self.input_buffer.move_cursor_to_line_start();
                }
            }
            KeyCode::End => {
                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                    self.input_buffer.move_cursor_to_document_end();
                } else {
                    self.input_buffer.move_cursor_to_line_end();
                }
            }
            // Additional Vim-like keybindings with Ctrl modifier
            KeyCode::Char('a') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                self.input_buffer.move_cursor_to_line_start();
            }
            KeyCode::Char('e') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                self.input_buffer.move_cursor_to_line_end();
            }
            KeyCode::Char('u') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                self.input_buffer.delete_to_line_start();
            }
            KeyCode::Char('k') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                self.input_buffer.delete_to_line_end();
            }
            KeyCode::Char('w') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                self.input_buffer.delete_word_backward();
            }
            KeyCode::Char('z') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                self.input_buffer.undo();
            }
            KeyCode::Char('y') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                self.input_buffer.redo();
            }
            KeyCode::Char('c') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                // Copy selected text or current line
                if self.input_buffer.get_selection().active {
                    let selection = self.input_buffer.get_selection();
                    let selected_text = self.input_buffer.get_selected_text(selection.start, selection.end);
                    if let Err(e) = copy_to_clipboard(&selected_text).await {
                        tracing::warn!("Failed to copy to clipboard: {}", e);
                    }
                } else {
                    // Copy current line if no selection
                    let content = self.input_buffer.get_content();
                    let cursor_pos = self.input_buffer.cursor_position();
                    let lines: Vec<&str> = content.lines().collect();
                    if cursor_pos.0 < lines.len() {
                        let current_line = lines[cursor_pos.0];
                        if let Err(e) = copy_to_clipboard(current_line).await {
                            tracing::warn!("Failed to copy to clipboard: {}", e);
                        }
                    }
                }
            }
            KeyCode::Char('v') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                // Paste from clipboard
                match paste_from_clipboard().await {
                    Ok(text) => {
                        self.input_buffer.insert_string(&text);
                        
                        // Update completions after paste
                        let content = self.input_buffer.get_content();
                        let cursor_pos = self.input_buffer.cursor_position();
                        if let Err(e) = self.completion.update_suggestions(&content, cursor_pos).await {
                            tracing::warn!("Failed to update completions after paste: {}", e);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to paste from clipboard: {}", e);
                    }
                }
            }
            KeyCode::Char('x') if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                // Cut selected text or current line
                if self.input_buffer.get_selection().active {
                    let selection = self.input_buffer.get_selection();
                    let selected_text = self.input_buffer.get_selected_text(selection.start, selection.end);
                    if let Err(e) = copy_to_clipboard(&selected_text).await {
                        tracing::warn!("Failed to copy to clipboard: {}", e);
                    }
                    // TODO: Delete the selected text
                } else {
                    // Cut current line if no selection
                    let content = self.input_buffer.get_content();
                    let cursor_pos = self.input_buffer.cursor_position();
                    let lines: Vec<&str> = content.lines().collect();
                    if cursor_pos.0 < lines.len() {
                        let current_line = lines[cursor_pos.0];
                        if let Err(e) = copy_to_clipboard(current_line).await {
                            tracing::warn!("Failed to copy to clipboard: {}", e);
                        }
                        // Delete the current line
                        self.input_buffer.delete_to_line_start();
                        self.input_buffer.delete_to_line_end();
                        self.input_buffer.delete_forward(); // Delete the newline if any
                    }
                }
            }
            _ => {}
        }

        Ok(None)
    }

    /// Handle a UI event from the application
    async fn handle_ui_event(&mut self, event: UiEvent) -> Result<()> {
        match event {
            UiEvent::PlanUpdated(plan) => {
                // Update plan visualization in real-time
                self.plan_component.set_plan(Some(plan));
                self.chat_component.add_message(
                    crate::ui::components::MessageRole::System,
                    "Plan updated and ready for execution".to_string(),
                );
            }
            UiEvent::TaskStarted(task_started) => {
                // Show task start notification
                self.chat_component.add_message(
                    crate::ui::components::MessageRole::System,
                    format!("🏃 Started: {}", task_started.task_description),
                );
            }
            UiEvent::TaskCompleted(task_completed) => {
                // Show task completion with status
                let status_icon = if task_completed.result.success { "✅" } else { "❌" };
                self.chat_component.add_message(
                    crate::ui::components::MessageRole::System,
                    format!("{} Completed: {}", status_icon, task_completed.task_description),
                );
            }
            UiEvent::TaskFailed(task_failed) => {
                // Show task failure
                self.chat_component.add_message(
                    crate::ui::components::MessageRole::System,
                    format!("❌ Failed: {} - {}", task_failed.task_description, task_failed.error),
                );
            }
            UiEvent::ExecutionStateChanged(state) => {
                // Update status component with new execution state
                let mut status = ApplicationStatus::default();
                status.execution_state = state.clone();
                self.status_component.update_status(status);
                
                self.chat_component.add_message(
                    crate::ui::components::MessageRole::System,
                    format!("Execution state: {}", state),
                );
            }
            UiEvent::ProviderChanged(provider, model) => {
                // Update status with new provider/model
                let mut status = ApplicationStatus::default();
                status.active_provider = provider.clone();
                status.active_model = model.clone();
                self.status_component.update_status(status);
                
                self.chat_component.add_message(
                    crate::ui::components::MessageRole::System,
                    format!("Switched to {} / {}", provider, model),
                );
            }
            UiEvent::WorkingDirectoryChanged(workdir) => {
                // Update working directory across components
                self.working_directory = std::path::PathBuf::from(&workdir);
                self.file_browser.set_working_directory(self.working_directory.clone()).await?;
                
                let mut status = ApplicationStatus::default();
                status.working_directory = workdir.clone();
                self.status_component.update_status(status);
                
                self.chat_component.add_message(
                    crate::ui::components::MessageRole::System,
                    format!("Working directory: {}", workdir),
                );
            }
            UiEvent::ContextRefreshed => {
                // Notify about context refresh
                self.chat_component.add_message(
                    crate::ui::components::MessageRole::System,
                    "Context refreshed successfully".to_string(),
                );
            }
            UiEvent::StatusUpdate(status_update) => {
                // Update application status
                let mut status = ApplicationStatus::default();
                
                if let Some(exec_state) = status_update.execution_state {
                    status.execution_state = exec_state;
                }
                if let Some(provider) = status_update.active_provider {
                    status.active_provider = provider;
                }
                if let Some(model) = status_update.active_model {
                    status.active_model = model;
                }
                if let Some(workdir) = status_update.working_directory {
                    status.working_directory = workdir;
                }
                if let Some(files) = status_update.context_files {
                    status.context_files = files;
                }
                if let Some(memory) = status_update.memory_usage {
                    status.memory_usage = Some(memory);
                }
                
                self.status_component.update_status(status);
            }
            UiEvent::Error(error) => {
                // Handle error display
                tracing::error!("UI Error: {}", error);
                self.chat_component.add_message(
                    crate::ui::components::MessageRole::System,
                    format!("❌ Error: {}", error),
                );
            }
            UiEvent::SubmitPrompt(prompt) => {
                // Add user prompt to chat history
                self.chat_component.add_message(
                    crate::ui::components::MessageRole::User,
                    prompt,
                );
            }
            _ => {}
        }

        Ok(())
    }

    /// Render the UI
    fn render(&self, f: &mut ratatui::Frame) {
        // Main layout: split between chat/plan area and bottom panels
        let main_chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .margin(1)
            .constraints([
                ratatui::layout::Constraint::Min(1),      // Main content area
                ratatui::layout::Constraint::Length(3),   // Input area
                ratatui::layout::Constraint::Length(1),   // Status area
            ])
            .split(f.area());

        // Split main content area between chat and plan
        let content_chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Percentage(60), // Chat area
                ratatui::layout::Constraint::Percentage(40), // Plan area
            ])
            .split(main_chunks[0]);

        // Render chat component
        self.chat_component.render(f, content_chunks[0]);

        // Render plan component
        self.plan_component.render(f, content_chunks[1]);

        // Render input area
        self.render_input_area(f, main_chunks[1]);

        // Render status component
        self.status_component.render(f, main_chunks[2]);

        // Render file browser overlay if visible
        if self.file_browser.is_visible() {
            self.file_browser.render(f, f.area());
        }
    }

    /// Render the input area
    fn render_input_area(&self, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        use ratatui::widgets::{Block, Borders, Paragraph};
        use ratatui::text::Span;

        let input_content = self.input_buffer.get_content();
        let cursor_pos = self.input_buffer.cursor_position();

        let input = Paragraph::new(Span::raw(&input_content))
            .block(Block::default().borders(Borders::ALL).title("Input"));

        f.render_widget(input, area);

        // Render cursor
        if cursor_pos.1 < area.width as usize - 2 {
            f.set_cursor_position((
                area.x + cursor_pos.1 as u16 + 1,
                area.y + cursor_pos.0 as u16 + 1,
            ));
        }

        // Render completions if available
        if let Some(completions) = self.completion.get_suggestions() {
            if !completions.is_empty() {
                // Render completion popup
                self.render_completion_popup(f, area, completions);
            }
        }
    }

    /// Render completion popup
    fn render_completion_popup(
        &self,
        f: &mut ratatui::Frame,
        input_area: ratatui::layout::Rect,
        completions: &[String],
    ) {
        use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
        use ratatui::style::{Color, Modifier, Style};

        let items: Vec<ListItem> = completions
            .iter()
            .enumerate()
            .map(|(i, completion)| {
                let style = if Some(i) == self.completion.get_active_index() {
                    Style::default().bg(Color::Blue).fg(Color::White)
                } else {
                    Style::default()
                };
                ListItem::new(completion.as_str()).style(style)
            })
            .collect();

        let completions_height = completions.len().min(10) as u16 + 2; // +2 for borders
        let completion_area = ratatui::layout::Rect {
            x: input_area.x,
            y: input_area.y.saturating_sub(completions_height),
            width: input_area.width,
            height: completions_height,
        };

        let completion_list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Completions"));

        f.render_widget(completion_list, completion_area);
    }

    /// Get access to the input buffer service
    pub fn input_buffer(&mut self) -> &mut InputBufferService {
        &mut self.input_buffer
    }

    /// Get access to the history service
    pub fn history(&mut self) -> &mut HistoryService {
        &mut self.history
    }

    /// Get access to the completion service
    pub fn completion(&mut self) -> &mut CompletionService {
        &mut self.completion
    }

    /// Get the event sender for external components
    pub fn event_sender(&self) -> mpsc::UnboundedSender<UiEvent> {
        self.event_sender.clone()
    }

    /// Add a message to the chat component
    pub fn add_chat_message(&mut self, role: crate::ui::components::MessageRole, content: String) {
        self.chat_component.add_message(role, content);
    }

    /// Update the current plan
    pub fn update_plan(&mut self, plan: crate::planning::Plan) {
        self.plan_component.set_plan(Some(plan));
    }

    /// Clear the current plan
    pub fn clear_plan(&mut self) {
        self.plan_component.set_plan(None);
    }

    /// Update application status
    pub fn update_status(&mut self, status: ApplicationStatus) {
        self.status_component.update_status(status);
    }

    /// Get current working directory
    pub fn get_working_directory(&self) -> &std::path::PathBuf {
        &self.working_directory
    }

    /// Set working directory and update all components
    pub async fn set_working_directory(&mut self, workdir: std::path::PathBuf) -> Result<()> {
        self.working_directory = workdir.clone();
        self.file_browser.set_working_directory(workdir).await?;
        Ok(())
    }

    /// Load history from file
    pub async fn load_history<P: AsRef<std::path::Path>>(&mut self, path: P) -> Result<()> {
        self.history.load_from_file(path).await
    }

    /// Save history to file
    pub async fn save_history<P: AsRef<std::path::Path>>(&self, path: P) -> Result<()> {
        self.history.save_to_file(path).await
    }

    /// Check if any special overlays are active
    pub fn has_active_overlay(&self) -> bool {
        self.file_browser.is_visible() || !self.completion.get_suggestions().unwrap_or(&[]).is_empty()
    }
}

impl Default for UiManager {
    fn default() -> Self {
        Self::new()
    }
}