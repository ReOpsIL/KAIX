//! User interface services with independent stateful components

pub mod services;
pub mod components;
pub mod events;

pub use services::{InputBufferService, HistoryService, CompletionService};
pub use components::{ChatComponent, PlanComponent, StatusComponent};
pub use events::{UiEvent, KeyEvent, InputEvent};

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
    /// Event channel sender
    event_sender: mpsc::UnboundedSender<UiEvent>,
    /// Event channel receiver
    event_receiver: mpsc::UnboundedReceiver<UiEvent>,
}

impl UiManager {
    /// Create a new UI manager
    pub fn new() -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();

        Self {
            input_buffer: InputBufferService::new(),
            history: HistoryService::new(),
            completion: CompletionService::new(),
            event_sender,
            event_receiver,
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
        match key.code {
            KeyCode::Esc => {
                // Exit application
                return Ok(Some(UiEvent::Quit));
            }
            KeyCode::Enter => {
                // Submit current input
                let input = self.input_buffer.get_content();
                if !input.trim().is_empty() {
                    self.history.add_entry(input.clone()).await?;
                    self.input_buffer.clear();
                    return Ok(Some(UiEvent::SubmitPrompt(input)));
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
                
                // Update completions
                let cursor_pos = self.input_buffer.cursor_position();
                let content = self.input_buffer.get_content();
                self.completion.update_suggestions(&content, cursor_pos).await?;
            }
            KeyCode::Backspace => {
                self.input_buffer.delete_backward();
            }
            KeyCode::Delete => {
                self.input_buffer.delete_forward();
            }
            KeyCode::Left => {
                self.input_buffer.move_cursor_left();
            }
            KeyCode::Right => {
                self.input_buffer.move_cursor_right();
            }
            _ => {}
        }

        Ok(None)
    }

    /// Handle a UI event from the application
    async fn handle_ui_event(&mut self, event: UiEvent) -> Result<()> {
        match event {
            UiEvent::PlanUpdated(_) => {
                // Handle plan updates
            }
            UiEvent::TaskCompleted(_) => {
                // Handle task completion
            }
            UiEvent::Error(error) => {
                // Handle error display
                tracing::error!("UI Error: {}", error);
            }
            _ => {}
        }

        Ok(())
    }

    /// Render the UI
    fn render(&self, f: &mut ratatui::Frame) {
        let chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .margin(1)
            .constraints([
                ratatui::layout::Constraint::Min(1),      // Chat area
                ratatui::layout::Constraint::Length(3),   // Input area
                ratatui::layout::Constraint::Length(1),   // Status area
            ])
            .split(f.size());

        // Render chat component
        let chat_component = ChatComponent::new();
        chat_component.render(f, chunks[0]);

        // Render input component
        self.render_input_area(f, chunks[1]);

        // Render status component
        let status_component = StatusComponent::new();
        status_component.render(f, chunks[2]);
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
            f.set_cursor(
                area.x + cursor_pos.1 as u16 + 1,
                area.y + cursor_pos.0 as u16 + 1,
            );
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
}

impl Default for UiManager {
    fn default() -> Self {
        Self::new()
    }
}