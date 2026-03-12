# Observability & Telemetry

Probe Agent provides comprehensive observability through three mechanisms:

1. **SimpleTelemetry** — lightweight file/console trace exporter (no heavy dependencies)
2. **OTEL Log Bridge** — automatic forwarding of `console.log/info/warn/error` to OpenTelemetry Logs
3. **Tracer Events** — structured telemetry events emitted throughout the agent lifecycle

## Quick Start

### Standalone CLI

```bash
# File-based tracing
probe agent "query" --trace-file ./traces.jsonl

# Console tracing
probe agent "query" --trace-console
```

### SDK

```javascript
import { ProbeAgent, initializeSimpleTelemetryFromOptions } from '@anthropic-ai/probe';

const telemetry = initializeSimpleTelemetryFromOptions({
  traceFile: './traces.jsonl',
  traceConsole: false,
});

const agent = new ProbeAgent({
  path: '.',
  telemetry,
});
```

## SimpleTelemetry

A zero-dependency tracer that writes span data as JSON Lines to a file or console. Each span contains:

- `traceId`, `spanId` — correlation identifiers
- `name` — span name (e.g., `agent.session`, `ai.request`, `tool.call`)
- `startTime`, `endTime`, `duration` — timing
- `attributes` — structured key-value metadata
- `events` — timestamped events within the span
- `status` — `OK` or `ERROR`

### Span Types

| Span Name | Created By | Description |
|-----------|------------|-------------|
| `agent.session` | `createSessionSpan()` | Top-level session span |
| `ai.request` | `createAISpan()` | Individual LLM API call |
| `tool.call` | `createToolSpan()` | Tool execution |

## OTEL Log Bridge

When `initializeSimpleTelemetryFromOptions()` is called, it automatically patches `console.log`, `console.info`, `console.warn`, and `console.error` to:

1. **Emit OTEL Log Records** via `@opentelemetry/api-logs` (if installed)
2. **Append trace context** `[trace_id=... span_id=...]` to console output (if `@opentelemetry/api` is installed)

This is a no-op if the OpenTelemetry packages are not installed — the bridge gracefully degrades.

### Severity Mapping

| Console Method | OTEL SeverityNumber | OTEL SeverityText |
|---------------|--------------------:|-------------------|
| `console.log` | 9 | INFO |
| `console.info` | 9 | INFO |
| `console.warn` | 13 | WARN |
| `console.error` | 17 | ERROR |

### Integration with External OTEL Collectors

If your application configures an OpenTelemetry SDK with a `LoggerProvider` (e.g., via `@opentelemetry/sdk-logs`), all `console.*` calls from probe will automatically appear as log records in your collector. No additional configuration is needed beyond installing the packages:

```bash
npm install @opentelemetry/api @opentelemetry/api-logs
```

### Visor2 / External Tracer Integration

If you provide your own tracer adapter (e.g., visor2's `createProbeTracerAdapter()`), probe will emit events through your tracer. The OTEL log bridge only activates via `initializeSimpleTelemetryFromOptions()`, so external consumers that create `new SimpleTelemetry()` directly are not affected — they manage their own console instrumentation.

## Tracer Events Reference

All events are emitted via `SimpleAppTracer` methods. Each event includes `session.id` automatically.

### Agent Lifecycle

| Event | Method | Key Attributes |
|-------|--------|----------------|
| `iteration.step` | `recordIterationEvent('step')` | `iteration` |
| `context.compacted` | `addEvent()` | — |
| `completion_prompt.started` | `recordEvent()` | — |
| `completion_prompt.completed` | `recordEvent()` | — |

### AI Model

| Event | Method | Key Attributes |
|-------|--------|----------------|
| `ai.thinking` | `recordThinkingContent()` | `ai.thinking.content`, `ai.thinking.length`, `ai.thinking.hash` |
| `ai.tool_decision` | `recordToolDecision()` | `ai.tool_decision.name`, `ai.tool_decision.params` |

### Tool Execution

| Event | Method | Key Attributes |
|-------|--------|----------------|
| `tool.result` | `recordToolResult()` | `tool.name`, `tool.result`, `tool.duration_ms`, `tool.success` |

### Token Usage

| Event | Method | Key Attributes |
|-------|--------|----------------|
| `tokens.turn` | `recordTokenTurn()` | `tokens.input`, `tokens.output`, `tokens.total`, `tokens.cache_read`, `tokens.cache_write`, `tokens.context_used`, `tokens.context_remaining` |

### Conversation

| Event | Method | Key Attributes |
|-------|--------|----------------|
| `conversation.turn.{role}` | `recordConversationTurn()` | `conversation.role`, `conversation.content`, `conversation.content.length`, `conversation.content.hash` |

### Errors

| Event | Method | Key Attributes |
|-------|--------|----------------|
| `error.{type}` | `recordErrorEvent()` | `error.type`, `error.message`, `error.stack`, `error.recoverable`, `error.context` |

Error types include: `wrapped_tool`, `unrecognized_tool`, `no_tool_call`, `circuit_breaker`.

### Delegation

| Event | Method | Key Attributes |
|-------|--------|----------------|
| `delegation.subagent_created` | `addEvent()` | `delegation.timeout_ms`, `delegation.parent_remaining_ms`, `delegation.timeout_behavior` |
| `delegation.budget_capped` | `addEvent()` | `delegation.original_timeout_ms`, `delegation.capped_timeout_ms`, `delegation.parent_remaining_ms` |
| `delegation.tool_started` | `recordDelegationEvent()` | — |
| `delegation.completed` | `recordDelegationEvent()` | — |
| `delegation.failed` | `recordDelegationEvent()` | — |
| `delegation.parent_abort_phase1` | `addEvent()` | `delegation.deadline_ms` |
| `delegation.parent_abort_phase2` | `addEvent()` | — |

### Graceful Stop (Two-Phase)

| Event | Method | Key Attributes |
|-------|--------|----------------|
| `graceful_stop.external_trigger` | `addEvent()` | — |
| `graceful_stop.initiated` | `addEvent()` | `graceful_stop.reason`, `graceful_stop.active_subagents`, `graceful_stop.has_mcp_bridge`, `graceful_stop.deadline_ms` |
| `graceful_stop.signals_sent` | `addEvent()` | `graceful_stop.subagents_signalled`, `graceful_stop.subagent_errors`, `graceful_stop.mcp_servers_called`, `graceful_stop.mcp_servers_failed` |
| `graceful_stop.deadline_expired` | `addEvent()` | — |

### Graceful Timeout (Wind-Down)

| Event | Method | Key Attributes |
|-------|--------|----------------|
| `graceful_timeout.wind_down_started` | `addEvent()` | — |
| `graceful_timeout.wind_down_step` | `addEvent()` | `graceful_timeout.bonus_steps_remaining` |

### Negotiated Timeout (Observer)

| Event | Method | Key Attributes |
|-------|--------|----------------|
| `negotiated_timeout.observer_invoked` | `addEvent()` | — |
| `negotiated_timeout.observer_response` | `addEvent()` | — |
| `negotiated_timeout.observer_extended` | `addEvent()` | `negotiated_timeout.extension_ms` |
| `negotiated_timeout.observer_declined` | `addEvent()` | — |
| `negotiated_timeout.observer_exhausted` | `addEvent()` | — |
| `negotiated_timeout.observer_error` | `addEvent()` | — |
| `negotiated_timeout.abort_summary_started` | `addEvent()` | — |
| `negotiated_timeout.abort_summary_completed` | `addEvent()` | — |
| `negotiated_timeout.abort_summary_error` | `addEvent()` | — |

### Subagent Registry

| Event | Method | Key Attributes |
|-------|--------|----------------|
| `subagent.registered` | `addEvent()` | `subagent.session_id`, `subagent.active_count` |
| `subagent.unregistered` | `addEvent()` | `subagent.session_id`, `subagent.active_count` |

### MCP (Model Context Protocol)

| Event | Method | Key Attributes |
|-------|--------|----------------|
| `mcp.initialization.started` | `recordMcpEvent()` | — |
| `mcp.initialization.completed` | `recordMcpEvent()` | — |
| `mcp.server.connecting` | `recordMcpEvent()` | server name |
| `mcp.server.connected` | `recordMcpEvent()` | server name |
| `mcp.server.connection_failed` | `recordMcpEvent()` | server name, error |
| `mcp.server.disconnected` | `recordMcpEvent()` | server name |
| `mcp.tools.discovered` | `recordMcpEvent()` | tool count |
| `mcp.tools.filtered` | `recordMcpEvent()` | filter results |
| `mcp.tool.start` | `recordMcpToolStart()` | `mcp.tool.name`, `mcp.tool.server`, `mcp.tool.params` |
| `mcp.tool.end` | `recordMcpToolEnd()` | `mcp.tool.name`, `mcp.tool.server`, `mcp.tool.result`, `mcp.tool.duration_ms`, `mcp.tool.success` |
| `mcp.graceful_stop.sweep_completed` | `recordMcpEvent()` | `servers_with_tool`, `servers_total`, `results` |
| `mcp.disconnection.started` | `recordMcpEvent()` | — |
| `mcp.disconnection.completed` | `recordMcpEvent()` | — |

### Bash Permissions

| Event | Method | Key Attributes |
|-------|--------|----------------|
| `bash.permissions.initialized` | `recordBashEvent()` | — |
| `bash.permission.allowed` | `recordBashEvent()` | command |
| `bash.permission.denied` | `recordBashEvent()` | command |

### Task Management

| Event | Method | Key Attributes |
|-------|--------|----------------|
| `task.session_started` | `recordTaskEvent()` | — |
| `task.batch_created` | `recordTaskEvent()` | — |
| `task.validation_error` | `recordTaskEvent()` | — |

### Validation

| Event | Method | Key Attributes |
|-------|--------|----------------|
| `json_validation.started` | `recordJsonValidationEvent()` | — |
| `json_validation.completed` | `recordJsonValidationEvent()` | — |
| `mermaid_validation.started` | `recordMermaidValidationEvent()` | — |
| `mermaid_validation.completed` | `recordMermaidValidationEvent()` | — |
| `mermaid_validation.maid_fix_completed` | `recordMermaidValidationEvent()` | — |

## Debug Logging

All significant decisions and state changes are logged via `console.log("[DEBUG] ...")`. When the OTEL log bridge is active, these are automatically forwarded to the OTEL Logs pipeline.

Key debug log prefixes:
- `[DEBUG] [GracefulStop]` — graceful stop lifecycle
- `[DEBUG] [GracefulTimeout]` — graceful timeout wind-down
- `[DEBUG] [NegotiatedTimeout]` — negotiated timeout observer
- `[DEBUG] [Subagent]` — subagent registration/unregistration
- `[DEBUG] [MCP]` — MCP server operations
- `[DEBUG] [Delegation]` — delegate tool operations
- `[SimpleTelemetry]` — telemetry system status

## Trace File Format

When using `--trace-file`, spans are written as JSON Lines (one JSON object per line):

```json
{"traceId":"abc123","spanId":"def456","name":"tool.call","startTime":1710000000000,"endTime":1710000001000,"duration":1000,"attributes":{"tool.name":"search","session.id":"xyz"},"events":[],"status":"OK","timestamp":"2024-03-10T00:00:01.000Z"}
```

Each line is a complete JSON object that can be parsed independently, making it easy to stream, tail, or ingest into log aggregation systems.
