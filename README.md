# KAI-X

A sophisticated Rust-based AI coding assistant CLI that enhances your development workflow with intelligent code assistance.

## 🚀 Features

- **Agentic AI Coordination**: Sophisticated agentic planning coordinator with autonomous task execution and adaptive planning
- **Multiple LLM Provider Support**: Integration with OpenRouter and Gemini AI providers with streaming support
- **Advanced Terminal UI**: Rich terminal user interface with ratatui featuring chat components, plan visualization, status tracking, and progress monitoring
- **Intelligent Context Management**: Advanced context system with memory management, health monitoring, file modification tracking, and incremental updates
- **Smart Text Editing**: Professional text editing capabilities with input buffering, command history, fuzzy search, and intelligent auto-completion
- **Comprehensive Task Execution**: Multi-type task executor supporting file operations, command execution, code analysis, and content generation
- **Message-Based Communication**: Asynchronous message passing with priority handling for user prompts and system coordination
- **Performance Monitoring**: Built-in metrics tracking, health checks, and maintenance operations
- **File System Integration**: Smart file discovery with advanced filtering, glob pattern support, and ignore file handling
- **Extensible Architecture**: Modular design with trait-based abstractions for easy customization and extension

## 📦 Installation

### Prerequisites

- Rust 1.70+ (2021 edition)
- Cargo package manager

### From Source

```bash
git clone https://github.com/your-org/KAI-X.git
cd KAI-X
cargo build --release
```

The binary will be available at `target/release/kai`.

## 🛠️ Usage

Run KAI-X using the `kai` command:

```bash
# Basic usage
kai

# For help and available options
kai --help
```

## 🏗️ Architecture

KAI-X is built with a modular architecture consisting of:

- **LLM Module**: Integration with various AI language model providers
- **Execution Engine**: Task execution and queue management
- **Context Management**: Global state and context handling
- **UI Components**: Terminal user interface and interactions
- **Planning System**: Intelligent task planning and execution
- **Utilities**: Helper functions and common utilities

## 🔧 Configuration

KAI-X supports various configuration options through TOML files and environment variables. Configuration files are typically stored in your system's config directory.

## 🤝 Contributing

We welcome contributions! Please feel free to submit pull requests or open issues for bugs and feature requests.

### Development Setup

```bash
# Clone the repository
git clone https://github.com/your-org/KAI-X.git
cd KAI-X

# Install dependencies and run tests
cargo test

# Run in development mode
cargo run
```

## 📄 License

This project is licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT License ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## 🔗 Links

- [Repository](https://github.com/your-org/KAI-X)
- [Documentation](docs/)
- [Issues](https://github.com/your-org/KAI-X/issues)

---

**KAI-X** - Empowering developers with AI-driven coding assistance.