# Environment Variables

Complete reference of environment variables for Probe CLI and Probe Agent.

---

## AI Provider Keys

| Variable | Description |
|----------|-------------|
| `ANTHROPIC_API_KEY` | Anthropic Claude API key |
| `OPENAI_API_KEY` | OpenAI GPT API key |
| `GOOGLE_GENERATIVE_AI_API_KEY` | Google Gemini API key |
| `AWS_ACCESS_KEY_ID` | AWS access key (for Bedrock) |
| `AWS_SECRET_ACCESS_KEY` | AWS secret key (for Bedrock) |
| `AWS_SESSION_TOKEN` | AWS session token (optional) |
| `AWS_REGION` | AWS region (for Bedrock) |

**Example:**
```bash
export ANTHROPIC_API_KEY=sk-ant-...
export OPENAI_API_KEY=sk-...
```

---

## Probe CLI Variables

### Debug & Logging

| Variable | Values | Description |
|----------|--------|-------------|
| `DEBUG` | `1` or unset | Enable detailed debug output with timing |
| `NO_COLOR` | Any value | Disable colored output |
| `RUST_BACKTRACE` | `1` or `full` | Show Rust stack traces on errors |

**Example:**
```bash
DEBUG=1 probe search "query" ./
NO_COLOR=1 probe search "query" ./ | less
RUST_BACKTRACE=1 probe search "query" ./
```

### Performance Tuning

| Variable | Values | Default | Description |
|----------|--------|---------|-------------|
| `PROBE_PARSER_POOL_SIZE` | Number | max(CPU cores, 4) | Tree-sitter parser thread pool size |
| `PROBE_TREE_CACHE_SIZE` | Number | Auto | Parse tree cache size |
| `PROBE_NO_PARSER_WARMUP` | `1` or unset | unset | Skip parser warmup phase |
| `PROBE_OPTIMIZE_BLOCKS` | `1` or unset | unset | Enable block optimization |

**Example:**
```bash
# Use 8 parser threads
PROBE_PARSER_POOL_SIZE=8 probe search "query" ./

# Skip warmup for faster cold start
PROBE_NO_PARSER_WARMUP=1 probe search "query" ./
```

### SIMD Control

| Variable | Values | Default | Description |
|----------|--------|---------|-------------|
| `DISABLE_SIMD_TOKENIZATION` | `1` or unset | unset | Disable SIMD tokenization |
| `DISABLE_SIMD_RANKING` | `1` or unset | unset | Disable SIMD ranking |
| `DISABLE_SIMD_PATTERN_MATCHING` | `1` or unset | unset | Disable SIMD patterns |

**Example:**
```bash
# Disable all SIMD (for debugging)
DISABLE_SIMD_RANKING=1 probe search "query" ./
```

### Behavior

| Variable | Values | Default | Description |
|----------|--------|---------|-------------|
| `PROBE_NO_GITIGNORE` | `1` or unset | unset | Ignore .gitignore files globally |
| `PROBE_SESSION_ID` | String | - | Default session ID for caching |

**Example:**
```bash
# Search all files including ignored
PROBE_NO_GITIGNORE=1 probe search "query" ./
```

---

## MCP Variables

### Configuration

| Variable | Values | Default | Description |
|----------|--------|---------|-------------|
| `MCP_CONFIG_PATH` | File path | - | Path to MCP config file |
| `MCP_MAX_TIMEOUT` | Milliseconds | 1800000 | Maximum timeout (30s min, 2h max) |

**Example:**
```bash
export MCP_CONFIG_PATH=/path/to/mcp.config.json
export MCP_MAX_TIMEOUT=120000  # 2 minutes
```

### Per-Server Configuration

Environment variables can configure individual MCP servers:

```bash
# Pattern: MCP_SERVERS_<NAME>_<PROPERTY>
MCP_SERVERS_GITHUB_COMMAND=npx
MCP_SERVERS_GITHUB_ARGS=-y,@modelcontextprotocol/server-github
MCP_SERVERS_GITHUB_TRANSPORT=stdio
MCP_SERVERS_GITHUB_ENABLED=true
MCP_SERVERS_GITHUB_TIMEOUT=60000
MCP_SERVERS_GITHUB_ALLOWLIST=search_*,get_*
MCP_SERVERS_GITHUB_BLOCKLIST=delete_*

# Use these to override config file settings
```

### Debug

| Variable | Values | Description |
|----------|--------|-------------|
| `DEBUG_MCP` | `1` or unset | Enable MCP debug logging |

**Example:**
```bash
DEBUG_MCP=1 probe mcp
```

---

## Probe Agent SDK Variables

### Timeouts

| Variable | Values | Default | Description |
|----------|--------|---------|-------------|
| `ENGINE_ACTIVITY_TIMEOUT` | Milliseconds | 180000 | Engine activity timeout (3 min) |

**Range:** 5000 (5s) to 600000 (10 min)

### Provider Selection

The SDK auto-detects providers based on these environment variables:

```bash
# Priority order for auto-detection:
# 1. Claude Code CLI (if 'claude' command exists)
# 2. Codex CLI (if 'codex' command exists)
# 3. Anthropic (if ANTHROPIC_API_KEY set)
# 4. OpenAI (if OPENAI_API_KEY set)
# 5. Google (if GOOGLE_GENERATIVE_AI_API_KEY set)
# 6. Bedrock (if AWS credentials set)
```

---

## Node.js Variables

| Variable | Values | Description |
|----------|--------|-------------|
| `NODE_OPTIONS` | Options | Node.js runtime options |
| `NODE_ENV` | `development`, `production` | Environment mode |

**Example:**
```bash
# Increase memory for large codebases
NODE_OPTIONS=--max-old-space-size=8192 npx probe-chat ./large-codebase
```

---

## Quick Reference

### Minimal Setup

```bash
# Just need one API key
export ANTHROPIC_API_KEY=sk-ant-...
probe search "query" ./
```

### Performance Optimized

```bash
export PROBE_PARSER_POOL_SIZE=8
export PROBE_NO_PARSER_WARMUP=1
probe search "query" ./
```

### Debug Mode

```bash
export DEBUG=1
export DEBUG_MCP=1
export RUST_BACKTRACE=1
probe search "query" ./
```

### MCP Server

```bash
export MCP_CONFIG_PATH=~/.config/probe/mcp.json
export MCP_MAX_TIMEOUT=120000
probe mcp
```

---

## Configuration File Locations

These locations are checked for configuration (in order):

### MCP Config
1. `$MCP_CONFIG_PATH` (if set)
2. `./.mcp/config.json`
3. `./mcp.config.json`
4. `~/.config/probe/mcp.json`
5. `~/.mcp/config.json`
6. `~/Library/Application Support/Claude/mcp_config.json` (macOS)

### Claude Desktop (for MCP integration)
- macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
- Linux: `~/.config/Claude/claude_desktop_config.json`
- Windows: `%APPDATA%\Claude\claude_desktop_config.json`

---

## Related Documentation

- [FAQ](./faq.md) - Common questions
- [Troubleshooting](./troubleshooting.md) - Problem solutions
- [Performance Tuning](../probe-cli/performance.md) - Optimization guide
- [MCP Protocol](../probe-agent/protocols/mcp.md) - MCP configuration
