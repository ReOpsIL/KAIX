//! UI components for rendering different parts of the interface

use crate::planning::{Plan, Task, TaskStatus};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Spans, Text},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Wrap},
    Frame,
};

/// Chat component for displaying conversation history
pub struct ChatComponent {
    messages: Vec<ChatMessage>,
}

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

impl ChatComponent {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    pub fn add_message(&mut self, role: MessageRole, content: String) {
        self.messages.push(ChatMessage {
            role,
            content,
            timestamp: chrono::Utc::now(),
        });
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self.messages
            .iter()
            .map(|msg| {
                let style = match msg.role {
                    MessageRole::User => Style::default().fg(Color::Blue),
                    MessageRole::Assistant => Style::default().fg(Color::Green),
                    MessageRole::System => Style::default().fg(Color::Yellow),
                };

                let role_prefix = match msg.role {
                    MessageRole::User => "You: ",
                    MessageRole::Assistant => "AI: ",
                    MessageRole::System => "System: ",
                };

                let content = format!("{}{}", role_prefix, msg.content);
                ListItem::new(Spans::from(vec![Span::styled(content, style)]))
            })
            .collect();

        let chat_list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Chat"))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));

        f.render_widget(chat_list, area);
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }

    pub fn get_messages(&self) -> &[ChatMessage] {
        &self.messages
    }
}

impl Default for ChatComponent {
    fn default() -> Self {
        Self::new()
    }
}

/// Plan component for displaying current plan and task status
pub struct PlanComponent {
    current_plan: Option<Plan>,
}

impl PlanComponent {
    pub fn new() -> Self {
        Self {
            current_plan: None,
        }
    }

    pub fn set_plan(&mut self, plan: Option<Plan>) {
        self.current_plan = plan;
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        if let Some(plan) = &self.current_plan {
            self.render_plan(f, area, plan);
        } else {
            self.render_no_plan(f, area);
        }
    }

    fn render_plan(&self, f: &mut Frame, area: Rect, plan: &Plan) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Plan info
                Constraint::Min(1),    // Task list
            ])
            .split(area);

        // Plan info
        let plan_info = Paragraph::new(vec![
            Spans::from(vec![
                Span::styled("Plan: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&plan.description),
            ]),
            Spans::from(vec![
                Span::styled("Status: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{:?}", plan.status),
                    self.get_status_style(&plan.status),
                ),
            ]),
        ])
        .block(Block::default().borders(Borders::ALL).title("Current Plan"))
        .wrap(Wrap { trim: true });

        f.render_widget(plan_info, chunks[0]);

        // Task list
        self.render_task_list(f, chunks[1], &plan.tasks);
    }

    fn render_task_list(&self, f: &mut Frame, area: Rect, tasks: &[Task]) {
        let items: Vec<ListItem> = tasks
            .iter()
            .enumerate()
            .map(|(i, task)| {
                let status_symbol = match task.status {
                    TaskStatus::Pending => "⏸",
                    TaskStatus::Ready => "▶",
                    TaskStatus::InProgress => "⏳",
                    TaskStatus::Completed => "✅",
                    TaskStatus::Failed => "❌",
                    TaskStatus::Skipped => "⏭",
                };

                let style = match task.status {
                    TaskStatus::Pending => Style::default().fg(Color::Gray),
                    TaskStatus::Ready => Style::default().fg(Color::Yellow),
                    TaskStatus::InProgress => Style::default().fg(Color::Blue),
                    TaskStatus::Completed => Style::default().fg(Color::Green),
                    TaskStatus::Failed => Style::default().fg(Color::Red),
                    TaskStatus::Skipped => Style::default().fg(Color::DarkGray),
                };

                let content = format!("{} {}. {}", status_symbol, i + 1, task.description);
                ListItem::new(Spans::from(vec![Span::styled(content, style)]))
            })
            .collect();

        let task_list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Tasks"))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));

        f.render_widget(task_list, area);
    }

    fn render_no_plan(&self, f: &mut Frame, area: Rect) {
        let no_plan = Paragraph::new("No active plan")
            .block(Block::default().borders(Borders::ALL).title("Current Plan"))
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Gray));

        f.render_widget(no_plan, area);
    }

    fn get_status_style(&self, status: &crate::planning::PlanStatus) -> Style {
        match status {
            crate::planning::PlanStatus::Ready => Style::default().fg(Color::Yellow),
            crate::planning::PlanStatus::Executing => Style::default().fg(Color::Blue),
            crate::planning::PlanStatus::Paused => Style::default().fg(Color::Magenta),
            crate::planning::PlanStatus::Completed => Style::default().fg(Color::Green),
            crate::planning::PlanStatus::Failed => Style::default().fg(Color::Red),
            crate::planning::PlanStatus::Cancelled => Style::default().fg(Color::DarkGray),
        }
    }
}

impl Default for PlanComponent {
    fn default() -> Self {
        Self::new()
    }
}

/// Status component for showing application status
pub struct StatusComponent {
    status: ApplicationStatus,
}

#[derive(Debug, Clone)]
pub struct ApplicationStatus {
    pub execution_state: String,
    pub active_provider: String,
    pub active_model: String,
    pub working_directory: String,
    pub context_files: usize,
    pub memory_usage: Option<u64>,
}

impl StatusComponent {
    pub fn new() -> Self {
        Self {
            status: ApplicationStatus::default(),
        }
    }

    pub fn update_status(&mut self, status: ApplicationStatus) {
        self.status = status;
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(20), // Execution state
                Constraint::Length(25), // Provider/model
                Constraint::Length(30), // Working directory
                Constraint::Min(1),     // Context info
            ])
            .split(area);

        // Execution state
        let execution_state = Paragraph::new(format!("State: {}", self.status.execution_state))
            .style(self.get_execution_state_style());
        f.render_widget(execution_state, chunks[0]);

        // Provider and model
        let provider_info = Paragraph::new(format!(
            "Provider: {}/{}",
            self.status.active_provider, self.status.active_model
        ))
        .style(Style::default().fg(Color::Cyan));
        f.render_widget(provider_info, chunks[1]);

        // Working directory
        let workdir = Paragraph::new(format!("Dir: {}", self.status.working_directory))
            .style(Style::default().fg(Color::White));
        f.render_widget(workdir, chunks[2]);

        // Context and memory info
        let mut info_text = format!("Files: {}", self.status.context_files);
        if let Some(memory) = self.status.memory_usage {
            info_text.push_str(&format!(" | Mem: {}MB", memory / 1024 / 1024));
        }
        
        let context_info = Paragraph::new(info_text)
            .style(Style::default().fg(Color::Green))
            .alignment(Alignment::Right);
        f.render_widget(context_info, chunks[3]);
    }

    fn get_execution_state_style(&self) -> Style {
        match self.status.execution_state.as_str() {
            "Idle" => Style::default().fg(Color::Gray),
            "Planning" => Style::default().fg(Color::Yellow),
            "Executing" => Style::default().fg(Color::Blue),
            "Paused" => Style::default().fg(Color::Magenta),
            "Cancelled" => Style::default().fg(Color::Red),
            _ => Style::default().fg(Color::White),
        }
    }
}

impl Default for StatusComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for ApplicationStatus {
    fn default() -> Self {
        Self {
            execution_state: "Idle".to_string(),
            active_provider: "None".to_string(),
            active_model: "None".to_string(),
            working_directory: ".".to_string(),
            context_files: 0,
            memory_usage: None,
        }
    }
}

/// Progress component for showing task progress
pub struct ProgressComponent {
    current_task: Option<String>,
    progress: f64, // 0.0 to 1.0
}

impl ProgressComponent {
    pub fn new() -> Self {
        Self {
            current_task: None,
            progress: 0.0,
        }
    }

    pub fn set_task(&mut self, task: Option<String>) {
        self.current_task = task;
    }

    pub fn set_progress(&mut self, progress: f64) {
        self.progress = progress.max(0.0).min(1.0);
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        if let Some(task) = &self.current_task {
            let progress_bar = Gauge::default()
                .block(Block::default().borders(Borders::ALL).title("Progress"))
                .gauge_style(Style::default().fg(Color::Blue))
                .percent((self.progress * 100.0) as u16)
                .label(task.as_str());

            f.render_widget(progress_bar, area);
        }
    }
}

impl Default for ProgressComponent {
    fn default() -> Self {
        Self::new()
    }
}