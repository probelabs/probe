# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with the probe codebase.

**IMPORTANT**: ALWAYS use code-search-agent to navigate the codebase and ask questions about it.

## Project Overview

Probe is an AI-friendly, fully local, semantic code search tool built in Rust that combines ripgrep's speed with tree-sitter's AST parsing. It's designed to power AI coding assistants with precise, context-aware code search and extraction capabilities.

## Development Guidelines

### 1. Testing Requirements

**EVERY feature, bug fix, or change MUST include tests:**
- Unit tests for new functions/modules (in same file using `#[cfg(test)]`)
- Integration tests for cross-module functionality (in `tests/` directory)
- CLI tests for command-line interface changes
- Property-based tests with `proptest` for complex logic
- Test coverage for edge cases and error conditions

**Before committing:**
```bash
make test           # Run all tests
make test-unit      # Run unit tests only
make test-cli       # Run CLI tests
make test-property  # Run property-based tests
```

**LSP Integration Testing:**
```bash
# Run specific LSP tests with mock servers
cargo test -p lsp-daemon mock_lsp_server_test --lib -- --nocapture
cargo test -p lsp-daemon core_lsp_operation_tests --lib -- --nocapture
cargo test -p lsp-daemon lsp_symbol_resolution_tests --lib -- --nocapture

# Run all LSP-related integration tests
cargo test -p lsp-daemon --test "*lsp*" -- --nocapture
```

**LSP Mock Server Infrastructure:**
The project includes comprehensive mock LSP servers for testing (`lsp-daemon/tests/mock_lsp/`):
- **4 Language Servers**: rust-analyzer, pylsp, gopls, TypeScript server
- **Realistic Scenarios**: Success, empty arrays, null responses, errors, timeouts, sequences
- **Full LSP Protocol**: Call hierarchy, references, definitions, symbols, hover
- **Edge Case Testing**: Validates proper handling of empty vs null responses
- **Database Integration**: Tests actual persistence and caching of LSP data

Mock servers simulate realistic response times (25-200ms) and can be configured for:
- Normal operation testing
- Error handling validation
- Timeout scenarios
- Performance benchmarking
- Cache behavior verification

### 2. Error Handling

**Key principles:**
- Always use proper error handling with `anyhow`
- Return `Result<T>` for all fallible operations
- Use `.context()` to add error context
- Create custom error types when domain-specific errors needed
- Never use `.unwrap()` except in tests

### 3. Code Quality Standards

**Before EVERY commit run these commands:**
```bash
make format         # Format code with rustfmt
make lint           # Run clippy linter
make test           # Run all tests
# OR simply:
make fix-all        # Runs format + lint fixes + tests
```

**Code organization principles:**
- Keep modules focused and single-purpose
- Use descriptive names (avoid abbreviations like `cfg`, `ctx`)
- Add doc comments (`///`) for all public APIs
- Keep functions under 50 lines when possible
- Group related functionality in modules

### 4. Performance Considerations

- Profile before optimizing using `DEBUG=1` environment variable
- Document performance-critical code with comments
- Prefer lazy evaluation and iterators over collecting
- Use `rayon` for CPU-bound parallel tasks
- Leverage caching for expensive operations
- See `docs/PERFORMANCE_OPTIMIZATION.md` for detailed methodology

### 5. Adding New Features

**Process for new features:**
1. Search existing code for similar patterns using probe itself
2. Write tests FIRST (TDD approach)
3. Implement the feature
4. Ensure all tests pass
5. Update relevant documentation
6. Run `make fix-all` before committing

**For new language support:**
1. Add tree-sitter parser to `Cargo.toml`
2. Create module in `src/language/` implementing `Language` trait
3. Add comprehensive tests including test detection
4. Register in `src/language/factory.rs`
5. Update docs in `site/supported-languages.md`

## Common Commands

```bash
# Building
make build              # Debug build
cargo build --release   # Release build with optimizations

# Testing & Quality
make test              # All tests
make test-unit         # Unit tests only
make lint              # Run clippy
make format            # Format code
make fix-all           # Fix everything automatically

# Running probe
cargo run -- search "query" ./path
cargo run -- extract file.rs:42
probe search "function" ./src --max-results 10

# Performance debugging
DEBUG=1 probe search "query" ./path  # Shows timing information
```

## Architecture Quick Reference

### Core Components
- `src/language/` - Language parsers using tree-sitter
- `src/search/` - Search engine with Elastic query support
- `src/extract/` - Code extraction logic
- `src/ranking.rs` - BM25 and TF-IDF ranking
- `src/simd_ranking.rs` - SIMD-optimized ranking

### Key Design Patterns
- **Result<T> everywhere**: All fallible operations return Result
- **Lazy evaluation**: Parse only what's needed
- **Parallel processing**: Use rayon for file processing
- **Caching**: Multiple cache levels (parser pool, tree cache, search cache)
- **Zero-copy where possible**: Use references and slices

## GitHub Integration

**ALWAYS use `gh` CLI for GitHub operations:**
```bash
gh issue view <number> --comments    # View issue with full discussion
gh pr view <number> --comments       # View PR with reviews
gh pr create                         # Create pull request
```

## Project-Specific Patterns

### Testing Patterns
- Unit tests use `#[cfg(test)]` modules in same file
- Integration tests go in `tests/` directory  
- Use `assert!`, `assert_eq!`, `assert_ne!` macros
- Property tests with `proptest` for fuzzing
- Test both success and error cases

### Module Organization
- Keep modules focused and single-purpose
- Use descriptive module and function names
- Group related functionality together
- Unit tests go in `#[cfg(test)]` modules in same file
- Integration tests go in `tests/` directory

### Common Idioms
- Use `Arc<>` for shared immutable data
- Prefer `&str` over `String` for function parameters
- Use `PathBuf` for owned paths, `&Path` for borrowed
- Implement `Display` and `Debug` for public types
- Use `#[derive()]` for common traits

## Important Notes

### NPM Package Management
When adding files to `examples/chat/`, ALWAYS update the `files` array in `examples/chat/package.json`.

### Pre-commit Hook
The project has git hooks in `.githooks/`. Install with:
```bash
make install-hooks
```

### Debugging Tips
- Use `DEBUG=1` for verbose output
- Check `error.log` for detailed errors
- Use `RUST_BACKTRACE=1` for stack traces
- Profile with `cargo flamegraph` for performance

### Tree-sitter Debugging

**When encountering tree-sitter parsing issues:**
- Use the standalone debugging script: `./test_tree_sitter_standalone.rs`
- Tests parsing for multiple languages (Rust, Python, TypeScript, JavaScript)
- Shows parsed AST structure for debugging
- Helpful for pattern matching and parser compatibility issues

### LSP Architecture & Debugging

#### Architecture Overview
The LSP integration uses a daemon-based architecture:

```
CLI Client → IPC Socket → LSP Daemon → Server Manager → Language Servers
                              ↓
                        In-Memory Log Buffer (1000 entries)
                              ↓
                        Universal Cache System (database-backed)
```

**Key Components:**
- **LSP Daemon**: Persistent background service at `lsp-daemon/src/daemon.rs`
- **Server Manager**: Pool management at `lsp-daemon/src/server_manager.rs`
- **LSP Client**: IPC communication at `src/lsp_integration/client.rs`
- **Protocol Layer**: Request/response types at `lsp-daemon/src/protocol.rs`
- **Logging System**: In-memory circular buffer at `lsp-daemon/src/logging.rs`

#### Debugging LSP Issues

**CRITICAL: Avoid Rust Build Lock Contention**
- Build first with `cargo build`, then use the binary
- Don't run multiple `cargo run` commands simultaneously
- Use `./target/debug/probe` or installed `probe` binary

**1. View LSP daemon logs (in-memory, no files):**
```bash
probe lsp logs              # View last 50 log entries
probe lsp logs -n 100       # View last 100 entries
probe lsp logs --follow     # Follow logs in real-time (polls every 500ms)
```

**2. Check daemon status and server pools:**
```bash
probe lsp status            # Show daemon status, uptime, and server pools
probe lsp shutdown          # Stop daemon cleanly
probe lsp restart           # Restart daemon (clears in-memory logs)
```

**3. Debug in foreground mode:**
```bash
# Run daemon in foreground with debug logging
./target/debug/probe lsp start -f --log-level debug

# In another terminal, test LSP operations
./target/debug/probe extract file.rs#symbol --lsp
```

**4. Common LSP issues and solutions:**

| Issue | Cause | Solution |
|-------|-------|----------|
| **No call hierarchy data** | Language server still indexing | Wait 10-15s for rust-analyzer to index |
| **Timeout errors** | Large codebase or slow language server | Increase timeout in client config |
| **Connection refused** | Daemon not running | Daemon auto-starts, check `probe lsp status` |
| **Empty responses** | Symbol not at function definition | Use exact function name position |
| **Incomplete message** | Concurrent request conflict | Retry the operation |
| **25s timeout errors** | Duplicate document opening calls | Check for concurrent `textDocument/didOpen` - avoid opening same document multiple times |

**5. Language Server Timings:**
- **rust-analyzer**: 10-15s initial indexing for large projects
- **pylsp**: 2-3s for Python projects
- **gopls**: 3-5s for Go modules
- **typescript-language-server**: 5-10s for node_modules

**6. Log Analysis Commands:**
```bash
# Check for errors
probe lsp logs -n 200 | grep ERROR

# Monitor specific language server
probe lsp logs --follow | grep rust-analyzer

# Check initialization timing
probe lsp logs | grep "initialize.*response"

# View call hierarchy requests
probe lsp logs | grep "prepareCallHierarchy\|incomingCalls\|outgoingCalls"
```

**7. Performance Monitoring:**
The in-memory log buffer stores:
- Timestamp with microsecond precision
- Log level (ERROR, WARN, INFO, DEBUG)
- Source file and line number
- Target component (e.g., "lsp_protocol", "lsp_stderr")
- Full message content including JSON-RPC payloads

**8. Daemon Communication:**
- Uses Unix domain sockets on macOS/Linux: `/var/folders/.../lsp-daemon.sock`
- Named pipes on Windows: `\\.\pipe\lsp-daemon`
- Binary protocol with JSON serialization
- UUID-based request tracking for concurrent operations
- See `docs/LSP_CLIENT_GUIDE.md` for complete client implementation guide

### Per-Workspace Cache System

#### What is Per-Workspace Caching?

Probe now implements sophisticated per-workspace caching that creates separate cache instances for each workspace, enabling:

**Key Benefits:**
- **Isolation**: Each project has its own cache, preventing cache pollution between projects
- **Monorepo Support**: Nested workspaces in monorepos get their own caches automatically
- **Intelligent Routing**: Files are cached in the nearest workspace (e.g., backend/src/main.rs goes to backend workspace)
- **Team Collaboration**: Workspace-specific caches can be shared within teams
- **Resource Management**: LRU eviction of least-used workspace caches when memory limits are reached

#### Cache Directory Structure

```
~/Library/Caches/probe/lsp/workspaces/         # macOS
~/.cache/probe/lsp/workspaces/                  # Linux
%LOCALAPPDATA%/probe/lsp/workspaces/            # Windows

├── abc123_my-rust-project/
│   ├── cache.db                               # unified cache database
│   └── metadata.json                          # cache statistics
├── def456_backend-service/
│   ├── cache.db
│   └── metadata.json
└── ghi789_frontend-app/
    ├── cache.db
    └── metadata.json
```

**Directory Naming Convention:**
- Format: `{workspace_hash}_{workspace_name}/`
- Hash: First 6 chars of SHA256 hash of workspace absolute path
- Name: Sanitized workspace directory name (safe for filesystems)

#### Cache Resolution Strategy

The system uses a **nearest workspace wins** strategy:

1. **File Analysis**: For any file (e.g., `/project/backend/src/auth.rs`)
2. **Workspace Discovery**: Walk up directory tree looking for workspace markers
3. **Workspace Selection**: Choose nearest workspace (`/project/backend/` beats `/project/`)
4. **Cache Routing**: Route all cache operations to that workspace's cache

**Workspace Detection Markers:**
- **Rust**: `Cargo.toml`
- **TypeScript/JavaScript**: `package.json`, `tsconfig.json`
- **Python**: `pyproject.toml`, `setup.py`, `requirements.txt`
- **Go**: `go.mod`
- **Java**: `pom.xml`, `build.gradle`
- **C/C++**: `CMakeLists.txt`
- **Generic**: `.git`, `README.md`

#### CLI Commands for Workspace Cache Management

**List workspace caches:**
```bash
probe lsp cache list                           # Show all workspace caches
probe lsp cache list --detailed               # Include cache statistics
probe lsp cache list --format json            # JSON output for scripting
```

**View workspace cache information:**
```bash
probe lsp cache info                           # Show info for all workspaces
probe lsp cache info /path/to/workspace        # Show info for specific workspace
probe lsp cache info --format json            # JSON format
```

**Clear workspace caches:**
```bash
probe lsp cache clear-workspace                # Clear all workspace caches (with confirmation)
probe lsp cache clear-workspace /path/to/workspace  # Clear specific workspace
probe lsp cache clear-workspace --force        # Skip confirmation prompt
```

**Cache statistics:**
```bash
probe lsp cache stats                          # Combined stats across all workspaces
probe lsp cache stats --detailed              # Per-workspace breakdown
```

#### Configuration

**Environment Variables:**
- `PROBE_LSP_WORKSPACE_CACHE_MAX`: Max concurrent open caches (default: 8)
- `PROBE_LSP_WORKSPACE_CACHE_SIZE_MB`: Size limit per workspace (default: 100MB)
- `PROBE_LSP_WORKSPACE_LOOKUP_DEPTH`: Max parent dirs to search (default: 3)
- `PROBE_LSP_WORKSPACE_CACHE_DIR`: Custom cache directory location

**Configuration File:** `~/.config/probe/lsp.toml` for persistent settings

#### Troubleshooting Workspace Cache Issues

**1. Cache Directory Permissions:**
```bash
# Check cache directory exists and is writable
ls -la ~/Library/Caches/probe/lsp/workspaces/
# Should show drwx------ (700) permissions

# Fix permissions if needed
chmod 700 ~/Library/Caches/probe/lsp/workspaces/
```

**2. Cache Not Found for File:**
```bash
# Debug workspace resolution for a specific file
probe lsp debug workspace /path/to/file.rs

# Check which workspace a file maps to
probe lsp cache info /path/to/project/
```

**3. Cache Performance Issues:**
```bash
# Check if too many caches are open
probe lsp cache stats --detailed

# Look for cache evictions in logs
probe lsp logs -n 100 | grep "evicted\|LRU"

# Increase max open caches if needed
export PROBE_LSP_WORKSPACE_CACHE_MAX=16
```

**4. Disk Space Issues:**
```bash
# Check cache sizes
probe lsp cache list --detailed

# Clean up old entries
probe lsp cache compact --clean-expired

# Clear unused workspace caches
probe lsp cache clear-workspace --force
```

#### Performance Implications

**Memory Usage:**
- Each open workspace cache uses ~5-20MB of RAM
- Default limit of 8 concurrent caches = ~40-160MB max
- LRU eviction automatically manages memory pressure

**Disk Usage:**
- Each workspace cache limited to 100MB by default
- Compressed storage reduces disk usage by ~60-70%
- Automatic cleanup of entries older than 30 days

**Cache Hit Rates:**
- Per-workspace caches typically achieve 90-95% hit rates
- Better isolation means fewer false cache misses
- Nested workspaces benefit from focused caching

#### Migration from Global Cache

**Automatic Migration:**
- No manual migration needed
- Old global cache continues to work as fallback
- New workspace caches gradually populate with usage
- Old cache can be cleared after workspace caches are established

**Verifying Migration:**
```bash
# Check that workspace caches are being used
probe lsp cache stats --detailed

# Should show multiple workspace entries, not just global cache
# Look for entries like "workspace_abc123_my-project"
```

#### Database-First Cache Debugging

**Database Infrastructure Validation:**
The database-first LSP caching system (Milestone 31) uses SQLite databases for persistent caching. Here's how to debug and validate the system:

**1. Database Creation Verification:**
```bash
# Check that database files are created
find ~/Library/Caches/probe/lsp/workspaces -name "cache.db" -exec ls -la {} \;

# Verify databases are valid SQLite files
find ~/Library/Caches/probe/lsp/workspaces -name "cache.db" -exec file {} \;
# Should show: "SQLite 3.x database"
```

**2. Database Content Inspection:**
```bash
# Check database schema and tables
sqlite3 ~/Library/Caches/probe/lsp/workspaces/*/cache.db ".schema"

# Count cache entries
sqlite3 ~/Library/Caches/probe/lsp/workspaces/*/cache.db "SELECT COUNT(*) FROM cache_entries;"

# View recent cache entries
sqlite3 ~/Library/Caches/probe/lsp/workspaces/*/cache.db "SELECT key, created_at FROM cache_entries ORDER BY created_at DESC LIMIT 10;"
```

**3. Cache Hit/Miss Debugging:**
```bash
# View cache statistics with hit rates
probe lsp cache stats

# Monitor cache operations in real-time
probe lsp logs --follow | grep -E "(HIT|MISS|DATABASE)"

# Test cache miss/hit cycle
probe lsp call definition src/main.rs:10:5  # First call (miss)
probe lsp call definition src/main.rs:10:5  # Second call (should hit)
```

**4. Workspace Isolation Validation:**
```bash
# List all workspace caches
probe lsp cache list --detailed

# Verify workspace-specific databases exist
ls -la ~/Library/Caches/probe/lsp/workspaces/*/

# Check workspace ID generation
echo "Current workspace:" $(pwd)
probe lsp status | grep -i workspace
```

**5. Database Performance Monitoring:**
```bash
# Monitor database operation times
probe lsp logs | grep "Database operation"

# Check database file sizes
du -h ~/Library/Caches/probe/lsp/workspaces/*/cache.db

# Verify database integrity
for db in ~/Library/Caches/probe/lsp/workspaces/*/cache.db; do
    echo "Checking $db"
    sqlite3 "$db" "PRAGMA integrity_check;"
done
```

**6. Common Database Issues and Solutions:**

| Issue | Symptom | Solution |
|-------|---------|----------|
| **Database not created** | No cache.db files found | Check workspace detection: `probe lsp init --workspace .` |
| **Schema missing** | SQLite error on operations | Restart daemon to trigger migration: `probe lsp restart` |
| **No cache hits** | 0% hit rate after multiple calls | Check cache key generation in debug logs |
| **Database corruption** | SQLite integrity check fails | Clear and recreate: `probe lsp cache clear-workspace --force` |
| **Permission errors** | Access denied to cache directory | Fix permissions: `chmod 700 ~/Library/Caches/probe/lsp/workspaces/` |

**7. Debug Log Analysis:**
```bash
# Look for database creation messages
probe lsp logs | grep "DATABASE_CACHE_ADAPTER.*Creating"

# Check for workspace cache routing
probe lsp logs | grep "WORKSPACE_CACHE_ROUTER"

# Monitor SQLite backend operations
probe lsp logs | grep "SQLite.*backend"

# Track cache key generation
probe lsp logs --follow | grep "cache.*key"
```

**8. Performance Validation:**
```bash
# Test concurrent database operations
for i in {1..5}; do
    probe lsp call definition src/main.rs:$((10+i)):5 &
done
wait

# Verify no database locks or corruption after concurrent access
sqlite3 ~/Library/Caches/probe/lsp/workspaces/*/cache.db "PRAGMA integrity_check;"
```

**9. Production Readiness Checklist:**
- [ ] Database files created in workspace directories
- [ ] SQLite integrity checks pass
- [ ] Cache hit rates above 70% after warmup
- [ ] No errors in database operation logs
- [ ] Concurrent operations complete successfully
- [ ] Workspace isolation working (separate databases per workspace)

**10. Emergency Database Recovery:**
```bash
# Complete cache reset (nuclear option)
probe lsp shutdown
rm -rf ~/Library/Caches/probe/lsp/workspaces/*/cache.db
probe lsp start -f --log-level debug

# Selective workspace cache reset
probe lsp cache clear-workspace /path/to/workspace --force

# Export/backup before major changes
probe lsp cache export --output backup-$(date +%Y%m%d).json
```

The database-first caching system is considered production-ready when:
- All validation checks pass
- Cache hit rates are consistently above 70%
- No database integrity issues under concurrent load
- Workspace isolation is functioning correctly

#### Best Practices

**For Monorepos:**
- Each sub-project gets its own cache automatically
- Shared libraries cached in root workspace
- Configure larger cache limits for monorepos (e.g., set `PROBE_LSP_WORKSPACE_CACHE_MAX=16`)

**For Development Teams:**
- Workspace caches can be backed up and shared
- Export/import commands work on per-workspace basis
- Cache names include workspace path hash for uniqueness

**For CI/CD:**
- Workspace caches work great in containerized environments
- No git dependencies - pure filesystem-based detection
- Cache sharing between builds of same workspace is automatic

## LSP Debugging & Troubleshooting

### Common LSP Issues and Debugging Steps

#### 1. LSP Daemon Crashes or Stops Responding

**Symptoms:**
- Commands hang indefinitely
- "Connection refused" errors
- No response from LSP daemon

**Debugging Steps:**
```bash
# 1. Check daemon status
probe lsp status

# 2. View recent logs for crash information
probe lsp logs -n 100

# 3. Look for specific error patterns
probe lsp logs -n 200 | grep -E "(ERROR|WARN|panic|crash|timeout)"

# 4. Check for connection issues
probe lsp logs | grep -E "(Client connected|Client disconnected|Broken pipe)"

# 5. Restart daemon cleanly
probe lsp restart
```

#### 2. LSP Request Timeouts

**Symptoms:**
- "Request processing timed out after 25s"
- "Broken pipe (os error 32)"
- Large file processing failures

**Common Causes:**
- **Large files**: Files > 100KB can cause timeouts during `textDocument/didOpen`
- **rust-analyzer indexing**: Initial workspace indexing takes 10-15s
- **Multiple duplicate requests**: Concurrent `didOpen` calls for same document

**Debugging Commands:**
```bash
# Check for timeout patterns
probe lsp logs | grep -E "(timed out|timeout|25s)"

# Monitor large file operations
probe lsp logs | grep -E "(didOpen|TRUNCATED)"

# Watch for duplicate document operations
probe lsp logs --follow | grep -E "(didOpen|didClose)"
```

**Solutions:**
- Wait for rust-analyzer to complete initial indexing (10-15s)
- Avoid calling same file position multiple times concurrently
- Check file sizes before processing (skip files > 50KB)

#### 3. Language Server Initialization Issues

**Symptoms:**
- "Discovering sysroot" messages
- "file not found" errors immediately after daemon start
- LSP responses contain setup/fetching messages

**Debugging:**
```bash
# Check language server initialization progress
probe lsp logs | grep -E "(Fetching|Discovering|initialize)"

# Monitor workspace registration
probe lsp logs | grep -E "(workspace.*registered|Ensuring workspace)"

# Check for premature requests during setup
probe lsp logs | grep -E "(Cache miss.*proceeding to LSP)"
```

**Solutions:**
- Wait 15-30 seconds after daemon start before making requests
- Let rust-analyzer complete workspace indexing
- Avoid rapid-fire requests during startup

#### 4. Database-Related LSP Issues

**Symptoms:**
- "Database operation failed" in logs
- Cache misses despite recent requests
- Workspace creation errors

**Debugging:**
```bash
# Check database operations
probe lsp logs | grep -E "(DATABASE|SQLite|cache\.db)"

# Monitor workspace cache creation
probe lsp logs | grep -E "(Creating workspace cache|Successfully created.*backend)"

# Check for SQL compatibility issues
probe lsp logs | grep -E "(SQL.*failed|unexpected row|PRAGMA)"
```

#### 5. Background Task Issues

**Symptoms:**
- High CPU usage
- Excessive log messages
- Background processes not working

**Debugging:**
```bash
# Monitor checkpoint tasks (should run every 5s)
probe lsp logs | grep -i checkpoint

# Check for background task errors
probe lsp logs | grep -E "(checkpoint.*failed|background.*error)"

# Monitor task spawn/completion
probe lsp logs --follow | grep -E "(spawned|completed|task)"
```

### LSP Log Analysis Patterns

#### Successful Request Flow:
```
>>> TO LSP: {"method":"textDocument/definition"...}
<<< FROM LSP: {"result": [...]}
Cache stored for /path/to/file.rs:line:col
```

#### Failed Request Patterns:
```bash
# Timeout pattern
>>> TO LSP: {"method":"textDocument/definition"...}
# ... long delay ...
Request processing timed out after 25s
Failed to send response: Broken pipe

# File not found pattern
>>> TO LSP: {"method":"textDocument/definition"...}
<<< FROM LSP: {"error":{"code":-32603,"message":"file not found"}}

# Initialization conflict pattern
>>> TO LSP: {"method":"textDocument/definition"...}
<<< FROM LSP: {"method":"window/workDoneProgress/create"...}
<<< FROM LSP: "Discovering sysroot"
<<< FROM LSP: {"error": "file not found"}
```

### LSP Performance Monitoring

#### Monitor Request Response Times:
```bash
# Check for slow requests (>1s)
probe lsp logs | grep -E "(Cache miss|proceeding to LSP)" | head -10

# Monitor cache hit rates
probe lsp cache stats

# Watch real-time LSP communication
probe lsp logs --follow | grep -E "(>>> TO LSP|<<< FROM LSP)"
```

#### Database Performance:
```bash
# Check checkpoint frequency and success
probe lsp logs | grep checkpoint | tail -20

# Monitor database creation times
probe lsp logs | grep -E "(Creating workspace cache|Successfully created.*SQLite)"

# Check for database lock issues
probe lsp logs | grep -E "(database.*lock|SQLite.*busy)"
```

### Emergency Recovery Procedures

#### Complete LSP Reset:
```bash
# 1. Stop all processes
probe lsp shutdown

# 2. Clear all caches
probe lsp cache clear-workspace --force

# 3. Remove daemon socket (if stuck)
rm -f /var/folders/*/T/lsp-daemon.sock

# 4. Start fresh
probe lsp start -f --log-level debug
```

#### Selective Workspace Reset:
```bash
# Clear specific workspace cache
probe lsp cache clear-workspace /path/to/workspace --force

# Restart without clearing all caches
probe lsp restart
```

### Prevention Best Practices

1. **Avoid Concurrent Requests**: Don't make multiple LSP calls for the same file simultaneously
2. **Wait for Initialization**: Allow 15-30s after daemon start before heavy usage
3. **Monitor File Sizes**: Be cautious with files > 50KB
4. **Regular Log Monitoring**: Check `probe lsp logs` periodically for warnings
5. **Workspace Awareness**: Understand which workspace each file belongs to

### Log Retention and Cleanup

```bash
# Logs are kept in-memory (1000 entries max)
# To clear logs, restart daemon:
probe lsp restart

# For long-term debugging, redirect to file:
probe lsp start -f --log-level debug 2>&1 | tee /tmp/lsp-debug.log
```

## Getting Help

1. Search codebase first: `probe search "topic" ./src`
2. Check existing tests for usage examples
3. Review similar implementations
4. Consult docs in `site/` directory

Remember: **Quality > Speed**. Write tests, handle errors properly, and maintain code standards.

## Critical Development Patterns

### Database & Async Operations

**Key Database Rules:**
- Each `:memory:` DuckDB connection creates isolated database - apply schema per connection
- Use file locking for cross-process database safety
- Never use `.unwrap()` on database operations in production
- For in-memory databases, apply schema directly in connection creation method

**Database Backend Selection:**

### Cache System Architecture

**Universal Cache Design:**
- Single unified cache layer for all LSP operations
- Persistent workspace-based storage with per-project isolation  
- Direct database access for optimal performance

**Cache Key Generation:**
- Use consistent hash algorithms across all components
- Include workspace_id, method_name, file_path, and content_hash
- Use Blake3 for workspace ID hashing

### Testing & Build Practices

**Rust Build Lock Avoidance:**
- Build first with `cargo build`, then use the binary
- Avoid running multiple `cargo run` commands simultaneously
- Use `./target/debug/probe` for concurrent operations

**Test Data Requirements:**
- CLI limit tests need sufficient data to actually trigger limits
- Multi-term search tests need content containing all search terms
- Performance tests should include realistic data sizes

**Database Storage Testing:**
- Always test actual persistence and retrieval, not stubs
- Verify data persists across cache instance recreation
- Use real database connections in tests, not mocks

**Critical Testing Rules:**
- NEVER bypass pre-commit hooks with `--no-verify`
- NEVER disable tests to hide compilation errors - fix root causes
- Run `cargo fmt`, `cargo clippy`, `cargo check` separately when debugging
- Always use 10-minute timeouts for Rust compilation operations
- Test actual database persistence, not stub implementations

### Workspace Resolution & LSP

**Symbol Position Finding:**
- Always use tree-sitter for deterministic position finding
- Use AST-based lookup, not text search
- Never use hardcoded position tables

**LSP Debugging:**
- Check daemon status before direct database access
- Restart daemon after code changes to avoid source/binary mismatches  
- Add detailed cache key logging for debugging invisible mismatches
- Use `probe lsp logs --follow` for real-time debugging

### Git & Version Control

**Git Operations:**
- ALWAYS use `git2` crate instead of shell commands when requested
- Handle git workspaces and modified file detection properly
- Use commit hash + timestamp for git-aware versioning

**Commit Process:**
- Run `cargo fmt` to fix formatting
- Run `cargo clippy --fix` to fix linting issues
- Run `cargo check` to verify compilation
- Run `make test` for full test suite
- Commit with 10-minute timeout for operations

## Architecture Guidelines

### Agent Usage Patterns

**When to use @agent-architect:**
- Complex multi-file refactoring (>5 files)
- Database migrations or backend changes
- System architecture modifications
- Any task requiring systematic analysis across modules

**Agent Session Structure:**
- Break complex work into separate @agent-architect sessions per phase
- Provide comprehensive detailed instructions including file paths
- Define specific success criteria and scope for each session

**Agent Usage Guidelines:**
- Provide detailed architectural context and constraints
- Specify file locations and success criteria
- Define clear scope boundaries for complex changes

**Why detailed instructions matter:**
- Prevents architectural decisions that conflict with existing patterns
- Ensures proper database backend selection (local vs cloud)
- Avoids stub implementations that bypass actual functionality
- Provides clear scope boundaries for complex multi-file changes

### Error Prevention Patterns

**Database Deadlocks:**
- Use transactional DDL with `IF NOT EXISTS` clauses
- Implement process-local guards with path-based keys  
- Add file locking for cross-process safety
- Use connection customizers for per-connection settings

**Cache Inconsistencies:**
- Ensure storage and retrieval use identical serialization (bincode vs JSON)
- Verify workspace ID generation uses same algorithm everywhere
- Check field ordering in JSON parameters for cache keys
- Test persistence across daemon restarts early

**LSP Timeouts:**  
- Use `spawn_blocking` for database operations in async contexts
- Check for blocking I/O operations in async handlers
- Implement proper timeout handling for language server communication

### Performance Optimization

**Build Performance:**
- Avoid bundled compilation features in development builds
- Use conditional features for dev vs release builds  
- Profile CI build times when adding native dependencies

**Cache Performance:**
- Implement LRU eviction for memory management
- Use prefix-based clearing for content-addressed caches
- Monitor hit rates (should achieve 90-95% for workspace caches)
- Measure performance improvements (expect 10-100x speedup)

## LSP Client Implementation

For detailed information on implementing an LSP client that communicates with the probe daemon, see:
**[docs/LSP_CLIENT_GUIDE.md](docs/LSP_CLIENT_GUIDE.md)**

This guide includes:
- Complete client implementation examples (Python, Rust, TypeScript)
- Wire protocol specification
- Request/response types
- Socket path discovery
- Connection management best practices
- Debugging tips and common issues# Trigger CI re-run
# Triggering CI re-run
# Test change for consent mechanism
# Another test change for consent mechanism
# Test change for consent mechanism
- Always run Bash command with 10 minute timeout
