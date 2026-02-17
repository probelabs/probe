<p align="center">
  <img src="logo.png?2" alt="Probe Logo" width="400">
</p>

# Probe

**We read code 10x more than we write it.** Probe is a code and markdown context engine, with a built-in agent, made to work on enterprise-scale codebases.

Today's AI coding tools use a caveman approach: grep some files, read random lines, hope for the best. It works on toy projects. It falls apart on real codebases.

**Probe is a context engine built for reading and reasoning.** It treats your code as code—not text. AST parsing understands structure. Semantic search finds what matters. You get complete, meaningful context in a single call.

**The Probe Agent** is purpose-built for code understanding. It knows how to wield the Probe engine expertly—searching, extracting, and reasoning across your entire codebase. Perfect for spec-driven development, code reviews, onboarding, and any task where understanding comes before writing.

**One Probe call captures what takes other tools 10+ agentic loops**—deeper, cleaner, and far less noise.

---

## Table of Contents

- [Why Probe?](#why-probe)
- [Quick Start](#quick-start)
- [Features](#features)
- [Usage Modes](#usage-modes)
  - [Probe Agent (MCP)](#probe-agent-mcp)
  - [Raw MCP Tools](#raw-mcp-tools)
  - [CLI Agent](#cli-agent)
  - [Direct CLI Commands](#direct-cli-commands)
  - [Node.js SDK](#nodejs-sdk)
- [LLM Script](#llm-script)
- [Installation](#installation)
- [Supported Languages](#supported-languages)
- [Documentation](#documentation)
- [Environment Variables](#environment-variables)
- [Contributing](#contributing)
- [License](#license)

---

## Why Probe?

| Traditional Approach | Probe |
|---------------------|-------|
| Grep + read random lines | Semantic search with Elasticsearch syntax |
| Treats code as text | Understands code structure via tree-sitter AST |
| Returns fragments | Returns complete functions, classes, structs |
| Requires indexing | Zero setup, instant results |
| 10+ loops to gather context | One call, complete picture |
| Struggles at scale | Built for million-line codebases |

---

## Quick Start

### Option 1: Probe Agent via MCP (Recommended)

Our built-in agent natively integrates with Claude Code, using its authentication—no extra API keys needed.

Add to `~/.claude/claude_desktop_config.json`:
```json
{
  "mcpServers": {
    "probe": {
      "command": "npx",
      "args": ["-y", "@probelabs/probe@latest", "agent", "--mcp"]
    }
  }
}
```

The Probe Agent is purpose-built to read and reason about code. It piggybacks on Claude Code's auth (or Codex auth), or works with any model via your own API key (e.g., `GOOGLE_API_KEY`).

### Option 2: Raw Probe Tools via MCP

If you prefer direct access to search/query/extract tools without the agent layer:

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

### Option 3: Direct CLI (No MCP)

Use Probe directly from your terminal—no AI editor required:

```bash
# Semantic search with Elasticsearch syntax
npx -y @probelabs/probe search "authentication AND login" ./src

# Extract code block at line 42
npx -y @probelabs/probe extract src/main.rs:42

# AST pattern matching
npx -y @probelabs/probe query "fn $NAME($$$) -> Result<$RET>" --language rust
```

### Option 4: CLI Agent

Ask questions about any codebase directly from your terminal:

```bash
# One-shot question (works with any LLM provider)
npx -y @probelabs/probe@latest agent "How is authentication implemented?"

# With code editing capabilities
npx -y @probelabs/probe@latest agent "Refactor the login function" --allow-edit
```

---

## Features

- **Code-Aware**: Tree-sitter AST parsing understands your code's actual structure
- **Semantic Search**: Elasticsearch-style queries (`AND`, `OR`, `NOT`, phrases, filters)
- **Complete Context**: Returns entire functions, classes, or structs—not fragments
- **One Call, Full Context**: Captures what takes other tools 10+ loops to gather
- **Zero Indexing**: Instant results on any codebase, no setup required
- **Fully Local**: Your code never leaves your machine
- **Blazing Fast**: Ripgrep-powered scanning handles million-line codebases
- **Smart Ranking**: BM25, TF-IDF, and hybrid algorithms surface what matters
- **Multi-Language**: Rust, Python, JavaScript, TypeScript, Go, C/C++, Java, and more

---

## Usage Modes

### Probe Agent (MCP)

The recommended way to use Probe with AI editors. The Probe Agent is a specialized coding assistant that reasons about your code—not just pattern matches.

```json
{
  "mcpServers": {
    "probe": {
      "command": "npx",
      "args": ["-y", "@probelabs/probe@latest", "agent", "--mcp"]
    }
  }
}
```

**Why use the agent?**
- Purpose-built to understand and reason about code
- Piggybacks on Claude Code / Codex authentication (or use your own API key)
- Smarter multi-step reasoning for complex questions
- Built-in code editing, task delegation, and more

**Agent options:**

| Option | Description |
|--------|-------------|
| `--path <dir>` | Search directory (default: current) |
| `--provider <name>` | AI provider: `anthropic`, `openai`, `google` |
| `--model <name>` | Override model name |
| `--prompt <type>` | Persona: `code-explorer`, `engineer`, `code-review`, `architect` |
| `--allow-edit` | Enable code modification |
| `--enable-delegate` | Enable task delegation to subagents |
| `--enable-bash` | Enable bash command execution |
| `--max-iterations <n>` | Max tool iterations (default: 30) |

---

### Raw MCP Tools

Direct access to Probe's search, query, and extract tools—without the agent layer. Use this when you want your AI editor to call Probe tools directly.

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

**Available tools:**
- `search` - Semantic code search with Elasticsearch-style queries
- `query` - AST-based structural pattern matching
- `extract` - Extract code blocks by line number or symbol name

---

### CLI Agent

Run the Probe Agent directly from your terminal:

```bash
# One-shot question
npx -y @probelabs/probe@latest agent "How does the ranking algorithm work?"

# Specify search path
npx -y @probelabs/probe@latest agent "Find API endpoints" --path ./src

# Enable code editing
npx -y @probelabs/probe@latest agent "Add error handling to login()" --allow-edit

# Use custom persona
npx -y @probelabs/probe@latest agent "Review this code" --prompt code-review
```

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

# Boolean operators (Elasticsearch syntax)
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

## LLM Script

Probe Agent can use the `execute_plan` tool to run deterministic, multi-step code analysis tasks. LLM Script is a sandboxed JavaScript DSL where the AI generates executable plans combining search, extraction, and LLM reasoning in a single pipeline.

```javascript
// AI-generated LLM Script example (await is auto-injected, don't write it)
const files = search("authentication login")
const chunks = chunk(files)
const analysis = map(chunks, c => LLM("Summarize auth patterns", c))
return analysis.join("\n")
```

**Key features:**
- **Agent integration** - Probe Agent calls `execute_plan` tool to run scripts
- **Auto-await** - Async calls are automatically awaited (don't write `await`)
- **All tools available** - `search()`, `query()`, `extract()`, `LLM()`, `map()`, `chunk()`, plus any MCP tools
- **Sandboxed execution** - Safe, isolated JavaScript environment with timeout protection

See the full [LLM Script Documentation](./docs/llm-script.md) for syntax and examples.

---

## Installation

### NPM (Recommended)

```bash
npm install -g @probelabs/probe
```

### curl (macOS/Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/probelabs/probe/main/install.sh | bash
```

### PowerShell (Windows)

```powershell
iwr -useb https://raw.githubusercontent.com/probelabs/probe/main/install.ps1 | iex
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

Full documentation available at [probelabs.com/probe](https://probelabs.com/probe) or browse locally in [`docs/`](./docs/).

### Getting Started
- [Quick Start](./docs/quick-start.md) - Get up and running in 5 minutes
- [Installation](./docs/installation.md) - NPM, curl, Docker, and building from source
- [Features Overview](./docs/features.md) - Core capabilities

### Probe CLI
- [Search Command](./docs/probe-cli/search.md) - Elasticsearch-style semantic search
- [Extract Command](./docs/probe-cli/extract.md) - Extract code blocks with full AST context
- [Query Command](./docs/probe-cli/query.md) - AST-based structural pattern matching
- [CLI Reference](./docs/probe-cli/cli-reference.md) - Complete command-line reference

### Probe Agent
- [Agent Overview](./docs/probe-agent/overview.md) - What is Probe Agent and when to use it
- [API Reference](./docs/probe-agent/sdk/api-reference.md) - ProbeAgent class documentation
- [Node.js SDK](./docs/probe-agent/sdk/nodejs-sdk.md) - Full Node.js SDK reference
- [MCP Integration](./docs/probe-agent/protocols/mcp-integration.md) - Editor integration guide
- [LLM Script](./docs/llm-script.md) - Programmable orchestration DSL

### Guides & Reference
- [Query Patterns](./docs/guides/query-patterns.md) - Effective search strategies
- [Architecture](./docs/reference/architecture.md) - System design and internals
- [Environment Variables](./docs/reference/environment-variables.md) - All configuration options
- [FAQ](./docs/reference/faq.md) - Frequently asked questions

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
