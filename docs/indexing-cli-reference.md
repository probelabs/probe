---
title: LSP Indexing CLI Reference
description: Complete command-line interface reference for Probe's LSP indexing system
---

# LSP Indexing CLI Reference

This document provides comprehensive documentation for all CLI commands related to Probe's LSP indexing system.

## Command Overview

Probe's LSP indexing functionality is accessible through several command groups:

```bash
probe [GLOBAL_OPTIONS] COMMAND [COMMAND_OPTIONS]
```

### Global LSP Options

These options can be used with any command that supports LSP features:

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--lsp` | Flag | `false` | Enable LSP features for this command |
| `--lsp-timeout <MS>` | Integer | `30000` | Request timeout in milliseconds |
| `--lsp-no-cache` | Flag | `false` | Disable caching for this request |
| `--lsp-socket <PATH>` | String | Auto | Custom daemon socket path |

## Core Commands

### `probe extract` (with LSP)

Extract code with enhanced LSP information including call hierarchy.

```bash
probe extract <FILE_PATH>#<SYMBOL> --lsp [OPTIONS]
```

#### Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `<FILE_PATH>` | Yes | Path to source file |
| `<SYMBOL>` | Yes | Symbol name (function, class, etc.) |

#### Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--lsp` | Flag | `false` | Enable LSP call hierarchy extraction |
| `--output <FORMAT>` | String | `text` | Output format: `text`, `json`, `xml` |
| `--context-lines <N>` | Integer | `5` | Additional context lines around symbol |
| `--include-tests` | Flag | `false` | Include test files in call hierarchy |
| `--max-depth <N>` | Integer | `3` | Maximum call hierarchy depth |

#### Examples

```bash
# Basic LSP extraction
probe extract src/auth.rs#authenticate --lsp

# JSON output for programmatic use
probe extract src/calculator.rs#calculate --lsp --output json

# Extended context with test inclusion
probe extract src/api.rs#handle_request --lsp \
  --context-lines 10 \
  --include-tests \
  --max-depth 5

# No caching for debugging
probe extract src/main.rs#main --lsp --lsp-no-cache
```

#### Sample Output

```bash
$ probe extract src/calculator.rs#add --lsp

File: src/calculator.rs
Lines: 15-20
Type: function
Language: Rust

LSP Information:
  Incoming Calls:
    - calculate_total (src/billing.rs:42)
    - run_computation (src/main.rs:28)
    - test_addition (tests/calc_test.rs:15)
  
  Outgoing Calls:
    - validate_input (src/validation.rs:10)
    - log_operation (src/logging.rs:5)

fn add(a: i32, b: i32) -> i32 {
    validate_input(a, b);
    let result = a + b;
    log_operation("add", &[a, b], result);
    result
}
```

### `probe search` (with LSP)

Enhanced search with LSP symbol information.

```bash
probe search <QUERY> [PATH] --lsp [OPTIONS]
```

#### Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `<QUERY>` | Yes | Search query using elastic search syntax |
| `<PATH>` | No | Directory to search (default: current) |

#### Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--lsp` | Flag | `false` | Enrich results with LSP information |
| `--max-results <N>` | Integer | `50` | Maximum number of results |
| `--language <LANG>` | String | All | Filter by language |
| `--symbol-type <TYPE>` | String | All | Filter by symbol type |
| `--include-call-info` | Flag | `false` | Include incoming/outgoing call counts |

#### Examples

```bash
# Search with LSP enrichment
probe search "authenticate" src/ --lsp

# Filter by symbol type
probe search "handler" --lsp --symbol-type function

# Include call hierarchy statistics
probe search "calculate" --lsp --include-call-info --max-results 20
```

## LSP Daemon Commands

### `probe lsp status`

Display daemon status and workspace information.

```bash
probe lsp status [OPTIONS]
```

#### Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--detailed` | Flag | `false` | Show detailed server and cache statistics |
| `--memory` | Flag | `false` | Include memory usage information |
| `--json` | Flag | `false` | Output in JSON format |
| `--refresh` | Flag | `false` | Force refresh of cached status |

#### Examples

```bash
# Basic status
probe lsp status

# Detailed status with memory info
probe lsp status --detailed --memory

# JSON output for scripts
probe lsp status --json
```

#### Sample Output

```bash
$ probe lsp status --detailed

LSP Daemon Status: ✓ Running
Uptime: 2h 34m 12s
PID: 12345
Socket: /tmp/probe-lsp-daemon.sock
Memory Usage: 156 MB

Active Language Servers: 3
  ✓ rust-analyzer (2 workspaces, ready)
  ✓ typescript-language-server (1 workspace, ready)  
  ✓ pylsp (1 workspace, ready)

Workspaces (4 total):
  /home/user/rust-project (Rust) - Ready
  /home/user/web-app/frontend (TypeScript) - Ready
  /home/user/web-app/backend (Rust) - Ready
  /home/user/scripts (Python) - Ready

Cache Statistics:
  Call Hierarchy: 1,243 entries (89% hit rate)
  Definitions: 856 entries (92% hit rate)
  References: 432 entries (85% hit rate)
  Hover: 234 entries (94% hit rate)
  Total Memory: 45 MB

Recent Activity:
  Requests (last hour): 127
  Average Response Time: 15ms
  Errors (last hour): 2
```

### `probe lsp start`

Start the LSP daemon.

```bash
probe lsp start [OPTIONS]
```

#### Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `-f, --foreground` | Flag | `false` | Run in foreground (don't daemonize) |
| `--log-level <LEVEL>` | String | `info` | Log level: `error`, `warn`, `info`, `debug`, `trace` |
| `--socket <PATH>` | String | Auto | Custom socket path |
| `--max-connections <N>` | Integer | `100` | Maximum concurrent connections |
| `--cache-size <N>` | Integer | `500` | Cache entries per operation type |
| `--cache-ttl <SECONDS>` | Integer | `1800` | Cache TTL in seconds |
| `--memory-limit <MB>` | Integer | None | Memory limit in megabytes |
| `--config <PATH>` | String | Auto | Configuration file path |

#### Examples

```bash
# Start daemon with default settings
probe lsp start

# Development mode (foreground with debug logging)
probe lsp start -f --log-level debug

# Production configuration
probe lsp start \
  --cache-size 2000 \
  --cache-ttl 7200 \
  --memory-limit 1024 \
  --max-connections 200

# Custom socket path
probe lsp start --socket /var/run/probe-lsp.sock
```

### `probe lsp restart`

Restart the LSP daemon.

```bash
probe lsp restart [OPTIONS]
```

#### Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--timeout <SECONDS>` | Integer | `30` | Shutdown timeout |
| `--preserve-cache` | Flag | `false` | Keep cache during restart |
| `--wait` | Flag | `true` | Wait for restart to complete |

#### Examples

```bash
# Basic restart
probe lsp restart

# Quick restart with cache preservation
probe lsp restart --preserve-cache --timeout 10

# Restart without waiting
probe lsp restart --no-wait
```

### `probe lsp shutdown`

Stop the LSP daemon.

```bash
probe lsp shutdown [OPTIONS]
```

#### Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--timeout <SECONDS>` | Integer | `30` | Graceful shutdown timeout |
| `--force` | Flag | `false` | Force shutdown (SIGKILL) |
| `--cleanup` | Flag | `true` | Clean up socket and cache files |

#### Examples

```bash
# Graceful shutdown
probe lsp shutdown

# Force shutdown with cleanup
probe lsp shutdown --force --cleanup

# Quick shutdown
probe lsp shutdown --timeout 5
```

## Workspace Management

### `probe lsp init-workspaces`

Initialize language servers for discovered workspaces.

```bash
probe lsp init-workspaces <PATH> [OPTIONS]
```

#### Arguments

| Argument | Required | Description |
|----------|----------|-------------|
| `<PATH>` | Yes | Root path to scan for workspaces |

#### Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `-r, --recursive` | Flag | `false` | Scan directories recursively |
| `-l, --languages <LANGS>` | String | All | Comma-separated language list |
| `--timeout <SECONDS>` | Integer | `30` | Initialization timeout per workspace |
| `--parallel` | Flag | `true` | Initialize workspaces in parallel |
| `--force` | Flag | `false` | Force re-initialization |
| `--dry-run` | Flag | `false` | Show what would be initialized |

#### Supported Languages

- `rust` - Rust projects (Cargo.toml)
- `typescript` - TypeScript/JavaScript projects (package.json)
- `python` - Python projects (pyproject.toml, setup.py)
- `go` - Go projects (go.mod)
- `java` - Java projects (pom.xml, build.gradle)
- `cpp` - C/C++ projects (compile_commands.json)

#### Examples

```bash
# Initialize all workspaces in current directory
probe lsp init-workspaces .

# Recursive initialization
probe lsp init-workspaces /home/user/projects --recursive

# Initialize only specific languages
probe lsp init-workspaces . --languages rust,typescript,python

# Dry run to see what would be initialized
probe lsp init-workspaces . --recursive --dry-run

# Sequential initialization for debugging
probe lsp init-workspaces . --recursive --no-parallel --timeout 60

# Force re-initialization
probe lsp init-workspaces . --force
```

#### Sample Output

```bash
$ probe lsp init-workspaces . --recursive

Discovering workspaces in: /home/user/projects
Scanning recursively...

Found 5 workspaces:
  ✓ /home/user/projects/rust-app (Rust)
  ✓ /home/user/projects/web-frontend (TypeScript)
  ✓ /home/user/projects/api-server (Rust)
  ✓ /home/user/projects/scripts (Python)
  ✓ /home/user/projects/mobile-app (TypeScript)

Initializing language servers...
  ✓ rust-analyzer for /home/user/projects/rust-app (3.2s)
  ✓ typescript-language-server for /home/user/projects/web-frontend (2.1s)
  ✓ rust-analyzer for /home/user/projects/api-server (1.8s)
  ✓ pylsp for /home/user/projects/scripts (1.5s)
  ✓ typescript-language-server for /home/user/projects/mobile-app (2.3s)

Summary:
  Initialized: 5 workspaces
  Languages: rust (2), typescript (2), python (1)
  Total time: 4.2s
  Errors: 0
```

### `probe lsp workspaces`

List and manage registered workspaces.

```bash
probe lsp workspaces [SUBCOMMAND] [OPTIONS]
```

#### Subcommands

| Subcommand | Description |
|------------|-------------|
| `list` | List all registered workspaces (default) |
| `add <PATH> <LANGUAGE>` | Manually add a workspace |
| `remove <PATH>` | Remove a workspace |
| `refresh <PATH>` | Refresh workspace state |

#### Options for `list`

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--language <LANG>` | String | All | Filter by language |
| `--status <STATUS>` | String | All | Filter by status |
| `--json` | Flag | `false` | JSON output |
| `--detailed` | Flag | `false` | Show detailed information |

#### Examples

```bash
# List all workspaces
probe lsp workspaces list

# List only Rust workspaces
probe lsp workspaces list --language rust

# Detailed workspace information
probe lsp workspaces list --detailed

# Add workspace manually
probe lsp workspaces add /path/to/project rust

# Remove workspace
probe lsp workspaces remove /path/to/project

# Refresh workspace state
probe lsp workspaces refresh /path/to/project
```

## Logging and Monitoring

### `probe lsp logs`

View and follow LSP daemon logs.

```bash
probe lsp logs [OPTIONS]
```

#### Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `-n, --lines <N>` | Integer | `50` | Number of recent log entries |
| `-f, --follow` | Flag | `false` | Follow logs in real-time |
| `--level <LEVEL>` | String | All | Filter by log level |
| `--grep <PATTERN>` | String | None | Filter by regex pattern |
| `--since <TIME>` | String | None | Show logs since timestamp |
| `--json` | Flag | `false` | Output in JSON format |
| `--no-color` | Flag | `false` | Disable colored output |

#### Log Levels

- `error` - Error conditions
- `warn` - Warning messages
- `info` - Informational messages  
- `debug` - Debug information
- `trace` - Detailed tracing

#### Examples

```bash
# View recent logs
probe lsp logs

# Follow logs in real-time
probe lsp logs --follow

# View only errors
probe lsp logs --level error -n 100

# Filter by pattern
probe lsp logs --grep "rust-analyzer" -n 200

# Logs since specific time
probe lsp logs --since "2024-01-01 10:00:00"

# JSON output for processing
probe lsp logs --json -n 1000 | jq '.[] | select(.level == "error")'
```

#### Sample Output

```bash
$ probe lsp logs -n 10

2024-01-15 14:30:15.123 INFO  [lsp_daemon] Starting LSP daemon on socket /tmp/probe-lsp-daemon.sock
2024-01-15 14:30:15.124 INFO  [server_manager] Registered rust-analyzer for language Rust
2024-01-15 14:30:15.125 INFO  [server_manager] Registered typescript-language-server for language TypeScript
2024-01-15 14:30:16.200 INFO  [workspace_resolver] Discovered workspace: /home/user/rust-project (Rust)
2024-01-15 14:30:17.156 INFO  [call_graph_cache] Cache HIT for calculate_total at src/main.rs:42:8
2024-01-15 14:30:18.203 DEBUG [lsp_protocol] Incoming call hierarchy request for src/auth.rs:15:4
2024-01-15 14:30:18.204 DEBUG [lsp_protocol] Outgoing: prepareCallHierarchy request to rust-analyzer
2024-01-15 14:30:18.267 DEBUG [lsp_protocol] Incoming: prepareCallHierarchy response from rust-analyzer
2024-01-15 14:30:18.268 INFO  [call_graph_cache] Cache MISS for authenticate at src/auth.rs:15:4
2024-01-15 14:30:18.345 INFO  [call_graph_cache] Cached call hierarchy for authenticate (45 nodes)
```

### `probe lsp stats`

Display daemon performance statistics.

```bash
probe lsp stats [OPTIONS]
```

#### Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--detailed` | Flag | `false` | Show detailed statistics |
| `--reset` | Flag | `false` | Reset statistics after display |
| `--watch <SECONDS>` | Integer | None | Continuously update every N seconds |
| `--json` | Flag | `false` | JSON output |
| `--csv` | Flag | `false` | CSV output for analysis |

#### Examples

```bash
# Basic statistics
probe lsp stats

# Detailed stats
probe lsp stats --detailed

# Watch stats in real-time
probe lsp stats --watch 5

# CSV output for analysis
probe lsp stats --csv > lsp-stats.csv
```

## Indexing Management

### `probe lsp index`

Start indexing a workspace with language-specific processing pipelines.

```bash
probe lsp index [OPTIONS]
```

#### Arguments & Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--workspace <PATH>` | String | `.` | Workspace path to index |
| `--languages <LIST>` | String | All | Comma-separated language list |
| `--recursive` | Flag | `false` | Index nested workspaces recursively |
| `--max-workers <N>` | Integer | CPU count | Maximum worker threads |
| `--memory-budget <MB>` | Integer | `512` | Memory budget in MB |
| `--format <FORMAT>` | String | `terminal` | Output format: `terminal`, `json` |
| `--progress` | Flag | `true` | Show progress bar |
| `--wait` | Flag | `false` | Wait for completion before returning |

#### Examples

```bash
# Index current workspace with all languages
probe lsp index

# Index specific languages only
probe lsp index --languages rust,typescript,python

# Recursive indexing with custom settings
probe lsp index --recursive --max-workers 16 --memory-budget 2048

# Index and wait for completion
probe lsp index --wait --progress

# JSON output for scripting
probe lsp index --format json --languages go
```

### `probe lsp index-status`

Show detailed indexing status and progress.

```bash
probe lsp index-status [OPTIONS]
```

#### Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--format <FORMAT>` | String | `terminal` | Output format: `terminal`, `json` |
| `--detailed` | Flag | `false` | Show per-file progress details |
| `--follow` | Flag | `false` | Follow progress like tail -f |
| `--interval <SECS>` | Integer | `1` | Update interval for follow mode |

#### Examples

```bash
# Basic status
probe lsp index-status

# Detailed per-file status
probe lsp index-status --detailed

# Follow indexing progress
probe lsp index-status --follow --interval 2

# JSON output for monitoring
probe lsp index-status --format json --detailed
```

### `probe lsp index-stop`

Stop ongoing indexing operations.

```bash
probe lsp index-stop [OPTIONS]
```

#### Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--force` | Flag | `false` | Force stop even if in progress |
| `--format <FORMAT>` | String | `terminal` | Output format: `terminal`, `json` |

#### Examples

```bash
# Graceful stop
probe lsp index-stop

# Force stop
probe lsp index-stop --force

# JSON output
probe lsp index-stop --format json
```

### `probe lsp index-config`

Configure indexing settings and behavior.

```bash
probe lsp index-config <SUBCOMMAND> [OPTIONS]
```

#### Subcommands

| Subcommand | Description |
|------------|-------------|
| `show` | Show current configuration |
| `set` | Set configuration options |
| `reset` | Reset to defaults |

#### `probe lsp index-config show`

```bash
probe lsp index-config show [--format FORMAT]
```

#### `probe lsp index-config set`

```bash
probe lsp index-config set [OPTIONS]
```

| Option | Type | Description |
|--------|------|-------------|
| `--max-workers <N>` | Integer | Maximum worker threads |
| `--memory-budget <MB>` | Integer | Memory budget in MB |
| `--exclude <PATTERNS>` | String | Comma-separated exclude patterns |
| `--include <PATTERNS>` | String | Comma-separated include patterns |
| `--max-file-size <MB>` | Integer | Maximum file size to index |
| `--incremental` | Boolean | Enable incremental indexing |

#### `probe lsp index-config reset`

```bash
probe lsp index-config reset [--format FORMAT]
```

#### Examples

```bash
# Show current config
probe lsp index-config show

# Set performance options
probe lsp index-config set --max-workers 20 --memory-budget 4096

# Set file patterns
probe lsp index-config set --exclude "*.log,target/*,node_modules/*"

# Enable incremental mode
probe lsp index-config set --incremental true

# Reset to defaults
probe lsp index-config reset
```

## Cache Management

### `probe lsp cache`

Manage LSP caches with content-addressed storage.

```bash
probe lsp cache <SUBCOMMAND> [OPTIONS]
```

#### Subcommands

| Subcommand | Description |
|------------|-------------|
| `stats` | Show comprehensive cache statistics |
| `clear` | Clear cache entries with optional filtering |
| `export` | Export cache contents to JSON |

**Note**: Cache uses content-addressed storage with MD5 hashing for automatic invalidation when files change. Cache provides 250,000x+ performance improvements for repeated queries.

#### `probe lsp cache stats`

Show comprehensive cache statistics including hit rates and performance metrics.

```bash
probe lsp cache stats
```

No options required - displays full cache statistics:
- Total cached nodes and unique symbols
- Files tracked in cache
- Cache hit rates for all operations
- Memory usage and average response times
- Per-operation statistics (CallHierarchy, Definition, References, Hover)

#### `probe lsp cache clear`

```bash
probe lsp cache clear [OPTIONS]
```

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--operation <OP>` | String | All | Clear specific operation cache |
| `--file <PATH>` | String | None | Clear entries for specific file |
| `--confirm` | Flag | `false` | Skip confirmation prompt |

#### `probe lsp cache export`

```bash
probe lsp cache export [OPTIONS]
```

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--operation <OP>` | String | All | Export specific operation cache |
| `--output <PATH>` | String | Stdout | Output file path |
| `--format <FORMAT>` | String | `json` | Export format: `json`, `csv` |

#### Examples

```bash
# View comprehensive cache statistics
probe lsp cache stats
# Output shows: nodes, symbols, files, hit rates, memory usage, response times

# Clear all cache entries
probe lsp cache clear

# Clear specific operation cache
probe lsp cache clear --operation CallHierarchy
probe lsp cache clear --operation Definition
probe lsp cache clear --operation References
probe lsp cache clear --operation Hover

# Export all cache data to JSON
probe lsp cache export

# Export specific operation cache
probe lsp cache export --operation CallHierarchy

# Monitor cache performance in real-time
watch -n 1 'probe lsp cache stats'
```

## Configuration Management

### `probe lsp config`

Manage LSP configuration.

```bash
probe lsp config <SUBCOMMAND> [OPTIONS]
```

#### Subcommands

| Subcommand | Description |
|------------|-------------|
| `get [KEY]` | Get configuration value(s) |
| `set <KEY> <VALUE>` | Set configuration value |
| `reset [KEY]` | Reset to default value(s) |
| `validate` | Validate configuration |
| `paths` | Show configuration file paths |
| `init` | Create default configuration |

#### Examples

```bash
# View all configuration
probe lsp config get

# Get specific value
probe lsp config get cache.size_per_operation

# Set configuration value
probe lsp config set cache.ttl_seconds 7200

# Reset to defaults
probe lsp config reset cache

# Validate configuration
probe lsp config validate

# Show config file locations
probe lsp config paths

# Create default config file
probe lsp config init
```

## Troubleshooting Commands

### `probe lsp check`

Run system health checks.

```bash
probe lsp check [OPTIONS]
```

#### Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--fix` | Flag | `false` | Attempt to fix issues automatically |
| `--verbose` | Flag | `false` | Show detailed check results |
| `--json` | Flag | `false` | JSON output |

#### Examples

```bash
# Basic health check
probe lsp check

# Detailed check with auto-fix
probe lsp check --fix --verbose
```

### `probe lsp debug`

Debug LSP operations.

```bash
probe lsp debug <OPERATION> [OPTIONS]
```

#### Operations

| Operation | Description |
|-----------|-------------|
| `request <FILE> <SYMBOL>` | Debug specific LSP request |
| `cache-lookup <KEY>` | Debug cache lookup |
| `workspace <PATH>` | Debug workspace resolution |
| `server <LANGUAGE>` | Debug language server state |

#### Examples

```bash
# Debug LSP request
probe lsp debug request src/main.rs main

# Debug cache lookup
probe lsp debug cache-lookup "src/main.rs:main:42:8"

# Debug workspace resolution
probe lsp debug workspace /path/to/project
```

## Exit Codes

All LSP commands use standard exit codes:

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | General error |
| `2` | Command line argument error |
| `3` | Configuration error |
| `4` | Network/communication error |
| `5` | Permission error |
| `6` | File not found error |
| `7` | Timeout error |
| `8` | Cache error |
| `9` | Language server error |

## Shell Completion

Generate shell completion scripts:

```bash
# Bash
probe lsp completion bash > /etc/bash_completion.d/probe-lsp

# Zsh  
probe lsp completion zsh > ~/.zsh/completions/_probe-lsp

# Fish
probe lsp completion fish > ~/.config/fish/completions/probe-lsp.fish

# PowerShell
probe lsp completion powershell > $PROFILE
```

## Environment Variables

See the [Configuration Reference](./indexing-configuration.md#environment-variables) for complete environment variable documentation.

## Next Steps

- **[Language-Specific Guide](./indexing-languages.md)** - Per-language indexing details
- **[Performance Guide](./indexing-performance.md)** - Optimization strategies
- **[API Reference](./indexing-api-reference.md)** - Integration guide for developers