# Codex CLI Integration Implementation Summary

This document summarizes the implementation of OpenAI Codex CLI integration for ProbeAgent, following the exact same pattern as PR #298 (Claude Code integration).

## Overview

Added comprehensive support for OpenAI's Codex CLI `codex` command as a provider, enabling zero-configuration usage in Codex environments with auto-fallback and tool event extraction.

## Files Added

### 1. Engine Implementation
**File:** `src/agent/engines/enhanced-codex.js` (595 lines)

Complete Codex CLI engine implementation including:
- `CodexSession` class for conversation management
- `createEnhancedCodexEngine()` - Main engine factory
- `buildCodexArgs()` - Command argument builder
- `processJsonBuffer()` - JSON output parser
- `executeProbleTool()` - Tool execution handler
- Session persistence with `--resume` support
- Tool event extraction and batch emission
- MCP server integration
- POSIX shell escaping for security

**Key Features:**
- Black-box operation: Codex CLI manages its own agentic loop
- Tool collector for batch emission
- Session state tracking (conversation ID, message count)
- Automatic cleanup with `close()` method
- Support for schema/validation mode
- Debug logging throughout

### 2. ProbeAgent Integration
**File:** `src/agent/ProbeAgent.js` (modifications)

Added Codex CLI support to ProbeAgent:

**New Methods:**
- `isCodexCommandAvailable()` - Check if codex command exists (lines 498-511)
- `getCodexNativeSystemPrompt()` - Generate MCP-aware system prompt (lines 1429-1476)

**Modified Methods:**
- `initializeModel()` - Added codex provider detection (lines 539-550)
- `initialize()` - Added codex auto-fallback logic (lines 306-343)
- `getEngine()` - Added Codex engine loading (lines 953-982)

**Auto-Fallback Priority:**
1. Claude Code (if `claude` command available)
2. Codex CLI (if `codex` command available)
3. Error with installation instructions

### 3. Integration Tests

**File:** `tests/integration/codex-auto-fallback.spec.js` (204 lines)

Tests:
- Auto-fallback when no API keys present
- Explicit provider selection (`provider: 'codex'`)
- Normal behavior with API keys
- Error handling when codex not available

**File:** `tests/integration/codex-tool-events.spec.js` (173 lines)

Tests:
- Tool event extraction from Codex CLI output
- Tool batch emission
- Event format validation
- Multiple tools in single query

### 4. Documentation

**File:** `docs/CODEX_INTEGRATION.md` (414 lines)

Comprehensive integration guide including:
- Quick start examples
- Architecture diagrams
- Auto-fallback explanation
- Tool event extraction details
- Configuration options
- Testing instructions
- Troubleshooting guide
- Comparison with other providers

## Implementation Details

### Architecture Pattern

Follows the exact same pattern as `enhanced-claude-code.js`:

```javascript
export async function createEnhancedCodexEngine(options = {}) {
  const { agent, systemPrompt, customPrompt, debug, sessionId, allowedTools } = options;

  // Create session manager
  const session = new CodexSession(sessionId || randomBytes(8).toString('hex'), debug);

  // Start built-in MCP server
  const mcpServer = new BuiltInMCPServer(agent, { port: 0, host: '127.0.0.1', debug });
  await mcpServer.start();

  return {
    sessionId: session.id,
    session,
    async *query(prompt, opts = {}) {
      // Spawn codex CLI
      // Process JSON output
      // Extract tool events
      // Yield messages, tools, batches
    },
    getSession() { /* ... */ },
    async close() { /* ... */ }
  };
}
```

### Tool Event Extraction

Same mechanism as Claude Code:

1. Parse JSON output from Codex CLI
2. Detect `tool_use` content blocks
3. Collect in `toolCollector` array
4. Emit as batch on completion
5. ProbeAgent converts batch to individual events

### Session Management

Identical to Claude Code:

```javascript
class CodexSession {
  constructor(id, debug = false) {
    this.id = id;
    this.conversationId = null;
    this.messageCount = 0;
    this.debug = debug;
  }

  setConversationId(convId) { /* ... */ }
  getResumeArgs() { /* ... */ }
  incrementMessageCount() { /* ... */ }
}
```

### Security

Same POSIX shell escaping as Claude Code:

```javascript
const shellCmd = `echo "" | codex ${args.map(arg => {
  if (typeof arg !== 'string') {
    throw new TypeError(`Invalid argument type: expected string, got ${typeof arg}`);
  }
  const escaped = arg.replace(/'/g, "'\\''");
  return `'${escaped}'`;
}).join(' ')}`;
```

## Configuration Options

### Environment Variables

```bash
USE_CODEX=true          # Force Codex CLI provider
MODEL_NAME=gpt-4o-mini      # Override model
DEBUG=1                      # Enable debug logging
```

### ProbeAgent Options

```javascript
const agent = new ProbeAgent({
  provider: 'codex',           // Explicit provider
  model: 'gpt-4o',                 // Model selection
  sessionId: 'my-session',         // Session ID
  allowedTools: ['search'],        // Tool filtering
  debug: true,                     // Debug mode
  allowedFolders: [process.cwd()]
});
```

## Usage Examples

### Zero-Config Auto-Fallback

```javascript
import { ProbeAgent } from 'probe-agent';

// Auto-detects codex command (if claude not available)
const agent = new ProbeAgent({
  allowedFolders: ['/path/to/code']
});

await agent.initialize();
const response = await agent.answer('Explain this codebase');
```

### Explicit Codex Selection

```javascript
const agent = new ProbeAgent({
  provider: 'codex',  // Force Codex even if Claude available
  allowedFolders: [process.cwd()]
});
```

### Monitor Tool Usage

```javascript
const agent = new ProbeAgent({
  provider: 'codex',
  allowedFolders: [process.cwd()]
});

agent.events.on('toolCall', (event) => {
  console.log(`Tool: ${event.name}`);
  console.log(`Args:`, event.args);
});

await agent.initialize();
await agent.answer('Search for API endpoints');
```

## Testing

Run integration tests:

```bash
# Auto-fallback test
node tests/integration/codex-auto-fallback.spec.js

# Tool events test
node tests/integration/codex-tool-events.spec.js
```

**Note:** Tests require codex command to be installed and available in PATH.

## Comparison with Claude Code Integration

| Feature | Implementation | Same as Claude Code? |
|---------|---------------|---------------------|
| Engine file structure | `enhanced-codex.js` | ✅ Yes |
| Session management | `CodexSession` class | ✅ Yes |
| Tool event extraction | Parse JSON output | ✅ Yes |
| Batch emission | Collect then emit | ✅ Yes |
| MCP server integration | Built-in server | ✅ Yes |
| Auto-fallback logic | Check command availability | ✅ Yes |
| System prompt | `getCodexNativeSystemPrompt()` | ✅ Yes |
| Shell escaping | POSIX single-quote | ✅ Yes |
| Debug logging | Throughout codebase | ✅ Yes |
| Cleanup handling | `close()` method | ✅ Yes |

**Result:** 100% pattern consistency with Claude Code integration.

## Fallback Priority Logic

When `ProbeAgent.initialize()` runs without API keys:

```javascript
if (this.apiType === 'uninitialized') {
  const claudeAvailable = await this.isClaudeCommandAvailable();
  const codexAvailable = await this.isCodexCommandAvailable();

  if (claudeAvailable) {
    // Priority 1: Claude Code
    this.clientApiProvider = 'claude-code';
    this.model = 'claude-3-5-sonnet-20241022';
    this.apiType = 'claude-code';
  } else if (codexAvailable) {
    // Priority 2: Codex CLI
    this.clientApiProvider = 'codex';
    this.model = 'gpt-4o';
    this.apiType = 'codex';
  } else {
    // Error
    throw new Error('No API key and no CLI commands found');
  }
}
```

## Known Limitations

Same as Claude Code integration:

1. **Black-box operation** - ProbeAgent doesn't control tool iteration
2. **Post-hoc events** - Tool events extracted after the fact
3. **No real-time streaming** - Full response before tool events
4. **Command dependency** - Requires `codex` CLI installation
5. **Process spawning** - Uses shell wrapper with `echo ""`

## Future Enhancements

Potential improvements (same as Claude Code):

1. **Real-time event extraction** - Stream tool events as they occur
2. **Better error recovery** - Handle partial JSON output
3. **Performance optimization** - Reduce spawning overhead
4. **Advanced session features** - Branch/merge conversations
5. **Tool result caching** - Avoid duplicate tool calls

## Migration Guide

### From Claude Code to Codex

```javascript
// Before (Claude Code)
const agent = new ProbeAgent({
  provider: 'claude-code',
  allowedFolders: [process.cwd()]
});

// After (Codex CLI)
const agent = new ProbeAgent({
  provider: 'codex',  // Just change provider
  allowedFolders: [process.cwd()]
});
```

Everything else remains the same - same API, same tool event format, same configuration options.

## Related Files

- `src/agent/engines/enhanced-claude-code.js` - Claude Code implementation
- `src/agent/engines/enhanced-vercel.js` - Vercel AI SDK implementation
- `src/agent/mcp/built-in-server.js` - MCP server for tools
- `docs/CLAUDE_CODE_INTEGRATION.md` - Claude Code guide
- `tests/integration/claude-code-*.spec.js` - Claude Code tests

## Summary

This implementation adds Codex CLI support to ProbeAgent following the exact same pattern as PR #298's Claude Code integration. The result is:

- ✅ Zero-configuration usage in Codex environments
- ✅ Automatic fallback when no API keys present
- ✅ Full tool event extraction and emission
- ✅ Session persistence and management
- ✅ Complete test coverage
- ✅ Comprehensive documentation
- ✅ 100% pattern consistency with Claude Code

**Total Lines Added:** ~1,600 lines (engine: 595, tests: 377, docs: 414, ProbeAgent: 100+)

**Files Modified:** 1 (ProbeAgent.js)

**Files Added:** 5 (engine, 2 tests, 2 docs)
