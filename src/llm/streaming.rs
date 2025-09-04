//! Streaming support for LLM responses with agentic operations

use super::{LlmError, TokenUsage, ToolCall, TaskRefinementContext};
use async_trait::async_trait;
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::pin::Pin;

/// Represents a chunk of streaming response data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    /// The incremental content for this chunk
    pub content: Option<String>,
    /// Tool calls that were completed in this chunk
    pub tool_calls: Option<Vec<ToolCall>>,
    /// Indicates if this is the final chunk
    pub finish_reason: Option<String>,
    /// Token usage information (usually only in the final chunk)
    pub usage: Option<TokenUsage>,
    /// Raw chunk data for debugging
    pub raw_data: Option<serde_json::Value>,
}

impl StreamChunk {
    /// Create a new content chunk
    pub fn content<S: Into<String>>(content: S) -> Self {
        Self {
            content: Some(content.into()),
            tool_calls: None,
            finish_reason: None,
            usage: None,
            raw_data: None,
        }
    }

    /// Create a tool call chunk
    pub fn tool_calls(tool_calls: Vec<ToolCall>) -> Self {
        Self {
            content: None,
            tool_calls: Some(tool_calls),
            finish_reason: None,
            usage: None,
            raw_data: None,
        }
    }

    /// Create a final chunk with finish reason and usage
    pub fn finish<S: Into<String>>(finish_reason: S, usage: Option<TokenUsage>) -> Self {
        Self {
            content: None,
            tool_calls: None,
            finish_reason: Some(finish_reason.into()),
            usage,
            raw_data: None,
        }
    }

    /// Check if this is a content chunk
    pub fn is_content(&self) -> bool {
        self.content.is_some()
    }

    /// Check if this is a tool call chunk
    pub fn is_tool_call(&self) -> bool {
        self.tool_calls.is_some()
    }

    /// Check if this is the final chunk
    pub fn is_final(&self) -> bool {
        self.finish_reason.is_some()
    }
}

/// Stream of chunks from an LLM response
pub type LlmStream = Pin<Box<dyn Stream<Item = Result<StreamChunk, LlmError>> + Send>>;

/// Trait for LLM providers that support streaming responses
#[async_trait]
pub trait StreamingLlmProvider {
    /// Generate a streaming response
    async fn generate_stream(
        &self,
        messages: &[super::Message],
        model: &str,
        tools: Option<&[super::ToolDefinition]>,
        config: Option<&super::GenerationConfig>,
    ) -> Result<LlmStream, LlmError>;

    /// Stream plan generation with real-time updates
    async fn generate_plan_stream(
        &self,
        prompt: &str,
        context: &str,
        model: &str,
    ) -> Result<LlmStream, LlmError>;

    /// Stream task refinement with progressive detail
    async fn refine_task_stream(
        &self,
        task: &crate::planning::Task,
        context: &TaskRefinementContext,
        model: &str,
    ) -> Result<LlmStream, LlmError>;

    /// Stream task result analysis with incremental insights
    async fn analyze_task_result_stream(
        &self,
        task: &crate::planning::Task,
        execution_result: &super::TaskExecutionResult,
        expected_outcome: &str,
        model: &str,
    ) -> Result<LlmStream, LlmError>;
}

/// Utility for collecting streaming responses into a complete response
pub struct StreamCollector {
    content_buffer: String,
    tool_calls: Vec<ToolCall>,
    finish_reason: Option<String>,
    usage: Option<TokenUsage>,
}

/// Specialized collector for streaming plan generation
pub struct PlanStreamCollector {
    plan_buffer: String,
    is_json_complete: bool,
    brace_count: i32,
    bracket_count: i32,
}

/// Specialized collector for streaming task refinement
pub struct TaskRefinementCollector {
    refinement_buffer: String,
    current_section: RefinementSection,
}

/// Specialized collector for streaming task analysis
pub struct TaskAnalysisCollector {
    analysis_buffer: String,
    json_buffer: String,
    in_json_block: bool,
    brace_count: i32,
}

/// Sections of task refinement streaming
#[derive(Debug, Clone, PartialEq)]
enum RefinementSection {
    Context,
    Analysis,
    Instruction,
    Complete,
}

impl StreamCollector {
    /// Create a new stream collector
    pub fn new() -> Self {
        Self {
            content_buffer: String::new(),
            tool_calls: Vec::new(),
            finish_reason: None,
            usage: None,
        }
    }

    /// Process a stream chunk
    pub fn process_chunk(&mut self, chunk: &StreamChunk) -> Result<(), LlmError> {
        // Collect content
        if let Some(content) = &chunk.content {
            self.content_buffer.push_str(content);
        }

        // Collect tool calls
        if let Some(tool_calls) = &chunk.tool_calls {
            self.tool_calls.extend(tool_calls.clone());
        }

        // Set finish reason and usage
        if let Some(finish_reason) = &chunk.finish_reason {
            self.finish_reason = Some(finish_reason.clone());
        }

        if let Some(usage) = &chunk.usage {
            self.usage = Some(usage.clone());
        }

        Ok(())
    }

    /// Get the final collected response
    pub fn into_response(self) -> crate::llm::LlmResponse {
        crate::llm::LlmResponse {
            content: if self.content_buffer.is_empty() {
                None
            } else {
                Some(self.content_buffer)
            },
            tool_calls: if self.tool_calls.is_empty() {
                None
            } else {
                Some(self.tool_calls)
            },
            finish_reason: self.finish_reason.unwrap_or_else(|| "stop".to_string()),
            usage: self.usage,
        }
    }

    /// Collect all chunks from a stream into a complete response
    pub async fn collect_stream(mut stream: LlmStream) -> Result<crate::llm::LlmResponse, LlmError> {
        use futures::StreamExt;
        
        let mut collector = Self::new();
        
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            collector.process_chunk(&chunk)?;
            
            // If this is the final chunk, we can stop processing
            if chunk.is_final() {
                break;
            }
        }
        
        Ok(collector.into_response())
    }
}

impl PlanStreamCollector {
    /// Create a new plan stream collector
    pub fn new() -> Self {
        Self {
            plan_buffer: String::new(),
            is_json_complete: false,
            brace_count: 0,
            bracket_count: 0,
        }
    }

    /// Process a chunk and track JSON completion
    pub fn process_chunk(&mut self, chunk: &StreamChunk) -> Result<bool, LlmError> {
        if let Some(content) = &chunk.content {
            self.plan_buffer.push_str(content);
            
            // Track JSON structure completion
            for ch in content.chars() {
                match ch {
                    '{' => self.brace_count += 1,
                    '}' => self.brace_count -= 1,
                    '[' => self.bracket_count += 1,
                    ']' => self.bracket_count -= 1,
                    _ => {}
                }
            }
            
            // Check if JSON is complete
            if self.brace_count == 0 && self.bracket_count == 0 && !self.plan_buffer.trim().is_empty() {
                self.is_json_complete = true;
            }
        }
        
        Ok(self.is_json_complete || chunk.is_final())
    }

    /// Get the current plan buffer
    pub fn get_current_plan(&self) -> &str {
        &self.plan_buffer
    }

    /// Check if the plan is complete
    pub fn is_complete(&self) -> bool {
        self.is_json_complete
    }

    /// Parse the final plan
    pub fn into_plan(self) -> Result<crate::planning::Plan, LlmError> {
        let plan_json: serde_json::Value = serde_json::from_str(&self.plan_buffer)
            .map_err(|e| LlmError::InvalidResponse {
                message: format!("Failed to parse streamed plan JSON: {}", e),
            })?;

        crate::planning::Plan::from_json(&plan_json)
            .map_err(|e| LlmError::InvalidResponse {
                message: format!("Invalid streamed plan structure: {}", e),
            })
    }
}

impl TaskRefinementCollector {
    /// Create a new task refinement collector
    pub fn new() -> Self {
        Self {
            refinement_buffer: String::new(),
            current_section: RefinementSection::Context,
        }
    }

    /// Process a chunk and track refinement progress
    pub fn process_chunk(&mut self, chunk: &StreamChunk) -> Result<RefinementSection, LlmError> {
        if let Some(content) = &chunk.content {
            self.refinement_buffer.push_str(content);
            
            // Detect section transitions based on content patterns
            if content.contains("Analysis:") || content.contains("## Analysis") {
                self.current_section = RefinementSection::Analysis;
            } else if content.contains("Instruction:") || content.contains("## Instruction") ||
                     content.contains("## Refined") {
                self.current_section = RefinementSection::Instruction;
            }
            
            if chunk.is_final() {
                self.current_section = RefinementSection::Complete;
            }
        }
        
        Ok(self.current_section.clone())
    }

    /// Get the current refinement buffer
    pub fn get_current_refinement(&self) -> &str {
        &self.refinement_buffer
    }

    /// Get the current section
    pub fn current_section(&self) -> &RefinementSection {
        &self.current_section
    }

    /// Extract the final refined instruction
    pub fn into_instruction(self) -> String {
        // Extract the final instruction from the buffer
        // This could be more sophisticated with proper parsing
        self.refinement_buffer
    }
}

impl TaskAnalysisCollector {
    /// Create a new task analysis collector
    pub fn new() -> Self {
        Self {
            analysis_buffer: String::new(),
            json_buffer: String::new(),
            in_json_block: false,
            brace_count: 0,
        }
    }

    /// Process a chunk and track JSON analysis completion
    pub fn process_chunk(&mut self, chunk: &StreamChunk) -> Result<bool, LlmError> {
        if let Some(content) = &chunk.content {
            self.analysis_buffer.push_str(content);
            
            // Track JSON blocks in the response
            for ch in content.chars() {
                if ch == '{' {
                    if self.brace_count == 0 {
                        self.in_json_block = true;
                        self.json_buffer.clear();
                    }
                    self.brace_count += 1;
                }
                
                if self.in_json_block {
                    self.json_buffer.push(ch);
                }
                
                if ch == '}' {
                    self.brace_count -= 1;
                    if self.brace_count == 0 && self.in_json_block {
                        self.in_json_block = false;
                        // JSON block is complete
                        return Ok(true);
                    }
                }
            }
        }
        
        Ok(chunk.is_final())
    }

    /// Get the current analysis buffer
    pub fn get_current_analysis(&self) -> &str {
        &self.analysis_buffer
    }

    /// Get the current JSON buffer
    pub fn get_json_buffer(&self) -> &str {
        &self.json_buffer
    }

    /// Parse the final analysis
    pub fn into_analysis(self) -> Result<crate::llm::TaskAnalysis, LlmError> {
        // Try to parse the JSON buffer first, fall back to analysis buffer
        let json_content = if !self.json_buffer.trim().is_empty() {
            &self.json_buffer
        } else {
            // Try to extract JSON from the analysis buffer
            if let Some(start) = self.analysis_buffer.find('{') {
                if let Some(end) = self.analysis_buffer.rfind('}') {
                    &self.analysis_buffer[start..=end]
                } else {
                    &self.analysis_buffer
                }
            } else {
                &self.analysis_buffer
            }
        };

        let analysis_json: serde_json::Value = serde_json::from_str(json_content)
            .map_err(|e| LlmError::InvalidResponse {
                message: format!("Failed to parse streamed analysis JSON: {}. Content: {}", e, json_content),
            })?;

        // Parse into TaskAnalysis (similar to the provider implementations)
        let success = analysis_json["success"].as_bool().unwrap_or(false);
        let summary = analysis_json["summary"].as_str().unwrap_or("Analysis unavailable").to_string();
        let details = analysis_json["details"].as_str().unwrap_or("").to_string();
        let extracted_data = analysis_json.get("extracted_data").cloned();
        let next_steps = analysis_json["next_steps"].as_array()
            .map(|arr| arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<String>>());
        let context_updates = analysis_json["context_updates"].as_object()
            .map(|obj| obj.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect());
        let modified_files = analysis_json["modified_files"].as_array()
            .map(|arr| arr.iter()
                .filter_map(|v| v.as_str().map(|s| std::path::PathBuf::from(s)))
                .collect());
        let error = if !success {
            Some(analysis_json["error"].as_str().unwrap_or("Task failed").to_string())
        } else {
            None
        };
        let metadata = analysis_json["metadata"].as_object()
            .map(|obj| obj.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect())
            .unwrap_or_default();

        Ok(crate::llm::TaskAnalysis {
            success,
            summary,
            details,
            extracted_data,
            next_steps,
            context_updates,
            modified_files,
            error,
            metadata,
        })
    }
}

// Default implementations
impl Default for PlanStreamCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for TaskRefinementCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for TaskAnalysisCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for StreamCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Server-Sent Events (SSE) parser for streaming responses
pub struct SseParser {
    buffer: String,
}

impl SseParser {
    pub fn new() -> Self {
        Self {
            buffer: String::new(),
        }
    }

    /// Parse incoming data and extract complete SSE events
    pub fn parse(&mut self, data: &str) -> Vec<SseEvent> {
        self.buffer.push_str(data);
        let mut events = Vec::new();
        
        while let Some(event_end) = self.buffer.find("\n\n") {
            let event_data = self.buffer[..event_end].to_string();
            self.buffer = self.buffer[event_end + 2..].to_string();
            
            if let Some(event) = Self::parse_event(&event_data) {
                events.push(event);
            }
        }
        
        events
    }

    fn parse_event(data: &str) -> Option<SseEvent> {
        let mut event_type = None;
        let mut event_data = String::new();
        let mut event_id = None;
        
        for line in data.lines() {
            if line.is_empty() {
                continue;
            }
            
            if let Some(colon_pos) = line.find(':') {
                let field = &line[..colon_pos];
                let value = line[colon_pos + 1..].trim_start();
                
                match field {
                    "event" => event_type = Some(value.to_string()),
                    "data" => {
                        if !event_data.is_empty() {
                            event_data.push('\n');
                        }
                        event_data.push_str(value);
                    }
                    "id" => event_id = Some(value.to_string()),
                    _ => {} // Ignore other fields
                }
            }
        }
        
        if !event_data.is_empty() {
            Some(SseEvent {
                event_type,
                data: event_data,
                id: event_id,
            })
        } else {
            None
        }
    }
}

impl Default for SseParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Represents a Server-Sent Event
#[derive(Debug, Clone)]
pub struct SseEvent {
    pub event_type: Option<String>,
    pub data: String,
    pub id: Option<String>,
}

impl SseEvent {
    /// Check if this is a completion event (indicating end of stream)
    pub fn is_completion(&self) -> bool {
        self.event_type.as_deref() == Some("done") || 
        self.data == "[DONE]" ||
        self.data.contains("\"finish_reason\"")
    }
}

/// Utility functions for working with streaming responses
pub mod utils {
    use super::*;
    use futures::StreamExt;
    
    /// Convert a stream to an async iterator of content strings
    pub fn content_stream(stream: LlmStream) -> impl Stream<Item = Result<String, LlmError>> {
        stream.filter_map(|chunk_result| async move {
            match chunk_result {
                Ok(chunk) => chunk.content.map(Ok),
                Err(e) => Some(Err(e)),
            }
        })
    }
    
    /// Get only the final response from a stream (ignoring intermediate chunks)
    pub async fn final_response(mut stream: LlmStream) -> Result<crate::llm::LlmResponse, LlmError> {
        let mut last_chunk = None;
        
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            if chunk.is_final() {
                last_chunk = Some(chunk);
                break;
            }
        }
        
        if let Some(chunk) = last_chunk {
            Ok(crate::llm::LlmResponse {
                content: chunk.content,
                tool_calls: chunk.tool_calls,
                finish_reason: chunk.finish_reason.unwrap_or_else(|| "stop".to_string()),
                usage: chunk.usage,
            })
        } else {
            Err(LlmError::InvalidResponse {
                message: "Stream ended without final chunk".to_string(),
            })
        }
    }
    
    /// Merge multiple streams into a single stream (useful for parallel processing)
    pub fn merge_streams(streams: Vec<LlmStream>) -> LlmStream {
        use futures::stream::select_all;
        Box::pin(select_all(streams))
    }

    /// Collect a plan from a streaming response
    pub async fn collect_plan_stream(mut stream: LlmStream) -> Result<crate::planning::Plan, LlmError> {
        use futures::StreamExt;
        
        let mut collector = PlanStreamCollector::new();
        
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            let is_complete = collector.process_chunk(&chunk)?;
            
            if is_complete {
                break;
            }
        }
        
        collector.into_plan()
    }

    /// Collect refined task instruction from a streaming response
    pub async fn collect_task_refinement_stream(mut stream: LlmStream) -> Result<String, LlmError> {
        use futures::StreamExt;
        
        let mut collector = TaskRefinementCollector::new();
        
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            collector.process_chunk(&chunk)?;
            
            if chunk.is_final() {
                break;
            }
        }
        
        Ok(collector.into_instruction())
    }

    /// Collect task analysis from a streaming response
    pub async fn collect_task_analysis_stream(mut stream: LlmStream) -> Result<crate::llm::TaskAnalysis, LlmError> {
        use futures::StreamExt;
        
        let mut collector = TaskAnalysisCollector::new();
        
        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            let is_complete = collector.process_chunk(&chunk)?;
            
            if is_complete {
                break;
            }
        }
        
        collector.into_analysis()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_stream_collector() {
        let chunks = vec![
            StreamChunk::content("Hello"),
            StreamChunk::content(" world"),
            StreamChunk::finish("stop", Some(TokenUsage {
                prompt_tokens: 10,
                completion_tokens: 15,
                total_tokens: 25,
            })),
        ];
        
        let mut collector = StreamCollector::new();
        for chunk in &chunks {
            collector.process_chunk(chunk).unwrap();
        }
        
        let response = collector.into_response();
        assert_eq!(response.content, Some("Hello world".to_string()));
        assert_eq!(response.finish_reason, "stop");
        assert!(response.usage.is_some());
    }

    #[tokio::test]
    async fn test_plan_stream_collector() {
        let mut collector = PlanStreamCollector::new();
        
        let chunks = vec![
            StreamChunk::content("{"),
            StreamChunk::content("\"description\": \"Test plan\","),
            StreamChunk::content("\"tasks\": []"),
            StreamChunk::content("}"),
            StreamChunk::finish("stop", None),
        ];
        
        for chunk in &chunks {
            collector.process_chunk(chunk).unwrap();
        }
        
        assert!(collector.is_complete());
        assert_eq!(collector.get_current_plan(), r#"{"description": "Test plan","tasks": []}"#);
    }

    #[tokio::test]
    async fn test_task_analysis_collector() {
        let mut collector = TaskAnalysisCollector::new();
        
        let chunks = vec![
            StreamChunk::content("Analysis complete: {"),
            StreamChunk::content("\"success\": true,"),
            StreamChunk::content("\"summary\": \"Task completed successfully\"}"),
            StreamChunk::finish("stop", None),
        ];
        
        for chunk in &chunks {
            collector.process_chunk(chunk).unwrap();
        }
        
        assert_eq!(collector.get_json_buffer(), r#"{"success": true,"summary": "Task completed successfully"}"#);
    }

    #[tokio::test]
    async fn test_collect_stream() {
        let chunks = vec![
            Ok(StreamChunk::content("Test")),
            Ok(StreamChunk::content(" message")),
            Ok(StreamChunk::finish("stop", None)),
        ];
        
        let stream = Box::pin(stream::iter(chunks));
        let response = StreamCollector::collect_stream(stream).await.unwrap();
        
        assert_eq!(response.content, Some("Test message".to_string()));
        assert_eq!(response.finish_reason, "stop");
    }

    #[test]
    fn test_sse_parser() {
        let mut parser = SseParser::new();
        
        let events = parser.parse("event: message\ndata: Hello\n\nevent: done\ndata: [DONE]\n\n");
        
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, Some("message".to_string()));
        assert_eq!(events[0].data, "Hello");
        assert_eq!(events[1].event_type, Some("done".to_string()));
        assert_eq!(events[1].data, "[DONE]");
        assert!(events[1].is_completion());
    }

    #[test]
    fn test_chunk_types() {
        let content_chunk = StreamChunk::content("Hello");
        assert!(content_chunk.is_content());
        assert!(!content_chunk.is_tool_call());
        assert!(!content_chunk.is_final());
        
        let final_chunk = StreamChunk::finish("stop", None);
        assert!(!final_chunk.is_content());
        assert!(!final_chunk.is_tool_call());
        assert!(final_chunk.is_final());
    }

    #[test]
    fn test_refinement_section_tracking() {
        let mut collector = TaskRefinementCollector::new();
        
        assert_eq!(*collector.current_section(), RefinementSection::Context);
        
        collector.process_chunk(&StreamChunk::content("## Analysis\nStarting analysis...")).unwrap();
        assert_eq!(*collector.current_section(), RefinementSection::Analysis);
        
        collector.process_chunk(&StreamChunk::content("## Refined Instruction\nExecute the following:")).unwrap();
        assert_eq!(*collector.current_section(), RefinementSection::Instruction);
    }
}