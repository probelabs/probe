---
layout: home
title: Probe - Local, AI-Ready Code Intelligence
hero:
  name: Probe
  text: "Local, AI-Ready\nCode Intelligence"
  tagline: Make AI work with large codebases, and natively understand it.
  image:
    src: /logo.png
    alt: Probe Logo
  actions:
    - theme: brand
      text: Get Started
      link: /quick-start
    - theme: alt
      text: GitHub Repo
      link: https://github.com/probelabs/probe
---

# Probe Documentation

Probe is a code and markdown context engine with a built-in AI agent, designed for enterprise-scale codebases. It combines ripgrep speed with tree-sitter AST parsing for semantic code search.

---

## Getting Started

| Document | Description |
|----------|-------------|
| [Quick Start](./quick-start.md) | Get up and running in 5 minutes |
| [Installation](./installation.md) | NPM, curl, Docker, and building from source |
| [Features Overview](./features.md) | Core capabilities and what makes Probe different |

---

## Probe CLI

The core semantic code search engine built in Rust.

| Document | Description |
|----------|-------------|
| [Search Command](./probe-cli/search.md) | Elasticsearch-style semantic search |
| [Extract Command](./probe-cli/extract.md) | Extract code blocks with full AST context |
| [Query Command](./probe-cli/query.md) | AST-based structural pattern matching |
| [CLI Reference](./probe-cli/cli-reference.md) | Complete command-line reference |
| [Extraction Reference](./probe-cli/extraction-reference.md) | Advanced extraction techniques |
| [Performance](./probe-cli/performance.md) | SIMD optimization and caching |

---

## Probe Agent

AI agent SDK for building intelligent coding assistants.

### Overview

| Document | Description |
|----------|-------------|
| [Agent Overview](./probe-agent/overview.md) | What is Probe Agent and when to use it |
| [AI Integration](./probe-agent/ai-integration.md) | Complete AI integration reference |

### SDK Reference

| Document | Description |
|----------|-------------|
| [Getting Started](./probe-agent/sdk/getting-started.md) | Build your first AI agent |
| [API Reference](./probe-agent/sdk/api-reference.md) | ProbeAgent class documentation |
| [Node.js SDK](./probe-agent/sdk/nodejs-sdk.md) | Full Node.js SDK reference |
| [Tools Reference](./probe-agent/sdk/tools-reference.md) | Search, query, extract, edit, bash tools |
| [Engines & Providers](./probe-agent/sdk/engines.md) | Anthropic, OpenAI, Google, Bedrock |
| [Storage Adapters](./probe-agent/sdk/storage-adapters.md) | Session persistence |
| [Hooks System](./probe-agent/sdk/hooks.md) | Lifecycle and tool event hooks |
| [Retry & Fallback](./probe-agent/sdk/retry-fallback.md) | Multi-provider resilience |

### Chat Interface

| Document | Description |
|----------|-------------|
| [CLI Usage](./probe-agent/chat/cli-usage.md) | Interactive terminal chat |
| [Web Interface](./probe-agent/chat/web-interface.md) | Browser-based chat |
| [Configuration](./probe-agent/chat/configuration.md) | API keys and settings |

### Protocols

| Document | Description |
|----------|-------------|
| [MCP Protocol](./probe-agent/protocols/mcp.md) | Model Context Protocol server |
| [MCP Integration](./probe-agent/protocols/mcp-integration.md) | Editor integration guide |
| [MCP Server](./probe-agent/protocols/mcp-server.md) | Server configuration |
| [ACP Protocol](./probe-agent/protocols/acp.md) | Agent Communication Protocol |

### Advanced Features

| Document | Description |
|----------|-------------|
| [Delegation](./probe-agent/advanced/delegation.md) | Task delegation to subagents |
| [Skills System](./probe-agent/advanced/skills.md) | Discoverable capabilities |
| [Task Management](./probe-agent/advanced/tasks.md) | Multi-step operation tracking |
| [Context Compaction](./probe-agent/advanced/context-compaction.md) | Token limit management |

---

## LLM Script

Programmable orchestration engine for deterministic, multi-step code analysis.

| Document | Description |
|----------|-------------|
| [LLM Script](./llm-script.md) | Sandboxed JavaScript DSL for AI-generated executable plans |

---

## Guides

| Document | Description |
|----------|-------------|
| [Query Patterns](./guides/query-patterns.md) | Effective search strategies |
| [Agent Workflows](./guides/agent-workflows.md) | Common AI agent patterns |
| [GitHub Actions](./guides/github-actions.md) | CI/CD integration |
| [Security](./guides/security.md) | Sandboxing and permissions |
| [Windows Guide](./guides/windows.md) | Windows-specific setup |

---

## Use Cases

| Document | Description |
|----------|-------------|
| [AI Code Editors](./use-cases/ai-code-editors.md) | Cursor, Windsurf, Claude Code integration |
| [CLI AI Workflows](./use-cases/cli-ai-workflows.md) | Terminal-based AI coding |
| [Building AI Tools](./use-cases/building-ai-tools.md) | Create custom AI tools |
| [Advanced CLI](./use-cases/advanced-cli.md) | Power user CLI patterns |
| [Team Chat](./use-cases/team-chat.md) | Deploy for team collaboration |
| [Web Interface Deployment](./use-cases/deploying-probe-web-interface.md) | Self-hosted web UI |

---

## Reference

| Document | Description |
|----------|-------------|
| [Architecture](./reference/architecture.md) | System design and internals |
| [How It Works](./reference/how-it-works.md) | Technical deep-dive |
| [Environment Variables](./reference/environment-variables.md) | All configuration options |
| [Output Formats](./reference/output-formats.md) | JSON, XML, Markdown output |
| [Language Support](./reference/language-support.md) | How Probe understands code |
| [Supported Languages](./reference/supported-languages.md) | All supported languages |
| [Adding Languages](./reference/adding-languages.md) | Contribute new language support |
| [Glossary](./reference/glossary.md) | Terms and definitions |
| [FAQ](./reference/faq.md) | Frequently asked questions |
| [Limits](./reference/limits.md) | Token limits and constraints |
| [Troubleshooting](./reference/troubleshooting.md) | Common issues and solutions |

---

## Contributing

| Document | Description |
|----------|-------------|
| [Contributing Guide](https://github.com/probelabs/probe/blob/main/CONTRIBUTING.md) | How to contribute |
| [Documentation Structure](./contributing/documentation-structure.md) | Docs organization |
| [Documentation Maintenance](./contributing/documentation-maintenance.md) | Maintainer guide |
| [Cross-References](./contributing/documentation-cross-references.md) | Link patterns |

---

## Release Information

| Document | Description |
|----------|-------------|
| [Changelog](./changelog.md) | Version history and release notes |
| [GitHub Releases](https://github.com/probelabs/probe/releases) | Download binaries |
