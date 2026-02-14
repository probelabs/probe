# Probe Documentation

Probe is a semantic code search and AI agent platform consisting of two complementary products: **Probe CLI** (a Rust-based code search tool) and **Probe Agent** (a Node.js SDK for building AI coding assistants). Together, they provide a complete solution for AI-powered code understanding and interaction.

---

## Getting Started

| Document | Description |
|----------|-------------|
| [Quick Start](./quick-start.md) | Get up and running with Probe in under 5 minutes |
| [Installation](./installation.md) | Platform-specific installation instructions |
| [Features Overview](./features.md) | High-level overview of Probe capabilities |

---

## Probe CLI (Rust Tool)

The core semantic code search engine built in Rust for maximum performance.

### Core Commands

| Document | Description |
|----------|-------------|
| [Search Command](./probe-cli/search.md) | Semantic code search with Elasticsearch-style queries |
| [Extract Command](./probe-cli/extract.md) | Extract code blocks with full context |
| [Query Command](./probe-cli/query.md) | AST-based structural code queries |
| [CLI Reference](./cli-mode.md) | Complete command-line reference |

### Configuration & Advanced

| Document | Description |
|----------|-------------|
| [Output Formats](./output-formats.md) | JSON, XML, Markdown, and other output formats |
| [Performance Tuning](./probe-cli/performance.md) | SIMD optimization, caching, and parallel processing |
| [Language Support](./supported-languages.md) | Supported programming languages and parsers |

---

## Probe Agent (Node.js SDK)

AI agent orchestration platform for building intelligent coding assistants.

### Getting Started

| Document | Description |
|----------|-------------|
| [Agent Overview](./probe-agent/overview.md) | What is Probe Agent and when to use it |
| [SDK Quick Start](./probe-agent/sdk/getting-started.md) | Build your first AI agent in 5 minutes |
| [API Reference](./probe-agent/sdk/api-reference.md) | Complete ProbeAgent class documentation |

### SDK Reference

| Document | Description |
|----------|-------------|
| [Tools Reference](./probe-agent/sdk/tools-reference.md) | Available tools: search, query, extract, edit, bash |
| [Engines & Providers](./probe-agent/sdk/engines.md) | Anthropic, OpenAI, Google, Bedrock, Claude Code, Codex |
| [Storage Adapters](./probe-agent/sdk/storage-adapters.md) | Session persistence and custom storage backends |
| [Hooks System](./probe-agent/sdk/hooks.md) | Event hooks for lifecycle, tools, and streaming |
| [Retry & Fallback](./probe-agent/sdk/retry-fallback.md) | Multi-provider resilience and error recovery |

### Probe Chat

| Document | Description |
|----------|-------------|
| [Chat CLI Usage](./probe-agent/chat/cli-usage.md) | Interactive terminal chat interface |
| [Web Interface](./probe-agent/chat/web-interface.md) | Browser-based chat with code highlighting |
| [Configuration](./probe-agent/chat/configuration.md) | API keys, models, and chat settings |

### Protocols & Integration

| Document | Description |
|----------|-------------|
| [MCP Protocol](./probe-agent/protocols/mcp.md) | Model Context Protocol server and client |
| [ACP Protocol](./probe-agent/protocols/acp.md) | Agent Communication Protocol for advanced orchestration |
| [MCP Server Setup](./mcp-server.md) | Configure Probe as an MCP server for AI editors |

### Advanced Features

| Document | Description |
|----------|-------------|
| [Delegation](./probe-agent/advanced/delegation.md) | Distribute tasks to specialized subagents |
| [Skills System](./probe-agent/advanced/skills.md) | Discoverable agent capabilities |
| [Task Management](./probe-agent/advanced/tasks.md) | Track multi-step operations |
| [Context Compaction](./probe-agent/advanced/context-compaction.md) | Manage conversation history and token limits |

---

## Guides & Best Practices

| Document | Description |
|----------|-------------|
| [Query Patterns](./guides/query-patterns.md) | Effective search and query strategies |
| [Agent Workflows](./guides/agent-workflows.md) | Common patterns for AI coding assistants |
| [Security Considerations](./guides/security.md) | Sandboxing, permissions, and safe execution |

---

## Reference

| Document | Description |
|----------|-------------|
| [Glossary](./reference/glossary.md) | Terms: AST, tree-sitter, BM25, MCP, ACP, and more |
| [FAQ](./reference/faq.md) | Frequently asked questions |
| [Limits & Constraints](./reference/limits.md) | Token limits, timeouts, and system constraints |
| [Environment Variables](./reference/environment-variables.md) | All configuration environment variables |
| [Troubleshooting](./reference/troubleshooting.md) | Common issues and solutions |

---

## Integrations

| Document | Description |
|----------|-------------|
| [AI Code Editors](./use-cases/integrating-probe-into-ai-code-editors.md) | Cursor, Windsurf, Claude Code integration |
| [GitHub Actions](./guides/github-actions.md) | CI/CD integration for code analysis |
| [Web Interface Deployment](./use-cases/deploying-probe-web-interface.md) | Deploy Probe Chat for teams |

---

## Contributing

| Document | Description |
|----------|-------------|
| [Contributing Guide](https://github.com/probelabs/probe/blob/main/CONTRIBUTING.md) | How to contribute to Probe |
| [Architecture](./reference/architecture.md) | System design and internals |
| [Adding Languages](./adding-languages.md) | Add support for new programming languages |
| [Documentation Guide](./contributing/documentation-maintenance.md) | Contributing to documentation |

---

## Release Information

| Document | Description |
|----------|-------------|
| [Changelog](./changelog.md) | Version history and release notes |
| [GitHub Releases](https://github.com/probelabs/probe/releases) | Download binaries and release assets |
