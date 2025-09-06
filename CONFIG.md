# KAI-X Configuration Guide

This guide explains how to configure KAI-X using the `config.toml` file.

## Configuration File Location

KAI-X looks for its configuration file in the standard config directory for your platform:

- **macOS**: `~/.config/kai-x/config.toml` 
- **Linux**: `~/.config/kai-x/config.toml`
- **Windows**: `%APPDATA%\kai-x\config.toml`

## Quick Setup

1. **Copy the default configuration**:
   ```bash
   # Create the config directory
   mkdir -p ~/.config/kai-x
   
   # Copy the default config.toml from the project root
   cp config.toml ~/.config/kai-x/config.toml
   ```

2. **Set your API key**:
   ```bash
   export OPENROUTER_API_KEY="your-api-key-here"
   ```

3. **Verify configuration**:
   ```bash
   kai status
   ```

## Configuration Structure

The configuration file is organized into several sections:

### Provider Settings

```toml
# Currently active provider
active_provider = "openrouter"
active_model = "anthropic/claude-3-haiku"

[providers.openrouter]
default_model = "anthropic/claude-3-haiku"

[providers.gemini]  
default_model = "gemini-pro"
```

**Important**: 
- API keys are **never** stored in the config file
- API keys are read from environment variables: `OPENROUTER_API_KEY`, `GEMINI_API_KEY`, etc.
- Base URLs are hardcoded in the application and cannot be changed

### UI Preferences

```toml
[ui]
theme = "auto"                 # "light", "dark", or "auto"
history_limit = 1000           # Number of commands to remember
show_progress = true           # Show progress bars
auto_complete_paths = true     # Auto-complete file paths with @
key_bindings = "default"       # "default", "vim", or "emacs"
```

### Context Management

```toml
[context]
max_file_size = 1048576       # 1MB max file size
max_context_size = 100000     # 100k character limit
detailed_summaries = true     # Generate detailed file summaries

# Prioritize these file types
priority_extensions = ["rs", "js", "ts", "py", "java", "go"]

# Exclude these patterns
exclude_patterns = [
    "node_modules/**",
    "target/**", 
    ".git/**"
]
```

### Task Execution

```toml
[execution]
max_concurrent_tasks = 4       # Run up to 4 tasks at once
default_timeout_seconds = 300  # 5 minute timeout
auto_retry = false             # Don't auto-retry failed tasks
max_retries = 3                # Max retry attempts
pause_on_error = true          # Pause when tasks fail
```

### Logging

```toml
[logging]
level = "info"                 # "trace", "debug", "info", "warn", "error"
log_to_file = false            # Enable file logging
include_timestamps = true      # Include timestamps
# log_file = "/path/to/logfile.log"  # Optional log file path
```

## Environment Variables

KAI-X uses environment variables for sensitive information:

| Variable | Description |
|----------|-------------|
| `OPENROUTER_API_KEY` | API key for OpenRouter |
| `GEMINI_API_KEY` | API key for Google Gemini |
| `OPENAI_API_KEY` | API key for OpenAI |
| `ANTHROPIC_API_KEY` | API key for Anthropic Claude |

Set them in your shell profile (`~/.bashrc`, `~/.zshrc`, etc.):

```bash
export OPENROUTER_API_KEY="your-key-here"
export GEMINI_API_KEY="your-gemini-key"
```

## Supported Providers

KAI-X has built-in support for these LLM providers:

| Provider | Base URL | Environment Variable |
|----------|----------|---------------------|
| OpenRouter | `https://openrouter.ai/api/v1` | `OPENROUTER_API_KEY` |
| Google Gemini | `https://generativelanguage.googleapis.com/v1beta` | `GEMINI_API_KEY` |
| OpenAI | `https://api.openai.com/v1` | `OPENAI_API_KEY` |
| Anthropic | `https://api.anthropic.com` | `ANTHROPIC_API_KEY` |

## Managing Providers

Add providers using the CLI:

```bash
# Add known providers (base URL is automatically configured)
kai provider add openrouter
kai provider add gemini
kai provider add openai
kai provider add anthropic

# Switch between providers
kai provider set gemini

# List all providers
kai provider list
```

## Configuration Commands

Use these commands to manage configuration:

```bash
# View current status and configuration
kai status

# Initialize/reset configuration
kai init --force

# Set working directory
kai --workdir /path/to/your/project

# Use slash commands in interactive mode
/model gpt-4                  # Change model
/provider openai              # Switch provider
/workdir /new/path            # Change working directory
/reset-context                # Reset context
```

## Troubleshooting

### "No providers configured"
- Ensure the config file exists at the correct location
- Check that your environment variable is set: `echo $OPENROUTER_API_KEY`
- Run `kai status` to verify configuration

### "API key not found"
- Set the appropriate environment variable for your provider
- Ensure the variable name matches exactly (e.g., `OPENROUTER_API_KEY`)
- Restart your terminal after setting environment variables

### "Configuration validation failed"
- Check the config file syntax with a TOML validator
- Ensure all required fields are present
- Run `kai init --force` to regenerate default configuration

## Example Full Configuration

Here's a complete example configuration file:

```toml
# Provider settings
active_provider = "openrouter"
active_model = "anthropic/claude-3-haiku"
working_directory = "/home/user/projects/my-app"

[providers.openrouter]
default_model = "anthropic/claude-3-haiku"

[providers.gemini]
default_model = "gemini-pro"

# UI settings
[ui]
theme = "dark"
history_limit = 2000
show_progress = true
auto_complete_paths = true
key_bindings = "vim"

# Context settings  
[context]
max_file_size = 2097152  # 2MB
max_context_size = 150000
detailed_summaries = true
priority_extensions = ["rs", "ts", "py", "md"]
exclude_patterns = ["node_modules/**", "target/**", ".git/**"]

# Execution settings
[execution]
max_concurrent_tasks = 6
default_timeout_seconds = 600  # 10 minutes
auto_retry = true
max_retries = 5
pause_on_error = false

# Logging settings
[logging]
level = "debug"
log_to_file = true
log_file = "/home/user/.local/share/kai-x/debug.log"
include_timestamps = true
```

This configuration:
- Uses OpenRouter with Claude 3 Haiku
- Has dark theme with vim keybindings
- Allows larger files (2MB) and more context (150k chars)
- Runs 6 concurrent tasks with auto-retry enabled
- Logs debug information to a file

## Security Best Practices

1. **Never commit API keys** to version control
2. **Use environment variables** for all sensitive data
3. **Set appropriate file permissions** on config files: `chmod 600 ~/.config/kai-x/config.toml`
4. **Regularly rotate API keys** according to your provider's recommendations
5. **Use the most restrictive API permissions** available for your use case

For more information, see the main project documentation.