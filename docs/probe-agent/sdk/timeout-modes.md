# Timeout Modes

The ProbeAgent SDK provides three timeout modes that control what happens when `maxOperationTimeout` is reached during an AI operation. Each mode offers different trade-offs between time control and result quality.

## Overview

| Mode | Behavior | Tools during wind-down | Best for |
|------|----------|----------------------|----------|
| `graceful` (default) | AI gets bonus steps to write a final answer | Disabled (`toolChoice: 'none'`) | Short tasks, predictable completion |
| `hard` | Operation aborts immediately | N/A | Strict time budgets, batch processing |
| `negotiated` | Independent observer LLM evaluates and decides | Available during extension | Long-running tasks with delegates/subagents |

## Quick Start

```javascript
import { ProbeAgent } from '@probelabs/probe';

const agent = new ProbeAgent({
  path: '/path/to/project',
  maxOperationTimeout: 300000,   // 5 minutes
  timeoutBehavior: 'negotiated', // or 'graceful', 'hard'
});
```

## Mode 1: Graceful (Default)

When the timeout fires, the AI is told to stop calling tools and provide its best answer with what it has. It gets a configurable number of "bonus steps" to write the response.

```javascript
const agent = new ProbeAgent({
  path: '/path/to/project',
  maxOperationTimeout: 300000,        // 5 minutes
  timeoutBehavior: 'graceful',
  gracefulTimeoutBonusSteps: 4,       // default: 4, range: 1-20
});
```

**Flow:**

1. `maxOperationTimeout` fires
2. `prepareStep` injects a "Do NOT call any more tools" message with `toolChoice: 'none'`
3. The AI writes a final text response using findings gathered so far
4. After bonus steps are exhausted, `stopWhen` forces the loop to end
5. If the AI produced no text, a fallback message with collected tool results is returned

**When to use:** Simple queries, tasks that don't involve long-running delegates, situations where you want the AI to always attempt a response.

## Mode 2: Hard

When the timeout fires, the operation aborts immediately with no wind-down period.

```javascript
const agent = new ProbeAgent({
  path: '/path/to/project',
  maxOperationTimeout: 300000,
  timeoutBehavior: 'hard',
});
```

**Flow:**

1. `maxOperationTimeout` fires
2. The `AbortController` signal fires immediately
3. The operation throws an error (or returns a timeout message)

**When to use:** Batch processing, strict SLA enforcement, situations where partial results are not useful.

## Mode 3: Negotiated (Observer Pattern)

When the timeout fires, a **separate LLM call** (the "timeout observer") runs independently of the main agent loop. The observer evaluates whether in-flight work is worth continuing and decides to grant more time or trigger a graceful wind-down.

This is the most sophisticated mode and is designed for long-running tasks where the main loop may be blocked by delegates or MCP tools.

```javascript
const agent = new ProbeAgent({
  path: '/path/to/project',
  maxOperationTimeout: 300000,            // 5 min initial timeout
  timeoutBehavior: 'negotiated',
  negotiatedTimeoutBudget: 1800000,       // 30 min total extra time
  negotiatedTimeoutMaxRequests: 3,         // up to 3 extensions
  negotiatedTimeoutMaxPerRequest: 600000,  // max 10 min per extension
});
```

### How the Observer Works

The observer is a separate `generateText` call that runs independently — it is **not** part of the main agent loop. This means it works even when the main loop is blocked waiting for a delegate subagent or a long-running MCP tool.

**Flow:**

```
Normal operation
       │
       ▼
maxOperationTimeout fires
       │
       ▼
Observer LLM call (independent generateText)
  ├── Sees which tools are running and for how long
  ├── Evaluates whether work is productive or stuck
  │
  ├── EXTEND: Grant more time
  │     ├── Sets new timeout timer
  │     ├── Queues extension message for main loop
  │     └── Main loop continues normally (tools available)
  │
  └── DECLINE: Trigger wind-down
        ├── Sets gracefulTimeoutState.triggered = true
        ├── Aborts in-flight tools via AbortController
        └── Makes dedicated summary LLM call
              └── AI reports what it accomplished with full context
```

### In-Flight Tool Tracking

The negotiated mode tracks all active tool calls via the `toolCall` event emitter. The observer prompt includes human-readable durations:

```
Currently running tools:
- delegate({"task":"analyze auth module"}) — running for 3m 45s
- search({"query":"error handling"}) — running for 12s
```

### Stuck-Loop Detection

The observer prompt includes guidance to detect and decline extensions for stuck agents:

- Agent is repeating the same tool calls
- Agent is making no progress toward the goal
- Tools have been running for an unusually long time
- The task appears to be in an infinite loop

### Abort Summary

When the observer declines an extension, it:

1. Aborts all in-flight tools via `AbortController`
2. Makes a dedicated `generateText` call with the full conversation history
3. The AI provides a detailed summary of what it accomplished and what remains

The summary call is aware of:
- **JSON schema requirements** — if a schema is configured, the summary returns valid JSON matching the schema (no markdown notice prepended)
- **Task status** — if task management is enabled, the summary acknowledges completed and incomplete tasks
- **Streaming** — the summary text is sent to `onStream` callbacks so streaming consumers see the output

### Extension Message Delivery

When the observer grants an extension, the message is queued and delivered to the main loop via `prepareStep` on its next step:

```
⏰ Granted 5 more minute(s) (reason: delegate still analyzing authentication module).
Extensions remaining: 2. Budget remaining: 25 min.
```

Tools remain available during extensions — unlike graceful wind-down, there is no `toolChoice: 'none'`.

### Exhaustion Fallback

When all extensions are used or the budget is exceeded, the negotiated mode falls back to the existing graceful wind-down machinery. The `completionPrompt` (if configured) is skipped after an abort summary to avoid redundant LLM calls.

## Configuration Reference

### Constructor Options

| Option | Type | Default | Env Var | Description |
|--------|------|---------|---------|-------------|
| `requestTimeout` | `number` | `120000` | `REQUEST_TIMEOUT` | Per-request timeout (ms) |
| `maxOperationTimeout` | `number` | `300000` | `MAX_OPERATION_TIMEOUT` | Overall operation timeout (ms) |
| `timeoutBehavior` | `string` | `'graceful'` | `TIMEOUT_BEHAVIOR` | `'graceful'`, `'hard'`, or `'negotiated'` |
| `gracefulTimeoutBonusSteps` | `number` | `4` | `GRACEFUL_TIMEOUT_BONUS_STEPS` | Bonus steps for graceful wind-down (1-20) |
| `negotiatedTimeoutBudget` | `number` | `1800000` | `NEGOTIATED_TIMEOUT_BUDGET` | Total extension budget in ms (1min-2hr) |
| `negotiatedTimeoutMaxRequests` | `number` | `3` | `NEGOTIATED_TIMEOUT_MAX_REQUESTS` | Max extension count (1-10) |
| `negotiatedTimeoutMaxPerRequest` | `number` | `600000` | `NEGOTIATED_TIMEOUT_MAX_PER_REQUEST` | Max ms per extension (1min-1hr) |

### Environment Variables

```bash
# Core timeout
MAX_OPERATION_TIMEOUT=300000             # Overall timeout in ms
REQUEST_TIMEOUT=120000                   # Per-request timeout in ms

# Timeout behavior
TIMEOUT_BEHAVIOR=negotiated              # graceful | hard | negotiated

# Graceful mode
GRACEFUL_TIMEOUT_BONUS_STEPS=4           # Steps for wind-down (1-20)

# Negotiated mode
NEGOTIATED_TIMEOUT_BUDGET=1800000        # Total extension budget (ms)
NEGOTIATED_TIMEOUT_MAX_REQUESTS=3        # Max extensions
NEGOTIATED_TIMEOUT_MAX_PER_REQUEST=600000 # Max per extension (ms)
```

## Telemetry

All timeout operations are instrumented with OTEL tracing when a tracer is configured:

| Span/Event | Description |
|------------|-------------|
| `negotiated_timeout.observer` | Observer LLM call span |
| `negotiated_timeout.observer_invoked` | Observer started with tool context |
| `negotiated_timeout.observer_response` | Raw observer decision |
| `negotiated_timeout.extended` | Extension granted with duration |
| `negotiated_timeout.declined` | Extension declined with reason |
| `negotiated_timeout.exhausted` | All extensions used, falling back |
| `negotiated_timeout.observer_error` | Observer call failed |
| `negotiated_timeout.abort_summary_started` | Summary call initiated |
| `negotiated_timeout.abort_summary_completed` | Summary produced |
| `negotiated_timeout.abort_summary_error` | Summary call failed |

## Examples

### Long-Running Analysis with Negotiated Timeout

```javascript
const agent = new ProbeAgent({
  path: '/path/to/large/codebase',
  provider: 'google',
  model: 'gemini-2.0-flash',
  timeoutBehavior: 'negotiated',
  maxOperationTimeout: 60000,             // 1 min initial
  negotiatedTimeoutMaxRequests: 5,         // up to 5 extensions
  negotiatedTimeoutMaxPerRequest: 300000,  // 5 min each
  negotiatedTimeoutBudget: 900000,         // 15 min total extra
  enableDelegate: true,                    // subagents for parallel work
});

const result = await agent.answer(
  'Perform a comprehensive security audit of this codebase'
);
// Observer will extend if delegates are doing productive work,
// or decline if the agent is stuck in a loop
```

### Schema Output with Timeout Safety

```javascript
const agent = new ProbeAgent({
  path: '/path/to/project',
  timeoutBehavior: 'negotiated',
  maxOperationTimeout: 120000,
});

const result = await agent.answer(
  'Analyze the error handling patterns',
  [],
  {
    schema: JSON.stringify({
      type: 'object',
      properties: {
        patterns: { type: 'array', items: { type: 'string' } },
        coverage: { type: 'number' },
        recommendations: { type: 'array', items: { type: 'string' } },
      },
      required: ['patterns'],
    }),
  }
);

// Even if timeout fires, result will be valid JSON matching the schema
const data = JSON.parse(result);
```

### Streaming with Timeout

```javascript
const agent = new ProbeAgent({
  path: '/path/to/project',
  timeoutBehavior: 'negotiated',
  maxOperationTimeout: 60000,
});

const result = await agent.answer(
  'Search for all API endpoints',
  [],
  {
    onStream: (chunk) => {
      process.stdout.write(chunk);
      // Receives chunks during normal operation AND the abort summary
    },
  }
);
```
