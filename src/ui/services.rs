//! Independent stateful UI services

use crate::utils::errors::KaiError;
use crate::Result;
use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
use std::collections::VecDeque;
use std::path::Path;
use tokio::fs;

/// Input buffer service managing multi-line text input
#[derive(Debug)]
pub struct InputBufferService {
    /// Text content as lines
    lines: Vec<String>,
    /// Current cursor position (row, column)
    cursor: (usize, usize),
    /// Maximum history size
    max_lines: usize,
}

impl InputBufferService {
    /// Create a new input buffer service
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor: (0, 0),
            max_lines: 1000,
        }
    }

    /// Insert a character at the cursor position
    pub fn insert_char(&mut self, c: char) {
        if self.cursor.0 >= self.lines.len() {
            self.lines.push(String::new());
        }

        let line = &mut self.lines[self.cursor.0];
        if self.cursor.1 <= line.len() {
            line.insert(self.cursor.1, c);
            self.cursor.1 += 1;
        }
    }

    /// Insert a string at the cursor position
    pub fn insert_string(&mut self, s: &str) {
        for c in s.chars() {
            if c == '\n' {
                self.handle_newline();
            } else {
                self.insert_char(c);
            }
        }
    }

    /// Handle newline insertion
    pub fn handle_newline(&mut self) {
        if self.cursor.0 >= self.lines.len() {
            self.lines.push(String::new());
        }

        let line = &self.lines[self.cursor.0];
        let remaining = line[self.cursor.1..].to_string();
        
        // Truncate current line
        self.lines[self.cursor.0].truncate(self.cursor.1);
        
        // Insert new line
        self.cursor.0 += 1;
        self.cursor.1 = 0;
        self.lines.insert(self.cursor.0, remaining);

        // Limit total lines
        if self.lines.len() > self.max_lines {
            self.lines.remove(0);
            if self.cursor.0 > 0 {
                self.cursor.0 -= 1;
            }
        }
    }

    /// Delete character backward (backspace)
    pub fn delete_backward(&mut self) {
        if self.cursor.1 > 0 {
            // Delete character in current line
            self.lines[self.cursor.0].remove(self.cursor.1 - 1);
            self.cursor.1 -= 1;
        } else if self.cursor.0 > 0 {
            // Merge with previous line
            let current_line = self.lines.remove(self.cursor.0);
            self.cursor.0 -= 1;
            self.cursor.1 = self.lines[self.cursor.0].len();
            self.lines[self.cursor.0].push_str(&current_line);
        }
    }

    /// Delete character forward (delete key)
    pub fn delete_forward(&mut self) {
        if self.cursor.0 < self.lines.len() {
            let line = &mut self.lines[self.cursor.0];
            if self.cursor.1 < line.len() {
                line.remove(self.cursor.1);
            } else if self.cursor.0 + 1 < self.lines.len() {
                // Merge with next line
                let next_line = self.lines.remove(self.cursor.0 + 1);
                self.lines[self.cursor.0].push_str(&next_line);
            }
        }
    }

    /// Move cursor left
    pub fn move_cursor_left(&mut self) {
        if self.cursor.1 > 0 {
            self.cursor.1 -= 1;
        } else if self.cursor.0 > 0 {
            self.cursor.0 -= 1;
            self.cursor.1 = self.lines[self.cursor.0].len();
        }
    }

    /// Move cursor right
    pub fn move_cursor_right(&mut self) {
        if self.cursor.0 < self.lines.len() {
            let line_len = self.lines[self.cursor.0].len();
            if self.cursor.1 < line_len {
                self.cursor.1 += 1;
            } else if self.cursor.0 + 1 < self.lines.len() {
                self.cursor.0 += 1;
                self.cursor.1 = 0;
            }
        }
    }

    /// Move cursor up
    pub fn move_cursor_up(&mut self) {
        if self.cursor.0 > 0 {
            self.cursor.0 -= 1;
            let line_len = self.lines[self.cursor.0].len();
            if self.cursor.1 > line_len {
                self.cursor.1 = line_len;
            }
        }
    }

    /// Move cursor down
    pub fn move_cursor_down(&mut self) {
        if self.cursor.0 + 1 < self.lines.len() {
            self.cursor.0 += 1;
            let line_len = self.lines[self.cursor.0].len();
            if self.cursor.1 > line_len {
                self.cursor.1 = line_len;
            }
        }
    }

    /// Move cursor to beginning of line
    pub fn move_cursor_to_line_start(&mut self) {
        self.cursor.1 = 0;
    }

    /// Move cursor to end of line
    pub fn move_cursor_to_line_end(&mut self) {
        if self.cursor.0 < self.lines.len() {
            self.cursor.1 = self.lines[self.cursor.0].len();
        }
    }

    /// Get the current content as a single string
    pub fn get_content(&self) -> String {
        self.lines.join("\n")
    }

    /// Set the content, replacing all current content
    pub fn set_content(&mut self, content: String) {
        self.lines = content.lines().map(|s| s.to_string()).collect();
        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.cursor = (0, 0);
    }

    /// Clear all content
    pub fn clear(&mut self) {
        self.lines = vec![String::new()];
        self.cursor = (0, 0);
    }

    /// Get the current cursor position
    pub fn cursor_position(&self) -> (usize, usize) {
        self.cursor
    }

    /// Apply a completion at the cursor position
    pub fn apply_completion(&mut self, completion: &str) {
        // Find the start of the current word
        let line = &self.lines[self.cursor.0];
        let mut word_start = self.cursor.1;
        
        while word_start > 0 {
            let c = line.chars().nth(word_start - 1).unwrap_or(' ');
            if c.is_whitespace() || c == '@' || c == '/' {
                break;
            }
            word_start -= 1;
        }

        // Replace the partial word with the completion
        let before = &line[..word_start];
        let after = &line[self.cursor.1..];
        let new_line = format!("{}{}{}", before, completion, after);
        
        self.lines[self.cursor.0] = new_line;
        self.cursor.1 = word_start + completion.len();
    }

    /// Get the current word under the cursor
    pub fn get_current_word(&self) -> String {
        if self.cursor.0 >= self.lines.len() {
            return String::new();
        }

        let line = &self.lines[self.cursor.0];
        let mut word_start = self.cursor.1;
        let mut word_end = self.cursor.1;

        // Find word boundaries
        while word_start > 0 {
            let c = line.chars().nth(word_start - 1).unwrap_or(' ');
            if c.is_whitespace() {
                break;
            }
            word_start -= 1;
        }

        while word_end < line.len() {
            let c = line.chars().nth(word_end).unwrap_or(' ');
            if c.is_whitespace() {
                break;
            }
            word_end += 1;
        }

        line[word_start..word_end].to_string()
    }
}

impl Default for InputBufferService {
    fn default() -> Self {
        Self::new()
    }
}

/// History service for managing command history
#[derive(Debug)]
pub struct HistoryService {
    /// History entries (most recent last)
    entries: VecDeque<String>,
    /// Current position in history navigation (-1 means current input)
    position: isize,
    /// Maximum number of entries to keep
    max_entries: usize,
    /// Current input being edited (not yet in history)
    current_input: String,
}

impl HistoryService {
    /// Create a new history service
    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            position: -1,
            max_entries: 1000,
            current_input: String::new(),
        }
    }

    /// Add an entry to the history
    pub async fn add_entry(&mut self, entry: String) -> Result<()> {
        if entry.trim().is_empty() {
            return Ok(());
        }

        // Don't add duplicate consecutive entries
        if let Some(last) = self.entries.back() {
            if last == &entry {
                return Ok(());
            }
        }

        self.entries.push_back(entry);

        // Limit size
        if self.entries.len() > self.max_entries {
            self.entries.pop_front();
        }

        // Reset position
        self.position = -1;
        self.current_input.clear();

        Ok(())
    }

    /// Navigate up in history (to older entries)
    pub fn navigate_up(&mut self) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }

        if self.position == -1 {
            // First time navigating up, save current input
            self.position = self.entries.len() as isize - 1;
        } else if self.position > 0 {
            self.position -= 1;
        }

        if self.position >= 0 && (self.position as usize) < self.entries.len() {
            Some(self.entries[self.position as usize].clone())
        } else {
            None
        }
    }

    /// Navigate down in history (to newer entries)
    pub fn navigate_down(&mut self) -> Option<String> {
        if self.position == -1 {
            return None; // Already at current input
        }

        self.position += 1;

        if self.position >= self.entries.len() as isize {
            // Back to current input
            self.position = -1;
            Some(self.current_input.clone())
        } else {
            Some(self.entries[self.position as usize].clone())
        }
    }

    /// Search history with a query
    pub fn search(&self, query: &str) -> Vec<String> {
        if query.is_empty() {
            return Vec::new();
        }

        let matcher = SkimMatcherV2::default();
        let mut matches: Vec<(i64, String)> = self.entries
            .iter()
            .filter_map(|entry| {
                matcher.fuzzy_match(entry, query).map(|score| (score, entry.clone()))
            })
            .collect();

        // Sort by score (highest first)
        matches.sort_by(|a, b| b.0.cmp(&a.0));

        matches.into_iter().map(|(_, entry)| entry).take(10).collect()
    }

    /// Get all entries
    pub fn get_entries(&self) -> Vec<String> {
        self.entries.iter().cloned().collect()
    }

    /// Clear all history
    pub fn clear(&mut self) {
        self.entries.clear();
        self.position = -1;
        self.current_input.clear();
    }

    /// Load history from file
    pub async fn load_from_file<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        match fs::read_to_string(path).await {
            Ok(content) => {
                for line in content.lines() {
                    let entry = line.trim();
                    if !entry.is_empty() {
                        self.entries.push_back(entry.to_string());
                    }
                }

                // Limit size
                while self.entries.len() > self.max_entries {
                    self.entries.pop_front();
                }

                Ok(())
            }
            Err(_) => Ok(()), // File doesn't exist yet, that's fine
        }
    }

    /// Save history to file
    pub async fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = self.entries.iter().cloned().collect::<Vec<_>>().join("\n");
        
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent).await
                .map_err(|e| KaiError::ui(format!("Failed to create history directory: {}", e)))?;
        }

        fs::write(path, content).await
            .map_err(|e| KaiError::ui(format!("Failed to save history: {}", e)))
    }
}

impl Default for HistoryService {
    fn default() -> Self {
        Self::new()
    }
}

/// Completion service for slash commands and file paths
#[derive(Debug)]
pub struct CompletionService {
    /// Current suggestions
    suggestions: Vec<String>,
    /// Currently active suggestion index
    active_index: Option<usize>,
    /// Fuzzy matcher for filtering suggestions
    matcher: SkimMatcherV2,
}

impl CompletionService {
    /// Create a new completion service
    pub fn new() -> Self {
        Self {
            suggestions: Vec::new(),
            active_index: None,
            matcher: SkimMatcherV2::default(),
        }
    }

    /// Update suggestions based on current input
    pub async fn update_suggestions(&mut self, input: &str, cursor_pos: (usize, usize)) -> Result<()> {
        self.suggestions.clear();
        self.active_index = None;

        let lines: Vec<&str> = input.lines().collect();
        if cursor_pos.0 >= lines.len() {
            return Ok(());
        }

        let current_line = lines[cursor_pos.0];
        if cursor_pos.1 > current_line.len() {
            return Ok(());
        }

        // Find the current word
        let mut word_start = cursor_pos.1;
        while word_start > 0 {
            let c = current_line.chars().nth(word_start - 1).unwrap_or(' ');
            if c.is_whitespace() {
                break;
            }
            word_start -= 1;
        }

        let current_word = &current_line[word_start..cursor_pos.1];

        if current_word.starts_with('/') {
            // Slash command completion
            self.update_slash_completions(current_word).await?;
        } else if current_word.starts_with('@') {
            // File path completion
            self.update_file_completions(current_word).await?;
        } else if current_word.starts_with('#') {
            // History completion would go here
            self.update_history_completions(current_word).await?;
        }

        if !self.suggestions.is_empty() {
            self.active_index = Some(0);
        }

        Ok(())
    }

    /// Update slash command completions
    async fn update_slash_completions(&mut self, query: &str) -> Result<()> {
        let commands = vec![
            "/model",
            "/list-models",
            "/provider",
            "/reset-context",
            "/refresh-context",
            "/help",
            "/workdir",
            "/history",
            "/clear",
            "/status",
            "/cancel",
            "/pause",
            "/resume",
        ];

        let query_without_slash = &query[1..]; // Remove the '/' prefix
        
        if query_without_slash.is_empty() {
            self.suggestions = commands.iter().map(|s| s.to_string()).collect();
        } else {
            let mut matches: Vec<(i64, String)> = commands
                .iter()
                .filter_map(|cmd| {
                    let cmd_without_slash = &cmd[1..]; // Remove the '/' prefix
                    self.matcher.fuzzy_match(cmd_without_slash, query_without_slash)
                        .map(|score| (score, cmd.to_string()))
                })
                .collect();

            matches.sort_by(|a, b| b.0.cmp(&a.0));
            self.suggestions = matches.into_iter().map(|(_, cmd)| cmd).collect();
        }

        Ok(())
    }

    /// Update file path completions
    async fn update_file_completions(&mut self, query: &str) -> Result<()> {
        if query.len() < 2 {
            return Ok(());
        }

        let path_query = &query[1..]; // Remove the '@' prefix
        let search_path = if path_query.is_empty() {
            ".".to_string()
        } else {
            // Get the directory part
            match Path::new(path_query).parent() {
                Some(parent) if parent != Path::new("") => parent.to_string_lossy().to_string(),
                _ => ".".to_string(),
            }
        };

        // Read directory contents
        match fs::read_dir(&search_path).await {
            Ok(mut entries) => {
                let mut paths = Vec::new();
                
                while let Some(entry) = entries.next_entry().await
                    .map_err(|e| KaiError::ui(format!("Failed to read directory entry: {}", e)))? {
                    
                    let path = entry.path();
                    let path_str = path.to_string_lossy().to_string();
                    
                    // Filter based on query
                    if path_query.is_empty() || path_str.contains(path_query) {
                        paths.push(format!("@{}", path_str));
                    }
                }

                // Sort and limit results
                paths.sort();
                self.suggestions = paths.into_iter().take(10).collect();
            }
            Err(_) => {
                // Directory doesn't exist or can't be read
                self.suggestions.clear();
            }
        }

        Ok(())
    }

    /// Update history completions
    async fn update_history_completions(&mut self, _query: &str) -> Result<()> {
        // This would integrate with the history service
        // For now, just return empty
        Ok(())
    }

    /// Get current suggestions
    pub fn get_suggestions(&self) -> Option<&[String]> {
        if self.suggestions.is_empty() {
            None
        } else {
            Some(&self.suggestions)
        }
    }

    /// Get the active suggestion index
    pub fn get_active_index(&self) -> Option<usize> {
        self.active_index
    }

    /// Get the currently active completion
    pub fn get_active_completion(&self) -> Option<String> {
        if let Some(index) = self.active_index {
            self.suggestions.get(index).cloned()
        } else {
            None
        }
    }

    /// Move to the next suggestion
    pub fn next_suggestion(&mut self) {
        if let Some(index) = self.active_index {
            self.active_index = Some((index + 1) % self.suggestions.len());
        }
    }

    /// Move to the previous suggestion
    pub fn previous_suggestion(&mut self) {
        if let Some(index) = self.active_index {
            self.active_index = Some(
                if index == 0 { 
                    self.suggestions.len() - 1 
                } else { 
                    index - 1 
                }
            );
        }
    }

    /// Clear all suggestions
    pub fn clear_suggestions(&mut self) {
        self.suggestions.clear();
        self.active_index = None;
    }
}

impl Default for CompletionService {
    fn default() -> Self {
        Self::new()
    }
}