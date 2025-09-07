# KAI-X Architecture Documentation

This directory contains comprehensive architecture documentation for the KAI-X AI coding assistant project. The documentation is organized into several focused documents, each covering different aspects of the system architecture.

## 📁 Documentation Structure

### 1. [System Architecture](system-architecture.md)
**Overview**: High-level system architecture and component organization
- **Contains**: Layered architecture diagram with all major components
- **Audience**: Developers, architects, and stakeholders seeking system overview
- **Key Diagrams**: 
  - High-level system architecture with color-coded layers
  - Component descriptions and responsibilities
  - Architectural patterns and design principles

### 2. [Data Flow Diagrams](data-flow-diagram.md)
**Overview**: Detailed data flow patterns throughout the system
- **Contains**: Sequence diagrams, flowcharts, and state diagrams
- **Audience**: Developers working on data processing and system integration
- **Key Diagrams**:
  - Main application data flow (sequence diagram)
  - Task execution flow (flowchart)
  - LLM provider data flow
  - Context management flow (state diagram)
  - Configuration flow
  - Error handling flow

### 3. [Module Interactions](module-interactions.md)
**Overview**: Inter-module dependencies and communication patterns
- **Contains**: Dependency graphs, interaction matrices, and communication protocols
- **Audience**: Developers working on module integration and system design
- **Key Diagrams**:
  - Module dependency graph
  - Layered architecture overview
  - Communication patterns (request-response, event-driven, factory)
  - Module responsibility matrix
  - Coupling analysis

### 4. [Component Relationships](component-relationships.md)
**Overview**: Detailed internal architecture of each module
- **Contains**: Class diagrams, internal component structures, and execution patterns
- **Audience**: Developers working on specific modules or components
- **Key Diagrams**:
  - Execution engine internal architecture (class diagram)
  - LLM provider architecture (class diagram)
  - Context management architecture
  - UI component architecture
  - Planning system architecture
  - Configuration system architecture
  - Agentic loop state diagram
  - Streaming architecture

## 🏗️ System Overview

KAI-X is a sophisticated Rust-based AI coding assistant that follows a modular, layered architecture with console-based interaction:

### Core Architecture Layers
1. **User Interface Layer**: Console chat interface, CLI interface, and input/output handling
2. **Core Application Layer**: Main entry point, configuration manager, context manager
3. **AI Processing Layer**: LLM provider abstraction, OpenRouter/Gemini providers, prompt templates
4. **Execution Layer**: Execution engine, task executor, dual-queue system, adaptive task decomposition
5. **Planning & Context Layer**: Planning manager, global context, plan context
6. **Utilities & Support Layer**: Debug system, file operations, HTTP retry logic, templates

### Key Architectural Patterns
- **Console-based Interface**: Simple, efficient terminal interaction (moved from TUI to console)
- **Async/Await**: Extensive asynchronous programming with tokio runtime
- **Trait-based Abstraction**: Flexible LLM provider implementations (LlmProvider trait)
- **Dual-Queue System**: High-priority user prompts + main task execution queue
- **Adaptive Task Decomposition**: LLM-powered failure analysis and alternative task generation
- **Security-first Design**: Strict workdir validation and sandboxing
- **Context Management**: Global project context + temporary plan context

## 🔄 System Flow Summary

1. **Initialization**: Configuration loading → Workdir validation → Context initialization → ExecutionEngine setup
2. **User Interaction**: Console input → Prompt queuing → Plan generation → Task execution monitoring
3. **AI Processing**: Prompt templates → Provider selection → API calls → Structured JSON responses
4. **Task Execution**: Plan generation → Task decomposition → Individual task execution → Adaptive failure handling
5. **Context Management**: File monitoring → Context updates → Plan context maintenance

## 🚀 Current Implementation Status

- ✅ **Console Chat Interface**: Fully implemented, replacing TUI framework
- ✅ **Configuration System**: Complete TOML-based configuration with environment variable support
- ✅ **LLM Providers**: OpenRouter and Gemini implementations with streaming support
- ✅ **Execution Engine**: Dual-queue system with concurrent task processing
- ✅ **Adaptive Task Decomposition**: LLM-powered failure analysis and alternative generation
- ✅ **Security Validation**: Workdir enforcement and path validation
- ✅ **Debug System**: Comprehensive debug tracing with configurable levels
- ✅ **Context Management**: Global context with file change monitoring

## 🧩 Module Responsibilities

| Module | Primary Function | Key Components |
|--------|-----------------|----------------|
| **config** | Configuration management | ConfigManager, TomlConfigProvider, ConfigData |
| **context** | State and context management | ContextManager, GlobalContext, PlanContext |
| **execution** | Task orchestration & adaptive decomposition | ExecutionEngine, TaskExecutor, AgenticPlanningCoordinator |
| **llm** | AI provider abstraction | LlmProvider trait, OpenRouter/Gemini providers, PromptTemplates |
| **planning** | Intelligent task planning | PlanningManager, Plan/Task structures |
| **ui** | Console interface | ConsoleChat, UiManager, SlashCommands, FileSystemBrowser |
| **utils** | Common utilities | Debug system, HTTP retry, Templates, FileSystem operations |

## 📋 Usage Guide

### For New Developers
1. Start with [System Architecture](system-architecture.md) for overall understanding
2. Review [Data Flow Diagrams](data-flow-diagram.md) to understand system behavior
3. Consult [Module Interactions](module-interactions.md) for integration work
4. Reference [Component Relationships](component-relationships.md) for detailed implementation

### For System Architects
- [System Architecture](system-architecture.md): Design patterns and architectural decisions
- [Module Interactions](module-interactions.md): Coupling analysis and dependency management
- [Component Relationships](component-relationships.md): Internal component design

### For Feature Developers
- [Data Flow Diagrams](data-flow-diagram.md): Understanding request processing
- [Component Relationships](component-relationships.md): Module-specific implementation details
- [Module Interactions](module-interactions.md): Cross-module communication patterns

## 🔧 Maintenance Notes

These architecture diagrams should be updated when:
- New modules are added to the system
- Module responsibilities change significantly
- New communication patterns are introduced
- Major refactoring affects component relationships
- New external dependencies are added

## 📊 Diagram Formats

All diagrams are created using [Mermaid](https://mermaid.js.org/) syntax for:
- Easy version control and diff tracking
- Automatic rendering in GitHub and documentation platforms
- Simple text-based editing and maintenance
- Integration with development workflows

## 🎯 Quick Navigation

- **System Overview**: [System Architecture](system-architecture.md)
- **Data Processing**: [Data Flow Diagrams](data-flow-diagram.md)
- **Module Design**: [Module Interactions](module-interactions.md)
- **Internal Structure**: [Component Relationships](component-relationships.md)

---

**Last Updated**: 2025-09-06  
**Version**: 1.1 (Console Interface Implementation)  
**Maintainer**: Development Team  

## 📊 Recent Major Changes

### v1.1 (September 2025)
- **UI Architecture Change**: Migrated from TUI framework (ratatui) to simple console-based chat interface
- **Adaptive Task Decomposition**: Added LLM-powered failure analysis and alternative task generation
- **Enhanced Security**: Implemented strict workdir validation and path sandboxing
- **Debug System**: Comprehensive debug tracing with KAI_DEBUG environment variable support
- **Configuration System**: Complete TOML-based configuration with environment variable integration
- **Execution Engine**: Dual-queue system with concurrent task processing and real-time status updates
- **Plan Generation Timeout**: Extended timeout from 10s to 60s for complex project planning