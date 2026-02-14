# MCP Protocol

The Model Context Protocol (MCP) enables Probe to integrate with AI editors and external tools. Probe supports MCP both as a **server** (exposing Probe's search capabilities) and as a **client** (consuming tools from other MCP servers).

---

## TL;DR

```bash
# Run Probe as MCP server
probe mcp

# Use in Claude Code (claude_desktop_config.json)
{
  "mcpServers": {
    "probe": {
      "command": "probe",
      "args": ["mcp"]
    }
  }
}
```

---

## MCP Server Mode

### Starting the Server

```bash
# Basic MCP server
probe mcp

# With specific directory
probe mcp --path ./my-project

# Via npx
npx -y @probelabs/probe@latest mcp
```

### Tools Exposed

When running as an MCP server, Probe exposes these tools:

| Tool | Description |
|------|-------------|
| `search` | Semantic code search with Elasticsearch syntax |
| `query` | AST-based structural queries |
| `extract` | Extract code blocks from files |
| `listFiles` | List files in directory |
| `searchFiles` | Find files by name pattern |

### Tool Schemas

**search**
```json
{
  "name": "search",
  "description": "Search for code patterns using semantic search",
  "inputSchema": {
    "type": "object",
    "properties": {
      "query": { "type": "string", "description": "Search query" },
      "path": { "type": "string", "description": "Directory to search" },
      "maxResults": { "type": "integer", "default": 10 }
    },
    "required": ["query"]
  }
}
```

**query**
```json
{
  "name": "query",
  "description": "Perform AST-based structural queries",
  "inputSchema": {
    "type": "object",
    "properties": {
      "pattern": { "type": "string", "description": "AST-grep pattern" },
      "path": { "type": "string" },
      "language": { "type": "string" }
    },
    "required": ["pattern"]
  }
}
```

**extract**
```json
{
  "name": "extract",
  "description": "Extract code blocks from files",
  "inputSchema": {
    "type": "object",
    "properties": {
      "files": {
        "type": "array",
        "items": { "type": "string" },
        "description": "File paths or file:line specs"
      }
    },
    "required": ["files"]
  }
}
```

---

## AI Editor Integration

### Claude Code / Claude Desktop

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "probe": {
      "command": "probe",
      "args": ["mcp"],
      "env": {}
    }
  }
}
```

Or using npx:

```json
{
  "mcpServers": {
    "probe": {
      "command": "npx",
      "args": ["-y", "@probelabs/probe@latest", "mcp"]
    }
  }
}
```

### Cursor

Add to Cursor settings:

```json
{
  "mcp": {
    "servers": {
      "probe": {
        "command": "probe",
        "args": ["mcp"]
      }
    }
  }
}
```

### Windsurf

Add to Windsurf MCP configuration:

```json
{
  "mcpServers": {
    "probe": {
      "command": "npx",
      "args": ["-y", "@probelabs/probe@latest", "mcp"]
    }
  }
}
```

---

## MCP Client Mode

ProbeAgent can consume tools from external MCP servers.

### Configuration

```javascript
import { ProbeAgent } from '@probelabs/probe/agent';

const agent = new ProbeAgent({
  path: './src',
  enableMcp: true,
  mcpConfig: {
    mcpServers: {
      'github': {
        command: 'npx',
        args: ['-y', '@modelcontextprotocol/server-github'],
        transport: 'stdio',
        enabled: true,
        env: {
          GITHUB_TOKEN: process.env.GITHUB_TOKEN
        }
      },
      'filesystem': {
        command: 'npx',
        args: ['-y', '@modelcontextprotocol/server-filesystem', '/path/to/allowed'],
        transport: 'stdio',
        enabled: true
      }
    }
  }
});

await agent.initialize();
// MCP tools now available with mcp__ prefix
```

### Configuration File

Create `.mcp/config.json` or `mcp.config.json`:

```json
{
  "mcpServers": {
    "github": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "transport": "stdio",
      "enabled": true,
      "timeout": 30000
    },
    "postgres": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-postgres"],
      "transport": "stdio",
      "enabled": true,
      "env": {
        "DATABASE_URL": "postgresql://..."
      }
    },
    "custom-http": {
      "url": "http://localhost:3000/mcp",
      "transport": "http",
      "enabled": true
    }
  },
  "settings": {
    "timeout": 30000,
    "debug": false
  }
}
```

### Configuration Priority

1. `MCP_CONFIG_PATH` environment variable
2. `./.mcp/config.json` (local project)
3. `./mcp.config.json` (local project)
4. `~/.config/probe/mcp.json` (user config)
5. `~/.mcp/config.json` (Claude compatible)
6. `~/Library/Application Support/Claude/mcp_config.json` (macOS)

---

## Transport Types

| Transport | Configuration | Use Case |
|-----------|---------------|----------|
| `stdio` | `command`, `args` | Local spawned processes |
| `sse` | `url` | Server-Sent Events |
| `websocket` | `url` (ws://) | WebSocket connections |
| `http` | `url` | HTTP REST endpoints |

### Examples

**stdio (Local Process):**
```json
{
  "github": {
    "command": "npx",
    "args": ["-y", "@modelcontextprotocol/server-github"],
    "transport": "stdio"
  }
}
```

**HTTP (Remote Server):**
```json
{
  "custom-api": {
    "url": "http://localhost:3000/mcp",
    "transport": "http"
  }
}
```

**WebSocket:**
```json
{
  "realtime": {
    "url": "ws://localhost:8080/mcp",
    "transport": "websocket"
  }
}
```

---

## Method Filtering

Control which MCP methods are available:

```json
{
  "mcpServers": {
    "github": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-github"],
      "transport": "stdio",
      "enabled": true,
      "allowedMethods": ["search_*", "get_*"],
      "blockedMethods": ["delete_*", "dangerous_*"]
    }
  }
}
```

**Wildcard Patterns:**
- `*` - Match all
- `prefix_*` - Match starting with prefix
- `*_suffix` - Match ending with suffix
- `prefix_*_suffix` - Match with prefix and suffix

---

## Timeout Configuration

```json
{
  "mcpServers": {
    "slow-server": {
      "command": "node",
      "args": ["server.js"],
      "transport": "stdio",
      "timeout": 60000
    }
  },
  "settings": {
    "timeout": 30000
  }
}
```

**Environment Variable:**
```bash
MCP_MAX_TIMEOUT=120000  # 2 minutes max
```

**Limits:**
- Minimum: 30 seconds
- Maximum: 2 hours (7200000ms)
- Default: 30 seconds

---

## Environment Variables

**Configuration:**
```bash
MCP_CONFIG_PATH=/path/to/config.json    # Config file path
MCP_MAX_TIMEOUT=120000                   # Max timeout (ms)
```

**Per-Server (via env vars):**
```bash
MCP_SERVERS_GITHUB_COMMAND=npx
MCP_SERVERS_GITHUB_ARGS=-y,@modelcontextprotocol/server-github
MCP_SERVERS_GITHUB_TRANSPORT=stdio
MCP_SERVERS_GITHUB_ENABLED=true
MCP_SERVERS_GITHUB_TIMEOUT=60000
MCP_SERVERS_GITHUB_ALLOWLIST=search_*,get_*
MCP_SERVERS_GITHUB_BLOCKLIST=delete_*
```

---

## Debugging

Enable debug logging:

```bash
DEBUG=1 probe mcp
# or
DEBUG_MCP=1 probe mcp
```

**Debug Output:**
```
[MCP DEBUG] Connecting to server: github
[MCP INFO] Server connected: github
[MCP DEBUG] Tool discovered: github__search_code
[MCP DEBUG] Tool call started: github__search_code
[MCP DEBUG] Tool call completed: github__search_code (245ms)
```

---

## Telemetry Events

When using with a tracer:

```javascript
const agent = new ProbeAgent({
  enableMcp: true,
  tracer: myTracer  // OpenTelemetry tracer
});
```

**Events:**
- `initialization.started` / `initialization.completed`
- `server.connecting` / `server.connected` / `server.connection_failed`
- `tools.discovered` / `tools.filtered`
- `tool.call_started` / `tool.call_completed` / `tool.call_failed`
- `disconnection.started` / `disconnection.completed`

---

## Built-in Server Implementation

The built-in MCP server runs in-process:

```javascript
import { MCPXmlBridge } from '@probelabs/probe/agent';

// The bridge handles MCP tool integration
const bridge = new MCPXmlBridge();
await bridge.initialize(config);

// Get available tools
const tools = bridge.getTools();

// Execute a tool
const result = await bridge.executeToolCall('search', { query: 'login' });
```

**Server Capabilities:**
```json
{
  "name": "probe-builtin",
  "version": "1.0.0",
  "capabilities": {
    "tools": {}
  }
}
```

---

## Common MCP Servers

| Server | Package | Purpose |
|--------|---------|---------|
| GitHub | `@modelcontextprotocol/server-github` | GitHub operations |
| Filesystem | `@modelcontextprotocol/server-filesystem` | File operations |
| PostgreSQL | `@modelcontextprotocol/server-postgres` | Database queries |
| Brave Search | `@modelcontextprotocol/server-brave-search` | Web search |
| Memory | `@modelcontextprotocol/server-memory` | Key-value storage |

---

## Related Documentation

- [MCP Server Setup](../../mcp-server.md) - Quick setup guide
- [AI Code Editors](../../use-cases/integrating-probe-into-ai-code-editors.md) - Editor integration
- [ACP Protocol](./acp.md) - Advanced agent communication
- [Tools Reference](../sdk/tools-reference.md) - All available tools
