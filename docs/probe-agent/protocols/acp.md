# ACP Protocol

The Agent Communication Protocol (ACP) is an advanced protocol for agent-to-agent and agent-to-tool communication. It provides session management, streaming, and structured tool execution beyond what MCP offers.

---

## Overview

ACP is designed for:

- **Multi-agent systems** with independent conversation contexts
- **Session management** with persistence and isolation
- **Tool lifecycle tracking** with detailed progress notifications
- **Bidirectional communication** over JSON-RPC 2.0

---

## When to Use ACP vs MCP

| Feature | MCP | ACP |
|---------|-----|-----|
| Tool exposure | ✓ | ✓ |
| Session management | - | ✓ |
| Conversation history | - | ✓ |
| Tool lifecycle tracking | - | ✓ |
| Streaming notifications | - | ✓ |
| Multi-session support | - | ✓ |
| Editor integration | ✓ | - |

**Use MCP when:**
- Integrating with AI editors (Cursor, Claude Code)
- Simple tool exposure is sufficient
- No session state needed

**Use ACP when:**
- Building multi-agent systems
- Need conversation persistence
- Require detailed tool execution tracking
- Building custom AI platforms

---

## Starting the ACP Server

```bash
# Start ACP server via stdio
node index.js --acp

# With options
node index.js --acp --path ./my-project --debug
```

**Server Options:**

| Option | Description |
|--------|-------------|
| `--path` | Working directory |
| `--debug` | Enable debug logging |
| `--provider` | AI provider name |
| `--model` | AI model name |
| `--allow-edit` | Enable file editing |
| `--enable-delegate` | Enable task delegation |
| `--enable-mcp` | Enable MCP integration |

---

## Protocol Specification

### Transport

- **Protocol**: JSON-RPC 2.0
- **Transport**: stdio (stdin/stdout)
- **Message Delimiter**: Newline (`\n`)
- **Timeout**: 30 seconds per request

### Message Format

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "methodName",
  "params": {}
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {}
}
```

**Notification (no response expected):**
```json
{
  "jsonrpc": "2.0",
  "method": "notificationName",
  "params": {}
}
```

**Error:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "error": {
    "code": -32600,
    "message": "Invalid Request"
  }
}
```

---

## Request Methods

### initialize

Initialize the ACP connection.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "initialize",
  "params": {
    "protocolVersion": "1"
  }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "protocolVersion": "1",
    "serverInfo": {
      "name": "probe-acp",
      "version": "1.0.0"
    },
    "capabilities": {
      "tools": [
        { "name": "search", "kind": "search" },
        { "name": "query", "kind": "query" },
        { "name": "extract", "kind": "extract" }
      ],
      "sessionManagement": true,
      "streaming": true,
      "permissions": false
    }
  }
}
```

---

### newSession

Create a new conversation session.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "newSession",
  "params": {}
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "result": {
    "sessionId": "abc123",
    "mode": "NORMAL",
    "createdAt": "2025-01-15T10:30:00Z"
  }
}
```

---

### loadSession

Load an existing session.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "loadSession",
  "params": {
    "sessionId": "abc123"
  }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "result": {
    "sessionId": "abc123",
    "mode": "NORMAL",
    "history": [
      { "role": "user", "content": "Previous message" },
      { "role": "assistant", "content": "Previous response" }
    ]
  }
}
```

---

### setSessionMode

Change session mode.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "setSessionMode",
  "params": {
    "sessionId": "abc123",
    "mode": "PLANNING"
  }
}
```

**Modes:**
- `NORMAL` - Standard interactive mode
- `PLANNING` - Planning/analysis mode

---

### prompt

Send a message and get AI response.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "prompt",
  "params": {
    "sessionId": "abc123",
    "message": "How does authentication work in this codebase?"
  }
}
```

**Response:**
```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "result": {
    "sessionId": "abc123",
    "content": [
      {
        "type": "text",
        "text": "The authentication system uses JWT tokens..."
      }
    ],
    "timestamp": "2025-01-15T10:31:00Z"
  }
}
```

---

### cancel

Cancel ongoing operations.

**Request:**
```json
{
  "jsonrpc": "2.0",
  "id": 6,
  "method": "cancel",
  "params": {
    "sessionId": "abc123"
  }
}
```

---

### File Operations

**readTextFile:**
```json
{
  "method": "readTextFile",
  "params": {
    "path": "src/auth.ts"
  }
}
```

**writeTextFile:**
```json
{
  "method": "writeTextFile",
  "params": {
    "path": "src/new-file.ts",
    "content": "export const value = 42;"
  }
}
```

---

## Notification Methods

### toolCallProgress

Sent during tool execution.

```json
{
  "jsonrpc": "2.0",
  "method": "toolCallProgress",
  "params": {
    "sessionId": "abc123",
    "toolCall": {
      "id": "tool-1",
      "name": "search",
      "kind": "search",
      "status": "in_progress",
      "startTime": 1705315860000
    }
  }
}
```

**Tool Call Status:**
- `pending` - Waiting to execute
- `in_progress` - Currently executing
- `completed` - Successfully completed
- `failed` - Execution failed

---

### messageChunk

Streaming message chunks.

```json
{
  "jsonrpc": "2.0",
  "method": "messageChunk",
  "params": {
    "sessionId": "abc123",
    "chunk": "The authentication system",
    "index": 0
  }
}
```

---

### sessionUpdated

Session state changed.

```json
{
  "jsonrpc": "2.0",
  "method": "sessionUpdated",
  "params": {
    "sessionId": "abc123",
    "mode": "PLANNING",
    "updatedAt": "2025-01-15T10:32:00Z"
  }
}
```

---

## Tool Definitions

### search

```json
{
  "name": "search",
  "description": "Search for code patterns using semantic search",
  "kind": "search",
  "inputSchema": {
    "properties": {
      "query": { "type": "string" },
      "path": { "type": "string" },
      "max_results": { "type": "number", "default": 10 },
      "allow_tests": { "type": "boolean", "default": true },
      "exact": { "type": "boolean" },
      "session": { "type": "string" },
      "nextPage": { "type": "boolean" }
    },
    "required": ["query"]
  }
}
```

### query

```json
{
  "name": "query",
  "description": "Perform structural queries using AST patterns",
  "kind": "query",
  "inputSchema": {
    "properties": {
      "pattern": { "type": "string" },
      "path": { "type": "string" },
      "language": { "type": "string" },
      "max_results": { "type": "number", "default": 10 },
      "allow_tests": { "type": "boolean", "default": true }
    },
    "required": ["pattern"]
  }
}
```

### extract

```json
{
  "name": "extract",
  "description": "Extract specific code blocks from files",
  "kind": "extract",
  "inputSchema": {
    "properties": {
      "files": { "type": "array", "items": { "type": "string" } },
      "context_lines": { "type": "number" },
      "allow_tests": { "type": "boolean", "default": true },
      "format": { "type": "string", "enum": ["plain", "markdown", "json"] }
    },
    "required": ["files"]
  }
}
```

### delegate

```json
{
  "name": "delegate",
  "description": "Delegate tasks to specialized subagents",
  "kind": "execute",
  "inputSchema": {
    "properties": {
      "task": { "type": "string" }
    },
    "required": ["task"]
  }
}
```

---

## Error Codes

| Code | Name | Description |
|------|------|-------------|
| -32700 | PARSE_ERROR | Invalid JSON |
| -32600 | INVALID_REQUEST | Invalid request structure |
| -32601 | METHOD_NOT_FOUND | Unknown method |
| -32602 | INVALID_PARAMS | Invalid parameters |
| -32603 | INTERNAL_ERROR | Internal server error |
| -32001 | UNSUPPORTED_PROTOCOL_VERSION | Protocol version mismatch |
| -32002 | SESSION_NOT_FOUND | Session doesn't exist |
| -32003 | PERMISSION_DENIED | Operation not permitted |
| -32004 | TOOL_EXECUTION_FAILED | Tool execution error |

---

## Client Implementation

### Simple Client Example

```javascript
import { spawn } from 'child_process';

class SimpleACPClient {
  constructor() {
    this.requestId = 0;
    this.pending = new Map();
  }

  async start() {
    this.server = spawn('node', ['index.js', '--acp'], {
      stdio: ['pipe', 'pipe', 'pipe']
    });

    this.server.stdout.on('data', (data) => {
      const message = JSON.parse(data.toString().trim());
      this.handleMessage(message);
    });
  }

  handleMessage(message) {
    if (message.id && this.pending.has(message.id)) {
      const { resolve, reject } = this.pending.get(message.id);
      this.pending.delete(message.id);

      if (message.error) {
        reject(new Error(message.error.message));
      } else {
        resolve(message.result);
      }
    } else if (!message.id) {
      // Notification
      this.handleNotification(message.method, message.params);
    }
  }

  handleNotification(method, params) {
    console.log(`Notification: ${method}`, params);
  }

  async sendRequest(method, params = {}) {
    const id = ++this.requestId;

    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });

      const request = JSON.stringify({
        jsonrpc: '2.0',
        id,
        method,
        params
      }) + '\n';

      this.server.stdin.write(request);
    });
  }

  async initialize() {
    return this.sendRequest('initialize', { protocolVersion: '1' });
  }

  async createSession() {
    return this.sendRequest('newSession', {});
  }

  async sendPrompt(sessionId, message) {
    return this.sendRequest('prompt', { sessionId, message });
  }

  close() {
    this.server.stdin.end();
  }
}

// Usage
const client = new SimpleACPClient();
await client.start();

const { capabilities } = await client.initialize();
console.log('Capabilities:', capabilities);

const { sessionId } = await client.createSession();
console.log('Session:', sessionId);

const response = await client.sendPrompt(sessionId, 'How does auth work?');
console.log('Response:', response.content);

client.close();
```

---

## Protocol Flow

```
Client                          Server
  |                                |
  |------ initialize ------------->|
  |<----- capabilities ------------|
  |                                |
  |------ newSession ------------->|
  |<----- sessionId ---------------|
  |                                |
  |------ prompt ----------------->|
  |<----- toolCallProgress --------|  (notification)
  |<----- toolCallProgress --------|  (notification)
  |<----- messageChunk ------------|  (notification)
  |<----- response ----------------|
  |                                |
  |------ cancel ----------------->|
  |<----- acknowledged ------------|
  |                                |
  |------ close connection ------->|
```

---

## Debugging

Enable debug logging:

```bash
DEBUG=1 node index.js --acp
```

**Debug Output:**
```
[ACP] Server started
[ACP] Received: initialize
[ACP] Session created: abc123
[ACP] Tool call: search (in_progress)
[ACP] Tool call: search (completed, 245ms)
[ACP] Response sent
```

---

## Related Documentation

- [MCP Protocol](./mcp.md) - Model Context Protocol
- [Delegation](../advanced/delegation.md) - Task delegation
- [SDK API Reference](../sdk/api-reference.md) - ProbeAgent API
- [Agent Overview](../overview.md) - Agent architecture
