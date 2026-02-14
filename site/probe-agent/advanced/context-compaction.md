# Context & Memory Management

Managing context windows, token usage, and conversation memory.

---

## TL;DR

```javascript
// Token usage tracking
const usage = agent.getTokenUsage();
console.log(`Context: ${usage.contextWindow} tokens`);

// Clear history when needed
agent.clearHistory();

// History is automatically limited to 20 messages
```

---

## Context Window Tracking

### Token Counting

ProbeAgent uses `gpt-tokenizer` for accurate token counting:

```javascript
const agent = new ProbeAgent({ path: './src' });
await agent.answer('Question');

const usage = agent.getTokenUsage();
console.log({
  contextWindow: usage.contextWindow,   // Current context size
  request: usage.request,               // Last request tokens
  response: usage.response,             // Last response tokens
  total: usage.total                    // Total this session
});
```

### Token Estimation

| Content | Estimation |
|---------|------------|
| Text | Accurate via tiktoken |
| Base64 images | 500-2000 tokens based on size |
| URL images | ~1000 tokens |
| Per message overhead | 4 tokens |

---

## History Management

### Automatic Limits

History is automatically limited to prevent context overflow:

```javascript
const MAX_HISTORY_MESSAGES = 100;

// Oldest messages trimmed when limit exceeded
// this.history = this.history.slice(-MAX_HISTORY_MESSAGES)
```

### Manual History Control

```javascript
// Clear history
agent.clearHistory();

// Set history
agent.setHistory(messages);

// Add single message
agent.addMessage({ role: 'user', content: '...' });

// Get current history
console.log(agent.history.length);
```

---

## Message Visibility

### Display vs Agent History

Two history arrays are maintained:

| Type | Purpose | Content |
|------|---------|---------|
| Display History | UI rendering | Timestamps, visibility flags |
| Agent History | AI context | Minimal role + content |

### Visibility Filtering

```javascript
// Messages with visible: 1 included
// Messages with visible: 0 excluded from history

await storage.saveMessage(sessionId, {
  role: 'assistant',
  content: 'Internal processing...',
  visible: 0  // Hidden from restored history
});
```

---

## Token Usage Display

### Real-Time Updates

```javascript
agent.events.on('toolCall', (event) => {
  if (event.status === 'completed') {
    const usage = agent.getTokenUsage();
    updateUI(usage);
  }
});
```

### Usage Object

```javascript
{
  contextWindow: 15000,       // Current context size

  // Current request
  current: {
    request: 1234,
    response: 567,
    total: 1801,
    cacheRead: 500,
    cacheWrite: 200
  },

  // Session totals
  total: {
    request: 5234,
    response: 2567,
    total: 7801,
    cacheRead: 1500,
    cacheWrite: 600
  }
}
```

---

## Provider Context Windows

### Default Limits

| Provider | Model | Context |
|----------|-------|---------|
| Anthropic | Claude Sonnet | 200,000 |
| Anthropic | Claude Haiku | 200,000 |
| OpenAI | GPT-4o | 128,000 |
| OpenAI | GPT-4o-mini | 128,000 |
| Google | Gemini 1.5 Pro | 1,000,000+ |
| Google | Gemini 2.0 Flash | 1,000,000+ |

### Cache Optimization

Anthropic and OpenAI support prompt caching:

```javascript
// Anthropic cache
usage.anthropic = {
  cacheCreation: 100,  // Tokens written to cache
  cacheRead: 500       // Tokens read from cache
};

// OpenAI cache
usage.openai = {
  cachedPrompt: 1000   // Cached prompt tokens
};
```

---

## Session Management

### Session Lifecycle

```javascript
// New session with unique ID
const agent = new ProbeAgent({
  sessionId: crypto.randomUUID()
});

// Session ID persists across interactions
console.log(agent.sessionId);

// Clear session (generates new ID internally)
agent.clearHistory();
```

### Session Caching

```javascript
// Enable session caching for search results
const agent = new ProbeAgent({
  path: './src'
});

// Same query in same session returns cached results
await agent.answer('Find authentication code');
await agent.answer('Show me more authentication code');
// Second query uses session cache
```

---

## Image Handling

### Size Limits

```javascript
const MAX_IMAGE_FILE_SIZE = 20 * 1024 * 1024;  // 20MB
```

### Token Estimation

```javascript
// Base64 images
tokenEstimate = Math.max(500, Math.min(2000, base64Length / 1000));

// URL images
tokenEstimate = 1000;  // Conservative estimate
```

### Security

```javascript
// Images validated against allowed directories
// Path traversal prevented
// Size limits enforced
```

---

## Telemetry Integration

### Context Tracking Events

```javascript
// Events logged for observability
'history.trim' - { messagesBefore, messagesAfter, reason }
'history.update' - { messageCount, contextSize }
'history.clear' - { previousCount, reason }
'history.save' - { sessionId, messageCount }
```

### Span Attributes

```javascript
{
  'context.size': 15000,
  'context.message_count': 12,
  'context.limit_reached': false
}
```

---

## Best Practices

### 1. Monitor Token Usage

```javascript
const usage = agent.getTokenUsage();
if (usage.contextWindow > 150000) {
  console.warn('Approaching context limit');
}
```

### 2. Strategic History Clearing

```javascript
// Clear when switching topics
agent.clearHistory();

// Or when context is too large
if (agent.getTokenUsage().contextWindow > 100000) {
  agent.clearHistory();
  agent.addMessage({
    role: 'system',
    content: 'Previous context summarized: ...'
  });
}
```

### 3. Use Session Caching

```javascript
// Same session ID enables result caching
const agent = new ProbeAgent({
  sessionId: 'my-persistent-session'
});
```

### 4. Limit Result Size

```javascript
// Prevent large results from consuming context
await agent.answer('Search with limits', [], {
  maxIterations: 10
});
```

---

## Limitations

1. **No Automatic Compaction**: History limited by count, not tokens
2. **No Importance-Based Pruning**: FIFO removal only
3. **No Streaming Context**: Size calculated at turn start
4. **Approximate Image Tokens**: Heuristic estimation

---

## Configuration

### Environment Variables

```bash
# Debug context operations
DEBUG_CHAT=1

# Limit tool iterations
--max-iterations 30
```

### Constructor Options

```javascript
const agent = new ProbeAgent({
  requestTimeout: 120000,      // Per-request timeout
  maxOperationTimeout: 600000  // Total operation timeout
});
```

---

## Related Documentation

- [API Reference](../sdk/api-reference.md) - Full API docs
- [Token Usage](../chat/cli-usage.md#token-usage) - CLI display
- [Limits](../../reference/limits.md) - All system limits

