# System Architecture

Overview of Probe's architecture and component relationships.

---

## TL;DR

Probe is a three-layer system:
1. **Rust Core**: High-performance search and extraction
2. **Node.js SDK**: AI agent orchestration
3. **Interfaces**: CLI, Chat, MCP Server

---

## System Overview

```
┌─────────────────────────────────────────────────────────────┐
│                    USER INTERFACES                          │
├──────────────────┬──────────────────┬──────────────────────┤
│  CLI             │  Chat CLI/Web    │  MCP Server          │
│  (search,        │  (examples/chat) │  (npm/src/mcp)       │
│   extract,       │                  │                      │
│   query)         │                  │                      │
└────────┬─────────┴──────────┬───────┴──────────┬───────────┘
         │                    │                   │
    ┌────▼────────────────────▼───────────────────▼───────────┐
    │           NODE.JS SDK LAYER (@probelabs/probe)          │
    ├─────────────────────────────────────────────────────────┤
    │ • ProbeAgent - AI orchestration                         │
    │ • Tool Definitions - search, query, extract, etc.       │
    │ • Binary Execution - Rust CLI wrapper                   │
    │ • Session Management - History, storage                 │
    │ • Telemetry - OpenTelemetry integration                │
    └────┬────────────────────────────────────────────────────┘
         │
    ┌────▼────────────────────────────────────────────────────┐
    │              RUST CORE (src/)                           │
    ├─────────────────────────────────────────────────────────┤
    │  ┌──────────────┬──────────────┬──────────────┐        │
    │  │ SEARCH       │ EXTRACT      │ LANGUAGE     │        │
    │  │              │              │              │        │
    │  │ • Ripgrep    │ • Tree-sitter│ • Parsers    │        │
    │  │ • BM25/TF-IDF│ • Line/Symbol│ • AST Blocks │        │
    │  │ • Tokenization│             │ • Test Detection │    │
    │  └──────────────┴──────────────┴──────────────┘        │
    └─────────────────────────────────────────────────────────┘
```

---

## Rust Core

### Search Pipeline

```
SearchOptions
    ↓
Parse Query (elastic_query.rs)
    ↓
Tokenize & Normalize
    ↓
Scan Files (ripgrep)
    ↓
Parse AST (tree-sitter)
    ↓
Early Ranking (BM25)
    ↓
Extract Code Blocks
    ↓
Final Ranking
    ↓
Merge Adjacent Blocks
    ↓
Apply Limits
    ↓
LimitedSearchResults
```

### Key Modules

| Module | Path | Purpose |
|--------|------|---------|
| Search Runner | `src/search/search_runner.rs` | Main orchestration |
| Ripgrep | `src/search/ripgrep_searcher.rs` | Fast file scanning |
| Elastic Query | `src/search/elastic_query.rs` | Query parsing |
| Ranking | `src/ranking.rs` | BM25/TF-IDF scoring |
| SIMD Ranking | `src/simd_ranking.rs` | Optimized scoring |
| Tokenization | `src/search/tokenization.rs` | NLP processing |
| Language | `src/language/` | Tree-sitter parsers |
| Extract | `src/extract/` | Code extraction |

### Language Support

All languages implement the `LanguageImpl` trait:

```rust
pub trait LanguageImpl {
    fn get_tree_sitter_language(&self) -> TSLanguage;
    fn is_acceptable_parent(&self, node: &Node) -> bool;
    fn is_test_node(&self, node: &Node, source: &[u8]) -> bool;
    fn get_symbol_signature(&self, node: &Node, source: &[u8]) -> Option<String>;
}
```

**Supported:** Rust, JavaScript, TypeScript, Python, Go, C, C++, Java, Ruby, PHP, Swift, C#, HTML, Markdown, YAML

### Performance Optimizations

| Optimization | Purpose |
|--------------|---------|
| Parser Pool | Reuse tree-sitter parsers |
| Tree Cache | Cache parsed ASTs |
| SIMD Scoring | Vector operations |
| Early Ranking | Skip non-relevant files |
| Session Cache | Avoid duplicate results |
| Rayon | Parallel processing |

---

## Node.js SDK Layer

### ProbeAgent

The intelligent AI agent:

```javascript
const agent = new ProbeAgent({
  path: './src',
  provider: 'anthropic'
});

const response = await agent.answer('How does auth work?');
```

**Responsibilities:**
- Multi-turn conversation management
- Tool execution loop (max 30 iterations)
- JSON/Mermaid validation
- Token tracking
- Retry and fallback

### Tool System

| Tool | Function |
|------|----------|
| `search` | Semantic code search |
| `query` | AST pattern matching |
| `extract` | Code block extraction |
| `grep` | Ripgrep search |
| `bash` | Shell execution |
| `edit` | File modification |
| `delegate` | Sub-agent creation |

### Supporting Infrastructure

| Module | Purpose |
|--------|---------|
| `probeTool.js` | Rust binary execution |
| `delegate.js` | Sub-agent orchestration |
| `storage/` | Session persistence |
| `hooks/` | Event callbacks |
| `mcp/` | MCP server |
| `tokenCounter.js` | Token tracking |
| `RetryManager.js` | Automatic retries |
| `FallbackManager.js` | Provider fallback |

---

## User Interfaces

### CLI (Rust)

Direct command-line interface:

```bash
probe search "query" ./path
probe extract file.rs:42
probe query "pattern" --language rust
```

### Chat (Node.js)

Interactive AI chat:

```bash
probe-chat ./project          # CLI mode
probe-chat --web ./project    # Web mode
```

### MCP Server

Model Context Protocol integration:

```bash
npx -y @probelabs/probe mcp
```

Tools exposed: `search_code`, `query_code`, `extract_code`

---

## Data Flow

### Search Request

```
CLI Args → SearchOptions
    ↓
Rust: perform_probe()
    ↓
JSON Output → Node.js
    ↓
ProbeAgent processes result
    ↓
AI Response
```

### AI Agent Loop

```
User Message
    ↓
System Prompt + Context
    ↓
AI Provider (Anthropic/OpenAI/Google)
    ↓
Tool Call → Execute → Result
    ↓
Continue (up to 30 iterations)
    ↓
Final Response
```

---

## Integration Points

### Rust → Node.js

```javascript
// Binary execution
const result = await execFile('probe', ['search', query, path]);
const parsed = JSON.parse(result);
```

### Node.js → AI Providers

```javascript
import { streamText } from 'ai';
import { createAnthropic } from '@ai-sdk/anthropic';

const result = await streamText({
  model: createAnthropic()('claude-sonnet-4-6'),
  system: systemPrompt,
  messages: history,
  tools: toolDefinitions
});
```

### MCP Integration

```typescript
// STDIO transport
server.connect(new StdioServerTransport());

// Tools registered
server.setRequestHandler(ListToolsRequestSchema, handleListTools);
server.setRequestHandler(CallToolRequestSchema, handleCallTool);
```

---

## Module Organization

### Rust Core

```
src/
├── main.rs             # CLI entry point
├── cli.rs              # Argument parsing
├── lib.rs              # Public API
├── models.rs           # Data structures
├── ranking.rs          # BM25/TF-IDF
├── simd_ranking.rs     # SIMD scoring
├── query.rs            # AST patterns
├── grep.rs             # Ripgrep wrapper
├── search/             # Search pipeline
│   ├── mod.rs
│   ├── search_runner.rs
│   ├── ripgrep_searcher.rs
│   ├── elastic_query.rs
│   ├── tokenization.rs
│   ├── result_ranking.rs
│   └── ...
├── extract/            # Code extraction
│   ├── mod.rs
│   ├── processor.rs
│   ├── formatter.rs
│   └── ...
├── language/           # Language support
│   ├── mod.rs
│   ├── factory.rs
│   ├── rust.rs
│   ├── javascript.rs
│   └── ...
└── path_resolver/      # Dependency resolution
```

### Node.js SDK

```
npm/src/
├── index.js            # Public exports
├── agent/
│   ├── ProbeAgent.js   # Main agent class
│   └── tools.js        # Tool instantiation
├── tools/
│   ├── common.js       # Shared schemas
│   ├── vercel.js       # Vercel AI SDK
│   └── langchain.js    # LangChain
├── mcp/
│   └── index.ts        # MCP server
├── hooks/
│   └── index.js        # Hook system
├── storage/
│   └── JsonChatStorage.js
└── ...
```

### Chat Application

```
examples/chat/
├── index.js            # Entry point
├── webServer.js        # Web server
├── probeChat.js        # Chat wrapper
├── ChatSessionManager.js
├── auth.js             # Authentication
├── index.html          # Web UI
├── storage/
│   └── JsonChatStorage.js
└── implement/          # Code editing
```

---

## Key Design Patterns

### Error Handling

```rust
// Rust: Result<T> with anyhow
pub fn search(options: SearchOptions) -> Result<Vec<SearchResult>> {
    fs::read_to_string(path)
        .context("Failed to read file")?
}
```

```javascript
// Node.js: try/catch with context
try {
  const result = await search(options);
} catch (error) {
  throw new ProbeError(`Search failed: ${error.message}`);
}
```

### Thread Safety

```rust
// Parser pool with Arc
Arc<DashMap<String, Vec<Parser>>>

// Cache with concurrent access
DashMap<PathBuf, CachedTree>
```

### Event System

```javascript
// Tool execution events
agent.events.on('toolCall', (event) => {
  console.log(`Tool: ${event.name}, Status: ${event.status}`);
});
```

---

## Security Boundaries

| Boundary | Protection |
|----------|------------|
| File Access | `allowedFolders` configuration |
| Path Traversal | Canonicalization |
| Bash Execution | Allow/deny patterns |
| API Keys | Environment variables |
| Web Access | Basic authentication |

---

## Performance Characteristics

| Component | Performance |
|-----------|-------------|
| Ripgrep | ~1GB/s scanning |
| Tree-sitter | ~1ms per file parsing |
| SIMD Ranking | 4-8x faster scoring |
| Parser Pool | Avoid re-initialization |
| Session Cache | Deduplicated results |

---

## Related Documentation

- [Search Command](../probe-cli/search.md) - Search details
- [API Reference](../probe-agent/sdk/api-reference.md) - SDK API
- [MCP Protocol](../probe-agent/protocols/mcp.md) - MCP integration
- [Performance](../probe-cli/performance.md) - Optimization guide

