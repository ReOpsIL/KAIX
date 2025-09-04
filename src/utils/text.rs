//! Text processing utilities

use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};

/// Text processing utilities
pub struct TextProcessor {
    fuzzy_matcher: SkimMatcherV2,
}

impl Default for TextProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl TextProcessor {
    /// Create a new text processor
    pub fn new() -> Self {
        Self {
            fuzzy_matcher: SkimMatcherV2::default(),
        }
    }

    /// Perform fuzzy matching on a list of candidates
    pub fn fuzzy_match<'a>(
        &self,
        pattern: &str,
        candidates: &'a [String],
    ) -> Vec<(&'a String, i64)> {
        let mut matches: Vec<(&String, i64)> = candidates
            .iter()
            .filter_map(|candidate| {
                self.fuzzy_matcher
                    .fuzzy_match(candidate, pattern)
                    .map(|score| (candidate, score))
            })
            .collect();

        // Sort by score (higher is better)
        matches.sort_by(|a, b| b.1.cmp(&a.1));
        matches
    }

    /// Filter and sort strings by fuzzy match score
    pub fn filter_fuzzy(&self, pattern: &str, candidates: &[String]) -> Vec<String> {
        self.fuzzy_match(pattern, candidates)
            .into_iter()
            .map(|(candidate, _)| candidate.clone())
            .collect()
    }
}

/// Truncate text to a maximum length with ellipsis
pub fn truncate(text: &str, max_length: usize) -> String {
    if text.len() <= max_length {
        text.to_string()
    } else if max_length <= 3 {
        "...".to_string()
    } else {
        format!("{}...", &text[..max_length - 3])
    }
}

/// Split text into chunks of maximum size, preserving word boundaries when possible
pub fn chunk_text(text: &str, max_chunk_size: usize) -> Vec<String> {
    if text.len() <= max_chunk_size {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut current_chunk = String::new();
    let mut current_size = 0;

    for word in text.split_whitespace() {
        let word_len = word.len() + if current_chunk.is_empty() { 0 } else { 1 }; // +1 for space

        if current_size + word_len > max_chunk_size && !current_chunk.is_empty() {
            // Start new chunk
            chunks.push(current_chunk.trim().to_string());
            current_chunk = word.to_string();
            current_size = word.len();
        } else {
            // Add to current chunk
            if !current_chunk.is_empty() {
                current_chunk.push(' ');
                current_size += 1;
            }
            current_chunk.push_str(word);
            current_size += word.len();
        }
    }

    if !current_chunk.is_empty() {
        chunks.push(current_chunk.trim().to_string());
    }

    chunks
}

/// Extract common prefixes from a list of strings
pub fn common_prefix(strings: &[String]) -> String {
    if strings.is_empty() {
        return String::new();
    }

    if strings.len() == 1 {
        return strings[0].clone();
    }

    let first = &strings[0];
    let mut prefix_len = first.len();

    for string in &strings[1..] {
        prefix_len = first
            .chars()
            .zip(string.chars())
            .take(prefix_len)
            .take_while(|(a, b)| a == b)
            .count();

        if prefix_len == 0 {
            break;
        }
    }

    first.chars().take(prefix_len).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello world", 20), "hello world");
        assert_eq!(truncate("hello world", 8), "hello...");
        assert_eq!(truncate("hi", 2), "hi");
        assert_eq!(truncate("hello", 3), "...");
    }

    #[test]
    fn test_chunk_text() {
        let text = "hello world this is a test";
        let chunks = chunk_text(text, 10);
        assert!(chunks.iter().all(|chunk| chunk.len() <= 10));
        assert!(!chunks.is_empty());
    }

    #[test]
    fn test_common_prefix() {
        let strings = vec![
            "hello world".to_string(),
            "hello there".to_string(),
            "hello universe".to_string(),
        ];
        assert_eq!(common_prefix(&strings), "hello ");
    }
}