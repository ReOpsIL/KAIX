//! Comprehensive debug tracing and flow visualization system
//!
//! This module provides production-safe debug instrumentation for complete
//! application flow tracing and issue identification. All debug features
//! can be controlled via environment variables and configuration.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tracing::{debug, trace, info, error, span, Level};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Debug configuration controlled by environment variables
#[derive(Debug, Clone, Serialize)]
pub struct DebugConfig {
    /// Enable comprehensive flow tracing
    pub flow_tracing_enabled: bool,
    /// Enable performance timing measurements
    pub timing_enabled: bool,
    /// Enable state validation checks
    pub validation_enabled: bool,
    /// Debug output verbosity (0-5)
    pub verbosity_level: u8,
    /// Component-specific debug filters
    pub component_filters: HashMap<String, bool>,
    /// Enable async boundary tracing
    pub async_tracing_enabled: bool,
    /// Enable error propagation tracing
    pub error_tracing_enabled: bool,
    /// Maximum debug buffer size
    pub max_buffer_size: usize,
}

impl Default for DebugConfig {
    fn default() -> Self {
        Self {
            flow_tracing_enabled: std::env::var("KAI_DEBUG_FLOW").unwrap_or_default() == "1",
            timing_enabled: std::env::var("KAI_DEBUG_TIMING").unwrap_or_default() == "1", 
            validation_enabled: std::env::var("KAI_DEBUG_VALIDATION").unwrap_or_default() == "1",
            verbosity_level: std::env::var("KAI_DEBUG_LEVEL")
                .unwrap_or_default()
                .parse()
                .unwrap_or(1),
            component_filters: Self::parse_component_filters(),
            async_tracing_enabled: std::env::var("KAI_DEBUG_ASYNC").unwrap_or_default() == "1",
            error_tracing_enabled: std::env::var("KAI_DEBUG_ERRORS").unwrap_or_default() == "1",
            max_buffer_size: std::env::var("KAI_DEBUG_BUFFER_SIZE")
                .unwrap_or_default()
                .parse()
                .unwrap_or(10000),
        }
    }
}

impl DebugConfig {
    fn parse_component_filters() -> HashMap<String, bool> {
        let mut filters = HashMap::new();
        
        if let Ok(filter_str) = std::env::var("KAI_DEBUG_COMPONENTS") {
            for component in filter_str.split(',') {
                let component = component.trim();
                if !component.is_empty() {
                    filters.insert(component.to_string(), true);
                }
            }
        }
        
        // Default components always enabled if any debugging is on
        if std::env::var("KAI_DEBUG").unwrap_or_default() == "1" {
            filters.insert("main".to_string(), true);
            filters.insert("config".to_string(), true);
            filters.insert("llm".to_string(), true);
            filters.insert("ui".to_string(), true);
            filters.insert("planning".to_string(), true);
        }
        
        filters
    }
    
    /// Check if debug tracing is enabled for a component
    pub fn is_component_enabled(&self, component: &str) -> bool {
        self.component_filters.get(component).copied().unwrap_or(false)
    }
}

/// Execution flow checkpoint for tracking progress through the application
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowCheckpoint {
    pub id: String,
    pub component: String,
    pub function: String,
    pub checkpoint_name: String,
    pub timestamp: u64,
    pub sequence_number: u64,
    pub thread_id: String,
    pub async_context: Option<String>,
    pub execution_depth: u32,
    pub timing_info: Option<TimingInfo>,
    pub state_info: HashMap<String, serde_json::Value>,
    pub error_context: Option<String>,
    pub metadata: HashMap<String, String>,
}

/// Performance timing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingInfo {
    pub operation_start: u64,
    pub operation_duration_ms: Option<u64>,
    pub cumulative_time_ms: u64,
    pub memory_usage_bytes: Option<usize>,
}

/// Flow execution context for nested operations
#[derive(Debug, Clone)]
pub struct FlowContext {
    pub flow_id: String,
    pub parent_flow_id: Option<String>,
    pub component: String,
    pub operation: String,
    pub start_time: Instant,
    pub depth: u32,
    pub checkpoints: Vec<String>,
}

impl FlowContext {
    pub fn new(component: &str, operation: &str) -> Self {
        Self {
            flow_id: Uuid::new_v4().to_string(),
            parent_flow_id: None,
            component: component.to_string(),
            operation: operation.to_string(), 
            start_time: Instant::now(),
            depth: 0,
            checkpoints: Vec::new(),
        }
    }
    
    pub fn child(&self, component: &str, operation: &str) -> Self {
        Self {
            flow_id: Uuid::new_v4().to_string(),
            parent_flow_id: Some(self.flow_id.clone()),
            component: component.to_string(),
            operation: operation.to_string(),
            start_time: Instant::now(),
            depth: self.depth + 1,
            checkpoints: Vec::new(),
        }
    }
}

/// Global debug tracer instance
pub struct DebugTracer {
    config: DebugConfig,
    sequence_counter: Arc<Mutex<u64>>,
    checkpoints: Arc<Mutex<Vec<FlowCheckpoint>>>,
    active_flows: Arc<Mutex<HashMap<String, FlowContext>>>,
}

impl DebugTracer {
    /// Create a new debug tracer with environment-based configuration
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            config: DebugConfig::default(),
            sequence_counter: Arc::new(Mutex::new(0)),
            checkpoints: Arc::new(Mutex::new(Vec::new())),
            active_flows: Arc::new(Mutex::new(HashMap::new())),
        })
    }
    
    /// Start a new flow execution context
    pub fn start_flow(&self, component: &str, operation: &str) -> FlowContext {
        let context = FlowContext::new(component, operation);
        
        if self.config.flow_tracing_enabled && self.config.is_component_enabled(component) {
            let mut flows = self.active_flows.lock().unwrap();
            flows.insert(context.flow_id.clone(), context.clone());
            
            let span = span!(Level::DEBUG, "flow_start", 
                flow_id = %context.flow_id,
                component = component,
                operation = operation,
                depth = context.depth
            );
            let _enter = span.enter();
            debug!("🚀 [FLOW-START] {} -> {} (ID: {})", component, operation, &context.flow_id[..8]);
        }
        
        context
    }
    
    /// End a flow execution context
    pub fn end_flow(&self, context: &FlowContext) {
        if self.config.flow_tracing_enabled && self.config.is_component_enabled(&context.component) {
            let duration = context.start_time.elapsed();
            let mut flows = self.active_flows.lock().unwrap();
            flows.remove(&context.flow_id);
            
            let span = span!(Level::DEBUG, "flow_end",
                flow_id = %context.flow_id,
                component = %context.component,
                operation = %context.operation,
                duration_ms = duration.as_millis(),
                depth = context.depth,
                checkpoints = context.checkpoints.len()
            );
            let _enter = span.enter();
            debug!("🏁 [FLOW-END] {} <- {} (ID: {}, Duration: {:?}, Checkpoints: {})", 
                context.component, context.operation, &context.flow_id[..8], duration, context.checkpoints.len());
        }
    }
    
    /// Record a flow checkpoint with optional timing and state information
    pub fn checkpoint(
        &self,
        context: &mut FlowContext,
        checkpoint_name: &str,
        state: Option<HashMap<String, serde_json::Value>>,
    ) {
        if !self.config.flow_tracing_enabled || !self.config.is_component_enabled(&context.component) {
            return;
        }
        
        let sequence = {
            let mut counter = self.sequence_counter.lock().unwrap();
            *counter += 1;
            *counter
        };
        
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
            
        let timing_info = if self.config.timing_enabled {
            Some(TimingInfo {
                operation_start: context.start_time.elapsed().as_millis() as u64,
                operation_duration_ms: None,
                cumulative_time_ms: context.start_time.elapsed().as_millis() as u64,
                memory_usage_bytes: None, // Could add memory tracking here
            })
        } else {
            None
        };
        
        let checkpoint = FlowCheckpoint {
            id: Uuid::new_v4().to_string(),
            component: context.component.clone(),
            function: context.operation.clone(),
            checkpoint_name: checkpoint_name.to_string(),
            timestamp,
            sequence_number: sequence,
            thread_id: format!("{:?}", std::thread::current().id()),
            async_context: if self.config.async_tracing_enabled {
                Some(format!("task-{}", context.flow_id))
            } else {
                None
            },
            execution_depth: context.depth,
            timing_info,
            state_info: state.unwrap_or_default(),
            error_context: None,
            metadata: HashMap::new(),
        };
        
        context.checkpoints.push(checkpoint_name.to_string());
        
        // Store checkpoint in buffer
        {
            let mut checkpoints = self.checkpoints.lock().unwrap();
            checkpoints.push(checkpoint.clone());
            
            // Maintain buffer size limit
            if checkpoints.len() > self.config.max_buffer_size {
                let len = checkpoints.len();
                checkpoints.drain(0..len - self.config.max_buffer_size);
            }
        }
        
        // Log with appropriate level based on verbosity
        let indent = "  ".repeat(context.depth as usize);
        match self.config.verbosity_level {
            0 => {}
            1 => debug!("{}✓ [{}] {}", indent, context.component.to_uppercase(), checkpoint_name),
            2 => debug!("{}✓ [{}] {} (seq: {}, thread: {})", 
                indent, context.component.to_uppercase(), checkpoint_name, sequence, 
                &format!("{:?}", std::thread::current().id())[9..13]),
            3 => info!("{}✓ [{}] {} (seq: {}, time: {}ms, flow: {})", 
                indent, context.component.to_uppercase(), checkpoint_name, sequence,
                context.start_time.elapsed().as_millis(), &context.flow_id[..8]),
            4 => info!("{}✓ [{}] {} (seq: {}, time: {}ms, flow: {}, depth: {})", 
                indent, context.component.to_uppercase(), checkpoint_name, sequence,
                context.start_time.elapsed().as_millis(), &context.flow_id[..8], context.depth),
            _ => trace!("{}✓ [{}] {} (seq: {}, time: {}ms, flow: {}, depth: {}, state: {:?})", 
                indent, context.component.to_uppercase(), checkpoint_name, sequence,
                context.start_time.elapsed().as_millis(), &context.flow_id[..8], context.depth,
                checkpoint.state_info),
        }
    }
    
    /// Record an error with full propagation context
    pub fn error_checkpoint(
        &self,
        context: &mut FlowContext,
        error: &crate::utils::errors::KaiError,
        checkpoint_name: &str,
    ) {
        if !self.config.error_tracing_enabled || !self.config.is_component_enabled(&context.component) {
            return;
        }
        
        let mut state = HashMap::new();
        state.insert("error_category".to_string(), serde_json::Value::String(error.category().to_string()));
        state.insert("error_recoverable".to_string(), serde_json::Value::Bool(error.is_recoverable()));
        state.insert("error_message".to_string(), serde_json::Value::String(error.to_string()));
        
        let sequence = {
            let mut counter = self.sequence_counter.lock().unwrap();
            *counter += 1;
            *counter
        };
        
        let checkpoint = FlowCheckpoint {
            id: Uuid::new_v4().to_string(),
            component: context.component.clone(),
            function: context.operation.clone(),
            checkpoint_name: checkpoint_name.to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            sequence_number: sequence,
            thread_id: format!("{:?}", std::thread::current().id()),
            async_context: None,
            execution_depth: context.depth,
            timing_info: None,
            state_info: state,
            error_context: Some(error.to_string()),
            metadata: HashMap::new(),
        };
        
        {
            let mut checkpoints = self.checkpoints.lock().unwrap();
            checkpoints.push(checkpoint);
        }
        
        let indent = "  ".repeat(context.depth as usize);
        error!("{}❌ [{}] {} - {} ({})", 
            indent, context.component.to_uppercase(), checkpoint_name, 
            error.to_string(), error.category());
    }
    
    /// Validate application state at major checkpoints
    pub fn validate_state<T, F>(
        &self,
        context: &mut FlowContext,
        validation_name: &str,
        validator: F,
    ) -> Result<T, crate::utils::errors::KaiError>
    where
        F: FnOnce() -> Result<T, crate::utils::errors::KaiError>,
    {
        if !self.config.validation_enabled {
            return validator();
        }
        
        self.checkpoint(context, &format!("validate_{}_start", validation_name), None);
        
        match validator() {
            Ok(result) => {
                self.checkpoint(context, &format!("validate_{}_success", validation_name), None);
                Ok(result)
            }
            Err(error) => {
                self.error_checkpoint(context, &error, &format!("validate_{}_failed", validation_name));
                Err(error)
            }
        }
    }
    
    /// Get debug summary for troubleshooting
    pub fn get_debug_summary(&self) -> DebugSummary {
        let checkpoints = self.checkpoints.lock().unwrap();
        let active_flows = self.active_flows.lock().unwrap();
        
        let mut component_stats = HashMap::new();
        let mut error_count = 0;
        
        for checkpoint in checkpoints.iter() {
            let counter = component_stats.entry(checkpoint.component.clone()).or_insert(0);
            *counter += 1;
            
            if checkpoint.error_context.is_some() {
                error_count += 1;
            }
        }
        
        DebugSummary {
            total_checkpoints: checkpoints.len(),
            active_flows: active_flows.len(),
            component_stats,
            error_count,
            config: self.config.clone(),
        }
    }
}

/// Debug summary for monitoring and reporting
#[derive(Debug, Clone, Serialize)]
pub struct DebugSummary {
    pub total_checkpoints: usize,
    pub active_flows: usize,
    pub component_stats: HashMap<String, usize>,
    pub error_count: usize,
    pub config: DebugConfig,
}

// Global debug tracer instance
lazy_static::lazy_static! {
    pub static ref DEBUG_TRACER: Arc<DebugTracer> = DebugTracer::new();
}

/// Macro for easy flow tracing
#[macro_export]
macro_rules! debug_flow {
    ($component:expr, $operation:expr, $block:block) => {{
        let mut _flow_context = $crate::utils::debug::DEBUG_TRACER.start_flow($component, $operation);
        let _result = {$block};
        $crate::utils::debug::DEBUG_TRACER.end_flow(&_flow_context);
        _result
    }};
}

/// Macro for checkpoints
#[macro_export]
macro_rules! debug_checkpoint {
    ($context:expr, $name:expr) => {
        $crate::utils::debug::DEBUG_TRACER.checkpoint($context, $name, None)
    };
    ($context:expr, $name:expr, $state:expr) => {
        $crate::utils::debug::DEBUG_TRACER.checkpoint($context, $name, Some($state))
    };
}

/// Macro for error checkpoints 
#[macro_export]
macro_rules! debug_error {
    ($context:expr, $error:expr, $name:expr) => {
        $crate::utils::debug::DEBUG_TRACER.error_checkpoint($context, $error, $name)
    };
}

/// Helper function to check if debugging is enabled
pub fn is_debug_enabled() -> bool {
    std::env::var("KAI_DEBUG").unwrap_or_default() == "1" ||
    std::env::var("KAI_DEBUG_FLOW").unwrap_or_default() == "1" ||
    std::env::var("KAI_DEBUG_TIMING").unwrap_or_default() == "1"
}

/// Helper function to enable full debug tracing
pub fn enable_full_debug() {
    std::env::set_var("KAI_DEBUG", "1");
    std::env::set_var("KAI_DEBUG_FLOW", "1");
    std::env::set_var("KAI_DEBUG_TIMING", "1");
    std::env::set_var("KAI_DEBUG_VALIDATION", "1");
    std::env::set_var("KAI_DEBUG_ASYNC", "1");
    std::env::set_var("KAI_DEBUG_ERRORS", "1");
    std::env::set_var("KAI_DEBUG_LEVEL", "3");
}

/// Print debug configuration help
pub fn print_debug_help() {
    println!("KAI-X Debug Environment Variables:");
    println!("  KAI_DEBUG=1                    - Enable all debug features");
    println!("  KAI_DEBUG_FLOW=1               - Enable execution flow tracing");
    println!("  KAI_DEBUG_TIMING=1             - Enable performance timing");
    println!("  KAI_DEBUG_VALIDATION=1         - Enable state validation");
    println!("  KAI_DEBUG_ASYNC=1              - Enable async boundary tracing");
    println!("  KAI_DEBUG_ERRORS=1             - Enable error propagation tracing");
    println!("  KAI_DEBUG_LEVEL=0-5            - Set verbosity level (default: 1)");
    println!("  KAI_DEBUG_COMPONENTS=main,llm  - Enable specific components");
    println!("  KAI_DEBUG_BUFFER_SIZE=10000    - Set checkpoint buffer size");
}