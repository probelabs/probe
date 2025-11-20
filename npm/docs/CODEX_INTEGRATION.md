# OpenAI Codex CLI Integration Guide

Complete guide for using ProbeAgent with OpenAI's Codex CLI command for zero-configuration AI-powered code assistance.

## Table of Contents
- [Overview](#overview)
- [Quick Start](#quick-start)
- [How It Works](#how-it-works)
- [Auto-Fallback Feature](#auto-fallback-feature)
- [Tool Event Extraction](#tool-event-extraction)
- [Configuration](#configuration)
- [Examples](#examples)
- [Testing](#testing)
- [Troubleshooting](#troubleshooting)

## Overview

ProbeAgent now supports OpenAI's Codex CLI `codex` command as a provider, enabling:
- **Zero-configuration usage** in Codex environments
- **Automatic fallback** when no API keys are present
- **Black-box operation** - Codex CLI handles its own agentic loop
- **Tool event extraction** - Visibility into internal tool usage
- **Full MCP integration** - Access to Probe's semantic search tools

## Quick Start

### Automatic (Zero Config)

```javascript
import { ProbeAgent } from 'probe-agent';

// Works automatically if codex command is installed!
const agent = new ProbeAgent({
  allowedFolders: ['/path/to/your/code']
});

await agent.initialize();
const response = await agent.answer('Explain how this codebase works');
```

### Explicit Provider

```javascript
const agent = new ProbeAgent({
  provider: 'codex',  // Explicit
  allowedFolders: ['/path/to/your/code']
});
```

### Environment Variable

```bash
USE_CODEX=true node your-script.js
```

## How It Works

### Architecture

```
ProbeAgent
    ↓
provider: 'codex'
    ↓
Enhanced Codex Engine
    ↓
Spawns: codex --output-format json --mcp-config ...
    ↓
Codex CLI (black box)
  - Handles its own agentic loop
  - Uses MCP tools (mcp__probe__*)
  - Returns final response
    ↓
Tool Event Extraction
    ↓
Response + Tool Events
```

### Black Box Mode

Unlike the native engine which controls tool iteration:
- **Codex CLI manages its own loop** - ProbeAgent doesn't see intermediate steps
- **No XML formatting** - Uses native MCP protocol
- **Tool events extracted post-hoc** - Emitted as batch after response
- **Bypass tool loop** - ProbeAgent's iteration logic is skipped

### Key Components

1. **Enhanced Codex Engine** (`src/agent/engines/enhanced-codex.js`)
   - Spawns `codex` command with MCP configuration
   - Manages session persistence
   - Extracts tool events from response stream

2. **Built-in MCP Server** (`src/agent/mcp/built-in-server.js`)
   - Provides Probe tools via MCP protocol
   - Tools: search, extract, query, list_files, search_files
   - Automatically configured and started

3. **Auto-Detection Logic** (`src/agent/ProbeAgent.js`)
   - Checks for `codex` command availability
   - Falls back when no API keys present

## Auto-Fallback Feature

### Fallback Priority

When ProbeAgent is initialized without API keys:

1. **Check for `claude` command** → Use `claude-code` provider
2. **Check for `codex` command** → Use `codex` provider
3. **No CLI commands found** → Error with installation instructions

### How Auto-Fallback Works

```javascript
// In ProbeAgent.initialize()
async initialize() {
  // No API keys configured
  if (this.apiType === 'uninitialized') {
    const claudeAvailable = await this.isClaudeCommandAvailable();
    const codexAvailable = await this.isCodexCommandAvailable();

    if (claudeAvailable) {
      // Use claude-code (priority 1)
      this.clientApiProvider = 'claude-code';
    } else if (codexAvailable) {
      // Use codex (priority 2)
      this.clientApiProvider = 'codex';
    } else {
      // Error - neither available
      throw new Error('...');
    }
  }
}
```

### Force Codex CLI

Even if Claude Code is available, you can force Codex CLI:

```javascript
const agent = new ProbeAgent({
  provider: 'codex',  // Force Codex even if claude available
  allowedFolders: [process.cwd()]
});
```

Or via environment:

```bash
USE_CODEX=true node script.js
```

## Tool Event Extraction

### Why Extract Tool Events?

Codex CLI operates as a black box - ProbeAgent doesn't control tool execution. But users still want visibility into:
- Which tools were used
- What arguments were passed
- When tools were called

### How Extraction Works

The engine parses JSON output from Codex CLI:

```javascript
// Codex CLI emits JSON like:
{
  "type": "assistant",
  "message": {
    "content": [
      { "type": "text", "text": "..." },
      {
        "type": "tool_use",
        "id": "toolu_123",
        "name": "mcp__probe__search",
        "input": { "query": "ProbeAgent" }
      }
    ]
  }
}
```

Events are collected and emitted as a batch:

```javascript
agent.events.on('toolCall', (event) => {
  console.log(`Tool: ${event.name}`);
  console.log(`Status: ${event.status}`); // 'started' or 'completed'
  console.log(`Args:`, event.args);
  console.log(`Timestamp:`, event.timestamp);
});

agent.events.on('toolBatch', (batch) => {
  console.log(`Total tools used: ${batch.tools.length}`);
  console.log(`Batch timestamp: ${batch.timestamp}`);
});
```

## Configuration

### Model Selection

Default model is `gpt-4o`. Override with:

```javascript
const agent = new ProbeAgent({
  provider: 'codex',
  model: 'gpt-4o-mini',  // Use faster model
  allowedFolders: [process.cwd()]
});
```

Or via environment:

```bash
MODEL_NAME=gpt-4o-mini USE_CODEX=true node script.js
```

### Tool Filtering

Restrict which tools Codex CLI can access:

```javascript
const agent = new ProbeAgent({
  provider: 'codex',
  allowedTools: ['search', 'extract'],  // Only these tools
  allowedFolders: [process.cwd()]
});
```

Tools are automatically prefixed for MCP: `search` → `mcp__probe__search`

### Session Persistence

Sessions are automatically managed:

```javascript
const agent1 = new ProbeAgent({
  provider: 'codex',
  sessionId: 'my-session',
  allowedFolders: [process.cwd()]
});

await agent1.answer('What is ProbeAgent?');

// Later - resume same conversation
const agent2 = new ProbeAgent({
  provider: 'codex',
  sessionId: 'my-session',  // Same session ID
  allowedFolders: [process.cwd()]
});

await agent2.answer('Tell me more');  // Continues context
```

### Debug Mode

Enable verbose logging:

```javascript
const agent = new ProbeAgent({
  provider: 'codex',
  debug: true,
  allowedFolders: [process.cwd()]
});
```

Shows:
- Codex command execution
- Tool events as they're extracted
- Session management details
- MCP server lifecycle

## Examples

### Example 1: Zero-Config Code Exploration

```javascript
import { ProbeAgent } from 'probe-agent';

// Auto-detects codex command
const agent = new ProbeAgent({
  allowedFolders: ['/path/to/repo']
});

await agent.initialize();

// Codex will automatically use search/extract tools
const answer = await agent.answer('How does authentication work in this app?');
console.log(answer);
```

### Example 2: Monitor Tool Usage

```javascript
const agent = new ProbeAgent({
  provider: 'codex',
  allowedFolders: [process.cwd()],
  debug: true
});

await agent.initialize();

// Track all tools used
const toolsUsed = [];
agent.events.on('toolCall', (event) => {
  toolsUsed.push(event.name);
});

await agent.answer('Find all API endpoints in this codebase');

console.log('Tools used:', [...new Set(toolsUsed)]);
```

### Example 3: Custom Persona with Codex

```javascript
const agent = new ProbeAgent({
  provider: 'codex',
  predefinedPrompt: 'architect',  // Use architect persona
  allowedFolders: [process.cwd()]
});

await agent.initialize();

const analysis = await agent.answer('Analyze the architecture of this system');
```

### Example 4: Multi-Step Query with Session

```javascript
const sessionId = `codex-${Date.now()}`;

const agent = new ProbeAgent({
  provider: 'codex',
  sessionId,
  allowedFolders: [process.cwd()]
});

await agent.initialize();

// Step 1
await agent.answer('What components does this app have?');

// Step 2 - builds on previous context
await agent.answer('Which component handles user authentication?');

// Step 3 - continues same conversation
await agent.answer('Show me the authentication implementation');

// Clean up
if (agent.engine?.close) {
  await agent.engine.close();
}
```

## Testing

### Run Integration Tests

```bash
# Auto-fallback test
node npm/tests/integration/codex-auto-fallback.spec.js

# Tool event extraction test
node npm/tests/integration/codex-tool-events.spec.js
```

### Test Requirements

- Codex CLI must be installed and in PATH
- Tests temporarily remove API keys to trigger auto-fallback
- Tests verify tool event extraction works correctly

### What Tests Verify

1. **Auto-Fallback Test**
   - Codex CLI is detected when no API keys present
   - Provider switches to `codex`
   - Basic queries work end-to-end

2. **Tool Events Test**
   - Tool usage is captured from Codex output
   - Events have correct format: `{ name, args, status, timestamp, id }`
   - Batch emission works correctly

## Troubleshooting

### Codex Command Not Found

**Error:**
```
No API key provided and neither claude nor codex command found
```

**Solution:**
1. Install OpenAI Codex CLI from https://openai.com/codex
2. Verify installation: `codex --version`
3. Ensure `codex` is in your PATH

### Wrong Provider Selected

**Issue:** Claude Code is used instead of Codex CLI

**Reason:** Claude has higher priority in auto-fallback

**Solution:** Force Codex explicitly:
```javascript
const agent = new ProbeAgent({
  provider: 'codex',  // Force Codex
  allowedFolders: [process.cwd()]
});
```

### MCP Server Fails to Start

**Error:**
```
Failed to start built-in MCP server
```

**Solutions:**
1. Check port availability (uses ephemeral ports)
2. Verify Node.js version (requires Node 18+)
3. Enable debug mode to see detailed error:
   ```javascript
   const agent = new ProbeAgent({
     provider: 'codex',
     debug: true,
     allowedFolders: [process.cwd()]
   });
   ```

### Tool Events Not Emitted

**Issue:** `toolCall` events not firing

**Possible causes:**
1. Query didn't require tools
2. Codex CLI used internal reasoning only
3. Tool filtering blocked all tools

**Debug:**
```javascript
agent.events.on('toolBatch', (batch) => {
  console.log('Tools in batch:', batch.tools.length);
  console.log('Tools:', batch.tools.map(t => t.name));
});
```

### Session Resume Not Working

**Issue:** Context lost between queries

**Solutions:**
1. Use same `sessionId` for both agent instances
2. Don't close engine between queries:
   ```javascript
   await agent.answer('First query');
   await agent.answer('Second query');  // Same session
   // Only close at the very end
   await agent.engine.close();
   ```

### Performance Issues

**Issue:** Codex CLI is slow

**Solutions:**
1. Use faster model: `model: 'gpt-4o-mini'`
2. Limit tool access: `allowedTools: ['search']`
3. Reduce workspace size: `allowedFolders: ['/specific/path']`

## Comparison: Codex CLI vs Other Providers

| Feature | Codex CLI | Claude Code | Vercel AI SDK |
|---------|-----------|-------------|---------------|
| **API Key Required** | No | No | Yes |
| **Zero-Config** | ✅ | ✅ | ❌ |
| **Auto-Fallback** | ✅ (priority 2) | ✅ (priority 1) | ❌ |
| **Tool Control** | Black box | Black box | Full control |
| **Tool Events** | Post-hoc extraction | Post-hoc extraction | Real-time |
| **Session Persistence** | ✅ | ✅ | Manual |
| **MCP Integration** | Native | Native | XML parsing |
| **Best For** | OpenAI users | Claude users | Full control |

## Related Documentation

- [Claude Code Integration](./CLAUDE_CODE_INTEGRATION.md) - Similar guide for Claude
- [ProbeAgent API](../README.md) - Main ProbeAgent documentation
- [MCP Tools](./MCP_TOOLS.md) - Available MCP tools reference
- [Multi-Engine Demo](../examples/multi-engine-demo.js) - Example switching engines

## Contributing

Found a bug or have a feature request for Codex CLI integration?

1. Check existing issues: https://github.com/probelabs/probe/issues
2. Create new issue with `[codex]` prefix
3. Include debug logs: Set `debug: true` in ProbeAgent options
