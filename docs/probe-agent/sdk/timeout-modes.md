# Timeout Architecture

The ProbeAgent SDK has multiple timeout layers that operate at different levels of the execution stack. Understanding how they interact is essential for building reliable agents, especially those using delegates, MCP tools, or bash commands.

## Timeout Layers at a Glance

```
┌─────────────────────────────────────────────────────────┐
│  maxOperationTimeout (agent level)                      │
│  Controls: entire answer() call                         │
│  Default: 300s │ Env: MAX_OPERATION_TIMEOUT             │
│                                                         │
│  ┌───────────────────────────────────────────────────┐  │
│  │  requestTimeout (per LLM call)                    │  │
│  │  Controls: individual streamText/generateText     │  │
│  │  Default: 120s │ Env: REQUEST_TIMEOUT             │  │
│  └───────────────────────────────────────────────────┘  │
│                                                         │
│  ┌───────────────────────────────────────────────────┐  │
│  │  Delegate timeout (per delegate tool call)        │  │
│  │  Controls: subagent execution                     │  │
│  │  Default: 300s │ Env: DELEGATION_TIMEOUT          │  │
│  │                                                   │  │
│  │  ┌─────────────────────────────────────────────┐  │  │
│  │  │  Subagent's own requestTimeout              │  │  │
│  │  │  (inherited from parent config)             │  │  │
│  │  └─────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────┘  │
│                                                         │
│  ┌───────────────────────────────────────────────────┐  │
│  │  MCP tool timeout (per MCP server)                │  │
│  │  Controls: individual MCP tool calls              │  │
│  │  Default: 30s │ Per-server override available     │  │
│  └───────────────────────────────────────────────────┘  │
│                                                         │
│  ┌───────────────────────────────────────────────────┐  │
│  │  Bash timeout (per command)                       │  │
│  │  Controls: shell command execution                │  │
│  │  Default: 120s │ Max: 600s                        │  │
│  └───────────────────────────────────────────────────┘  │
│                                                         │
│  ┌───────────────────────────────────────────────────┐  │
│  │  Engine activity timeout (stream health)          │  │
│  │  Controls: gap between stream chunks              │  │
│  │  Default: 180s │ Env: ENGINE_ACTIVITY_TIMEOUT     │  │
│  └───────────────────────────────────────────────────┘  │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

**Important:** These timeouts are currently independent. A delegate with a 300s timeout inside a parent with a 300s `maxOperationTimeout` can consume the entire parent budget. See [Known Limitations](#known-limitations) for details.

## Layer 1: Operation Timeout (`maxOperationTimeout`)

The top-level timeout that governs the entire `answer()` call — including all tool calls, retries, fallbacks, and LLM requests within it.

| Setting | Default | Range | Env Var |
|---------|---------|-------|---------|
| `maxOperationTimeout` | 300000 ms (5 min) | 1s – 2hr | `MAX_OPERATION_TIMEOUT` |

When this fires, the behavior depends on `timeoutBehavior`:

### Graceful (default)

The AI gets bonus steps to wrap up without calling more tools.

```javascript
const agent = new ProbeAgent({
  path: '/path/to/project',
  maxOperationTimeout: 300000,
  timeoutBehavior: 'graceful',
  gracefulTimeoutBonusSteps: 4,    // range: 1-20
});
```

**Flow:**
1. Timer fires → `gracefulTimeoutState.triggered = true`
2. `prepareStep` injects wrap-up message with `toolChoice: 'none'`
3. AI writes final response using what it has (up to `bonusSteps` steps)
4. Safety net: hard abort 60s after soft timeout if wind-down stalls

### Hard

Immediate abort, no wind-down.

```javascript
const agent = new ProbeAgent({
  maxOperationTimeout: 300000,
  timeoutBehavior: 'hard',
});
```

### Negotiated (Observer Pattern)

A separate LLM call evaluates whether to extend or abort. Works even when the main loop is blocked by a delegate or MCP tool.

```javascript
const agent = new ProbeAgent({
  maxOperationTimeout: 300000,
  timeoutBehavior: 'negotiated',
  negotiatedTimeoutBudget: 1800000,        // 30 min total extra time
  negotiatedTimeoutMaxRequests: 3,          // up to 3 extensions
  negotiatedTimeoutMaxPerRequest: 600000,   // max 10 min per extension
});
```

**Flow:**
```
maxOperationTimeout fires
       │
       ▼
Observer LLM call (independent generateText)
  ├── Sees active tools and their durations ("delegate — running for 3m 45s")
  ├── Detects stuck/looping agents
  │
  ├── EXTEND → new timer, main loop continues with tools available
  └── DECLINE → abort in-flight tools → dedicated summary LLM call
```

**Observer details:**
- Tracks in-flight tools via `toolCall` event emitter with human-readable durations
- Stuck-loop detection guidance in prompt (repeating tool calls, no progress)
- On decline: aborts tools, then summary call with full conversation context
- Summary respects JSON schema (returns valid JSON, no markdown notice)
- Summary acknowledges task status (completed/incomplete tasks)
- Summary streams to `onStream` callback
- `completionPrompt` is skipped after abort summary

| Setting | Default | Range | Env Var |
|---------|---------|-------|---------|
| `negotiatedTimeoutBudget` | 1800000 ms (30 min) | 1min – 2hr | `NEGOTIATED_TIMEOUT_BUDGET` |
| `negotiatedTimeoutMaxRequests` | 3 | 1 – 10 | `NEGOTIATED_TIMEOUT_MAX_REQUESTS` |
| `negotiatedTimeoutMaxPerRequest` | 600000 ms (10 min) | 1min – 1hr | `NEGOTIATED_TIMEOUT_MAX_PER_REQUEST` |

## Layer 2: Request Timeout (`requestTimeout`)

Timeout for individual LLM API calls (each `streamText` or `generateText` invocation). Fires when a single request to the AI provider takes too long.

| Setting | Default | Range | Env Var |
|---------|---------|-------|---------|
| `requestTimeout` | 120000 ms (2 min) | 1s – 1hr | `REQUEST_TIMEOUT` |

This is independent of `maxOperationTimeout`. A single LLM call can take up to `requestTimeout` ms, and multiple calls can happen within one `answer()` operation. The retry system will re-attempt failed requests up to `retry.maxRetries` times.

## Layer 3: Delegate Timeout

When the agent uses the `delegate` tool to spawn a subagent, the delegate has its own timeout that is **independent of the parent's `maxOperationTimeout`**.

| Setting | Default | Env Var |
|---------|---------|---------|
| Delegate operation timeout | 300s (5 min) | `DELEGATION_TIMEOUT` or `DELEGATION_TIMEOUT_MS` or `DELEGATION_TIMEOUT_SECONDS` |
| Delegation queue timeout | 60s | `DELEGATION_QUEUE_TIMEOUT` |

**What happens when delegate timeout fires:**
1. Calls `subagent.cancel()` to abort the subagent
2. Throws `Delegation timed out after ${timeout} seconds`
3. Releases delegation slot, cleans up resources

**Queue timeout** is separate — it fires if a delegate is waiting for an available execution slot (concurrency limit reached) for too long.

**Abort signal propagation:** The parent's `AbortController` signal is passed to delegates via `parentAbortSignal`. When the parent aborts (e.g., from negotiated timeout decline), delegates receive the signal and cancel. However, the delegate's own timeout does not coordinate with the parent's remaining budget.

## Layer 4: MCP Tool Timeout

MCP (Model Context Protocol) tools have their own timeout system that is managed per-server.

| Setting | Default | Max | Env Var |
|---------|---------|-----|---------|
| Global default | 30000 ms (30s) | — | — |
| Max timeout cap | 1800000 ms (30 min) | — | `MCP_MAX_TIMEOUT` |
| Per-server override | global default | max cap | `MCP_SERVERS_<NAME>_TIMEOUT` |

**Configuration in MCP config file:**
```json
{
  "mcpServers": {
    "my-server": {
      "command": "my-mcp-server",
      "timeout": 60000
    }
  }
}
```

**Resolution order:**
1. Per-server `timeout` if specified in config
2. Global `settings.timeout` if specified
3. Default 30s

**What happens when MCP timeout fires:**
- Throws `MCP tool call timeout after ${timeout}ms`
- The error propagates to the agent as a tool error

MCP servers are **not aware** of the parent agent's `maxOperationTimeout`. An MCP tool call cannot consume more than `MCP_MAX_TIMEOUT` (default 30 min), but within that limit it runs independently.

## Layer 5: Bash Command Timeout

When `enableBash: true`, the bash tool has its own per-command timeout.

| Setting | Default | Range |
|---------|---------|-------|
| Command timeout | 120000 ms (2 min) | 1s – 10 min |

**What happens when bash timeout fires:**
1. Sends `SIGTERM` to the process group
2. If process doesn't exit within 5s, sends `SIGKILL`
3. Returns error: `Command timed out after ${timeout}ms`

Bash timeouts are **not configurable** via environment variable — they use the value from `bashConfig.timeout` or the per-call `timeout` parameter.

## Layer 6: Engine Activity Timeout

Monitors the health of LLM streaming connections. If no data arrives for too long, the stream is considered stalled.

| Setting | Default | Range | Env Var |
|---------|---------|-------|---------|
| Activity timeout | 180000 ms (3 min) | 5s – 10 min | `ENGINE_ACTIVITY_TIMEOUT` |

This is a **gap timer** — it resets every time a chunk arrives. It protects against LLM providers that accept the request but stop sending data. This is different from `requestTimeout`, which measures total elapsed time.

## Complete Reference

### All Timeouts

| Timeout | Layer | Default | Env Var | Configurable via SDK |
|---------|-------|---------|---------|---------------------|
| `maxOperationTimeout` | Operation | 300s | `MAX_OPERATION_TIMEOUT` | Yes |
| `requestTimeout` | Request | 120s | `REQUEST_TIMEOUT` | Yes |
| `timeoutBehavior` | Operation | `graceful` | `TIMEOUT_BEHAVIOR` | Yes |
| `gracefulTimeoutBonusSteps` | Operation | 4 | `GRACEFUL_TIMEOUT_BONUS_STEPS` | Yes |
| `negotiatedTimeoutBudget` | Operation | 30 min | `NEGOTIATED_TIMEOUT_BUDGET` | Yes |
| `negotiatedTimeoutMaxRequests` | Operation | 3 | `NEGOTIATED_TIMEOUT_MAX_REQUESTS` | Yes |
| `negotiatedTimeoutMaxPerRequest` | Operation | 10 min | `NEGOTIATED_TIMEOUT_MAX_PER_REQUEST` | Yes |
| Delegate operation | Delegate | 300s | `DELEGATION_TIMEOUT` | No (env only) |
| Delegation queue | Delegate | 60s | `DELEGATION_QUEUE_TIMEOUT` | No (env only) |
| MCP global default | MCP | 30s | — | Via MCP config |
| MCP max cap | MCP | 30 min | `MCP_MAX_TIMEOUT` | No (env only) |
| MCP per-server | MCP | global | `MCP_SERVERS_<NAME>_TIMEOUT` | Via MCP config |
| Bash command | Bash | 120s | — | Via `bashConfig` |
| Engine activity | Stream | 180s | `ENGINE_ACTIVITY_TIMEOUT` | No (env only) |
| Graceful hard abort safety | Internal | 60s | — | No (hardcoded) |
| File search (glob) | Internal | 10s | — | No (hardcoded) |

### All Environment Variables

```bash
# Operation level
MAX_OPERATION_TIMEOUT=300000               # Overall answer() timeout (ms)
REQUEST_TIMEOUT=120000                     # Per-LLM-call timeout (ms)
TIMEOUT_BEHAVIOR=graceful                  # graceful | hard | negotiated

# Graceful mode
GRACEFUL_TIMEOUT_BONUS_STEPS=4             # Wind-down steps (1-20)

# Negotiated mode
NEGOTIATED_TIMEOUT_BUDGET=1800000          # Total extension budget (ms)
NEGOTIATED_TIMEOUT_MAX_REQUESTS=3          # Max extension count (1-10)
NEGOTIATED_TIMEOUT_MAX_PER_REQUEST=600000  # Max per extension (ms)

# Delegate
DELEGATION_TIMEOUT=300                     # Delegate timeout (seconds)
DELEGATION_TIMEOUT_MS=300000               # Alternative: milliseconds
DELEGATION_TIMEOUT_SECONDS=300             # Alternative: seconds
DELEGATION_QUEUE_TIMEOUT=60000             # Queue wait timeout (ms)

# MCP
MCP_MAX_TIMEOUT=1800000                    # Max allowed MCP timeout (ms)
MCP_SERVERS_MYSERVER_TIMEOUT=60000         # Per-server override (ms)

# Engine
ENGINE_ACTIVITY_TIMEOUT=180000             # Stream chunk gap timeout (ms)
```

## Telemetry

All negotiated timeout operations are instrumented with OTEL tracing:

| Span/Event | Description |
|------------|-------------|
| `negotiated_timeout.observer` | Observer LLM call span |
| `negotiated_timeout.observer_invoked` | Observer started with tool context |
| `negotiated_timeout.observer_response` | Raw observer decision |
| `negotiated_timeout.extended` | Extension granted with duration |
| `negotiated_timeout.declined` | Extension declined with reason |
| `negotiated_timeout.exhausted` | All extensions used |
| `negotiated_timeout.observer_error` | Observer call failed |
| `negotiated_timeout.abort_summary_started` | Summary call initiated |
| `negotiated_timeout.abort_summary_completed` | Summary produced |
| `negotiated_timeout.abort_summary_error` | Summary call failed |

## Known Limitations

### Timeouts do not coordinate across layers

Each timeout layer operates independently. This means:

- A **delegate with a 300s timeout** inside a parent with a 300s `maxOperationTimeout` can consume the entire parent budget. The delegate doesn't know how much time the parent has left.
- An **MCP tool with a 30min timeout** could block the parent for its entire duration. The MCP server has no visibility into the parent's budget.
- The **negotiated timeout observer** can see that a delegate has been running for a long time and decide to abort, but by then most of the budget may already be consumed.

### Practical guidance

For agents using delegates or long-running MCP tools:

1. **Set delegate timeout lower than parent timeout.** If your `maxOperationTimeout` is 5 minutes, set `DELEGATION_TIMEOUT=120` (2 min) so the parent has time to process results or try alternatives.

2. **Use negotiated mode for complex agents.** The observer can detect a stuck delegate and abort it, even if the delegate's own timeout hasn't fired yet.

3. **Set MCP per-server timeouts explicitly.** Don't rely on the 30s default for slow MCP servers, but also don't let them exceed your parent budget.

4. **Leave headroom.** A rule of thumb: set sub-timeouts to at most 60-70% of the parent's budget to leave room for the agent to synthesize results.

## Examples

### Long-Running Analysis with Delegates

```javascript
const agent = new ProbeAgent({
  path: '/path/to/large/codebase',
  provider: 'google',
  model: 'gemini-2.0-flash',
  enableDelegate: true,
  timeoutBehavior: 'negotiated',
  maxOperationTimeout: 60000,              // 1 min initial
  negotiatedTimeoutMaxRequests: 5,          // up to 5 extensions
  negotiatedTimeoutMaxPerRequest: 300000,   // 5 min each
  negotiatedTimeoutBudget: 900000,          // 15 min total extra
});

// Set delegate timeout lower than parent budget
// DELEGATION_TIMEOUT=120 (env var, 2 minutes)

const result = await agent.answer(
  'Perform a comprehensive security audit of this codebase'
);
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
      },
      required: ['patterns'],
    }),
  }
);
// Even after timeout, result is valid JSON matching the schema
```

### Streaming with Timeout

```javascript
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
