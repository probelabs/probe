# Hooks System

The ProbeAgent hooks system provides event-driven callbacks for monitoring and customizing agent behavior. Hooks enable logging, analytics, debugging, and custom integrations.

---

## TL;DR

```javascript
const agent = new ProbeAgent({
  path: './src',
  hooks: {
    'tool:start': (data) => console.log(`Tool: ${data.name}`),
    'tool:end': (data) => console.log(`Done: ${data.name} (${data.duration}ms)`),
    'message:user': (data) => logMessage('user', data.message)
  }
});
```

---

## Hook Types

### Lifecycle Hooks

| Hook | Description | Data |
|------|-------------|------|
| `agent:initialized` | Agent initialization complete | `{ sessionId, agent }` |
| `agent:cleanup` | Agent cleanup started | `{ sessionId }` |

### Message Hooks

| Hook | Description | Data |
|------|-------------|------|
| `message:user` | User message received | `{ sessionId, message, images }` |
| `message:assistant` | Assistant response sent | `{ sessionId, content }` |
| `message:system` | System message added | `{ sessionId, content }` |

### Tool Execution Hooks

| Hook | Description | Data |
|------|-------------|------|
| `tool:start` | Tool execution started | `{ sessionId, name, params }` |
| `tool:end` | Tool execution finished | `{ sessionId, name, result, duration }` |
| `tool:error` | Tool execution failed | `{ sessionId, name, error }` |

### AI Streaming Hooks

| Hook | Description | Data |
|------|-------------|------|
| `ai:stream:start` | AI stream started | `{ sessionId }` |
| `ai:stream:delta` | AI stream chunk received | `{ sessionId, chunk }` |
| `ai:stream:end` | AI stream ended | `{ sessionId }` |

### Storage Hooks

| Hook | Description | Data |
|------|-------------|------|
| `storage:load` | History loaded | `{ sessionId, messages }` |
| `storage:save` | Message saved | `{ sessionId, message }` |
| `storage:clear` | History cleared | `{ sessionId }` |

### Iteration Hooks

| Hook | Description | Data |
|------|-------------|------|
| `iteration:start` | Agent iteration started | `{ sessionId, iteration }` |
| `iteration:end` | Agent iteration completed | `{ sessionId, iteration }` |

---

## Hook Constants

Import hook type constants for type safety:

```javascript
import { HOOK_TYPES } from '@probelabs/probe/agent';

const agent = new ProbeAgent({
  hooks: {
    [HOOK_TYPES.AGENT_INITIALIZED]: (data) => console.log('Ready'),
    [HOOK_TYPES.TOOL_START]: (data) => console.log(`Tool: ${data.name}`),
    [HOOK_TYPES.STORAGE_LOAD]: (data) => console.log(`Loaded ${data.messages.length} messages`)
  }
});
```

**Available Constants:**

```javascript
HOOK_TYPES = {
  AGENT_INITIALIZED: 'agent:initialized',
  AGENT_CLEANUP: 'agent:cleanup',
  MESSAGE_USER: 'message:user',
  MESSAGE_ASSISTANT: 'message:assistant',
  MESSAGE_SYSTEM: 'message:system',
  TOOL_START: 'tool:start',
  TOOL_END: 'tool:end',
  TOOL_ERROR: 'tool:error',
  AI_STREAM_START: 'ai:stream:start',
  AI_STREAM_DELTA: 'ai:stream:delta',
  AI_STREAM_END: 'ai:stream:end',
  STORAGE_LOAD: 'storage:load',
  STORAGE_SAVE: 'storage:save',
  STORAGE_CLEAR: 'storage:clear',
  ITERATION_START: 'iteration:start',
  ITERATION_END: 'iteration:end'
}
```

---

## Registration Methods

### Constructor Registration

```javascript
const agent = new ProbeAgent({
  path: './src',
  hooks: {
    'tool:start': async (data) => {
      await logToDatabase('tool_started', data);
    },
    'tool:end': async (data) => {
      await logToDatabase('tool_completed', data);
    }
  }
});
```

### Dynamic Registration

```javascript
// Register hook (returns unregister function)
const unregister = agent.hooks.on('message:user', (data) => {
  console.log(`User: ${data.message}`);
});

// Unregister when done
unregister();
```

### One-Time Hooks

```javascript
// Hook fires once then auto-unregisters
agent.hooks.once('agent:initialized', (data) => {
  console.log('Agent ready!');
});
```

### Manual Unregistration

```javascript
const callback = (data) => console.log(data);

// Register
agent.hooks.on('tool:start', callback);

// Unregister
agent.hooks.off('tool:start', callback);
```

---

## HookManager API

The `agent.hooks` property exposes the HookManager:

```javascript
// Register hook
agent.hooks.on(hookName, callback)    // Returns unregister function

// Register one-time hook
agent.hooks.once(hookName, callback)  // Returns unregister function

// Unregister hook
agent.hooks.off(hookName, callback)

// Clear hooks
agent.hooks.clear()                   // Clear all hooks
agent.hooks.clear('tool:start')       // Clear specific hook

// Query hooks
agent.hooks.getHookNames()            // Get registered hook names
agent.hooks.getCallbackCount('tool:start')  // Get callback count
```

---

## Callback Signatures

All callbacks receive a single `data` parameter and can be async:

```javascript
// Sync callback
agent.hooks.on('tool:start', (data) => {
  console.log(data.name);
});

// Async callback
agent.hooks.on('tool:end', async (data) => {
  await saveToDatabase(data);
});
```

### Data Parameters by Hook

**`agent:initialized`**
```javascript
{
  sessionId: string,  // Agent session ID
  agent: ProbeAgent   // Agent instance
}
```

**`message:user`**
```javascript
{
  sessionId: string,
  message: string,    // User's message
  images: string[]    // Attached images (optional)
}
```

**`tool:start`**
```javascript
{
  sessionId: string,
  name: string,       // Tool name
  params: object      // Tool parameters
}
```

**`tool:end`**
```javascript
{
  sessionId: string,
  name: string,
  result: any,        // Tool result
  duration: number    // Execution time (ms)
}
```

**`tool:error`**
```javascript
{
  sessionId: string,
  name: string,
  error: Error        // Error object
}
```

**`storage:load`**
```javascript
{
  sessionId: string,
  messages: ChatMessage[]  // Loaded history
}
```

**`storage:save`**
```javascript
{
  sessionId: string,
  message: ChatMessage,    // Saved message
  // Or for compaction:
  compacted: boolean,
  stats: object
}
```

---

## Execution Behavior

### Parallel Execution

Multiple callbacks for the same hook execute in parallel:

```javascript
agent.hooks.on('tool:start', async (data) => {
  await slowOperation1();  // These run
});
agent.hooks.on('tool:start', async (data) => {
  await slowOperation2();  // in parallel
});
```

### Error Isolation

Failed hooks don't break other hooks or the agent:

```javascript
agent.hooks.on('tool:start', () => {
  throw new Error('This fails');
});

agent.hooks.on('tool:start', () => {
  console.log('This still runs');
});
```

Errors are logged to `console.error` with hook name and callback index.

---

## Common Patterns

### Logging Middleware

```javascript
const agent = new ProbeAgent({
  path: './src',
  hooks: {
    'message:user': (data) => {
      console.log(`[${new Date().toISOString()}] USER: ${data.message}`);
    },
    'message:assistant': (data) => {
      console.log(`[${new Date().toISOString()}] ASSISTANT: ${data.content.substring(0, 100)}...`);
    },
    'tool:start': (data) => {
      console.log(`[TOOL] Starting: ${data.name}`);
    },
    'tool:end': (data) => {
      console.log(`[TOOL] Completed: ${data.name} (${data.duration}ms)`);
    },
    'tool:error': (data) => {
      console.error(`[TOOL ERROR] ${data.name}: ${data.error.message}`);
    }
  }
});
```

### Analytics Tracking

```javascript
const analytics = {
  toolUsage: new Map(),
  totalTokens: 0
};

const agent = new ProbeAgent({
  path: './src',
  hooks: {
    'tool:end': (data) => {
      const count = analytics.toolUsage.get(data.name) || 0;
      analytics.toolUsage.set(data.name, count + 1);
    },
    'iteration:end': (data) => {
      const usage = agent.getTokenUsage();
      analytics.totalTokens = usage.total.total;
    }
  }
});

// Later: get analytics
console.log('Tool usage:', Object.fromEntries(analytics.toolUsage));
console.log('Total tokens:', analytics.totalTokens);
```

### Progress Indicator

```javascript
let activeTools = 0;

const agent = new ProbeAgent({
  path: './src',
  hooks: {
    'tool:start': (data) => {
      activeTools++;
      updateSpinner(`Working: ${data.name} (${activeTools} active)`);
    },
    'tool:end': (data) => {
      activeTools--;
      if (activeTools === 0) {
        clearSpinner();
      }
    }
  }
});
```

### Custom Storage Integration

```javascript
const agent = new ProbeAgent({
  path: './src',
  hooks: {
    'storage:save': async (data) => {
      // Sync to external storage
      await externalDB.saveMessage(data.sessionId, data.message);
    },
    'storage:clear': async (data) => {
      await externalDB.clearSession(data.sessionId);
    }
  }
});
```

### Streaming Progress

```javascript
const agent = new ProbeAgent({
  path: './src',
  hooks: {
    'ai:stream:start': () => {
      process.stdout.write('Assistant: ');
    },
    'ai:stream:delta': (data) => {
      process.stdout.write(data.chunk);
    },
    'ai:stream:end': () => {
      console.log('\n');
    }
  }
});
```

---

## Related Documentation

- [API Reference](./api-reference.md) - Full ProbeAgent API
- [SDK Getting Started](./getting-started.md) - Quick start guide
- [Storage Adapters](./storage-adapters.md) - Custom storage
