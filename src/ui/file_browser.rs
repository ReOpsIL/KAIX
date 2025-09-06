//! Interactive file browser component with fuzzy search

use crate::utils::errors::KaiError;
use crate::Result;
use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// File browser component with fuzzy search and filtering
pub struct FileBrowserComponent {
    /// Current working directory
    working_directory: PathBuf,
    /// All discovered files
    all_files: Vec<FileEntry>,
    /// Filtered files based on search query
    filtered_files: Vec<FileEntry>,
    /// Current search query
    search_query: String,
    /// Currently selected index
    selected_index: usize,
    /// Whether the browser is visible
    visible: bool,
    /// Fuzzy matcher for filtering
    matcher: SkimMatcherV2,
    /// Maximum number of files to show
    max_results: usize,
}

/// Represents a file entry in the browser
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Relative path from working directory
    pub path: PathBuf,
    /// Display name (just the filename)
    pub name: String,
    /// Whether this is a directory
    pub is_directory: bool,
    /// File size in bytes (for files)
    pub size: Option<u64>,
    /// File extension for syntax highlighting
    pub extension: Option<String>,
    /// Fuzzy match score (used for sorting)
    pub score: Option<i64>,
}

impl FileBrowserComponent {
    /// Create a new file browser component
    pub fn new(working_directory: PathBuf) -> Self {
        Self {
            working_directory,
            all_files: Vec::new(),
            filtered_files: Vec::new(),
            search_query: String::new(),
            selected_index: 0,
            visible: false,
            matcher: SkimMatcherV2::default(),
            max_results: 50,
        }
    }

    /// Trigger the file browser with '@' character
    pub async fn trigger_browser(&mut self, current_query: &str) -> Result<()> {
        if current_query.starts_with('@') {
            let search_term = &current_query[1..]; // Remove '@' prefix
            self.search_query = search_term.to_string();
            self.visible = true;
            
            // Refresh file list if needed
            if self.all_files.is_empty() {
                self.refresh_file_list().await?;
            }
            
            // Update filtered results
            self.update_filtered_results();
        }
        
        Ok(())
    }

    /// Hide the file browser
    pub fn hide(&mut self) {
        self.visible = false;
        self.search_query.clear();
        self.selected_index = 0;
    }

    /// Update search query and filter results
    pub fn update_search(&mut self, query: &str) {
        if query.starts_with('@') {
            self.search_query = query[1..].to_string();
            self.update_filtered_results();
            self.selected_index = 0; // Reset selection
        }
    }

    /// Get the currently selected file path
    pub fn get_selected_file(&self) -> Option<&PathBuf> {
        if self.visible && self.selected_index < self.filtered_files.len() {
            Some(&self.filtered_files[self.selected_index].path)
        } else {
            None
        }
    }

    /// Move selection up
    pub fn select_previous(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        } else if !self.filtered_files.is_empty() {
            self.selected_index = self.filtered_files.len() - 1;
        }
    }

    /// Move selection down
    pub fn select_next(&mut self) {
        if self.selected_index + 1 < self.filtered_files.len() {
            self.selected_index += 1;
        } else {
            self.selected_index = 0;
        }
    }

    /// Check if browser is currently visible
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Refresh the file list by scanning the working directory
    pub async fn refresh_file_list(&mut self) -> Result<()> {
        let workdir = self.working_directory.clone();
        
        // Use blocking task for file system operations
        let files = tokio::task::spawn_blocking(move || -> Result<Vec<FileEntry>> {
            let mut entries = Vec::new();
            
            // First, add directories in the current working directory
            if let Ok(dir_entries) = std::fs::read_dir(&workdir) {
                for entry in dir_entries.flatten() {
                    if let Ok(metadata) = entry.metadata() {
                        if metadata.is_dir() {
                            let path = entry.path();
                            let relative_path = path.strip_prefix(&workdir).unwrap_or(&path).to_path_buf();
                            let name = entry.file_name().to_string_lossy().to_string();
                            
                            entries.push(FileEntry {
                                path: relative_path,
                                name,
                                is_directory: true,
                                size: None,
                                extension: None,
                                score: None,
                            });
                        }
                    }
                }
            }
            
            // Then walk through all files, respecting .gitignore and .aiignore
            for entry in WalkDir::new(&workdir)
                .max_depth(10) // Prevent excessive recursion
                .follow_links(false)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
            {
                let path = entry.path();
                let relative_path = path.strip_prefix(&workdir).unwrap_or(path).to_path_buf();
                
                // Skip hidden files and common ignore patterns
                if should_ignore_file(&relative_path) {
                    continue;
                }
                
                let name = path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unknown".to_string());
                
                let size = entry.metadata().ok().map(|m| m.len());
                let extension = path.extension()
                    .map(|ext| ext.to_string_lossy().to_string());
                
                entries.push(FileEntry {
                    path: relative_path,
                    name,
                    is_directory: false,
                    size,
                    extension,
                    score: None,
                });
            }
            
            // Sort: directories first, then files alphabetically
            entries.sort_by(|a, b| {
                match (a.is_directory, b.is_directory) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => a.name.cmp(&b.name),
                }
            });
            
            Ok(entries)
        }).await.map_err(|e| KaiError::ui(format!("Failed to scan files: {}", e)))??;
        
        self.all_files = files;
        self.update_filtered_results();
        
        Ok(())
    }

    /// Update filtered results based on current search query
    fn update_filtered_results(&mut self) {
        if self.search_query.is_empty() {
            // Show all files if no search query
            self.filtered_files = self.all_files.iter()
                .take(self.max_results)
                .cloned()
                .collect();
        } else {
            // Apply fuzzy matching
            let mut scored_files: Vec<FileEntry> = self.all_files
                .iter()
                .filter_map(|file| {
                    let search_target = if file.is_directory {
                        // For directories, search in the directory name
                        file.name.clone()
                    } else {
                        // For files, search in both filename and path
                        format!("{} {}", file.name, file.path.to_string_lossy())
                    };
                    
                    self.matcher
                        .fuzzy_match(&search_target, &self.search_query)
                        .map(|score| {
                            let mut file_entry = file.clone();
                            file_entry.score = Some(score);
                            file_entry
                        })
                })
                .collect();
            
            // Sort by score (highest first), with directories getting bonus points
            scored_files.sort_by(|a, b| {
                let score_a = a.score.unwrap_or(0) + if a.is_directory { 100 } else { 0 };
                let score_b = b.score.unwrap_or(0) + if b.is_directory { 100 } else { 0 };
                score_b.cmp(&score_a)
            });
            
            self.filtered_files = scored_files.into_iter()
                .take(self.max_results)
                .collect();
        }
        
        // Reset selection if out of bounds
        if self.selected_index >= self.filtered_files.len() {
            self.selected_index = 0;
        }
    }

    /// Render the file browser as an overlay
    pub fn render(&self, f: &mut Frame, area: Rect) {
        if !self.visible {
            return;
        }
        
        // Calculate popup area (centered, 80% width, 60% height)
        let popup_area = centered_rect(80, 60, area);
        
        // Create the file list items
        let items: Vec<ListItem> = self.filtered_files
            .iter()
            .enumerate()
            .map(|(i, file)| {
                let is_selected = i == self.selected_index;
                let style = if is_selected {
                    Style::default().bg(Color::Blue).fg(Color::White)
                } else {
                    Style::default()
                };
                
                let icon = if file.is_directory { "📁" } else { get_file_icon(&file.extension) };
                let size_info = if let Some(size) = file.size {
                    format_file_size(size)
                } else {
                    String::new()
                };
                
                let content = if file.is_directory {
                    format!("{} {}/", icon, file.name)
                } else {
                    format!("{} {} {}", icon, file.name, size_info)
                };
                
                ListItem::new(Line::from(vec![Span::styled(content, style)]))
            })
            .collect();
        
        // Create the list widget
        let title = if self.search_query.is_empty() {
            format!("📂 File Browser - {} files", self.filtered_files.len())
        } else {
            format!("🔍 Search: '{}' - {} matches", self.search_query, self.filtered_files.len())
        };
        
        let file_list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(title))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));
        
        // Render the popup
        f.render_widget(ratatui::widgets::Clear, popup_area); // Clear the area
        f.render_widget(file_list, popup_area);
        
        // Show help text at the bottom
        if popup_area.height > 5 {
            let help_area = Rect {
                x: popup_area.x,
                y: popup_area.y + popup_area.height - 2,
                width: popup_area.width,
                height: 1,
            };
            
            let help_text = Paragraph::new("↑↓: Navigate | Enter: Select | Esc: Cancel | Type to search")
                .style(Style::default().fg(Color::Gray));
            f.render_widget(help_text, help_area);
        }
    }

    /// Set working directory and refresh file list
    pub async fn set_working_directory(&mut self, workdir: PathBuf) -> Result<()> {
        self.working_directory = workdir;
        self.all_files.clear();
        self.filtered_files.clear();
        self.refresh_file_list().await
    }

    /// Get number of filtered results
    pub fn get_result_count(&self) -> usize {
        self.filtered_files.len()
    }
}

/// Check if a file should be ignored based on common patterns
fn should_ignore_file(path: &Path) -> bool {
    let path_str = path.to_string_lossy().to_lowercase();
    
    // Common ignore patterns
    let ignore_patterns = [
        ".git/", "node_modules/", "target/", ".cargo/", 
        "__pycache__/", ".pytest_cache/", ".vscode/",
        ".idea/", "*.tmp", "*.log", "*.lock", "*.pid",
        ".DS_Store", "Thumbs.db", "desktop.ini",
        "*.o", "*.so", "*.dylib", "*.dll", "*.exe",
        ".env", ".env.local", ".env.production",
    ];
    
    for pattern in &ignore_patterns {
        if pattern.ends_with('/') {
            // Directory pattern
            if path_str.contains(&pattern.to_lowercase()) {
                return true;
            }
        } else if pattern.starts_with('*') {
            // Extension pattern
            let ext = &pattern[1..];
            if path_str.ends_with(ext) {
                return true;
            }
        } else {
            // Exact match pattern
            if path_str.contains(&pattern.to_lowercase()) {
                return true;
            }
        }
    }
    
    // Also ignore hidden files (starting with .)
    if let Some(name) = path.file_name() {
        if name.to_string_lossy().starts_with('.') {
            return true;
        }
    }
    
    false
}

/// Get appropriate icon for file type
fn get_file_icon(extension: &Option<String>) -> &'static str {
    match extension.as_ref().map(|s| s.as_str()) {
        Some("rs") => "🦀",
        Some("js") | Some("ts") => "⚡",
        Some("py") => "🐍",
        Some("java") => "☕",
        Some("cpp") | Some("cc") | Some("c") => "⚙️",
        Some("go") => "🐹",
        Some("php") => "🐘",
        Some("rb") => "💎",
        Some("swift") => "🏃",
        Some("kt") => "🎯",
        Some("html") => "🌐",
        Some("css") => "🎨",
        Some("md") => "📝",
        Some("json") => "📋",
        Some("xml") => "📄",
        Some("yml") | Some("yaml") => "⚙️",
        Some("toml") => "🔧",
        Some("txt") => "📄",
        Some("png") | Some("jpg") | Some("jpeg") | Some("gif") => "🖼️",
        Some("mp4") | Some("avi") | Some("mov") => "🎬",
        Some("mp3") | Some("wav") | Some("flac") => "🎵",
        Some("zip") | Some("tar") | Some("gz") => "📦",
        _ => "📄",
    }
}

/// Format file size in human readable format
fn format_file_size(size: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = size as f64;
    let mut unit_index = 0;
    
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    
    if unit_index == 0 {
        format!("{}B", size as u64)
    } else {
        format!("{:.1}{}", size, UNITS[unit_index])
    }
}

/// Helper function to create a centered rectangle
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

impl FileEntry {
    /// Get display name with path for deeply nested files
    pub fn get_display_path(&self) -> String {
        if self.is_directory {
            format!("{}/", self.name)
        } else {
            self.path.to_string_lossy().to_string()
        }
    }
}