# Context Window Compaction

ProbeAgent includes automatic context window compaction to handle scenarios where conversations exceed the AI model's token limits.

## Overview

When interacting with AI models through ProbeAgent, conversations can grow large over multiple turns. If the context window limit is exceeded, ProbeAgent automatically compacts the conversation history by intelligently removing intermediate reasoning steps while preserving essential information.

## How It Works

### Conversation Structure

ProbeAgent conversations follow this structure:

```
<user message>
  ↓
<internal agentic monologue> (thinking, tool planning)
  ↓
<tool execution> (search, extract, etc.)
  ↓
<tool result>
  ↓
<final agent answer>
```

A "segment" consists of:
- User message (starting point)
- 0+ assistant monologue messages (internal reasoning with `<thinking>` tags, tool calls)
- Final answer (tool results or `attempt_completion`)

### Compaction Strategy

When a context limit error is detected, ProbeAgent:

1. **Identifies segments** in the conversation history
2. **Keeps all user messages** - preserves original questions/requests
3. **Keeps all final answers** - preserves tool results and completions
4. **Removes intermediate monologues** from older segments (thinking, tool planning)
5. **Preserves recent segments** completely (configurable, defaults to last 2 segments)
6. **Retries the request** with compacted messages

This ensures:
- ✅ No loss of user intent
- ✅ No loss of completed work
- ✅ Continuation from current state
- ✅ Significant token reduction

## Error Detection

The compactor automatically detects context limit errors from various AI providers:

- **Anthropic**: "context_length_exceeded", "prompt is too long"
- **OpenAI**: "maximum context length is X tokens"
- **Google/Gemini**: "input token count exceeds limit"
- **Generic patterns**: "tokens exceed", "too long", "over limit", etc.

## Manual Compaction

You can manually compact conversation history at any time using the `compactHistory()` method:

```javascript
const agent = new ProbeAgent({
  sessionId: 'my-session',
  path: './my-project'
});

// ... have some conversations ...

// Manually compact history
const stats = await agent.compactHistory();

console.log(`Removed ${stats.removed} messages`);
console.log(`Token savings: ${stats.tokensSaved}`);

// Compact with custom options
const stats2 = await agent.compactHistory({
  keepLastSegment: true,
  minSegmentsToKeep: 2  // Keep last 2 segments fully
});
```

This is useful when:
- You want to proactively reduce context before hitting limits
- You're monitoring token usage and want to optimize
- You want to clean up history at specific checkpoints
- Testing or debugging compaction behavior

## Configuration

Context compaction is enabled **automatically** when context limits are exceeded. No configuration required!

For manual compaction or advanced use:

```javascript
// Manual compaction
await agent.compactHistory({
  keepLastSegment: true,    // Keep the most recent segment intact
  minSegmentsToKeep: 1      // Number of recent segments to preserve fully
});
```

### Options

| Option | Default | Description |
|--------|---------|-------------|
| `keepLastSegment` | `true` | Always preserve the active/most recent segment |
| `minSegmentsToKeep` | `1` | Number of recent segments to keep fully (including monologues) |

## Example

### Before Compaction (150+ messages)

```
[System message]
Turn 1: User → Assistant(thinking) → Assistant(search) → Tool result
Turn 2: User → Assistant(thinking) → Assistant(extract) → Tool result
Turn 3: User → Assistant(thinking) → Assistant(implement) → Tool result
Turn 4: User → Assistant(thinking) → Assistant(bash) → Tool result
Turn 5: User → Assistant(thinking) → Assistant(search) ← ACTIVE
```

### After Compaction (~50 messages)

```
[System message]
Turn 1: User → Tool result (monologue removed)
Turn 2: User → Tool result (monologue removed)
Turn 3: User → Tool result (monologue removed)
Turn 4: User → Tool result (monologue removed)
Turn 5: User → Assistant(thinking) → Assistant(search) (preserved, active)
```

**Result**: ~66% reduction in messages, ~60% reduction in tokens

Only the **active segment** (Turn 5) retains its full internal monologue. All completed segments (Turns 1-4) are compressed to just User → Final Result.

## Statistics

When compaction occurs, ProbeAgent logs statistics:

```
[INFO] Context window limit exceeded. Compacting conversation...
[INFO] Removed 42 messages (66.7% reduction)
[INFO] Estimated token savings: 8450 tokens
```

With debug mode enabled:

```javascript
const agent = new ProbeAgent({
  debug: true,
  // ... other options
});
```

You'll see detailed compaction information:

```
[DEBUG] Compaction stats: {
  originalCount: 63,
  compactedCount: 21,
  removed: 42,
  reductionPercent: 66.7,
  originalTokens: 12800,
  compactedTokens: 4350,
  tokensSaved: 8450
}
```

## API Reference

### `agent.compactHistory(options)`

Manually compact conversation history.

**Parameters:**
- `options` (Object, optional)
  - `keepLastSegment` (boolean, default: `true`) - Preserve active segment
  - `minSegmentsToKeep` (number, default: `1`) - Number of recent segments to keep fully

**Returns:** Promise<Object> - Compaction statistics
```javascript
{
  originalCount: 63,          // Original message count
  compactedCount: 21,         // Compacted message count
  removed: 42,                // Messages removed
  reductionPercent: 66.7,     // Percentage reduction
  originalTokens: 12800,      // Estimated original tokens
  compactedTokens: 4350,      // Estimated compacted tokens
  tokensSaved: 8450           // Estimated tokens saved
}
```

**Example:**
```javascript
const stats = await agent.compactHistory();
console.log(`Saved ${stats.tokensSaved} tokens`);
```

## Testing

The context compaction functionality is fully tested. Run tests with:

```bash
npm test -- contextCompactor.test.js
npm test -- agent-compact-history.test.js
```

Test coverage includes:
- Error detection across multiple AI providers
- Message segment identification
- Compaction logic with various configurations
- Token estimation and statistics
- Real-world conversation scenarios
- Manual compaction API

## Technical Details

### Files

- **`src/agent/contextCompactor.js`** - Core compaction logic
- **`src/agent/ProbeAgent.js`** - Integration with error handling and API
  - Lines 1498-1542: Automatic error handling
  - Lines 2421-2482: Manual compaction method
- **`tests/contextCompactor.test.js`** - Core compaction test suite
- **`tests/agent-compact-history.test.js`** - Manual API test suite

### Functions

#### `isContextLimitError(error)`
Detects if an error indicates context window overflow.

#### `identifyMessageSegments(messages)`
Analyzes conversation history and identifies logical segments.

#### `compactMessages(messages, options)`
Performs intelligent compaction by removing intermediate monologues.

#### `calculateCompactionStats(originalMessages, compactedMessages)`
Computes reduction statistics and token savings.

#### `handleContextLimitError(error, messages, options)`
Main handler that orchestrates detection and compaction.

#### `agent.compactHistory(options)`
Public API method for manual history compaction.

## Limitations

1. **Minimum context**: Compaction cannot help if even the compacted history exceeds limits
2. **System message**: System messages are never removed (contain critical instructions)
3. **Token estimation**: Token counts are approximations (1 token ≈ 4 characters)
4. **Recent segments**: Always preserves configured minimum segments to maintain context quality

## Best Practices

1. **Use reasonable iteration limits**: Set `maxIterations` appropriately to avoid excessive history
2. **Monitor logs**: Check compaction logs to understand when/why it triggers
3. **Enable debug mode**: For development, use `debug: true` to see detailed statistics
4. **Test edge cases**: Ensure your application handles compaction gracefully

## Future Enhancements

Potential improvements being considered:

- Smarter segment importance scoring (keep more important monologues)
- Configurable compaction aggressiveness
- Semantic compression using embeddings
- Progressive compaction (multiple levels)
- Custom compaction strategies per use case

## Related Documentation

- [ProbeAgent README](./README.md) - Main ProbeAgent documentation
- [Retry and Fallback](./npm/src/agent/RetryManager.js) - Error handling and retry logic
- [MCP Integration](./MCP_INTEGRATION_SUMMARY.md) - Model Context Protocol support
