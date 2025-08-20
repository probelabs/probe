---
title: LSP Features
description: Language Server Protocol integration for advanced code intelligence
---

# LSP Features

Probe integrates with Language Server Protocol (LSP) to provide IDE-level code intelligence from the command line.

## Overview

The LSP integration enables advanced code analysis features by leveraging the same language servers that power modern IDEs. This provides semantic understanding of code beyond simple text matching.

## Key Features

### Call Hierarchy Analysis

See the complete call graph for any function:

```bash
probe extract src/main.rs#calculate_result --lsp
```

Output includes:
- **Incoming Calls**: Functions that call this function
- **Outgoing Calls**: Functions that this function calls

Each call includes the exact file location for easy navigation.

### Multi-Language Support

Probe automatically detects and uses appropriate language servers:

- **Rust** - rust-analyzer
- **Python** - Python LSP Server (pylsp)
- **Go** - gopls
- **TypeScript/JavaScript** - typescript-language-server
- **Java** - Eclipse JDT Language Server
- **C/C++** - clangd

### High-Performance Architecture

The LSP daemon provides:
- **Content-addressed caching** - 250,000x performance improvement for repeated queries
- **Background server management** - Persistent language server pools
- **Connection pooling** - Instant responses after warm-up
- **In-memory logging** - 1000 entries, no disk I/O overhead
- **Concurrent request handling** - Multiple requests processed simultaneously
- **Auto-invalidation** - Cache automatically updates when files change

## Getting Started

### Auto-Initialization

The LSP daemon automatically starts when you use the `--lsp` flag with any command:

```bash
# Extract with LSP features - daemon auto-starts if needed
probe extract src/auth.rs#validate_user --lsp

# Search with LSP enrichment - daemon auto-starts if needed
probe search "authentication" --lsp
```

**No manual setup required!** The daemon initialization is completely transparent:
- Automatically detects if daemon is running
- Starts daemon in background if needed  
- Waits for daemon to be ready
- Proceeds with your command

### Manual Daemon Management

While auto-initialization handles most use cases, you can also manage the daemon manually:

```bash
# Check daemon status and server pools
probe lsp status

# View in-memory logs (no files created)
probe lsp logs

# Follow logs in real-time
probe lsp logs --follow

# View more log entries
probe lsp logs -n 200

# Restart daemon (clears in-memory logs)
probe lsp restart

# Graceful shutdown
probe lsp shutdown

# Start in foreground with debug logging
probe lsp start -f --log-level debug
```

## Understanding Call Hierarchy

### Example Output

```
File: src/calculator.rs
Lines: 10-15
Type: function

LSP Information:
  Incoming Calls:
    - main (file:///src/main.rs:25)
    - test_calculate (file:///tests/calc_test.rs:10)
  Outgoing Calls:
    - add_numbers (file:///src/calculator.rs:20)
    - multiply (file:///src/calculator.rs:30)

fn calculate(a: i32, b: i32) -> i32 {
    let sum = add_numbers(a, b);
    multiply(sum, 2)
}
```

This shows:
- `calculate` is called by `main` and `test_calculate`
- `calculate` calls `add_numbers` and `multiply`

## Performance

### Cache Performance Benefits

The content-addressed cache provides extraordinary performance improvements:

| Operation | First Call | Cached Call | Speedup |
|-----------|------------|-------------|----------|
| **Call Hierarchy** | 200-2000ms | 1-5ms | **250,000x+** |
| **Go to Definition** | 50-500ms | 1-3ms | **50,000x+** |
| **Find References** | 100-1000ms | 2-8ms | **100,000x+** |
| **Hover Information** | 30-200ms | 1-2ms | **30,000x+** |

### Initial Indexing

Language servers need time to analyze your codebase:
- Small projects: 1-3 seconds
- Medium projects: 5-10 seconds  
- Large projects: 10-30 seconds

This only happens once - subsequent requests are instant thanks to caching.

### Optimization Tips

1. **Keep daemon running**: Better performance with warm servers and cache
2. **Use release builds**: `cargo build --release` for production
3. **Pre-warm workspaces**: Run `probe lsp init` after opening projects
4. **Monitor cache**: Use `probe lsp cache stats` to check hit rates
5. **Index in advance**: Use `probe lsp index --wait` for full project indexing

## Advanced Features

### Content-Addressed Cache

Probe uses MD5 content hashing for intelligent cache invalidation:
- **Automatic invalidation** - Cache updates when files change
- **Content-based keys** - Same symbol in different file versions cached separately
- **Dependency tracking** - Related symbols invalidated together
- **Massive speedups** - 250,000x faster for repeated queries

```bash
# View cache statistics
probe lsp cache stats

# Clear specific cache types
probe lsp cache clear --operation CallHierarchy

# Export cache for debugging
probe lsp cache export --operation Definition
```

### Workspace Detection

The daemon automatically detects project roots by looking for:
- `Cargo.toml` (Rust)
- `package.json` (JavaScript/TypeScript)
- `go.mod` (Go)
- `pyproject.toml` or `setup.py` (Python)
- `pom.xml` or `build.gradle` (Java)
- **Nested workspace support** - Automatically discovers all nested workspaces

### Indexing System

Powerful project-wide indexing with progress tracking:

```bash
# Start indexing current workspace
probe lsp index

# Index specific languages
probe lsp index --languages rust,typescript

# Index recursively with custom workers
probe lsp index --recursive --max-workers 8

# Check indexing status with details
probe lsp index-status --detailed

# Follow indexing progress
probe lsp index-status --follow

# Stop ongoing indexing
probe lsp index-stop
```

### Server Pooling

Multiple servers can run simultaneously:
- Different servers for different languages
- Multiple instances for concurrent requests
- Automatic cleanup of idle servers
- Health monitoring and automatic restart

### In-Memory Logging

Logs are stored in memory (last 1000 entries):
- No file permissions issues
- Zero disk I/O overhead
- Automatic rotation
- Microsecond-precision timestamps

## Troubleshooting

### No Call Hierarchy Data

**Cause**: Symbol not at function definition or language server still indexing
**Solution**: 
1. Ensure you're using the exact function name position
2. Wait 10-15s for rust-analyzer to complete indexing
3. Check daemon logs: `probe lsp logs`

### Slow Response

**Cause**: Language server indexing or cold cache
**Solution**: 
1. Wait for initial indexing (10-15s for Rust projects)
2. Subsequent requests will be 250,000x faster due to caching
3. Use `probe lsp index --wait` for full project indexing

### Connection Issues

**Cause**: Daemon startup issues or build lock conflicts
**Solution**: 
1. Daemon auto-starts with `--lsp` flag, no manual intervention needed
2. **Important**: Avoid Rust build lock conflicts:
   ```bash
   # WRONG - causes hangs due to build locks:
   cargo run -- lsp start -f &
   cargo run -- lsp status  # <-- This hangs!
   
   # CORRECT - build first, then use binary:
   cargo build
   ./target/debug/probe lsp start -f &
   ./target/debug/probe lsp status
   ```
3. Check logs: `probe lsp logs --follow`

### Cache Issues

**Cause**: Stale cache entries or memory pressure
**Solution**:
1. Clear cache: `probe lsp cache clear`
2. Check cache stats: `probe lsp cache stats`
3. Restart daemon to reset: `probe lsp restart`

## Configuration

### Environment Variables

```bash
# Custom timeout (milliseconds)
PROBE_LSP_TIMEOUT=300000 probe extract file.rs#fn --lsp

# Custom socket path
PROBE_LSP_SOCKET=/custom/socket probe lsp start
```

### Debug Mode

```bash
# Start with debug logging
probe lsp start -f --log-level debug

# View debug logs
probe lsp logs -n 100
```

## Use Cases

### Code Review

Understand unfamiliar code quickly:
```bash
probe extract src/auth/handler.rs#authenticate --lsp
```

### Refactoring

Identify all callers before changing APIs:
```bash
probe extract src/api/v1.rs#deprecated_endpoint --lsp | grep "Incoming"
```

### Test Coverage

Find which tests exercise specific functions:
```bash
probe extract src/core.rs#critical_function --lsp | grep test_
```

### Documentation

Generate comprehensive function documentation:
```bash
probe extract src/lib.rs#public_api --lsp > docs/api.md
```

## Future Roadmap

Planned enhancements:
- Go-to definition
- Find all references
- Hover documentation
- Code completion
- Rename refactoring
- Quick fixes

## Learn More

### Comprehensive Indexing Documentation

For detailed information about Probe's LSP indexing system:

- **[📖 Indexing Overview](./indexing-overview.md)** - What is indexing, benefits, and key concepts
- **🏗️ [Architecture Guide](./indexing-architecture.md)** - Deep dive into system internals and data flow
- **⚙️ [Configuration Reference](./indexing-configuration.md)** - Complete configuration options and environment variables
- **💻 [CLI Reference](./indexing-cli-reference.md)** - Detailed command documentation
- **🔧 [Language-Specific Guide](./indexing-languages.md)** - How each language is indexed and optimized
- **⚡ [Performance Guide](./indexing-performance.md)** - Optimization strategies and benchmarks
- **🔌 [API Reference](./indexing-api-reference.md)** - Integration guide for developers

### Additional Resources

- [Architecture Documentation](/docs/LSP_INTEGRATION.md)
- [Quick Reference](/docs/LSP_QUICK_REFERENCE.md)
- [Blog: LSP Integration Release](/blog/lsp-integration-release)