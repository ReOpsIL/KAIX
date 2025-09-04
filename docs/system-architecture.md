# KAI-X System Architecture

This document contains the architecture diagrams for the KAI-X AI coding assistant.

## High-Level System Architecture

```mermaid
graph TB
    subgraph "User Interface Layer"
        CLI[CLI Interface]
        TUI[Terminal UI Components]
        Events[Event System]
    end
    
    subgraph "Core Application"
        Main[main.rs - Entry Point]
        Config[Configuration Manager]
        Context[Context Manager]
        UI[UI Manager]
    end
    
    subgraph "AI Processing Layer"
        LLM[LLM Provider Factory]
        OpenRouter[OpenRouter Provider]
        Gemini[Gemini Provider]
        Streaming[Streaming Support]
    end
    
    subgraph "Execution Layer"
        Engine[Execution Engine]
        Executor[Task Executor]
        Queue[Task Queue System]
    end
    
    subgraph "Planning & Context"
        Planning[Planning Manager]
        PlanContext[Plan Context]
        GlobalContext[Global Context]
    end
    
    subgraph "Utilities & Support"
        Utils[Path Utils]
        FileSystem[File System Operations]
        Logging[Logging System]
    end
    
    %% User Interface connections
    CLI --> Main
    TUI --> UI
    Events --> UI
    
    %% Core Application connections
    Main --> Config
    Main --> Context
    Main --> Engine
    Main --> UI
    Main --> LLM
    
    %% AI Processing connections
    LLM --> OpenRouter
    LLM --> Gemini
    LLM --> Streaming
    Engine --> LLM
    Planning --> LLM
    
    %% Execution Layer connections
    Engine --> Executor
    Engine --> Queue
    Engine --> Planning
    Executor --> FileSystem
    Executor --> Utils
    
    %% Context connections
    Context --> GlobalContext
    Context --> PlanContext
    Planning --> PlanContext
    Engine --> Context
    
    %% Support connections
    Config --> Utils
    Executor --> Logging
    Engine --> Logging
    
    %% External integrations
    OpenRouter -.-> Internet[External AI APIs]
    Gemini -.-> Internet
    FileSystem -.-> Disk[Local File System]
    
    classDef userLayer fill:#e1f5fe
    classDef coreLayer fill:#f3e5f5
    classDef aiLayer fill:#e8f5e8
    classDef execLayer fill:#fff3e0
    classDef planLayer fill:#fce4ec
    classDef utilLayer fill:#f1f8e9
    
    class CLI,TUI,Events userLayer
    class Main,Config,Context,UI coreLayer
    class LLM,OpenRouter,Gemini,Streaming aiLayer
    class Engine,Executor,Queue execLayer
    class Planning,PlanContext,GlobalContext planLayer
    class Utils,FileSystem,Logging utilLayer
```

## Component Descriptions

### User Interface Layer
- **CLI Interface**: Command-line argument parsing and main entry point
- **Terminal UI Components**: Interactive terminal user interface elements
- **Event System**: Handles user input events and UI interactions

### Core Application
- **main.rs**: Application entry point, initializes all systems
- **Configuration Manager**: Handles application configuration and settings
- **Context Manager**: Manages application state and context
- **UI Manager**: Coordinates user interface components

### AI Processing Layer
- **LLM Provider Factory**: Creates and manages AI provider instances
- **OpenRouter Provider**: Integration with OpenRouter API
- **Gemini Provider**: Integration with Google Gemini API
- **Streaming Support**: Handles streaming responses from AI providers

### Execution Layer
- **Execution Engine**: Main orchestrator for task execution with agentic loop
- **Task Executor**: Executes individual tasks (file operations, commands, etc.)
- **Task Queue System**: Manages dual-queue system for task scheduling

### Planning & Context
- **Planning Manager**: Handles intelligent task planning
- **Plan Context**: Maintains context specific to execution plans
- **Global Context**: Manages global application state and context

### Utilities & Support
- **Path Utils**: File path manipulation utilities
- **File System Operations**: Low-level file system interactions
- **Logging System**: Application logging and tracing

## Key Architectural Patterns

1. **Modular Design**: Clear separation of concerns across different layers
2. **Async/Await**: Extensive use of asynchronous programming for performance
3. **Trait-based Abstraction**: AI providers implement common traits for flexibility
4. **Factory Pattern**: LLM providers are created through a factory for extensibility
5. **Event-Driven UI**: User interface responds to events for better interactivity
6. **Context Management**: Centralized state management across the application