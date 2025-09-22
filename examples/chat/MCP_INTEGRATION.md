# MCP (Model Context Protocol) Integration Guide

## Overview

The Probe Chat application now supports the Model Context Protocol (MCP), enabling it to connect to external tool servers and extend its capabilities beyond the built-in code search tools. This integration maintains backward compatibility with the existing XML-based tool syntax while adding support for MCP tools that use JSON parameters.

## Key Features

- ✅ **Multiple Transport Support**: stdio, WebSocket, SSE, and HTTP
- ✅ **Claude-Compatible Configuration**: Uses similar configuration format to Claude's MCP setup
- ✅ **Hybrid XML/JSON Syntax**: Native tools use XML parameters, MCP tools use JSON in `<params>` tags
- ✅ **Vercel AI SDK v5 Compatible**: Full support for the latest AI SDK version
- ✅ **Automatic Tool Discovery**: Dynamically discovers and registers tools from MCP servers
- ✅ **Seamless Integration**: MCP tools appear alongside native tools in the system prompt

## Quick Start

### 1. Enable MCP in ProbeChat

Set the environment variable or pass the option:

```bash
# Via environment variable
export ENABLE_MCP=1
npm start

# Or via command line
probe-chat --enable-mcp
```

### 2. Configure MCP Servers

Create a `.mcp/config.json` file in your project or home directory:

```json
{
  "mcpServers": {
    "probe": {
      "command": "npx",
      "args": ["-y", "@probelabs/probe@latest", "mcp"],
      "transport": "stdio",
      "enabled": true
    }
  }
}
```

## Configuration

### Configuration File Locations

The system looks for MCP configuration in these locations (in order):

1. Environment variable: `MCP_CONFIG_PATH`
2. Project directory: `./.mcp/config.json`
3. Project directory: `./mcp.config.json`
4. Home directory: `~/.config/probe/mcp.json`
5. Home directory: `~/.mcp/config.json`
6. Claude config: `~/Library/Application Support/Claude/mcp_config.json` (macOS)

### Configuration Format

```json
{
  "mcpServers": {
    "server-name": {
      "command": "command-to-run",
      "args": ["arg1", "arg2"],
      "transport": "stdio|websocket|sse|http",
      "enabled": true|false,
      "description": "Optional description",
      "env": {
        "ENV_VAR": "value"
      }
    }
  },
  "settings": {
    "timeout": 30000,
    "retryCount": 3,
    "debug": false
  }
}
```

### Environment Variables

You can also configure MCP servers via environment variables:

```bash
# Basic server configuration
export MCP_SERVERS_PROBE_COMMAND="npx"
export MCP_SERVERS_PROBE_ARGS="-y,@probelabs/probe@latest,mcp"
export MCP_SERVERS_PROBE_TRANSPORT="stdio"
export MCP_SERVERS_PROBE_ENABLED="true"

# WebSocket server
export MCP_SERVERS_CUSTOM_URL="ws://localhost:8080"
export MCP_SERVERS_CUSTOM_TRANSPORT="websocket"
export MCP_SERVERS_CUSTOM_ENABLED="true"
```

## Tool Syntax

### Native Tools (XML Parameters)

Native Probe tools continue to use XML parameter format:

```xml
<search>
  <query>authentication</query>
  <path>./src</path>
  <exact>true</exact>
</search>

<query>
  <pattern>class $NAME extends Component</pattern>
  <language>javascript</language>
</query>

<extract>
  <targets>file.js:10-20</targets>
  <format>markdown</format>
</extract>
```

### MCP Tools (JSON Parameters)

MCP tools use JSON within a `<params>` tag:

```xml
<probe_search_code>
<params>
{
  "query": "authentication",
  "path": "/absolute/path/to/project",
  "max_results": 10,
  "session": "session-id"
}
</params>
</probe_search_code>

<filesystem_read>
<params>
{
  "path": "/etc/hosts",
  "encoding": "utf-8"
}
</params>
</filesystem_read>
```

## Available MCP Servers

### Probe MCP Server

The official Probe MCP server provides code search capabilities:

```json
{
  "probe": {
    "command": "npx",
    "args": ["-y", "@probelabs/probe@latest", "mcp"],
    "transport": "stdio",
    "enabled": true
  }
}
```

**Tools provided:**
- `probe_search_code` - Elasticsearch-style code search
- `probe_query_code` - AST-based pattern matching
- `probe_extract_code` - Extract code blocks by file/line

### Filesystem Server

Access local filesystem:

```json
{
  "filesystem": {
    "command": "npx",
    "args": ["-y", "@modelcontextprotocol/server-filesystem", "/path/to/allow"],
    "transport": "stdio",
    "enabled": true
  }
}
```

### GitHub Server

Interact with GitHub API:

```json
{
  "github": {
    "command": "npx",
    "args": ["-y", "@modelcontextprotocol/server-github"],
    "transport": "stdio",
    "enabled": true,
    "env": {
      "GITHUB_TOKEN": "your-token-here"
    }
  }
}
```

### PostgreSQL Server

Database operations:

```json
{
  "postgres": {
    "command": "npx",
    "args": ["-y", "@modelcontextprotocol/server-postgres"],
    "transport": "stdio",
    "enabled": true,
    "env": {
      "DATABASE_URL": "postgresql://user:pass@localhost/db"
    }
  }
}
```

## Programmatic Usage

### Using ProbeChat with MCP

```javascript
import { ProbeChat } from './probeChat.js';

// Initialize with MCP enabled
const chat = new ProbeChat({
  enableMcp: true,
  mcpServers: {
    mcpServers: {
      'probe': {
        command: 'npx',
        args: ['-y', '@probelabs/probe@latest', 'mcp'],
        transport: 'stdio',
        enabled: true
      }
    }
  }
});

// Use chat normally - MCP tools are automatically available
const response = await chat.chat('Search for authentication code');

// Cleanup when done
await chat.cleanup();
```

### Using MCP Client Manager Directly

```javascript
import { MCPClientManager } from './mcpClientV2.js';

// Create manager
const manager = new MCPClientManager({ debug: true });

// Initialize with configuration
await manager.initialize({
  mcpServers: {
    'probe': {
      command: 'npx',
      args: ['-y', '@probelabs/probe@latest', 'mcp'],
      transport: 'stdio',
      enabled: true
    }
  }
});

// Call a tool
const result = await manager.callTool('probe_search_code', {
  query: 'function',
  path: '/path/to/project'
});

// Get tools for Vercel AI SDK
const tools = manager.getVercelTools();

// Cleanup
await manager.disconnect();
```

### Integration with Vercel AI SDK v5

```javascript
import { generateText, tool } from 'ai';
import { z } from 'zod';
import { MCPClientManager } from './mcpClientV2.js';

// Initialize MCP
const mcpManager = new MCPClientManager();
await mcpManager.initialize();

// Get tools in Vercel format
const mcpTools = mcpManager.getVercelTools();

// Wrap tools for AI SDK v5
const aiTools = {};
for (const [name, mcpTool] of Object.entries(mcpTools)) {
  aiTools[name] = tool({
    description: mcpTool.description,
    inputSchema: convertToZodSchema(mcpTool.inputSchema),
    execute: mcpTool.execute
  });
}

// Use with AI
const result = await generateText({
  model: yourModel,
  messages: [...],
  tools: aiTools
});
```

## Testing

### Test MCP Server Connection

```bash
node test-mcp-probe-server.js
```

### Test Full Integration

```bash
node test-full-mcp-integration.js
```

### Test with AI Model

```bash
# Requires API key
export ANTHROPIC_API_KEY="your-key"
node test-mcp-with-ai.js
```

## Troubleshooting

### MCP Server Not Connecting

1. Check that the command exists and is executable
2. For npx commands, ensure npm is installed
3. Check server logs with `DEBUG_MCP=1`

### Tools Not Appearing

1. Ensure server is enabled in configuration
2. Check that MCP is enabled (`ENABLE_MCP=1`)
3. Verify server provides tools with `listTools` method

### JSON Parsing Errors

1. Ensure JSON in `<params>` tag is valid
2. Use proper escaping for special characters
3. Check quotes are properly balanced

## Architecture

### Components

1. **MCPClientManager** - Manages connections to multiple MCP servers
2. **MCPXmlBridge** - Bridges XML syntax with MCP JSON tools
3. **ProbeChat** - Main chat interface with MCP support
4. **Transport Layers** - stdio, WebSocket, SSE, HTTP support

### Flow

1. ProbeChat initializes with MCP enabled
2. MCPClientManager connects to configured servers
3. Tools are discovered and registered
4. MCPXmlBridge converts tools to XML definitions
5. System prompt includes both native and MCP tools
6. AI generates tool calls in appropriate format
7. Parser distinguishes native (XML) vs MCP (JSON) tools
8. Tools are executed and results returned

## Migration from v4 to v5

The integration includes full support for Vercel AI SDK v5:

1. **Tool definitions**: Changed from `parameters` to `inputSchema`
2. **Message types**: Support for new `UIMessage` and `ModelMessage` types
3. **MCP support**: Native integration with `experimental_createMCPClient`

## Future Enhancements

- [ ] Support for MCP resources and prompts
- [ ] Tool result caching and optimization
- [ ] Dynamic tool loading/unloading
- [ ] MCP server health monitoring
- [ ] Tool usage analytics
- [ ] Custom MCP server development kit

## License

This MCP integration is part of the Probe Chat application and follows the same license terms.