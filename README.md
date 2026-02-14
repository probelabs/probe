<p align="center">
  <img src="logo.png?2" alt="Probe Logo" width="400">
</p>

# Probe

Probe is an **AI-friendly, fully local, semantic code search** tool designed to power the next generation of AI coding assistants. By combining the speed of [ripgrep](https://github.com/BurntSushi/ripgrep) with the code-aware parsing of [tree-sitter](https://tree-sitter.github.io/tree-sitter/), Probe delivers precise results with complete code blocks—perfect for large codebases and AI-driven development workflows.

---

## 30-Second Setup for Claude Code / Cursor / Windsurf

Add Probe to your AI editor to instantly enable intelligent code search across large codebases.

**Claude Code** - Add to `~/.claude/claude_desktop_config.json`:
```json
{
  "mcpServers": {
    "probe": {
      "command": "npx",
      "args": ["-y", "@probelabs/probe@latest", "mcp"]
    }
  }
}
```

**Cursor** - Add to `.cursor/mcp.json` in your project:
```json
{
  "mcpServers": {
    "probe": {
      "command": "npx",
      "args": ["-y", "@probelabs/probe@latest", "mcp"]
    }
  }
}
```

**Windsurf** - Add to `~/.codeium/windsurf/mcp_config.json`:
```json
{
  "mcpServers": {
    "probe": {
      "command": "npx",
      "args": ["-y", "@probelabs/probe@latest", "mcp"]
    }
  }
}
```

Then ask your AI:
> "Search the codebase for authentication implementations"
>
> "Find all functions related to error handling in src/"

---

## Table of Contents

- [Quick Start](#quick-start)
- [Features](#features)
- [Usage Modes](#usage-modes)
  - [MCP Server](#mcp-server-for-ai-editors)
  - [CLI Agent](#cli-agent)
  - [Interactive Chat](#interactive-chat)
  - [Direct CLI](#direct-cli-commands)
  - [Node.js SDK](#nodejs-sdk)
- [Installation](#installation)
- [Supported Languages](#supported-languages)
- [Documentation](#documentation)

---

## Quick Start

### Option 1: MCP Server (Recommended for AI Editors)

No installation needed - just add the config above to your AI editor.

### Option 2: CLI Agent

Ask questions about any codebase directly from your terminal:

```bash
# One-shot question
npx -y @probelabs/probe@latest agent "How is authentication implemented?"

# With specific path
npx -y @probelabs/probe@latest agent "Find error handling patterns" --path ./src

# With code editing capabilities
npx -y @probelabs/probe@latest agent "Refactor the login function" --allow-edit
```

### Option 3: Interactive Chat

```bash
# Set your API key
export ANTHROPIC_API_KEY=your_key  # or OPENAI_API_KEY

# Start interactive chat
npx -y @probelabs/probe-chat ./your-project
```

### Option 4: Direct CLI Commands

```bash
# Install globally
npm install -g @probelabs/probe

# Semantic search
probe search "authentication AND login" ./src

# Extract code block at line 42
probe extract src/main.rs:42

# AST pattern matching
probe query "fn $NAME($$$) -> Result<$RET>" --language rust
```

---

## Features

- **AI-Friendly**: Extracts **entire functions, classes, or structs** so AI models get full context
- **Fully Local**: Keeps your code on your machine—no external APIs for search
- **Blazing Fast**: Powered by ripgrep for instant scanning of massive codebases
- **Code-Aware**: Tree-sitter parsing understands code structure accurately
- **Smart Ranking**: BM25, TF-IDF, and hybrid ranking for relevant results
- **Multi-Language**: Rust, Python, JavaScript, TypeScript, Go, C/C++, Java, and more
- **Flexible**: MCP server, CLI agent, interactive chat, or direct SDK

---

## Usage Modes

### MCP Server (for AI Editors)

Run Probe as an MCP (Model Context Protocol) server to integrate with Claude Code, Cursor, Windsurf, and other AI editors.

```bash
npx -y @probelabs/probe@latest mcp
```

**Configuration options:**

```json
{
  "mcpServers": {
    "probe": {
      "command": "npx",
      "args": ["-y", "@probelabs/probe@latest", "mcp", "--timeout", "60"]
    }
  }
}
```

**Available tools when using MCP:**
- `search` - Semantic code search with Elasticsearch-style queries
- `query` - AST-based structural pattern matching
- `extract` - Extract code blocks by line number or symbol name

---

### CLI Agent

The agent provides AI-powered code exploration directly from your terminal.

```bash
# Basic usage
npx -y @probelabs/probe@latest agent "How does the ranking algorithm work?"

# Specify search path
npx -y @probelabs/probe@latest agent "Find API endpoints" --path ./src

# Use specific AI provider
npx -y @probelabs/probe@latest agent "Explain authentication" --provider anthropic

# Enable code editing
npx -y @probelabs/probe@latest agent "Add error handling to login()" --allow-edit

# Use custom persona
npx -y @probelabs/probe@latest agent "Review this code" --prompt code-review

# Run as MCP server
npx -y @probelabs/probe@latest agent --mcp

# Run as ACP server (Agent Communication Protocol)
npx -y @probelabs/probe@latest agent --acp
```

**Agent options:**

| Option | Description |
|--------|-------------|
| `--path <dir>` | Search directory (default: current) |
| `--provider <name>` | AI provider: `anthropic`, `openai`, `google` |
| `--model <name>` | Override model name |
| `--prompt <type>` | Persona: `code-explorer`, `engineer`, `code-review`, `support`, `architect` |
| `--allow-edit` | Enable code modification |
| `--enable-delegate` | Enable task delegation to subagents |
| `--enable-bash` | Enable bash command execution |
| `--allow-skills` | Enable skill discovery |
| `--allow-tasks` | Enable task tracking |
| `--max-iterations <n>` | Max tool iterations (default: 30) |
| `--mcp` | Run as MCP server |
| `--acp` | Run as ACP server |

---

### Interactive Chat

Full-featured chat interface with conversation history and streaming responses.

```bash
# Quick start
export ANTHROPIC_API_KEY=your_key
npx -y @probelabs/probe-chat ./your-project

# With web interface
npx -y @probelabs/probe-chat --web ./your-project

# With code editing
npx -y @probelabs/probe-chat --allow-edit ./your-project
```

**Features:**
- Multi-turn conversations with context
- Token usage tracking
- Code editing support (aider, claude-code backends)
- Web interface option
- Session persistence

---

### Direct CLI Commands

For scripting and direct code analysis.

#### Search Command

```bash
probe search <PATTERN> [PATH] [OPTIONS]
```

**Examples:**
```bash
# Basic search
probe search "authentication" ./src

# Boolean operators
probe search "error AND handling" ./
probe search "login OR auth" ./src
probe search "database NOT sqlite" ./

# Search hints (file filters)
probe search "function AND ext:rs" ./           # Only .rs files
probe search "class AND file:src/**/*.py" ./    # Python files in src/
probe search "error AND dir:tests" ./           # Files in tests/

# Limit results for AI context windows
probe search "API" ./ --max-tokens 10000
```

**Key options:**

| Option | Description |
|--------|-------------|
| `--max-tokens <n>` | Limit total tokens returned |
| `--max-results <n>` | Limit number of results |
| `--reranker <algo>` | Ranking: `bm25`, `tfidf`, `hybrid`, `hybrid2` |
| `--allow-tests` | Include test files |
| `--format <fmt>` | Output: `markdown`, `json`, `xml` |

#### Extract Command

```bash
probe extract <FILES> [OPTIONS]
```

**Examples:**
```bash
# Extract function at line 42
probe extract src/main.rs:42

# Extract by symbol name
probe extract src/main.rs#authenticate

# Extract line range
probe extract src/main.rs:10-50

# From compiler output
go test | probe extract
```

#### Query Command (AST Patterns)

```bash
probe query <PATTERN> [PATH] [OPTIONS]
```

**Examples:**
```bash
# Find all async functions in Rust
probe query "async fn $NAME($$$)" --language rust

# Find React components
probe query "function $NAME($$$) { return <$$$> }" --language javascript

# Find Python classes with specific method
probe query "class $CLASS: def __init__($$$)" --language python
```

---

### Node.js SDK

Use Probe programmatically in your applications.

```javascript
import { ProbeAgent } from '@probelabs/probe/agent';

// Create agent
const agent = new ProbeAgent({
  path: './src',
  provider: 'anthropic'
});

await agent.initialize();

// Ask questions
const response = await agent.answer('How does authentication work?');
console.log(response);

// Get token usage
console.log(agent.getTokenUsage());
```

**Direct functions:**

```javascript
import { search, extract, query } from '@probelabs/probe';

// Semantic search
const results = await search({
  query: 'authentication',
  path: './src',
  maxTokens: 10000
});

// Extract code
const code = await extract({
  files: ['src/auth.ts:42'],
  format: 'markdown'
});

// AST pattern query
const matches = await query({
  pattern: 'async function $NAME($$$)',
  path: './src',
  language: 'typescript'
});
```

**Vercel AI SDK integration:**

```javascript
import { tools } from '@probelabs/probe';

const { searchTool, queryTool, extractTool } = tools;

// Use with Vercel AI SDK
const result = await generateText({
  model: anthropic('claude-sonnet-4-5-20250929'),
  tools: {
    search: searchTool({ defaultPath: './src' }),
    query: queryTool({ defaultPath: './src' }),
    extract: extractTool({ defaultPath: './src' })
  },
  prompt: 'Find authentication code'
});
```

---

## Installation

### NPM (Recommended)

```bash
npm install -g @probelabs/probe
```

### curl (macOS/Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/buger/probe/main/install.sh | bash
```

### PowerShell (Windows)

```powershell
iwr -useb https://raw.githubusercontent.com/buger/probe/main/install.ps1 | iex
```

### From Source

```bash
git clone https://github.com/probelabs/probe.git
cd probe
cargo build --release
cargo install --path .
```

---

## Supported Languages

| Language | Extensions |
|----------|------------|
| Rust | `.rs` |
| JavaScript/JSX | `.js`, `.jsx` |
| TypeScript/TSX | `.ts`, `.tsx` |
| Python | `.py` |
| Go | `.go` |
| C/C++ | `.c`, `.h`, `.cpp`, `.cc`, `.hpp` |
| Java | `.java` |
| Ruby | `.rb` |
| PHP | `.php` |
| Swift | `.swift` |
| C# | `.cs` |
| Markdown | `.md` |

---

## Documentation

- **[Full Documentation](https://probelabs.com/docs)** - Complete guides and reference
- **[API Reference](https://probelabs.com/probe-agent/sdk/api-reference)** - SDK documentation
- **[MCP Protocol](https://probelabs.com/probe-agent/protocols/mcp)** - MCP integration guide
- **[CLI Reference](https://probelabs.com/cli-mode)** - All CLI commands and options

---

## Environment Variables

```bash
# AI Provider Keys
ANTHROPIC_API_KEY=sk-ant-...
OPENAI_API_KEY=sk-...
GOOGLE_API_KEY=...

# Provider Selection
FORCE_PROVIDER=anthropic
MODEL_NAME=claude-sonnet-4-5-20250929

# Custom Endpoints
ANTHROPIC_API_URL=https://your-proxy.com
OPENAI_API_URL=https://your-proxy.com

# Debug
DEBUG=1
```

---

## Contributing

We welcome contributions! See our [Contributing Guide](https://github.com/probelabs/probe/blob/main/CONTRIBUTING.md).

For questions or support:
- [GitHub Issues](https://github.com/probelabs/probe/issues)
- [Discord Community](https://discord.gg/hBN4UsTZ)

---

## License

Apache 2.0 - See [LICENSE](LICENSE) for details.
