# Task Delegation

Delegation allows ProbeAgent to distribute complex tasks to specialized subagents. Subagents run in-process using the ProbeAgent SDK, providing better performance and resource efficiency.

---

## TL;DR

```javascript
const agent = new ProbeAgent({
  path: './src',
  enableDelegate: true
});

// AI can now delegate tasks automatically
await agent.answer("Analyze authentication, payments, AND notifications");
// AI may delegate each to separate subagents
```

---

## How Delegation Works

When the AI receives a complex request with multiple distinct parts, it can delegate each part to a focused subagent:

```
User Request: "Analyze authentication, payments, and notifications"
                           │
                           ▼
              ┌────────────────────────┐
              │     Primary Agent      │
              │   (orchestrates work)  │
              └────────────────────────┘
                    │     │     │
           ┌────────┘     │     └────────┐
           ▼              ▼              ▼
    ┌──────────┐   ┌──────────┐   ┌──────────┐
    │ Subagent │   │ Subagent │   │ Subagent │
    │   Auth   │   │ Payments │   │  Notifs  │
    └──────────┘   └──────────┘   └──────────┘
           │              │              │
           └──────────────┼──────────────┘
                          ▼
              ┌────────────────────────┐
              │   Combined Response    │
              └────────────────────────┘
```

---

## Enabling Delegation

```javascript
const agent = new ProbeAgent({
  path: './src',
  enableDelegate: true,
  // Optional: configure limits
  // Uses environment variables by default
});
```

**Environment Variables:**

| Variable | Default | Description |
|----------|---------|-------------|
| `MAX_CONCURRENT_DELEGATIONS` | 3 | Max simultaneous subagents |
| `MAX_DELEGATIONS_PER_SESSION` | 10 | Max subagents per session |
| `DELEGATION_QUEUE_TIMEOUT` | 60000 | Queue wait timeout (ms). Set to 0 to disable. |
| `DELEGATION_TIMEOUT` | 300 | Subagent timeout (seconds) |
| `DELEGATION_TIMEOUT_SECONDS` | 300 | Alternative timeout (seconds, higher priority) |
| `DELEGATION_TIMEOUT_MS` | 300000 | Timeout in milliseconds (highest priority) |

---

## Delegate Tool

The AI uses the `delegate` tool to create subagents:

```xml
<delegate>
<task>Analyze authentication module for security vulnerabilities</task>
</delegate>
```

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `task` | string | Yes | Complete, self-contained task description |

### Automatic Inheritance

Subagents automatically inherit from the parent:
- Workspace path and allowed folders
- AI provider and model
- Bash configuration (if enabled)
- MCP configuration (if enabled)

### Subagent Configuration

Subagents are created with focused settings:
- `promptType: 'code-researcher'` (always)
- `enableDelegate: false` (prevents recursion)
- Limited iterations (respects parent's budget)
- Disabled mermaid/JSON validation (faster)

---

## Concurrency Management

### Default Limits

```javascript
// Defaults (configurable via environment)
maxConcurrent: 3        // Global limit
maxPerSession: 10       // Per-session limit
queueTimeout: 60000     // Queue wait timeout (ms)
```

### Queuing Behavior

When limits are reached, delegations queue:

1. Try immediate acquisition
2. If global limit reached, enter FIFO queue
3. Wait up to `queueTimeout` ms
4. Process when slot becomes available
5. Timeout error if wait exceeds limit

### Custom Delegation Manager

```javascript
import { DelegationManager } from '@probelabs/probe/agent';

const manager = new DelegationManager();
manager.maxConcurrent = 5;
manager.maxPerSession = 20;

const agent = new ProbeAgent({
  path: './src',
  enableDelegate: true,
  delegationManager: manager
});
```

---

## Usage Patterns

### AI-Driven Delegation

The AI automatically determines when to delegate:

```javascript
const agent = new ProbeAgent({
  path: './src',
  enableDelegate: true
});

// Complex request - AI may delegate
await agent.answer(`
  I need a comprehensive analysis:
  1. Security review of authentication
  2. Performance audit of database queries
  3. Code quality assessment of API handlers
`);
```

### Programmatic Delegation

Use the delegate function directly:

```javascript
import { delegate } from '@probelabs/probe/agent';

const result = await delegate({
  task: 'Find all SQL injection vulnerabilities',
  timeout: 300,
  path: '/path/to/project',
  provider: 'anthropic',
  debug: true
});

console.log(result);
```

### With Response Schema

Request structured output from subagents:

```javascript
const result = await delegate({
  task: 'List all API endpoints',
  schema: {
    type: 'object',
    properties: {
      endpoints: {
        type: 'array',
        items: {
          type: 'object',
          properties: {
            method: { type: 'string' },
            path: { type: 'string' },
            handler: { type: 'string' }
          }
        }
      }
    }
  }
});

const data = JSON.parse(result);
console.log(data.endpoints);
```

---

## Delegate Function Parameters

```javascript
delegate({
  // Required
  task: string,                    // Task description

  // Timeout
  timeout: number,                 // Seconds (default: 300)

  // Context inheritance
  path: string,                    // Workspace path
  allowedFolders: string[],        // Allowed folders
  provider: string,                // AI provider
  model: string,                   // AI model

  // Capabilities
  enableBash: boolean,             // Enable bash tool
  bashConfig: object,              // Bash configuration
  enableMcp: boolean,              // Enable MCP
  mcpConfig: object,               // MCP configuration

  // Advanced
  currentIteration: number,        // Parent's iteration count
  maxIterations: number,           // Max iterations allowed
  schema: object,                  // Response JSON schema
  parentSessionId: string,         // Parent session ID
  delegationManager: object,       // Custom manager

  // Debug
  debug: boolean,                  // Debug logging
  tracer: object                   // Telemetry tracer
})
```

---

## Recursion Prevention

Subagents cannot delegate further:

```javascript
// In subagent creation:
new ProbeAgent({
  enableDelegate: false  // Always false for subagents
});
```

This prevents:
- Infinite delegation chains
- Runaway resource usage
- Stack overflow errors

---

## Error Handling

### Common Errors

| Error | Cause | Solution |
|-------|-------|----------|
| `Task parameter is required` | Empty task | Provide non-empty task |
| `Maximum delegations per session reached` | Session limit exceeded | Wait or increase limit |
| `Delegation queue timeout` | Queue wait exceeded | Increase timeout or reduce load |
| `Delegation timed out` | Subagent took too long | Increase timeout |
| `Delegate agent returned empty response` | Subagent failed | Check task clarity |

### Cleanup Guarantees

Delegation always cleans up:
- Slots released even on error
- Timeouts cleared
- Counters decremented

```javascript
try {
  const result = await delegate({ task: '...' });
} catch (error) {
  // Slot already released
  console.error('Delegation failed:', error.message);
}
```

---

## Monitoring

### Get Statistics

```javascript
import { getDelegationStats } from '@probelabs/probe/agent';

const stats = getDelegationStats();
console.log({
  active: stats.globalActive,
  maxConcurrent: stats.maxConcurrent,
  maxPerSession: stats.maxPerSession,
  queueSize: stats.queueSize,
  sessions: stats.sessionCount
});
```

### Telemetry Events

If tracer is provided:

```javascript
// Events recorded:
'delegation.started'
'delegation.completed'
'delegation.failed'

// Attributes:
'delegation.session_id'
'delegation.parent_session_id'
'delegation.duration_ms'
'delegation.response_length'
'delegation.success'
```

---

## Best Practices

### 1. Clear Task Descriptions

```javascript
// Good: specific, self-contained
await delegate({
  task: 'Analyze the authentication module in src/auth/ for security vulnerabilities. Check for: hardcoded credentials, SQL injection, XSS, and improper session handling.'
});

// Bad: vague, dependent on context
await delegate({
  task: 'Check security'
});
```

### 2. Appropriate Timeouts

```javascript
// Quick analysis
await delegate({
  task: 'Count functions in utils.ts',
  timeout: 60
});

// Deep analysis
await delegate({
  task: 'Comprehensive security audit',
  timeout: 600
});
```

### 3. Monitor Resource Usage

```javascript
const stats = getDelegationStats();
if (stats.globalActive >= stats.maxConcurrent - 1) {
  console.warn('Near delegation limit');
}
```

---

## Related Documentation

- [Tools Reference](../sdk/tools-reference.md) - All available tools
- [API Reference](../sdk/api-reference.md) - ProbeAgent configuration
- [Environment Variables](../../reference/environment-variables.md) - Configuration
