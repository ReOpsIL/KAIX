# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

KAI-X is a sophisticated Rust-based AI coding assistant CLI application. It leverages Large Language Models (LLMs) to interpret user prompts, generate and execute complex plans, and intelligently manage the context of software projects. The project is currently in the architectural design phase with comprehensive specifications but minimal implementation.

## Development Commands

### Essential Rust Commands
```bash
# Build and run
cargo build                 # Build debug version
cargo run                   # Run debug version
cargo build --release      # Build release version
cargo run --release        # Run release version

# Development workflow
cargo check                 # Check for compilation errors (fast)
cargo fmt                   # Format code according to Rust standards
cargo clippy                # Run linter for code quality
cargo test                  # Run all tests
cargo test -- --nocapture  # Run tests with output
cargo test test_name        # Run specific test

# Project inspection
cargo tree                  # Show dependency tree
find . -name "*.rs" -type f # List all Rust source files
```

## Architecture and Design

### Core System Components
The project follows a modular, llm-based architecture with these key components:

1. **Interactive Command Center**: TUI-based chat loop using libraries like `inquire`
2. **LLM-Powered Planning Engine**: Hierarchical task decomposition with recursive refinement
3. **Dual-Context Management**: Global project context + temporary plan context
4. **Task Execution Engine**: Prioritized, interruptible task execution framework
5. **Pluggable LLM Provider Layer**: Abstract `LlmProvider` trait with concrete implementations

### Agent-Based Development Model
The project defines specialized modules for different development aspects:
- `system_architect`: Overall architecture and design consistency
- `planning`: LLM loop and task planning logic  
- `context_manager`: Context harvesting and file management
- `task_execution_specialist`: Primitive task execution
- `llm_integration_specialist`: LLM communication and prompt engineering
- `cli_interface_designer`: TUI and user experience

### Key Design Principles
- **Trait-based abstractions** for pluggable components
- **Sandboxed operations** constrained to working directory (workdir)
- **Interruptible execution** allowing plan modification mid-execution
- **Tool-first design** leveraging native LLM tool-use capabilities
- **Strict JSON schemas** for all LLM communication using `serde`

### Expected Project Structure
```
src/
├── main.rs                 // Entry point and CLI setup
├── lib.rs                  // Library root
├── planning/               // Planning engine components
├── context/                // Context management
├── execution/              // Task execution engine
├── llm/                    // LLM provider abstractions
├── ui/                     // Terminal user interface
├── config/                 // Configuration management
└── utils/                  // Shared utilities
```

## Code Style and Conventions

### Rust Standards
- **Edition**: Rust 2024
- **Naming**: PascalCase for structs/enums, snake_case for functions/variables
- **Error Handling**: Use `Result<T, E>` types with custom error enums
- **Async Patterns**: Use `tokio` runtime with cancellation tokens for interruptible operations
- **Serialization**: Use `serde` with strongly-typed structs for JSON communication

### Development Workflow
Before committing changes:
1. `cargo fmt` - Format code
2. `cargo clippy` - Address linter warnings
3. `cargo test` - Ensure all tests pass
4. `cargo build` - Verify compilation

## Important Files

- `docs/spec.md`: Comprehensive 259-line architectural specification
- `Cargo.toml`: Rust project configuration (currently minimal)
- `src/main.rs`: Entry point (currently "Hello, world!" placeholder)

## Current State

This project is in the early design phase. The architectural specification is comprehensive and detailed, but actual implementation is minimal. When working on this codebase, refer to the detailed specifications in the `docs/` directory to understand the intended design patterns and component interactions.