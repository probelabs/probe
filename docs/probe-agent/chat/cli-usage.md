# Chat CLI Usage

Probe Chat provides an interactive terminal interface for AI-powered code exploration.

---

## TL;DR

```bash
# Start interactive chat
npx -y @probelabs/probe-chat@latest ./my-project

# With specific provider
probe-chat --force-provider anthropic ./src

# Single message (non-interactive)
probe-chat --message "Find authentication code" ./src
```

---

## Installation

```bash
# Via npx (no install required)
npx -y @probelabs/probe-chat@latest

# Global install
npm install -g @probelabs/probe-chat

# Then use directly
probe-chat ./my-project
```

---

## Basic Usage

### Interactive Mode

```bash
# Start chat with current directory
probe-chat

# Chat with specific path
probe-chat /path/to/project

# With debug output
probe-chat --debug ./src
```

### Non-Interactive Mode

```bash
# Single message
probe-chat --message "How does authentication work?" ./src

# JSON output
probe-chat --message "List all API endpoints" --json ./src

# With images
probe-chat --message "Analyze this diagram" --images "./diagram.png" ./src

# Piped input
echo "Find error handling code" | probe-chat ./src
```

---

## Command Options

### Provider & Model

| Flag | Description |
|------|-------------|
| `-f, --force-provider <provider>` | Force provider: `anthropic`, `openai`, `google` |
| `-m, --model-name <model>` | Specify model name |

```bash
probe-chat --force-provider anthropic --model-name claude-sonnet-4-5-20250929 ./src
```

### Interface Mode

| Flag | Description |
|------|-------------|
| `-w, --web` | Run web interface |
| `-p, --port <port>` | Web server port (default: 8080) |

```bash
probe-chat --web --port 3000 ./src
```

### Session Management

| Flag | Description |
|------|-------------|
| `-s, --session-id <id>` | Specify session ID for continuity |

```bash
probe-chat --session-id my-session ./src
```

### Custom Prompts

| Flag | Description |
|------|-------------|
| `--prompt <value>` | Custom prompt or preset |

**Presets:** `architect`, `code-review`, `code-review-template`, `support`, `engineer`

```bash
# Use preset
probe-chat --prompt architect ./src

# Use custom file
probe-chat --prompt ./my-prompt.txt ./src
```

### Code Editing

| Flag | Description |
|------|-------------|
| `--allow-edit` | Enable code editing |
| `--implement-tool-backend <backend>` | Backend: `aider`, `claude-code` |
| `--implement-tool-timeout <ms>` | Timeout in milliseconds |

```bash
probe-chat --allow-edit --implement-tool-backend claude-code ./src
```

### Bash Execution

| Flag | Description |
|------|-------------|
| `--enable-bash` | Enable bash commands |
| `--bash-allow <patterns>` | Additional allowed patterns |
| `--bash-deny <patterns>` | Additional denied patterns |
| `--bash-timeout <ms>` | Command timeout |

```bash
probe-chat --enable-bash --bash-timeout 60000 ./src
```

### Tool Iterations

| Flag | Description |
|------|-------------|
| `--max-iterations <n>` | Max tool iterations (default: 30) |

```bash
probe-chat --max-iterations 50 ./src
```

### Telemetry

| Flag | Description |
|------|-------------|
| `--trace-file [path]` | Trace to file |
| `--trace-remote [endpoint]` | Trace to remote endpoint |
| `--trace-console` | Trace to console |

```bash
probe-chat --trace-file ./traces.jsonl ./src
```

---

## Interactive Commands

During a chat session:

| Command | Description |
|---------|-------------|
| `exit` or `quit` | End session |
| `usage` | Show token usage |
| `clear` | Clear history, start new session |

---

## Environment Variables

### API Keys

```bash
export ANTHROPIC_API_KEY=sk-ant-...
export OPENAI_API_KEY=sk-...
export GOOGLE_API_KEY=...
```

### Configuration

```bash
# Force provider
export FORCE_PROVIDER=anthropic

# Model
export MODEL_NAME=claude-sonnet-4-5-20250929

# Paths
export ALLOWED_FOLDERS=/path/to/project

# Debug
export DEBUG=true
```

---

## Examples

### Code Exploration

```bash
probe-chat ./my-project
```

```
You: How is authentication implemented?
Assistant: I'll search for authentication-related code...
[Uses search tool]
The authentication system uses JWT tokens...

You: Show me the login function
Assistant: [Uses extract tool]
Here's the login function from src/auth/login.ts...
```

### Searching Dependencies

Probe can search inside your project's dependencies. Ask the agent to look inside npm packages, Go modules, or Rust crates:

```
You: How does createAnthropic work in the @ai-sdk/anthropic package?
Assistant: I'll search inside the @ai-sdk/anthropic dependency...
[Uses search with path: js:@ai-sdk/anthropic]
The createAnthropic function initializes the Anthropic client...

You: Look at how gin handles middleware
Assistant: [Uses search with path: go:github.com/gin-gonic/gin]
Here's how the gin library implements middleware...

You: Find the Serialize trait in serde
Assistant: [Uses search with path: rust:serde]
The Serialize trait is defined in...
```

**Supported dependency prefixes:**
- `js:package-name` - npm packages (e.g., `js:express`, `js:@ai-sdk/anthropic`)
- `go:module/path` - Go modules (e.g., `go:github.com/gin-gonic/gin`)
- `rust:crate-name` - Rust crates (e.g., `rust:serde`)

### Code Review

```bash
probe-chat --prompt code-review ./src
```

```
You: Review the user service
Assistant: I'll analyze the user service code...
[Reviews code]

Code Review Findings:

Critical:
- SQL injection vulnerability in line 45

Important:
- Missing input validation
- No rate limiting

Suggestions:
- Consider adding caching
```

### Debugging

```bash
probe-chat --debug --enable-bash ./src
```

```
You: Find and fix the failing test
Assistant: [Searches for test files]
[Runs tests]
[Identifies issue]
[Suggests fix]
```

### Non-Interactive Pipeline

```bash
# Generate documentation
probe-chat --message "Generate API documentation" --json ./src > api-docs.json

# Security scan
probe-chat --prompt code-review --message "Find security issues" ./src

# Batch analysis
for dir in src/*/; do
  probe-chat --message "Summarize this module" "$dir"
done
```

---

## Token Usage

Monitor token consumption:

```
You: usage

Token Usage:
  Context Window: 150,000
  Current Request:
    Input: 1,234
    Output: 567
    Total: 1,801
  Session Total:
    Input: 5,234
    Output: 2,567
    Total: 7,801
  Cache:
    Read: 500
    Write: 200
```

---

## Troubleshooting

### No API Key

```
Error: No API key found

Solution:
export ANTHROPIC_API_KEY=sk-ant-...
```

### Provider Not Available

```bash
# List available providers
probe-chat --debug

# Force specific provider
probe-chat --force-provider openai
```

### Timeout Issues

```bash
# Increase timeout
probe-chat --max-iterations 100 ./large-project
```

---

## Related Documentation

- [Web Interface](./web-interface.md) - Browser-based chat
- [Configuration](./configuration.md) - All configuration options
- [MCP Protocol](../protocols/mcp.md) - MCP integration
