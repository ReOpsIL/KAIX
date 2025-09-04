# KAI-X Project Overview

## Purpose
KAI-X is an ambitious Rust-based AI coding assistant CLI application. It's designed to be a sophisticated command-line tool that leverages Large Language Models (LLMs) to interpret user prompts, generate and execute complex plans, and intelligently manage the context of software projects.

## Tech Stack
- **Language**: Rust (Edition 2024)
- **Toolchain**: Rust 1.89.0, Cargo 1.89.0
- **Target Platform**: macOS/Darwin (but designed for cross-platform compatibility)

## Current State
This appears to be an early-stage project with:
- Basic Rust project structure (Cargo.toml, src/main.rs with "Hello, world!")
- Comprehensive architectural specification (259 lines in docs/spec.md)
- Detailed agent definitions for different components
- No actual implementation yet - this is in the design/planning phase

## Key Features (Planned)
1. Interactive terminal user interface (TUI) with chat loop
2. LLM-powered planning engine with hierarchical task decomposition
3. Dual-context management (global project context + temporary plan context)
4. Prioritized, interruptible task execution framework
5. Pluggable LLM provider architecture (OpenRouter, Gemini)
6. File system browser integration
7. Rich text editing with vim-style commands
8. Slash commands for configuration
9. Working directory (workdir) sandboxing