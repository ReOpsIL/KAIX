# KAI-X Architecture and Code Structure

## High-Level Architecture
KAI-X is architected as a modular, event-driven CLI application with the following core components:

### 1. Interactive Command Center
- Persistent chat loop for user interaction
- Terminal User Interface (TUI) using libraries like `inquire`
- Real-time display of plan execution and task status

### 2. LLM-Powered Planning Engine
- Hierarchical task decomposition
- Recursive refinement of abstract tasks into executable primitives
- Structured plan generation using JSON schemas
- Support for plan interruption and modification

### 3. Dual-Context Management System
- **Global Context**: High-level project summary, updated iteratively
- **Temporary Plan Context**: Short-term memory for single plan execution
- Intelligent file filtering (respects .gitignore and .aiignore)

### 4. Task Execution Engine
- Prioritized dual-queue system (user prompts vs. main tasks)
- Primitive task types: read_file, write_file, execute_command, etc.
- Interruptible execution with async/await patterns

### 5. Pluggable LLM Provider Layer
- Abstract `LlmProvider` trait
- Concrete implementations for OpenRouter and Gemini
- Tool-use/function-calling integration

## Agent-Based Development Model
The project defines specialized agents for different aspects:

### Core Development Agents
- **system_architect**: Overall architecture and design consistency
- **planning_agent**: Agentic loop and task planning logic
- **context_manager**: Context harvesting and management
- **task_execution_specialist**: Primitive task execution
- **llm_integration_specialist**: LLM communication and prompt engineering
- **cli_interface_designer**: TUI and user experience
- **configuration_and_state_manager**: Settings and persistence

### Additional Support Agents  
- **integration_coordinator**: Testing and quality assurance
- Various Claude Code specific agents for CLI functionality

## Current Project Structure
```
KAI-X/
├── Cargo.toml          # Rust project configuration
├── src/
│   └── main.rs         # Entry point (currently just "Hello, world!")
├── docs/
│   ├── spec.md         # Comprehensive 259-line architectural specification
│   ├── agents.json     # Agent definitions for AI assistant development
│   └── org.json        # Claude Code specific agent definitions
├── target/             # Build artifacts
└── .claude/            # Claude Code agent configurations
```

## Key Design Principles
1. **Modular Services**: Independent, stateful UI services rather than monolithic state
2. **Sandboxed Operations**: All operations constrained to working directory (workdir)
3. **Interruptible Execution**: User can modify plans mid-execution
4. **Context-Aware**: Rich understanding of project state and user intent
5. **Tool-First Design**: Leverages native LLM tool-use capabilities