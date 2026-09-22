# Task Telemetry Event Contract

This document defines the telemetry event contract for Probe's task system. Consumers reading the event stream can reconstruct nested task trees across the main agent and delegated subagents.

## Event Types

All task events are emitted via `tracer.recordTaskEvent(eventType, data)` and appear as `task.<eventType>` in the telemetry stream.

| Event | Trigger |
|-------|---------|
| `task.session_started` | Task system initialized for an agent |
| `task.created` | Single task created |
| `task.batch_created` | Multiple tasks created in one call |
| `task.updated` | Single task updated |
| `task.batch_updated` | Multiple tasks updated in one call |
| `task.completed` | Single task completed |
| `task.batch_completed` | Multiple tasks completed in one call |
| `task.deleted` | Single task deleted |
| `task.batch_deleted` | Multiple tasks deleted in one call |
| `task.listed` | Task list queried |
| `task.validation_error` | Invalid task parameters |
| `task.error` | Execution error |
| `task.unknown_action` | Unrecognized action |

## Guaranteed Fields

### Agent Scope (present on every `task.*` event)

These fields identify which agent emitted the event and where it sits in the delegation hierarchy.

| Field | Type | Description |
|-------|------|-------------|
| `session.id` | `string` | Session ID of the agent (auto-added by tracer) |
| `agent.session_id` | `string` | Same as `session.id` — explicit for grouping |
| `agent.parent_session_id` | `string \| null` | Parent agent's session ID. `null` for the root agent. |
| `agent.root_session_id` | `string` | Root agent's session ID. Always points to the top-level agent. |
| `agent.kind` | `string` | Agent type: `"main"` or `"delegate"` |
| `delegation.task` | `string` | _(Only on delegates)_ The task description that spawned this subagent |
| `task.sequence` | `number` | Monotonically increasing counter for deterministic event ordering |

### How to reconstruct the tree

```
root_session_id
  └─ agent.session_id (kind=main, parent=null)
       ├─ task events for this agent
       └─ agent.session_id (kind=delegate, parent=main)
            ├─ task events for this subagent
            └─ agent.session_id (kind=delegate, parent=delegate)
                 └─ task events for nested subagent
```

**Rule**: Group events by `agent.session_id`. Nest groups using `agent.parent_session_id`. The root group has `agent.parent_session_id === null`.

### Single-Task Event Fields

Present on `task.created`, `task.updated`, `task.completed`, `task.deleted`:

| Field | Type | Description |
|-------|------|-------------|
| `task.action` | `string` | `"create"`, `"update"`, `"complete"`, `"delete"` |
| `task.id` | `string` | Task identifier |
| `task.title` | `string` | Task title |
| `task.status` | `string` | Current status: `"pending"`, `"in_progress"`, `"completed"`, `"cancelled"` |
| `task.priority` | `string \| null` | `"low"`, `"medium"`, `"high"`, `"critical"`, or `null` |
| `task.dependencies` | `string` | JSON-encoded array of dependency task IDs, e.g. `'["auth","db"]'` |
| `task.order` | `number` | 0-based position in the task list |
| `task.total_count` | `number` | Total tasks after this operation |
| `task.incomplete_remaining` | `number` | Pending/in_progress tasks remaining |

Additional fields per event type:

- **`task.created`**: `task.after` (insertion hint, or `null`)
- **`task.updated`**: `task.fields_updated` (comma-separated field names, e.g. `"status, priority"`)
- **`task.deleted`**: Task state is captured *before* deletion

### Batch Event Fields

Present on `task.batch_created`, `task.batch_updated`, `task.batch_completed`, `task.batch_deleted`:

| Field | Type | Description |
|-------|------|-------------|
| `task.action` | `string` | `"create"`, `"update"`, `"complete"`, `"delete"` |
| `task.count` | `number` | Number of tasks in this batch |
| `task.items_json` | `string` | JSON array of per-task objects (see below) |
| `task.total_count` | `number` | Total tasks after this operation |
| `task.incomplete_remaining` | `number` | Pending/in_progress tasks remaining |

**`task.items_json` schema** (per item):

```json
{
  "id": "auth",
  "title": "Auth module",
  "status": "pending",
  "priority": "high",
  "dependencies": ["setup"],
  "after": null,
  "order": 0
}
```

For `batch_deleted`, items capture the state *before* deletion (id, title, status).

### List Event Fields

Present on `task.listed`:

| Field | Type | Description |
|-------|------|-------------|
| `task.action` | `"list"` | |
| `task.total_count` | `number` | Total tasks |
| `task.incomplete_count` | `number` | Pending/in_progress tasks |
| `task.completed_count` | `number` | Completed tasks |
| `task.items_json` | `string` | JSON array — full snapshot of all tasks |

## Reconstructing Task State

### Current task list for any agent

Filter events by `agent.session_id`. Apply events in `task.sequence` order:

1. On `task.created` / `task.batch_created`: add tasks to the list
2. On `task.updated` / `task.batch_updated`: update existing tasks
3. On `task.completed` / `task.batch_completed`: mark tasks completed
4. On `task.deleted` / `task.batch_deleted`: remove tasks from the list

Alternatively, use the latest `task.listed` event's `task.items_json` for a full snapshot.

### Nested task tree

1. Group all task events by `agent.session_id`
2. Build the agent tree using `agent.parent_session_id`
3. Within each agent group, reconstruct the task list using sequence-ordered events
4. Render as a nested tree:

```
Main Agent (session: abc)
  ├─ [✓] Setup database
  ├─ [→] Implement auth
  └─ Delegate: "Fix login bug" (session: def, parent: abc)
       ├─ [✓] Patch auth handler
       └─ [→] Add tests
```

### Active task detection

The currently active task is the first task with `status === "in_progress"` in list order. If no task is in progress, the first `"pending"` task with all dependencies resolved is the next candidate.

## Example Event Stream

```jsonl
{"name":"task.session_started","attrs":{"session.id":"main-1","agent.session_id":"main-1","agent.parent_session_id":null,"agent.root_session_id":"main-1","agent.kind":"main","task.enabled":true}}
{"name":"task.batch_created","attrs":{"session.id":"main-1","task.sequence":1,"agent.session_id":"main-1","agent.parent_session_id":null,"agent.root_session_id":"main-1","agent.kind":"main","task.action":"create","task.count":2,"task.items_json":"[{\"id\":\"auth\",\"title\":\"Auth module\",\"status\":\"pending\",\"priority\":\"high\",\"dependencies\":[],\"after\":null,\"order\":0},{\"id\":\"api\",\"title\":\"API routes\",\"status\":\"pending\",\"priority\":null,\"dependencies\":[\"auth\"],\"after\":null,\"order\":1}]","task.total_count":2,"task.incomplete_remaining":2}}
{"name":"task.updated","attrs":{"session.id":"main-1","task.sequence":2,"agent.session_id":"main-1","agent.parent_session_id":null,"agent.root_session_id":"main-1","agent.kind":"main","task.action":"update","task.id":"auth","task.title":"Auth module","task.status":"in_progress","task.priority":"high","task.dependencies":"[]","task.order":0,"task.fields_updated":"status","task.total_count":2,"task.incomplete_remaining":2}}
{"name":"task.created","attrs":{"session.id":"sub-1","task.sequence":3,"agent.session_id":"sub-1","agent.parent_session_id":"main-1","agent.root_session_id":"main-1","agent.kind":"delegate","delegation.task":"Implement the auth module","task.action":"create","task.id":"task-1","task.title":"Write JWT middleware","task.status":"pending","task.priority":null,"task.dependencies":"[]","task.after":null,"task.order":0,"task.total_count":1,"task.incomplete_remaining":1}}
```
