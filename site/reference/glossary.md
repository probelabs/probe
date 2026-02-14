# Glossary

Technical terms and concepts used throughout Probe documentation.

---

## A

### ACP (Agent Communication Protocol)
An advanced protocol for agent-to-agent communication in Probe. Provides session management, streaming, and structured tool execution. See [ACP Protocol](../probe-agent/protocols/acp.md).

### AST (Abstract Syntax Tree)
A tree representation of source code structure. Probe uses tree-sitter to parse code into ASTs for precise structural queries. For example, `fn $NAME() { $$$BODY }` matches Rust function definitions.

### AST-grep
A structural code search tool that uses AST patterns. Probe integrates ast-grep for the `query` command.

---

## B

### BM25 (Best Matching 25)
A ranking algorithm used for information retrieval. BM25 is Probe's default reranker, balancing term frequency and document length for relevance scoring.

### Block
A contiguous section of code extracted by Probe. Blocks typically correspond to functions, classes, or other semantic units. Adjacent blocks within `--merge-threshold` lines are merged.

---

## C

### Claude Code
Anthropic's AI coding assistant CLI. Probe integrates with Claude Code as both an MCP server and as a backend for the implement tool.

### Codex
OpenAI's legacy code generation CLI. Probe supports Codex as an alternative engine for code generation tasks. Note: OpenAI's Codex API was deprecated in 2023; modern usage typically involves GPT-4 models instead.

### Context Compaction
The process of reducing conversation history to fit within token limits while preserving essential information. See [Context Compaction](../probe-agent/advanced/context-compaction.md).

### Context Window
The maximum number of tokens an AI model can process in a single request. Probe's `--max-tokens` flag helps stay within these limits.

---

## D

### Delegate
A ProbeAgent feature that distributes tasks to specialized subagents. Useful for complex multi-step operations. See [Delegation](../probe-agent/advanced/delegation.md).

---

## E

### Elasticsearch Syntax
Query syntax supported by Probe's search command. Includes operators like `AND`, `OR`, `NOT`, wildcards (`*`), and exact phrases (`"..."`).

### Engine
The AI provider/model combination used by ProbeAgent. Supported engines include Anthropic Claude, OpenAI GPT, Google Gemini, AWS Bedrock, Claude Code, and Codex.

### Extract
Probe command that retrieves complete code blocks from files by location (line number) or symbol name.

---

## F

### Fallback
A resilience strategy where ProbeAgent automatically tries alternative providers when the primary fails. See [Retry & Fallback](../probe-agent/sdk/retry-fallback.md).

---

## G

### Glob
A pattern matching syntax for file paths. Examples: `*.ts` (TypeScript files), `**/*.test.js` (all test files).

---

## H

### Hook
A callback function triggered by ProbeAgent lifecycle events. Hooks enable custom behavior for tool execution, message handling, and more. See [Hooks System](../probe-agent/sdk/hooks.md).

### Hybrid Ranking
A ranking algorithm combining BM25 and TF-IDF for improved relevance. Use `--reranker hybrid` or `--reranker hybrid2`.

---

## J

### JSON-RPC 2.0
The messaging protocol used by ACP. A lightweight remote procedure call protocol encoded in JSON.

---

## M

### MCP (Model Context Protocol)
A standard protocol for AI tool integration. Probe implements MCP for integration with AI editors like Cursor and Claude Code. See [MCP Protocol](../probe-agent/protocols/mcp.md).

### Merge Threshold
The `--merge-threshold` option controls how many lines apart code blocks can be while still being merged into a single result. Default is 5 lines.

---

## O

### Outline
Probe's default output format showing hierarchical code structure with symbols, line numbers, and brief descriptions.

---

## P

### Parser Pool
Probe's thread pool for tree-sitter parsers. Configurable via `PROBE_PARSER_POOL_SIZE` environment variable.

### Probe Agent
The Node.js SDK component of Probe for building AI coding assistants. Includes ProbeAgent class, tools, MCP/ACP support.

### Probe CLI
The Rust-based command-line tool for semantic code search. Includes search, extract, query, and grep commands.

### ProbeAgent
The main class in Probe's Node.js SDK for creating AI agents with code search capabilities.

---

## Q

### Query
Probe command for AST-based structural code search. Uses tree-sitter patterns like `fn $NAME() { $$$BODY }`.

---

## R

### Rayon
A Rust library for parallel processing. Probe uses Rayon for concurrent file scanning and processing.

### Reranker
An algorithm that scores and orders search results. Options include `bm25`, `tfidf`, `hybrid`, and BERT-based rerankers.

### Ripgrep
A fast line-oriented search tool. Probe uses ripgrep (via the `ignore` crate) for initial file scanning.

### Retry
Automatic retry of failed API requests with exponential backoff. See [Retry & Fallback](../probe-agent/sdk/retry-fallback.md).

---

## S

### Search Hint
Query filters in Probe's search syntax. Examples: `ext:ts` (file extension), `lang:python` (language), `dir:tests` (directory).

### Session
A conversation context in ProbeAgent. Sessions maintain history, token tracking, and state across multiple interactions.

### SIMD (Single Instruction, Multiple Data)
CPU-level parallelization for operations like tokenization and ranking. Probe uses SIMD for performance optimization.

### Skills
Discoverable agent capabilities in ProbeAgent. Skills can be listed and executed dynamically.

### Storage Adapter
An interface for persisting ProbeAgent conversation history. Default is `InMemoryStorageAdapter`.

### Stemming
Linguistic normalization that reduces words to their root form. "running" → "run". Enabled by default in Probe search.

---

## T

### TF-IDF (Term Frequency-Inverse Document Frequency)
A ranking algorithm that weighs term importance by frequency and rarity. Available via `--reranker tfidf`.

### Token
A unit of text processing for AI models. Roughly 4 characters in English. Probe tracks tokens for context window management.

### TokenCounter
A ProbeAgent class for tracking token usage across requests.

### Tool
A capability that AI agents can invoke. Probe provides tools like search, query, extract, edit, bash.

### Tree-sitter
A parser generator for programming languages. Probe uses tree-sitter for AST parsing and structural code analysis.

---

## V

### Vercel AI SDK
A framework for building AI applications. ProbeAgent uses Vercel AI SDK for model integration.

---

## W

### Workspace Root
The common parent directory for all paths in a ProbeAgent session. Computed from `allowedFolders`.

---

## X

### XML Tool Syntax
The format ProbeAgent uses for tool calls in conversations. Tools are invoked via XML tags like `<search><query>...</query></search>`.

---

## Related Documentation

- [FAQ](./faq.md) - Frequently asked questions
- [Troubleshooting](./troubleshooting.md) - Common issues
- [Environment Variables](./environment-variables.md) - Configuration reference
