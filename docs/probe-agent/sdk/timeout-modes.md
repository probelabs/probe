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
│  │  Budget-aware: capped to parent's remaining time  │  │
│  │                                                   │  │
│  │  ┌─────────────────────────────────────────────┐  │  │
│  │  │  Subagent's own timeouts                    │  │  │
│  │  │  (inherited from parent config)             │  │  │
│  │  │  timeoutBehavior, requestTimeout,           │  │  │
│  │  │  gracefulTimeoutBonusSteps                  │  │  │
│  │  └─────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────┘  │
│                                                         │
│  ┌───────────────────────────────────────────────────┐  │
│  │  MCP tool timeout (per MCP server)                │  │
│  │  Controls: individual MCP tool calls              │  │
│  │  Default: 30s │ Per-server override available     │  │
│  │  graceful_stop: optional cooperative shutdown     │  │
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
  gracefulStopDeadline: 45000,             // 45s for subagents to wind down
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
  └── DECLINE → two-phase graceful stop
                   │
                   ├── 1. Signal delegates: triggerGracefulWindDown()
                   ├── 2. Signal MCP servers: call graceful_stop tool
                   ├── 3. Wait for tools to finish (up to gracefulStopDeadline)
                   ├── 4. Hard abort if deadline expires
                   └── 5. Parent wind-down with collected results
```

**Observer details:**
- Tracks in-flight tools via `toolCall` event emitter with human-readable durations
- Stuck-loop detection guidance in prompt (repeating tool calls, no progress)
- On decline: **two-phase graceful stop** instead of immediate abort (see below)
- Summary respects JSON schema (returns valid JSON, no markdown notice)
- Summary acknowledges task status (completed/incomplete tasks)
- Summary streams to `onStream` callback
- `completionPrompt` is skipped after abort summary

| Setting | Default | Range | Env Var |
|---------|---------|-------|---------|
| `negotiatedTimeoutBudget` | 1800000 ms (30 min) | 1min – 2hr | `NEGOTIATED_TIMEOUT_BUDGET` |
| `negotiatedTimeoutMaxRequests` | 3 | 1 – 10 | `NEGOTIATED_TIMEOUT_MAX_REQUESTS` |
| `negotiatedTimeoutMaxPerRequest` | 600000 ms (10 min) | 1min – 1hr | `NEGOTIATED_TIMEOUT_MAX_PER_REQUEST` |
| `gracefulStopDeadline` | 45000 ms (45s) | 5s – 5min | `GRACEFUL_STOP_DEADLINE` |

## Two-Phase Graceful Stop

When the negotiated timeout observer declines an extension (or budget is exhausted), the system uses a two-phase shutdown to avoid losing work from running subagents and MCP tools.

### Phase 1: Signal wind-down

Instead of immediately aborting, the system signals all active work to finish:

- **Delegates (subagents):** `triggerGracefulWindDown()` is called, which sets the subagent's graceful timeout flag. The subagent finishes its current tool call, then enters wind-down mode (`toolChoice: 'none'`) to produce a summary of its work. This summary returns to the parent as a normal tool result.

- **MCP servers:** If a connected MCP server exposes a `graceful_stop` tool, the system calls it. Agent-type MCP servers can use this signal to wrap up long-running operations and return partial results. Servers without `graceful_stop` are unaffected.

### Phase 2: Hard abort deadline

A safety timer (`gracefulStopDeadline`, default 45s) starts. If the active tools haven't completed by the deadline, the system falls back to a hard abort via `AbortController.abort()`.

### Why two phases?

Without two-phase stop, a delegate that has been working for 3 minutes — searching, analyzing, collecting results — would be killed instantly and all its work would be lost. The parent's abort summary LLM call wouldn't have access to those results.

With two-phase stop, the delegate gets time to summarize its findings. The parent then has that context for its own wind-down, producing a much better final response.

## Layer 2: Request Timeout (`requestTimeout`)

Timeout for individual LLM API calls (each `streamText` or `generateText` invocation). Fires when a single request to the AI provider takes too long.

| Setting | Default | Range | Env Var |
|---------|---------|-------|---------|
| `requestTimeout` | 120000 ms (2 min) | 1s – 1hr | `REQUEST_TIMEOUT` |

This is independent of `maxOperationTimeout`. A single LLM call can take up to `requestTimeout` ms, and multiple calls can happen within one `answer()` operation. The retry system will re-attempt failed requests up to `retry.maxRetries` times.

## Layer 3: Delegate Timeout

When the agent uses the `delegate` tool to spawn a subagent, the delegate has its own timeout.

| Setting | Default | Env Var |
|---------|---------|---------|
| Delegate operation timeout | 300s (5 min) | `DELEGATION_TIMEOUT` or `DELEGATION_TIMEOUT_MS` or `DELEGATION_TIMEOUT_SECONDS` |
| Delegation queue timeout | 60s | `DELEGATION_QUEUE_TIMEOUT` |

### Budget-Aware Timeout

The delegate timeout is automatically **capped to the parent's remaining budget** (with 10% headroom). If the parent has 2 minutes of budget left, a delegate configured for 5 minutes will be capped to ~1.8 minutes.

```
effectiveTimeout = min(configuredTimeout, remainingParentBudget × 0.9)
```

This prevents delegates from consuming the parent's entire remaining time.

### Timeout Inheritance

Subagents inherit timeout settings from the parent agent:

| Setting | Inherited? | Notes |
|---------|-----------|-------|
| `timeoutBehavior` | Yes | Subagent gets `'graceful'` by default |
| `requestTimeout` | Yes | Same per-LLM-call timeout |
| `gracefulTimeoutBonusSteps` | Yes | Reduced to 2 for subagents (vs 4 for parent) |
| `maxOperationTimeout` | Computed | Set to `(externalTimeout - 15s)` so subagent winds down before external kill |

The subagent's own `maxOperationTimeout` is set 15 seconds shorter than its external deadline. This ensures the subagent's internal wind-down fires before the parent's external timeout kills it, giving the subagent time to produce a summary.

### What happens when delegate timeout fires

1. Calls `subagent.cancel()` to abort the subagent
2. Throws `Delegation timed out after ${timeout} seconds`
3. Releases delegation slot, cleans up resources

### What happens when parent abort signal fires (two-phase)

1. Calls `subagent.triggerGracefulWindDown()` — sets graceful timeout flag, does NOT abort
2. Subagent finishes current step, enters wind-down mode
3. Subagent returns its summary as a normal response
4. If subagent doesn't finish within 30s, hard cancel kicks in

**Queue timeout** is separate — it fires if a delegate is waiting for an available execution slot (concurrency limit reached) for too long.

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

### MCP `graceful_stop` Convention

Agent-type MCP servers can expose a `graceful_stop` tool to support cooperative shutdown. When the parent agent's negotiated timeout triggers a graceful stop, it will:

1. Check if each connected MCP server has a `graceful_stop` tool
2. Call it on servers that do (with a 5s timeout)
3. Well-behaved servers can use this signal to wrap up long-running operations and return partial results

**Implementing `graceful_stop` in your MCP server:**

```javascript
import { Server } from '@modelcontextprotocol/sdk/server/index.js';

const server = new Server({ name: 'my-agent', version: '1.0.0' },
  { capabilities: { tools: {} } });

let stopRequested = false;

// Register graceful_stop
server.setRequestHandler(CallToolRequestSchema, async (request) => {
  if (request.params.name === 'graceful_stop') {
    stopRequested = true;
    return { content: [{ type: 'text', text: 'Stop acknowledged' }] };
  }

  if (request.params.name === 'analyze') {
    // Long-running operation — check stopRequested between steps
    for (const chunk of chunks) {
      if (stopRequested) {
        return { content: [{ type: 'text', text: partialResults }] };
      }
      await processChunk(chunk);
    }
  }
});
```

This convention is:
- **Fully MCP-compliant** — `graceful_stop` is a regular tool call
- **Backwards-compatible** — servers without it are unaffected
- **Protocol-level** — no custom extensions needed

Note: MCP's built-in `notifications/cancelled` is fire-and-forget with no response. It tells the server "stop" but can't get partial results back. The `graceful_stop` tool works within the standard request/response model, allowing the server to return data.

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

## How Timeouts Coordinate

Timeout layers are categorized into two types:

**Safety-net timeouts** (bash, MCP per-call, engine activity) prevent individual operations from hanging indefinitely. These don't need to coordinate with the parent budget — they're bounded and short-lived.

**Budget timeouts** (operation, delegate) control how long an agent can work. These now coordinate:

- Delegate timeout is **capped to the parent's remaining budget** (×0.9 headroom)
- Subagent's internal `maxOperationTimeout` is set **15s shorter** than external deadline
- Subagents **inherit** `timeoutBehavior`, `requestTimeout`, and `gracefulTimeoutBonusSteps`
- Two-phase graceful stop **signals** subagents to wind down instead of killing them

**MCP timeouts** sit in between — the per-call timeout is a safety net, but for agent-type MCP servers doing extended work, the `graceful_stop` convention provides cooperative shutdown.

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
| `gracefulStopDeadline` | Operation | 45s | `GRACEFUL_STOP_DEADLINE` | Yes |
| Delegate operation | Delegate | 300s | `DELEGATION_TIMEOUT` | No (env only) |
| Delegation queue | Delegate | 60s | `DELEGATION_QUEUE_TIMEOUT` | No (env only) |
| MCP global default | MCP | 30s | — | Via MCP config |
| MCP max cap | MCP | 30 min | `MCP_MAX_TIMEOUT` | No (env only) |
| MCP per-server | MCP | global | `MCP_SERVERS_<NAME>_TIMEOUT` | Via MCP config |
| Bash command | Bash | 120s | — | Via `bashConfig` |
| Engine activity | Stream | 180s | `ENGINE_ACTIVITY_TIMEOUT` | No (env only) |
| Graceful hard abort safety | Internal | 60s | — | No (hardcoded) |
| Graceful stop hard abort | Internal | 45s | `GRACEFUL_STOP_DEADLINE` | Yes |
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
GRACEFUL_STOP_DEADLINE=45000               # Wind-down deadline for subagents (ms)

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
  gracefulStopDeadline: 60000,             // 60s for delegates to wind down
});

// Delegate timeout is automatically capped to remaining parent budget
// No need to set DELEGATION_TIMEOUT manually

const result = await agent.answer(
  'Perform a comprehensive security audit of this codebase'
);
```

### Multi-Agent MCP Collaboration

```javascript
const agent = new ProbeAgent({
  path: '/path/to/project',
  timeoutBehavior: 'negotiated',
  maxOperationTimeout: 120000,
  enableMcp: true,
  mcpConfig: {
    mcpServers: {
      'code-reviewer': {
        command: 'node',
        args: ['code-review-agent.js'],
        timeout: 90000,  // 90s per-call timeout
      },
    },
  },
});

// If the code-reviewer MCP server exposes a graceful_stop tool,
// it will be called when the parent agent's timeout triggers,
// allowing the reviewer to return partial results instead of
// being killed mid-review.
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
