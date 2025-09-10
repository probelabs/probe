# Agent Client Protocol (ACP) Implementation

This directory contains the implementation of the Agent Client Protocol (ACP) for the Probe AI agent. ACP is a standardized protocol for communication between code editors and AI coding agents, developed by Zed Industries.

## Overview

The ACP implementation enables Probe to work as an AI coding agent compatible with ACP-enabled editors like Zed and Neovim. It provides a JSON-RPC 2.0 based communication protocol with rich features including:

- Session management for conversation contexts
- Tool execution with lifecycle tracking  
- Streaming responses and notifications
- Permission system for code modifications
- Rich content types (text, images, resources)

## Architecture

### Core Components

- **`types.js`** - Protocol constants, types, and utility functions
- **`connection.js`** - JSON-RPC 2.0 communication over stdio
- **`server.js`** - Main ACP server implementation with session management
- **`tools.js`** - Tool integration and execution lifecycle management
- **`index.js`** - Module exports

### Protocol Flow

1. **Initialization**: Client negotiates protocol version and capabilities
2. **Session Management**: Create/load conversation contexts  
3. **Tool Execution**: Execute code search/analysis tools with progress tracking
4. **AI Interaction**: Process user prompts using ProbeAgent
5. **Cleanup**: Handle disconnection and resource cleanup

## Usage

### Starting ACP Server

```bash
# Start as ACP server
probe agent --acp

# With custom configuration
probe agent --acp --provider anthropic --path ./src --allow-edit
```

### Environment Variables

```bash
ANTHROPIC_API_KEY=your_key_here
OPENAI_API_KEY=your_key_here  
GOOGLE_API_KEY=your_key_here
DEBUG=1  # Enable debug logging
```

## Protocol Messages

### Initialize Request

```json
{
  "jsonrpc": "2.0",
  "method": "initialize",
  "params": {
    "protocolVersion": "1"
  },
  "id": 1
}
```

### Initialize Response

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocolVersion": "1",
    "serverInfo": {
      "name": "probe-agent-acp",
      "version": "1.0.0",
      "description": "Probe AI agent with code search capabilities"
    },
    "capabilities": {
      "tools": [
        {
          "name": "search",
          "description": "Search for code patterns and content",
          "kind": "search"
        }
      ],
      "sessionManagement": true,
      "streaming": true,
      "permissions": false
    }
  }
}
```

### New Session Request

```json
{
  "jsonrpc": "2.0", 
  "method": "newSession",
  "params": {
    "sessionId": "optional-custom-id",
    "mode": "normal"
  },
  "id": 2
}
```

### Prompt Request

```json
{
  "jsonrpc": "2.0",
  "method": "prompt", 
  "params": {
    "sessionId": "session-123",
    "message": "How does authentication work in this codebase?"
  },
  "id": 3
}
```

### Prompt Response

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "Authentication in this codebase uses JWT tokens..."
      }
    ],
    "sessionId": "session-123",
    "timestamp": "2025-01-15T10:30:00Z"
  }
}
```

## Tool Integration

### Available Tools

1. **Search Tool** (`search`)
   - Flexible text search with stemming
   - Supports regex patterns and elastic search syntax
   - Parameters: `query`, `path`, `max_results`, `allow_tests`

2. **Query Tool** (`query`) 
   - AST-based structural pattern matching
   - Language-aware code structure search
   - Parameters: `pattern`, `path`, `language`, `max_results`

3. **Extract Tool** (`extract`)
   - Extract specific code blocks from files
   - Line-based extraction with context
   - Parameters: `files`, `context_lines`, `allow_tests`, `format`

### Tool Call Lifecycle

Tools execute with full lifecycle tracking:

1. **Pending** - Tool call queued for execution
2. **In Progress** - Tool is actively executing  
3. **Completed** - Tool finished successfully with results
4. **Failed** - Tool execution failed with error

Progress is communicated via notifications:

```json
{
  "jsonrpc": "2.0",
  "method": "toolCallProgress",
  "params": {
    "sessionId": "session-123",
    "toolCallId": "tool-456", 
    "status": "completed",
    "result": "search results here..."
  }
}
```

## Session Management

### Session Types

- **Normal Mode**: Standard AI interaction
- **Planning Mode**: Strategic planning and design discussions

### Session Features

- Persistent conversation history
- Tool call tracking and status
- Session metadata (creation time, last update)
- Resource cleanup on disconnect

### Session Operations

```javascript
// Create new session
await server.handleNewSession({
  sessionId: 'custom-id',  // optional
  mode: 'normal'           // optional
});

// Load existing session  
await server.handleLoadSession({
  sessionId: 'existing-id'
});

// Set session mode
await server.handleSetSessionMode({
  sessionId: 'session-id',
  mode: 'planning'
});
```

## Error Handling

The implementation follows JSON-RPC 2.0 error handling:

### Standard Error Codes

- `-32700` Parse Error - Invalid JSON
- `-32600` Invalid Request - Invalid JSON-RPC format  
- `-32601` Method Not Found - Unknown method
- `-32602` Invalid Params - Invalid parameters
- `-32603` Internal Error - Server error

### Custom Error Codes

- `-32001` Unsupported Protocol Version
- `-32002` Session Not Found
- `-32003` Permission Denied  
- `-32004` Tool Execution Failed

### Error Response Example

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32602,
    "message": "Invalid params: sessionId required",
    "data": {
      "received": {},
      "expected": "object with sessionId field"
    }
  }
}
```

## Testing

The implementation includes comprehensive tests:

- **Unit Tests** - Individual component testing
- **Integration Tests** - Full protocol flow testing  
- **Error Handling Tests** - Edge cases and error conditions
- **Mock Testing** - Isolated testing with mocked dependencies

Run tests:

```bash
npm test npm/src/agent/acp/
```

## Development

### Adding New Methods

1. Add method constant to `types.js`
2. Implement handler in `server.js`
3. Add to request routing in `handleRequest()`
4. Write tests for the new method

### Adding New Tools

1. Implement tool in ProbeAgent's tool system
2. Add tool definition to `ACPToolManager.getToolDefinitions()`
3. Add execution case in `executeProbeTool()` 
4. Update capabilities in server

### Debugging

Enable debug mode for detailed logging:

```bash
DEBUG=1 probe agent --acp
```

Debug logs include:
- Message send/receive 
- Tool execution progress
- Session lifecycle events
- Error details and stack traces

## Compatibility

### Protocol Versions

- **ACP v1** - Full support for core protocol features
- Backwards compatible with future minor versions
- Forward compatible design for protocol evolution

### Editor Integration

- **Zed** - Native ACP support
- **Neovim** - Via ACP plugins (CodeCompanion, Avante.nvim)  
- **VS Code** - Potential future support
- **Other Editors** - Any editor implementing ACP client

### AI Model Support

- **Anthropic Claude** - Full support (Haiku, Sonnet, Opus)
- **OpenAI GPT** - Full support (GPT-4, GPT-4 Turbo)
- **Google Gemini** - Full support (Pro, Ultra)
- Model selection via `--provider` and `--model` flags

## Migration from MCP

For users migrating from MCP to ACP:

### Similarities
- Both use JSON-RPC for communication
- Similar tool execution concepts  
- Maintain conversation context

### Key Differences  
- ACP has richer session management
- Tool lifecycle tracking with status updates
- More granular permission system
- Enhanced content types and metadata

### Migration Steps
1. Update client to use ACP protocol
2. Change startup flag from `--mcp` to `--acp`
3. Adapt to new message formats (mostly compatible)
4. Leverage enhanced session and tool features

The ACP implementation maintains feature parity with MCP while adding enhanced capabilities for modern AI-assisted coding workflows.