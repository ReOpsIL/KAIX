# KAI-X Code Style and Conventions

## Rust Style Guidelines
Based on the project setup and architectural specification:

### General Conventions
- **Edition**: Rust 2024
- **Formatting**: Use `cargo fmt` for consistent formatting
- **Linting**: Use `cargo clippy` for code quality checks
- **Documentation**: Document public APIs with rustdoc comments

### Naming Conventions
- **Structs/Enums**: PascalCase (e.g., `LlmProvider`, `TaskStatus`)
- **Functions/Variables**: snake_case (e.g., `execute_command`, `plan_context`)
- **Constants**: SCREAMING_SNAKE_CASE (e.g., `MAX_CONTEXT_SIZE`)
- **Traits**: PascalCase with descriptive names (e.g., `LlmProvider`, `TaskExecutor`)

### Architectural Patterns
- **Trait-Based Abstractions**: Use traits for pluggable components (LLM providers, task executors)
- **Error Handling**: Use `Result<T, E>` types, implement custom error types
- **Async/Await**: Use async patterns for I/O operations and interruptible tasks
- **Serialization**: Use `serde` with strongly-typed structs for JSON communication

### Module Organization
```rust
// Expected structure based on specification
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

### JSON Schema Enforcement
- All LLM communication must use strict JSON schemas
- Use `serde` derives for automatic serialization/deserialization
- Validate schemas at compile time where possible

### Async Patterns
- Use `tokio` for async runtime
- Implement cancellation tokens for interruptible operations
- Use channels for communication between async tasks

### Security Considerations
- Sandbox all file operations to working directory
- Validate all user inputs and external command arguments
- Never execute unsanitized shell commands