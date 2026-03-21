---
layout: home
title: Probe Documentation
hero:
  name: Probe
  text: "Developer-First\nCode Intelligence"
  tagline: Local-first semantic code search, extraction, and agent workflows.
  image:
    src: /logo.png
    alt: Probe Logo
  actions:
    - theme: brand
      text: Quick Start
      link: /quick-start
    - theme: alt
      text: GitHub
      link: https://github.com/probelabs/probe
---

# Documentation

Pick a path based on what you need to do.

## Start Here

| Goal | Read |
|------|------|
| Install and run Probe quickly | [Quick Start](./quick-start.md), [Installation](./installation.md) |
| Understand major capabilities | [Features](./features.md) |
| Use CLI commands directly | [CLI Reference](./probe-cli/cli-reference.md) |
| Use Agent / MCP / SDK | [Probe Agent Overview](./probe-agent/overview.md) |
| Use LSP and indexing | [LSP Features](./lsp-features.md), [LSP Quick Reference](./lsp-quick-reference.md) |

## Probe CLI

| Document | Description |
|----------|-------------|
| [Search](./probe-cli/search.md) | Semantic code search and query syntax |
| [Extract](./probe-cli/extract.md) | Block extraction by line, range, symbol, or stdin/diff |
| [Symbols](./probe-cli/symbols.md) | File symbol tree / table of contents with line numbers |
| [Query](./probe-cli/query.md) | AST-grep structural search |
| [CLI Reference](./probe-cli/cli-reference.md) | Command matrix and options |

## LSP and Indexing

| Document | Description |
|----------|-------------|
| [LSP Features](./lsp-features.md) | What `--lsp` adds and when to use it |
| [LSP Quick Reference](./lsp-quick-reference.md) | Fast command cheat sheet |
| [Indexing Overview](./indexing-overview.md) | Indexing model and workflows |
| [Indexing CLI Reference](./indexing-cli-reference.md) | `probe lsp index*` command details |

## Probe Agent

| Document | Description |
|----------|-------------|
| [Agent Overview](./probe-agent/overview.md) | Platform overview and usage modes |
| [AI Integration](./probe-agent/ai-integration.md) | Providers, auth, and integration patterns |
| [MCP Integration](./probe-agent/protocols/mcp-integration.md) | Editor integration setup |
| [Node.js SDK](./probe-agent/sdk/nodejs-sdk.md) | Programmatic usage |
| [LLM Script](./llm-script.md) | Deterministic multi-step orchestration |

## Reference

| Document | Description |
|----------|-------------|
| [Architecture](./reference/architecture.md) | System architecture |
| [How It Works](./reference/how-it-works.md) | Pipeline and internals |
| [Environment Variables](./reference/environment-variables.md) | Runtime configuration |
| [Output Formats](./reference/output-formats.md) | JSON/XML/plain/markdown formats |
| [Language Support](./reference/language-support.md) | Parsing/extraction behavior by language |
| [Supported Languages](./reference/supported-languages.md) | File extensions and language matrix |
| [Troubleshooting](./reference/troubleshooting.md) | Common issues and fixes |

## Changelog

- [Changelog](./changelog.md)
