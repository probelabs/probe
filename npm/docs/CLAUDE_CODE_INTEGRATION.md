# Claude Code Integration Guide

Complete guide for using ProbeAgent with Claude Code's built-in `claude` command for zero-configuration AI-powered code assistance.

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

ProbeAgent now supports Claude Code's `claude` command as a provider, enabling:
- **Zero-configuration usage** in Claude Code environments
- **Automatic fallback** when no API keys are present
- **Black-box operation** - Claude Code handles its own agentic loop
- **Tool event extraction** - Visibility into internal tool usage
- **Full MCP integration** - Access to Probe's semantic search tools

## Quick Start

### Automatic (Zero Config)

```javascript
import { ProbeAgent } from 'probe-agent';

// Works automatically if claude command is installed!
const agent = new ProbeAgent({
  allowedFolders: ['/path/to/your/code']
});

await agent.initialize();
const response = await agent.answer('Explain how this codebase works');
```

### Explicit Provider

```javascript
const agent = new ProbeAgent({
  provider: 'claude-code',  // Explicit
  allowedFolders: ['/path/to/your/code']
});
```

### Environment Variable

```bash
USE_CLAUDE_CODE=true node your-script.js
```

## How It Works

### Architecture

```
ProbeAgent
    ↓
provider: 'claude-code'
    ↓
Enhanced Claude Code Engine
    ↓
Spawns: claude --output-format json --mcp-config ...
    ↓
Claude Code (black box)
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
- **Claude Code manages its own loop** - ProbeAgent doesn't see intermediate steps
- **No XML formatting** - Uses native MCP protocol
- **Tool events extracted post-hoc** - Emitted as batch after response
- **Bypass tool loop** - ProbeAgent's iteration logic is skipped

### Key Components

1. **Enhanced Claude Code Engine** (`src/agent/engines/enhanced-claude-code.js`)
   - Spawns `claude` command with MCP configuration
   - Manages session persistence
   - Extracts tool events from response stream

2. **Built-in MCP Server** (`src/agent/mcp/built-in-server.js`)
   - Provides Probe tools via MCP protocol
   - Tools: search, extract, query, list_files, search_files
   - Automatically configured and started

3. **Auto-Detection Logic** (`src/agent/ProbeAgent.js`)
   - Checks for `claude` command availability
   - Falls back when no API keys present

## Auto-Fallback Feature

### Detection Flow

```
Constructor → No API keys found
    ↓
Mark apiType='uninitialized'
    ↓
initialize() called
    ↓
Check: is claude command available?
    ↓
Yes → provider='claude-code'
No  → Throw error with instructions
```

### Priority Order

1. **Explicit provider** - Use if `options.provider` is set
2. **API keys** - Use first available (Anthropic, OpenAI, Google, AWS)
3. **Claude command** - Auto-fallback if installed
4. **Error** - Neither API keys nor claude available

### Error Messages

**No API keys, no claude command:**
```
Error: No API key provided and claude command not found. Please either:
1. Set an API key: ANTHROPIC_API_KEY, OPENAI_API_KEY, GOOGLE_GENERATIVE_AI_API_KEY, or AWS credentials
2. Install claude command from https://docs.claude.com/en/docs/claude-code
```

**With debug enabled:**
```
[DEBUG] No API keys found - will check for claude command in initialize()
[DEBUG] No API keys found, but claude command detected
[DEBUG] Auto-switching to claude-code provider
```

## Tool Event Extraction

### How It Works

Claude Code emits `assistant` messages containing tool use information:

```javascript
{
  type: 'assistant',
  message: {
    content: [
      { type: 'text', text: 'Let me search for that...' },
      { type: 'tool_use', id: 'toolu_123', name: 'Glob', input: {...} }
    ]
  }
}
```

The engine extracts these and emits as events:

```javascript
agent.events.on('toolCall', (event) => {
  console.log(event.name);      // 'Glob'
  console.log(event.status);    // 'started'
  console.log(event.timestamp); // '2025-11-20T10:21:30.935Z'
  console.log(event.args);      // { pattern: '*.js', path: '...' }
});
```

### Event Structure

```javascript
{
  timestamp: '2025-11-20T10:21:30.935Z',
  name: 'Glob',
  args: { pattern: '*.js', path: '/path/to/code' },
  id: 'toolu_012JAiRK7bho9vevZyyUYJgu',
  status: 'started'  // or 'completed'
}
```

### Batch Emission

Unlike native engine which emits in real-time:
- Events collected during response
- Emitted as batch after completion
- Maintains compatibility with native engine event listeners

## Configuration

### Options

```javascript
const agent = new ProbeAgent({
  // Provider
  provider: 'claude-code',     // Force claude-code provider

  // Required
  allowedFolders: ['/path'],   // Allowed code directories

  // Optional
  debug: true,                 // Enable debug logging
  sessionId: 'my-session',     // Custom session ID
  customPrompt: '...',         // Custom system prompt
  allowedTools: ['mcp__*'],    // Tool filtering
});
```

### Environment Variables

```bash
# Force claude-code provider
USE_CLAUDE_CODE=true

# Enable debug logging
DEBUG=probe:*
```

## Examples

### Basic Usage

```javascript
import { ProbeAgent } from 'probe-agent';

const agent = new ProbeAgent({
  allowedFolders: [process.cwd()]
});

await agent.initialize();

const response = await agent.answer(
  'Find all async functions that handle errors'
);

console.log(response);
```

### With Tool Event Monitoring

```javascript
const agent = new ProbeAgent({
  provider: 'claude-code',
  allowedFolders: [process.cwd()],
  debug: true
});

await agent.initialize();

// Track tool usage
const toolsUsed = [];
agent.events.on('toolCall', (event) => {
  if (event.status === 'started') {
    toolsUsed.push(event.name);
  }
});

const response = await agent.answer('Analyze this codebase');

console.log('Tools used:', [...new Set(toolsUsed)]);
```

### Multi-Step Queries

```javascript
// Claude Code handles multi-step internally
const agent = new ProbeAgent({
  provider: 'claude-code',
  allowedFolders: ['/my/project']
});

await agent.initialize();

// This might trigger multiple internal tool uses
const response = await agent.answer(
  'Find the authentication logic and explain how it works'
);
// Claude Code will search, extract, and analyze automatically
```

## Testing

### Integration Tests

Located in `tests/integration/`:

1. **claude-code-auto-fallback.test.js** - Tests automatic provider detection
2. **claude-code-tool-events.test.js** - Tests tool event extraction
3. **claude-code-multi-step.test.js** - Tests complex multi-step queries

Run tests:
```bash
npm test -- tests/integration/claude-code
```

### Manual Testing

```bash
# Test auto-fallback
node tests/integration/claude-code-auto-fallback.test.js

# Test tool events
node tests/integration/claude-code-tool-events.test.js
```

## Troubleshooting

### Claude command not found

**Problem:** `Error: claude command not found`

**Solution:**
- Install Claude Code from https://docs.claude.com/en/docs/claude-code
- Verify: `claude --version`

### Auto-fallback not working

**Problem:** Not detecting claude command

**Solutions:**
- Ensure `claude` is in PATH
- Check: `which claude`
- Verify API keys are not set (auto-fallback only when no keys)

### Tool events not appearing

**Problem:** No tool events emitted

**Possible causes:**
1. Query didn't trigger tool use
2. Need to enable event listeners before query
3. Check that `agent.events` exists

**Solution:**
```javascript
// Enable events BEFORE query
agent.events.on('toolCall', (event) => {
  console.log('Tool event:', event);
});

await agent.answer('query here');
```

### Session persistence issues

**Problem:** Context lost between queries

**Solution:**
- Use same `sessionId` for related queries
- Claude Code maintains conversation history automatically

### MCP tools not working

**Problem:** Tools not accessible

**Checks:**
1. MCP server started: Look for `[MCP] Built-in server started`
2. Tools registered: Check debug output for tool list
3. Tool filtering: Verify `allowedTools` configuration

## Best Practices

1. **Use in Claude Code Environment**
   - Designed for Claude Code's native environment
   - Leverages built-in authentication

2. **Let Claude Code Handle Complexity**
   - Don't try to control tool iterations
   - Trust the black-box approach
   - Monitor via tool events if needed

3. **Enable Debug for Development**
   ```javascript
   const agent = new ProbeAgent({
     provider: 'claude-code',
     debug: true  // See what's happening
   });
   ```

4. **Handle Errors Gracefully**
   ```javascript
   try {
     await agent.initialize();
     const response = await agent.answer(query);
   } catch (error) {
     if (error.message.includes('claude command not found')) {
       // Fallback to API key approach
     }
   }
   ```

## Comparison: Native vs Claude Code

| Feature | Native Engine | Claude Code Engine |
|---------|--------------|-------------------|
| Tool Loop | ProbeAgent controls (1-30 iterations) | Claude Code handles internally |
| Tool Events | Real-time (started/completed pairs) | Batch emission at end |
| Configuration | Requires API keys | Auto-detects claude command |
| Debug Output | Shows "Tool Loop Iteration X/Y" | Shows "bypassing tool loop" |
| MCP Protocol | XML-based tool format | Native MCP protocol |
| Use Case | Custom control, API-based | Claude Code environment |

## Related Documentation

- See `examples/probe-agent-cli.js` for CLI usage
- See `examples/multi-engine-demo.js` for switching between engines
- See main README for general ProbeAgent documentation

---

**Last Updated:** November 2025
**Version:** 0.6.0+
