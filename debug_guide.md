# KAI-X Debug Tracing Guide

KAI-X includes comprehensive debug tracing and flow visualization to help identify execution issues and performance bottlenecks. All debug features are production-safe and can be controlled via environment variables.

## Quick Start

Enable full debug tracing:
```bash
export KAI_DEBUG=1
kai --help
```

Show debug configuration options:
```bash
KAI_DEBUG_HELP=1 kai
```

## Environment Variables

### Core Debug Controls
- `KAI_DEBUG=1` - Enable all debug features
- `KAI_DEBUG_FLOW=1` - Enable execution flow tracing
- `KAI_DEBUG_TIMING=1` - Enable performance timing measurements
- `KAI_DEBUG_VALIDATION=1` - Enable state validation checks
- `KAI_DEBUG_ASYNC=1` - Enable async boundary tracing
- `KAI_DEBUG_ERRORS=1` - Enable error propagation tracing

### Verbosity Control
- `KAI_DEBUG_LEVEL=0-5` - Set debug verbosity (0=off, 5=maximum)
  - Level 0: No debug output
  - Level 1: Basic checkpoints (default)
  - Level 2: Add sequence numbers and thread IDs
  - Level 3: Add timing and flow IDs
  - Level 4: Add execution depth
  - Level 5: Add full state information

### Component Filtering
- `KAI_DEBUG_COMPONENTS=main,llm,ui` - Enable debug for specific components
  - Available components: main, config, llm, ui, planning, execution, context

### Performance Tuning
- `KAI_DEBUG_BUFFER_SIZE=10000` - Set checkpoint buffer size (default: 10000)

## Debug Output Examples

### Level 1 (Basic)
```
✓ [MAIN] cli_parsed
✓ [CONFIG] config_manager_created
✓ [LLM] provider_factory_success
```

### Level 3 (Detailed)
```
✓ [MAIN] cli_parsed (seq: 1, time: 2ms, flow: a1b2c3d4)
  ✓ [CONFIG] get_config_path_success (seq: 2, time: 5ms, flow: e5f6g7h8)
    ✓ [CONFIG] load_config_success (seq: 3, time: 12ms, flow: i9j0k1l2)
✓ [LLM] openrouter_provider_created (seq: 4, time: 45ms, flow: m3n4o5p6)
```

### Level 5 (Maximum)
```
✓ [MAIN] cli_parsed (seq: 1, time: 2ms, flow: a1b2c3d4, depth: 0, state: {"workdir": "None", "log_level": "info", "command": "Chat"})
  ✓ [CONFIG] config_manager_created (seq: 2, time: 15ms, flow: e5f6g7h8, depth: 1, state: {"config_path": "/Users/user/.config/kai-x/config.toml", "provider_count": 1})
```

## Execution Flow Tracing

The debug system traces execution through major application flows:

### 1. Application Startup
- CLI parsing and validation
- Configuration manager initialization
- LLM provider validation and creation
- System initialization
- Command execution branching

### 2. LLM Provider Operations
- Provider factory creation
- API key resolution
- HTTP client setup
- Request/response cycles
- Error handling and retries

### 3. Interactive Mode
- UI manager initialization
- Event loop processing
- Terminal event handling
- Async coordination between components

### 4. Single Prompt Mode
- Core systems initialization
- Context building
- Plan generation
- Output formatting

## Error Propagation Tracing

When `KAI_DEBUG_ERRORS=1` is enabled, all errors are traced with:
- Error category and recoverability status
- Full error chain propagation
- Context at point of failure
- Recovery attempts and outcomes

Example error trace:
```
❌ [CONFIG] load_config_failed - File system error: Permission denied (filesystem)
❌ [MAIN] config_manager_init_failed - Configuration error: Failed to load config (config)
```

## Performance Analysis

With `KAI_DEBUG_TIMING=1`, performance metrics are collected:

### Timing Information
- Operation start timestamps
- Duration measurements
- Cumulative execution time
- Per-component performance breakdown

### Memory Tracking
- Memory usage at major checkpoints (optional)
- Resource allocation patterns
- Cleanup verification

## Async Coordination Tracing

The debug system provides visibility into async boundaries:

### Flow Context Tracking
- Parent-child flow relationships
- Async task spawning
- Cross-component communication
- Event loop iterations

### Channel Monitoring
- Event queue depths
- Message passing patterns
- Channel closure detection
- Backpressure indicators

## Debug Summary Reports

At application exit, a debug summary is generated:

```
🔍 [DEBUG-SUMMARY] Checkpoints: 127, Active flows: 0, Errors: 2
🔍 [DEBUG-SUMMARY] Component activity: {"main": 15, "config": 8, "llm": 45, "ui": 59}
```

## Integration with Existing Logging

Debug tracing integrates with the existing `tracing` infrastructure:
- Uses structured logging with spans
- Respects log level configuration
- Compatible with external log aggregation
- Maintains performance when disabled

## Production Safety

All debug features are designed to be production-safe:
- Zero overhead when disabled
- Configurable buffer limits
- Selective component activation
- Graceful degradation under load

## Troubleshooting Common Issues

### High Memory Usage
```bash
# Reduce buffer size
export KAI_DEBUG_BUFFER_SIZE=1000
```

### Too Much Output
```bash
# Filter to specific components
export KAI_DEBUG_COMPONENTS=main,llm
export KAI_DEBUG_LEVEL=2
```

### Performance Impact
```bash
# Enable only timing, disable flow tracing
export KAI_DEBUG_TIMING=1
export KAI_DEBUG_FLOW=0
```

### Async Issues
```bash
# Focus on async coordination
export KAI_DEBUG_ASYNC=1
export KAI_DEBUG_COMPONENTS=ui,planning
export KAI_DEBUG_LEVEL=4
```

## Custom Debug Integration

For developers extending KAI-X:

```rust
use crate::utils::debug::DEBUG_TRACER;
use crate::{debug_flow, debug_checkpoint, debug_error};

fn my_function() -> Result<()> {
    debug_flow!("my_component", "my_operation", {
        let mut flow_context = DEBUG_TRACER.start_flow("my_component", "my_operation");
        
        debug_checkpoint!(flow_context, "operation_start");
        
        // Your code here
        
        match some_operation() {
            Ok(result) => {
                debug_checkpoint!(flow_context, "operation_success", Some({
                    let mut state = HashMap::new();
                    state.insert("result_size".to_string(), serde_json::Value::Number(
                        serde_json::Number::from(result.len() as u64)
                    ));
                    state
                }));
                Ok(())
            }
            Err(e) => {
                debug_error!(flow_context, &e, "operation_failed");
                Err(e)
            }
        }
    })
}
```

This comprehensive debug system provides complete visibility into KAI-X execution flow while maintaining production performance and safety.