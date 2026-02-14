# Storage Adapters

Persistent storage for chat sessions and conversation history.

---

## TL;DR

```javascript
import { JsonChatStorage } from '@probelabs/probe-chat';

const storage = new JsonChatStorage({
  webMode: true,      // File-based storage
  verbose: false      // Debug logging
});

await storage.initialize();
```

---

## Overview

Storage adapters provide persistent conversation history for Probe Chat sessions. The system supports:

- **File-based storage**: JSON files in `~/.probe/sessions/`
- **In-memory fallback**: Automatic fallback when filesystem unavailable
- **Session management**: Create, update, list, and delete sessions

---

## JsonChatStorage

The primary storage implementation.

### Constructor

```javascript
const storage = new JsonChatStorage({
  webMode: boolean,   // true = file storage, false = memory only
  verbose: boolean    // Enable debug logging
});
```

### Dual-Mode Operation

```
webMode: true
├── Primary: JSON file storage (~/.probe/sessions/)
└── Fallback: In-memory storage (on filesystem errors)

webMode: false
└── In-memory storage only (CLI mode)
```

### Storage Location

| Platform | Path |
|----------|------|
| Linux/macOS | `~/.probe/sessions/` |
| Windows | `%LOCALAPPDATA%\probe\sessions\` |

---

## API Reference

### initialize()

Initialize the storage adapter.

```javascript
await storage.initialize(): Promise<boolean>
```

- Creates storage directory if needed
- Returns `true` (falls back to memory on error)

---

### saveSession()

Save session metadata.

```javascript
await storage.saveSession({
  id: string,
  created_at: number,
  last_activity: number,
  first_message_preview: string | null,
  metadata: object
}): Promise<boolean>
```

---

### saveMessage()

Append a message to a session.

```javascript
await storage.saveMessage(sessionId: string, {
  role: 'user' | 'assistant' | 'toolCall',
  content: string,
  timestamp: number,
  display_type: string,
  visible: number | boolean,
  images?: array,
  metadata?: object
}): Promise<boolean>
```

---

### getSessionHistory()

Load session messages.

```javascript
const messages = await storage.getSessionHistory(
  sessionId: string,
  limit?: number    // Default: 100
): Promise<Message[]>
```

Returns only visible messages (`visible != 0`).

---

### listSessions()

List recent sessions.

```javascript
const sessions = await storage.listSessions(
  limit?: number,   // Default: 50
  offset?: number   // Default: 0
): Promise<Session[]>
```

Sorted by modification time (newest first).

---

### updateSessionActivity()

Update last activity timestamp.

```javascript
await storage.updateSessionActivity(
  sessionId: string,
  timestamp?: number
): Promise<boolean>
```

---

### deleteSession()

Delete a session.

```javascript
await storage.deleteSession(sessionId: string): Promise<boolean>
```

---

### pruneOldSessions()

Delete sessions older than cutoff.

```javascript
const deleted = await storage.pruneOldSessions(
  olderThanDays?: number  // Default: 30
): Promise<number>
```

---

### getStats()

Get storage statistics.

```javascript
const stats = await storage.getStats(): Promise<{
  session_count: number,
  message_count: number,
  visible_message_count: number,
  storage_type: 'json_files' | 'memory' | 'error'
}>
```

---

### isPersistent()

Check if using file storage.

```javascript
const persistent = storage.isPersistent(): boolean
```

---

### close()

Close the storage adapter.

```javascript
await storage.close(): Promise<void>
```

---

## Session Data Model

### Session Structure

```javascript
{
  id: "uuid-string",
  created_at: 1704067200000,      // Unix timestamp
  last_activity: 1704067456789,
  first_message_preview: "How does the search...",
  metadata: {
    apiProvider: "anthropic"
  },
  messages: [...]
}
```

### Message Structure

```javascript
{
  role: "user",                   // user, assistant, toolCall
  content: "How does authentication work?",
  timestamp: 1704067200000,
  display_type: "user",           // user, final, toolCall
  visible: 1,                     // 1 = visible, 0 = hidden
  images: [],
  metadata: {}
}
```

---

## Integration

### With ChatSessionManager

```javascript
import { ChatSessionManager } from '@probelabs/probe-chat';

const chat = new ChatSessionManager({
  sessionId: 'my-session',
  storage: new JsonChatStorage({ webMode: true })
});

await chat.initialize();  // Loads history from storage

const response = await chat.chat('Hello');
// Messages automatically saved to storage
```

### With Web Server

```javascript
// Global storage instance
const globalStorage = new JsonChatStorage({
  webMode: true,
  verbose: process.env.DEBUG_CHAT === '1'
});

await globalStorage.initialize();

// List sessions API
app.get('/api/sessions', async (req, res) => {
  const sessions = await globalStorage.listSessions(50);
  res.json({ sessions });
});

// Restore session API
app.get('/api/session/:id/history', async (req, res) => {
  const history = await globalStorage.getSessionHistory(req.params.id);
  res.json({ history });
});
```

---

## File Format

Each session stored as `{sessionId}.json`:

```json
{
  "id": "abc-123-def",
  "created_at": 1704067200000,
  "last_activity": 1704067456789,
  "first_message_preview": "How does the search...",
  "metadata": {
    "apiProvider": "anthropic"
  },
  "messages": [
    {
      "role": "user",
      "content": "How does the search function work?",
      "timestamp": 1704067200000,
      "display_type": "user",
      "visible": 1,
      "images": [],
      "metadata": {}
    },
    {
      "role": "assistant",
      "content": "The search function uses...",
      "timestamp": 1704067234567,
      "display_type": "final",
      "visible": 1,
      "images": [],
      "metadata": {}
    }
  ]
}
```

---

## Fallback Behavior

### Automatic Fallback Triggers

1. Cannot create storage directory
2. No write permissions
3. Disk full
4. File read/write errors
5. JSON parse errors (corrupted files)
6. CLI mode (webMode=false)

### Graceful Degradation

```javascript
// All operations succeed with memory fallback
const storage = new JsonChatStorage({ webMode: true });
await storage.initialize();  // Always returns true

// Check storage type
const stats = await storage.getStats();
if (stats.storage_type === 'memory') {
  console.warn('Using in-memory storage (data not persisted)');
}
```

---

## Error Handling

All operations handle errors gracefully:

```javascript
// No exceptions thrown
const history = await storage.getSessionHistory('invalid-id');
// Returns: [] (empty array)

const saved = await storage.saveMessage('id', message);
// Returns: false on error, true on success
```

---

## Message Visibility

### Visible Messages

Included in `getSessionHistory()`:
- User messages
- Assistant responses
- Tool call results for display

### Hidden Messages

Excluded from history:
- Internal processing steps
- Debug information
- Intermediate states

```javascript
// Save hidden message
await storage.saveMessage(sessionId, {
  role: 'assistant',
  content: 'Internal processing...',
  visible: 0  // Hidden
});

// Only visible messages returned
const history = await storage.getSessionHistory(sessionId);
// Does not include hidden message
```

---

## Session Lifecycle

### Creation

```javascript
// Automatic on first message
await storage.saveMessage(sessionId, firstMessage);
// Creates session if doesn't exist
```

### Activity Tracking

```javascript
// Update activity timestamp
await storage.updateSessionActivity(sessionId);

// Automatically updated on saveMessage
await storage.saveMessage(sessionId, message);
```

### Cleanup

```javascript
// Delete old sessions
const deleted = await storage.pruneOldSessions(7);  // 7 days
console.log(`Deleted ${deleted} old sessions`);

// Delete specific session
await storage.deleteSession(sessionId);
```

---

## Best Practices

### 1. Initialize Early

```javascript
const storage = new JsonChatStorage({ webMode: true });
await storage.initialize();
// Check storage type
const stats = await storage.getStats();
console.log(`Using ${stats.storage_type} storage`);
```

### 2. Handle Memory Fallback

```javascript
if (!storage.isPersistent()) {
  console.warn('Sessions will not persist across restarts');
}
```

### 3. Regular Cleanup

```javascript
// Weekly cleanup job
setInterval(async () => {
  const deleted = await storage.pruneOldSessions(30);
  console.log(`Cleaned up ${deleted} old sessions`);
}, 7 * 24 * 60 * 60 * 1000);
```

### 4. Session Limits

```javascript
// Limit history size
const history = await storage.getSessionHistory(sessionId, 100);
```

---

## Related Documentation

- [Chat CLI Usage](../chat/cli-usage.md) - CLI interface
- [Chat Configuration](../chat/configuration.md) - Configuration options
- [Web Interface](../chat/web-interface.md) - Web server

