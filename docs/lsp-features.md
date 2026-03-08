---
title: LSP Features
description: What Probe LSP adds, how to run it, and which commands matter day to day.
---

# LSP Features

Probe LSP adds semantic code intelligence on top of search/extract/query.

## What You Get

- Call hierarchy context during extraction with `--lsp`
- Direct symbol operations (`definition`, `references`, `hover`, `implementations`, etc.)
- Persistent daemon lifecycle for low-latency repeated operations
- Workspace indexing and progress reporting
- Log inspection and loop/anomaly analysis

## Fast Start

```bash
# Check daemon
probe lsp status

# Index current workspace
probe lsp index --workspace .
probe lsp index-status

# Use LSP-enriched extraction
probe extract src/main.rs#main --lsp
```

## Direct LSP Calls

```bash
probe lsp call definition src/main.rs#main
probe lsp call references src/main.rs:42:10
probe lsp call hover src/main.rs#main
probe lsp call call-hierarchy src/main.rs#main
probe lsp call implementations src/main.rs#SomeTrait
probe lsp call type-definition src/main.rs:42:10
probe lsp call document-symbols src/main.rs
probe lsp call workspace-symbols main
probe lsp call fqn src/main.rs#main
```

## Daemon Operations

```bash
probe lsp status
probe lsp start -f
probe lsp restart
probe lsp shutdown
probe lsp logs -n 200
probe lsp logs --follow
probe lsp logs --analyze -n 50000 --top 50
probe lsp doctor
```

## Indexing Operations

```bash
probe lsp index --workspace .
probe lsp index --workspace . --wait
probe lsp index --workspace . --recursive
probe lsp index-status
probe lsp index-stop
```

## Related Docs

- [LSP Quick Reference](./lsp-quick-reference.md)
- [Indexing Overview](./indexing-overview.md)
- [Indexing CLI Reference](./indexing-cli-reference.md)
