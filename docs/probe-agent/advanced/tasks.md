# Task Management

Task management enables structured tracking of multi-step operations. Tasks help organize complex work, manage dependencies, and ensure completion before finishing.

---

## TL;DR

```javascript
const agent = new ProbeAgent({
  path: './src',
  enableTasks: true
});

// AI will create and track tasks for complex requests
await agent.answer(`
  Implement user authentication:
  1. Create user model
  2. Add login endpoint
  3. Add registration endpoint
  4. Write tests
`);
```

---

## When to Use Tasks

### Use Tasks For

- **Multiple distinct deliverables**: "Fix bug A AND add feature B"
- **Sequential phases**: "Design, implement, test, document"
- **Complex investigations**: "Analyze auth, payments, AND notifications"
- **Explicit user requests**: "Create a plan for..."

### Skip Tasks For

- **Single-goal requests**: "How does authentication work?"
- **Simple questions**: "What does function X do?"
- **Internal multi-step work**: Multiple searches for ONE goal

---

## Enabling Tasks

```javascript
const agent = new ProbeAgent({
  path: './src',
  enableTasks: true
});
```

---

## Task Data Model

```javascript
{
  id: "task-1",                    // Auto-generated
  title: "Create user model",      // Required
  description: "Define User schema with email, password, timestamps",
  status: "pending",               // pending | in_progress | completed | cancelled
  priority: "high",                // low | medium | high | critical
  dependencies: ["task-1"],        // Tasks that must complete first
  createdAt: "2025-02-14T10:00:00Z",
  updatedAt: "2025-02-14T10:05:00Z",
  completedAt: "2025-02-14T10:10:00Z"  // Set when completed
}
```

### Status Values

| Status | Description |
|--------|-------------|
| `pending` | Not yet started |
| `in_progress` | Currently being worked on |
| `completed` | Finished successfully |
| `cancelled` | No longer needed |

### Priority Values

| Priority | Description |
|----------|-------------|
| `low` | Can be deferred |
| `medium` | Normal priority |
| `high` | Should prioritize |
| `critical` | Must do immediately |

---

## Task Tool

The AI uses the `task` tool to manage tasks:

### Create Task

```xml
<task>
<action>create</action>
<title>Create user model</title>
<description>Define User schema</description>
<priority>high</priority>
</task>
```

### Create Multiple Tasks

```xml
<task>
<action>create</action>
<tasks>[
  {"title": "Create user model", "priority": "high"},
  {"title": "Add login endpoint", "dependencies": ["task-1"]},
  {"title": "Write tests", "dependencies": ["task-2"]}
]</tasks>
</task>
```

### Update Task

```xml
<task>
<action>update</action>
<id>task-1</id>
<status>in_progress</status>
</task>
```

### Complete Task

```xml
<task>
<action>complete</action>
<id>task-1</id>
</task>
```

### Cancel Task

```xml
<task>
<action>update</action>
<id>task-1</id>
<status>cancelled</status>
</task>
```

### Delete Task

```xml
<task>
<action>delete</action>
<id>task-1</id>
</task>
```

### List Tasks

```xml
<task>
<action>list</action>
</task>
```

---

## Dependencies

Tasks can depend on other tasks:

```xml
<task>
<action>create</action>
<tasks>[
  {"title": "Phase 1: Research", "priority": "high"},
  {"title": "Phase 2: Design", "dependencies": ["task-1"]},
  {"title": "Phase 3: Implement", "dependencies": ["task-2"]},
  {"title": "Phase 4: Test", "dependencies": ["task-3"]}
]</tasks>
</task>
```

### Dependency Rules

1. **Dependencies must exist**: Cannot depend on non-existent tasks
2. **No circular dependencies**: A → B → A is invalid
3. **Automatic enforcement**: System prevents violations

### Task Ordering

Insert tasks at specific positions:

```xml
<task>
<action>create</action>
<title>New task</title>
<after>task-2</after>
</task>
```

---

## Task Workflow

### Phase 1: Planning

At the start of a complex request:

```xml
<task>
<action>create</action>
<tasks>[
  {"title": "Search authentication code", "priority": "high"},
  {"title": "Analyze login flow", "dependencies": ["task-1"]},
  {"title": "Review session management", "dependencies": ["task-1"]},
  {"title": "Summarize findings", "dependencies": ["task-2", "task-3"]}
]</tasks>
</task>
```

### Phase 2: Execution

Update status as you work:

```xml
<task>
<action>update</action>
<id>task-1</id>
<status>in_progress</status>
</task>

<!-- ... do work ... -->

<task>
<action>complete</action>
<id>task-1</id>
</task>
```

### Phase 3: Adaptation

Modify tasks as understanding evolves:

```xml
<!-- Split complex task -->
<task>
<action>update</action>
<id>task-3</id>
<status>cancelled</status>
</task>

<task>
<action>create</action>
<tasks>[
  {"title": "Review JWT tokens"},
  {"title": "Review refresh logic"}
]</tasks>
</task>
```

### Phase 4: Completion

Ensure all tasks are resolved before `attempt_completion`:

```xml
<task>
<action>list</action>
</task>

<!-- Complete or cancel remaining tasks -->
```

---

## Completion Blocking

The system blocks completion if tasks remain:

```
<task_completion_blocked>
You cannot complete yet. The following tasks are still unresolved:

Tasks:
- [pending] task-1: Search for authentication
- [in_progress] task-2: Analyze flow (blocked by: task-1)

Required action:
1. Complete or cancel each task
2. Call attempt_completion again
</task_completion_blocked>
```

---

## TaskManager API

### Core Methods

```javascript
const manager = new TaskManager({ debug: false });

// Create tasks
manager.createTask({ title: "Task 1", priority: "high" });
manager.createTasks([{ title: "Task 2" }, { title: "Task 3" }]);

// Read tasks
manager.getTask("task-1");
manager.listTasks();
manager.listTasks({ status: "pending" });

// Update tasks
manager.updateTask("task-1", { status: "in_progress" });
manager.updateTasks([{ id: "task-1", status: "completed" }]);

// Complete tasks
manager.completeTask("task-1");
manager.completeTasks(["task-1", "task-2"]);

// Delete tasks
manager.deleteTask("task-1");
manager.deleteTasks(["task-1", "task-2"]);
```

### Query Methods

```javascript
// Get incomplete tasks
manager.getIncompleteTasks();

// Get tasks ready to start (no blocking dependencies)
manager.getReadyTasks();

// Get tasks waiting for dependencies
manager.getBlockedTasks();

// Check if any incomplete tasks remain
manager.hasIncompleteTasks();
```

### Formatting Methods

```javascript
// Human-readable summary
console.log(manager.getTaskSummary());
// Output:
// Tasks:
// - [pending] task-1: Create user model
// - [in_progress] task-2: Add login (blocked by: task-1)

// XML for AI prompts
console.log(manager.formatTasksForPrompt());
// Output:
// <task_status>
//   <task id="task-1" status="pending">Create user model</task>
//   ...
// </task_status>
```

### Persistence

```javascript
// Export state
const state = manager.export();

// Import state
manager.import(state);

// Clear all tasks
manager.clear();
```

---

## Status Lifecycle

```
┌─────────┐
│ pending │──────────────────────────┐
└────┬────┘                          │
     │                               │
     ▼                               ▼
┌────────────┐                 ┌───────────┐
│in_progress │─────────────────│ cancelled │
└─────┬──────┘                 └───────────┘
      │
      ▼
┌───────────┐
│ completed │
└───────────┘
```

---

## Telemetry

If tracer is provided:

```javascript
// Events recorded:
'task.session_started'
'task.created'
'task.batch_created'
'task.updated'
'task.completed'
'task.deleted'
'task.listed'
```

---

## Best Practices

### 1. Create Tasks for Distinct Goals

```javascript
// Good: separate deliverables
createTasks([
  { title: "Implement login" },
  { title: "Implement logout" },
  { title: "Add tests" }
]);

// Bad: internal steps as tasks
createTasks([
  { title: "Search for login code" },
  { title: "Read login.ts" },
  { title: "Understand login" }
]);
```

### 2. Use Dependencies for Order

```javascript
createTasks([
  { title: "Design API", priority: "high" },
  { title: "Implement API", dependencies: ["task-1"] },
  { title: "Test API", dependencies: ["task-2"] }
]);
```

### 3. Adapt as You Learn

```javascript
// Discovered new work? Add a task
createTask({ title: "Fix discovered bug", priority: "critical" });

// Task too complex? Split it
updateTask("task-3", { status: "cancelled" });
createTasks([
  { title: "Part A of task-3" },
  { title: "Part B of task-3" }
]);
```

### 4. Resolve Before Completion

```javascript
// Check before attempting completion
if (manager.hasIncompleteTasks()) {
  console.log(manager.getTaskSummary());
  // Complete or cancel remaining tasks
}
```

---

## Related Documentation

- [Skills System](./skills.md) - Specialized agent capabilities
- [Tools Reference](../sdk/tools-reference.md) - Available tools
- [API Reference](../sdk/api-reference.md) - Configuration options
