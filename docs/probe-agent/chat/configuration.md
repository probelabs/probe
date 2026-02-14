# Chat Configuration

Complete configuration reference for Probe Chat CLI and Web interface.

---

## Environment Variables

### API Keys

```bash
# Anthropic Claude (recommended)
ANTHROPIC_API_KEY=sk-ant-...

# OpenAI GPT
OPENAI_API_KEY=sk-...

# Google Gemini
GOOGLE_API_KEY=...
```

### Provider Configuration

```bash
# Force specific provider
FORCE_PROVIDER=anthropic  # anthropic, openai, google

# Model override
MODEL_NAME=claude-sonnet-4-5-20250929

# Custom API endpoints
LLM_BASE_URL=https://your-proxy.com
ANTHROPIC_API_URL=https://your-anthropic-proxy.com
OPENAI_API_URL=https://your-openai-proxy.com
GOOGLE_API_URL=https://your-google-proxy.com
```

### Path Configuration

```bash
# Comma-separated list of search paths
ALLOWED_FOLDERS=/path/to/project1,/path/to/project2
```

### Web Interface

```bash
# Server port
PORT=8080

# Basic authentication
AUTH_ENABLED=1
AUTH_USERNAME=admin
AUTH_PASSWORD=your-password
```

### Debug & Logging

```bash
# Enable debug output
DEBUG=true
DEBUG_CHAT=1

# Suppress interactive prompts
PROBE_NON_INTERACTIVE=1
```

### Prompt Configuration

```bash
# Prompt preset
PROMPT_TYPE=architect  # architect, code-review, code-review-template, support, engineer

# Custom prompt file
CUSTOM_PROMPT=/path/to/prompt.txt
```

---

## Implementation Tool

### Backend Selection

```bash
# Primary backend
IMPLEMENT_TOOL_BACKEND=claude-code  # aider, claude-code

# Fallback backends (comma-separated)
IMPLEMENT_TOOL_FALLBACKS=aider,claude-code

# Timeout (seconds, 60-3600)
IMPLEMENT_TOOL_TIMEOUT=1200

# Config file path
IMPLEMENT_TOOL_CONFIG_PATH=/path/to/config.json
```

### Aider Backend

```bash
AIDER_MODEL=gpt-4
AIDER_TIMEOUT=300000
AIDER_AUTO_COMMIT=false
AIDER_ADDITIONAL_ARGS=--no-auto-commits
```

### Claude Code Backend

```bash
CLAUDE_CODE_MODEL=claude-3-5-sonnet-20241022
CLAUDE_CODE_MAX_TOKENS=8000
CLAUDE_CODE_TEMPERATURE=0.3
CLAUDE_CODE_MAX_TURNS=100
```

---

## Bash Execution

```bash
# Enable bash commands
ENABLE_BASH=1
```

---

## MCP Integration

```bash
# Enable MCP
ENABLE_MCP=1

# Config file path
MCP_CONFIG_PATH=/path/to/mcp/config.json

# Debug MCP
DEBUG_MCP=1
```

---

## Telemetry

```bash
# File tracing
OTEL_ENABLE_FILE=true
OTEL_FILE_PATH=./traces.jsonl

# Remote tracing
OTEL_ENABLE_REMOTE=true
OTEL_EXPORTER_OTLP_TRACES_ENDPOINT=http://localhost:4318/v1/traces

# Console tracing
OTEL_ENABLE_CONSOLE=true
```

---

## Configuration Files

### implement-config.json

```json
{
  "implement": {
    "defaultBackend": "claude-code",
    "fallbackBackends": ["aider"],
    "selectionStrategy": "auto",
    "maxConcurrentSessions": 3,
    "timeout": 300000,
    "retryAttempts": 2,
    "retryDelay": 5000
  },
  "backends": {
    "aider": {
      "command": "aider",
      "timeout": 300000,
      "maxOutputSize": 10485760,
      "additionalArgs": [],
      "environment": {},
      "autoCommit": false,
      "modelSelection": "auto"
    },
    "claude-code": {
      "timeout": 300000,
      "maxTokens": 8000,
      "temperature": 0.3,
      "model": "claude-3-5-sonnet-20241022",
      "systemPrompt": null,
      "tools": ["edit", "search", "bash"],
      "maxTurns": 100
    }
  }
}
```

### MCP Configuration (.mcp/config.json)

```json
{
  "mcpServers": {
    "probe": {
      "command": "npx",
      "args": ["-y", "@probelabs/probe@latest", "mcp"],
      "transport": "stdio",
      "enabled": true
    },
    "github": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "transport": "stdio",
      "enabled": false,
      "env": {
        "GITHUB_TOKEN": "your-token"
      }
    },
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/allowed/path"],
      "transport": "stdio",
      "enabled": false
    }
  },
  "settings": {
    "timeout": 30000,
    "retryCount": 3,
    "debug": false
  }
}
```

---

## Config File Locations

### Implementation Tool Config

Priority order:
1. `IMPLEMENT_TOOL_CONFIG_PATH` environment variable
2. `./implement-config.json` (local)
3. Default configuration

### MCP Config

Priority order:
1. `MCP_CONFIG_PATH` environment variable
2. `./.mcp/config.json`
3. `./mcp.config.json`
4. `~/.config/probe/mcp.json`
5. `~/.mcp/config.json`
6. `~/Library/Application Support/Claude/mcp_config.json` (macOS)

---

## CLI Flags Reference

### Provider & Model

```bash
-f, --force-provider <provider>    # anthropic, openai, google
-m, --model-name <model>           # Model identifier
```

### Interface

```bash
-w, --web                          # Web interface mode
-p, --port <port>                  # Web server port
-d, --debug                        # Debug mode
```

### Session

```bash
-s, --session-id <id>              # Session identifier
```

### Non-Interactive

```bash
--message <message>                # Single message, then exit
--json                             # JSON output
--images <urls>                    # Comma-separated image URLs
```

### Prompts

```bash
--prompt <value>                   # Preset or file path
--completion-prompt <prompt>       # Post-completion validation
--architecture-file <name>         # Architecture context file
```

### Code Editing

```bash
--allow-edit                       # Enable editing
--implement-tool-backend <backend> # aider, claude-code
--implement-tool-timeout <ms>      # Timeout
--implement-tool-config <path>     # Config file
--implement-tool-list-backends     # List backends
--implement-tool-backend-info <b>  # Backend info
```

### Bash

```bash
--enable-bash                      # Enable bash
--bash-allow <patterns>            # Allowed patterns
--bash-deny <patterns>             # Denied patterns
--no-default-bash-allow            # Disable default allow
--no-default-bash-deny             # Disable default deny
--bash-timeout <ms>                # Command timeout
--bash-working-dir <path>          # Working directory
```

### Tool Control

```bash
--max-iterations <n>               # Max tool iterations
```

### Telemetry

```bash
--trace-file [path]                # File tracing
--trace-remote [endpoint]          # Remote tracing
--trace-console                    # Console tracing
```

---

## Quick Start Examples

### Minimal Setup

```bash
export ANTHROPIC_API_KEY=sk-ant-...
probe-chat ./my-project
```

### Full Configuration

```bash
export ANTHROPIC_API_KEY=sk-ant-...
export FORCE_PROVIDER=anthropic
export MODEL_NAME=claude-sonnet-4-5-20250929
export ENABLE_MCP=1
export DEBUG=true

probe-chat \
  --allow-edit \
  --enable-bash \
  --prompt architect \
  --max-iterations 50 \
  ./my-project
```

### Web Server

```bash
export ANTHROPIC_API_KEY=sk-ant-...
export PORT=3000
export AUTH_ENABLED=1
export AUTH_USERNAME=admin
export AUTH_PASSWORD=secure-password

probe-chat --web ./my-project
```

---

## Related Documentation

- [CLI Usage](./cli-usage.md) - Interactive CLI guide
- [Web Interface](./web-interface.md) - Web UI guide
- [MCP Protocol](../protocols/mcp.md) - MCP configuration
- [Environment Variables](../../reference/environment-variables.md) - All variables
