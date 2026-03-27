/**
 * Task Tool - definition and executor for task management
 * @module agent/tasks/taskTool
 */

import { z } from 'zod';

/**
 * Schema for a single task item in batch operations
 */
export const taskItemSchema = z.object({
  id: z.string().describe('Unique task identifier. Use short descriptive slugs (e.g. "auth", "setup-db"). Dependencies reference these IDs.'),
  title: z.string().optional(),
  description: z.string().optional(),
  status: z.enum(['pending', 'in_progress', 'completed', 'cancelled']).optional(),
  priority: z.enum(['low', 'medium', 'high', 'critical']).optional(),
  dependencies: z.array(z.string()).optional().describe('Array of task IDs (from this batch or previously created) that must complete before this task can start.'),
  after: z.string().optional()
});

/**
 * Task schema for validation
 */
export const taskSchema = z.object({
  action: z.enum(['create', 'update', 'complete', 'delete', 'list']),
  // Accept both array and JSON string (AI models sometimes serialize as string)
  tasks: z.union([z.array(z.union([z.string(), taskItemSchema])), z.string()]).optional(),
  id: z.string().optional(),
  title: z.string().optional(),
  description: z.string().optional(),
  status: z.enum(['pending', 'in_progress', 'completed', 'cancelled']).optional(),
  priority: z.enum(['low', 'medium', 'high', 'critical']).optional(),
  dependencies: z.array(z.string()).optional(),
  after: z.string().optional()
});

/**
 * Task tool definition (legacy export, no longer used — tool is registered natively via taskSchema)
 */
export const taskToolDefinition = '';

/**
 * Task system prompt addition - guidance for AI on when and how to use tasks
 */
export const taskSystemPrompt = `[Task Management]

Use the task tool to track progress on complex requests with multiple distinct goals.

## When to Use Tasks

CREATE tasks when the request has **multiple separate deliverables**:
- "Fix bug A AND add feature B" → two tasks
- "Investigate auth, payments, AND notifications" → three tasks
- "Implement X, then add tests, then update docs" → three sequential tasks with dependencies

SKIP tasks for single-goal requests, even complex ones:
- "How does ranking work?" — just investigate and answer
- "Explain the authentication flow" — just trace and explain
Multiple internal steps (search, read, analyze) for one goal ≠ multiple tasks.

## Granularity

Tasks = logical units of work, not files or steps.
- "Fix 8 similar test files" → ONE task (same fix repeated)
- "Update API + tests + docs" → THREE tasks (different work types)
- Max 5-6 tasks at initial planning. You can always add more as work unfolds.

## Task Chains and Dependencies

Use dependencies to express ordering constraints. A task cannot start until all its dependencies are completed.

**Sequential chain**: tasks that must happen in order:
\`\`\`
tasks: [
  { id: "design", title: "Design the API schema" },
  { id: "implement", title: "Implement endpoints", dependencies: ["design"] },
  { id: "test", title: "Write integration tests", dependencies: ["implement"] }
]
\`\`\`
This creates: design → implement → test. You cannot start "implement" until "design" is completed.

**Fan-out**: multiple tasks that can run after one prerequisite:
\`\`\`
tasks: [
  { id: "setup", title: "Set up database" },
  { id: "auth", title: "Add auth module", dependencies: ["setup"] },
  { id: "api", title: "Add API routes", dependencies: ["setup"] }
]
\`\`\`
Both "auth" and "api" can start after "setup" is done.

**Fan-in**: one task that depends on multiple prerequisites:
\`\`\`
tasks: [
  { id: "auth", title: "Auth module" },
  { id: "api", title: "API routes" },
  { id: "e2e", title: "End-to-end tests", dependencies: ["auth", "api"] }
]
\`\`\`

## Adding Tasks Mid-Work

When new requirements emerge during execution, add tasks dynamically:

**Add a subtask after the current task**: Use \`after\` to insert it in the right position and \`dependencies\` to enforce ordering:
\`\`\`
action: "create", id: "fix-edge-case", title: "Handle null input edge case",
  dependencies: ["implement"], after: "implement"
\`\`\`

**Split a task**: If a task turns out to be bigger than expected, create new subtasks with dependencies, then cancel or complete the original.

**Insert into a chain**: Create a new task with dependencies on the predecessor and update the successor's dependencies:
\`\`\`
// Original chain: design → implement → test
// Insert "review" between implement and test:
action: "create", id: "review", title: "Code review", dependencies: ["implement"], after: "implement"
// Then update test to depend on review instead:
action: "update", id: "test", dependencies: ["review"]
\`\`\`

## Strict Workflow Rules

1. **Plan first**: Call task tool with action="create" and a tasks array before starting any work.
2. **One task at a time**: Set the current task to "in_progress" BEFORE you begin working on it.
3. **Complete IMMEDIATELY**: The moment you finish a task's work, call the task tool with action="complete" for that task RIGHT AWAY — in the same step, not later. Do NOT batch completions. Do NOT wait until the end. Every task completion should happen the instant its work is verified done.
4. **Verify before completing**: Do not mark a task as completed unless you have actually verified the result — code compiles, tests pass, output is correct. "I wrote the code" is not enough; confirm it works.
5. **Respect the chain**: Never work on a task whose dependencies are not yet completed. Check the task list if unsure.
6. **Adapt the plan**: If you discover new work during execution, add new tasks with proper dependencies. Do not silently do extra work without tracking it. If a task turns out to need subtasks, create them as dependent tasks.
7. **Finish clean**: All tasks must be "completed" or "cancelled" before providing your final answer. You cannot finish with pending tasks.

## Rules

- Dependencies are strictly enforced: a task CANNOT start until ALL its dependencies are completed
- Circular dependencies are rejected
- Completion is blocked while tasks remain unresolved
- ALWAYS mark tasks complete immediately — this is critical for progress tracking
`;

/**
 * Task guidance to inject at start of request
 */
export const taskGuidancePrompt = `Does this request have MULTIPLE DISTINCT GOALS?
- "Do A AND B AND C" (multiple goals) → Create tasks for each goal
- "Do X, then Y, then Z" (sequential goals) → Create tasks with dependencies: Y depends on X, Z depends on Y
- "Investigate/explain/find X" (single goal) → Skip tasks, just answer directly
Multiple internal steps for ONE goal = NO tasks needed.
If creating tasks: call the task tool with action="create" and a tasks array FIRST, using dependencies for ordering.
CRITICAL: Complete each task IMMEDIATELY when its work is done. Do not defer completions.`;

/**
 * Create task completion blocked message
 * @param {string} taskSummary - Summary of incomplete tasks
 * @returns {string} Formatted message
 */
export function createTaskCompletionBlockedMessage(taskSummary) {
  return `⚠️ You cannot finish — there are unresolved tasks:

${taskSummary}

You MUST resolve every task before providing your final answer:
- If the work is DONE → action="complete", id="<task-id>" (do this immediately!)
- If it is NOT needed → action="update", id="<task-id>", status="cancelled"
- If it is BLOCKED → complete its dependencies first, then come back to it

Do not provide a final answer until all tasks are completed or cancelled.`;
}

/**
 * Monotonic event sequence counter for deterministic replay ordering.
 * Shared across all task tool instances within the same process.
 */
let _globalSequence = 0;

/**
 * Serialize a task object into a flat telemetry-friendly payload.
 * @param {Object} task - Task from TaskManager
 * @param {number} index - Position in the task list (0-based)
 * @returns {Object} Flat task payload
 */
function serializeTask(task, index) {
  return {
    id: task.id,
    title: task.title,
    status: task.status,
    priority: task.priority || null,
    dependencies: task.dependencies || [],
    after: null, // 'after' is an insertion hint, not stored on the task
    order: index,
  };
}

/**
 * Create task tool instance
 * @param {Object} options - Configuration options
 * @param {import('./TaskManager.js').TaskManager} options.taskManager - TaskManager instance
 * @param {Object} [options.tracer] - Optional tracer for telemetry (SimpleAppTracer with session hierarchy)
 * @param {boolean} [options.debug=false] - Enable debug logging
 * @param {string} [options.delegationTask] - Description of the delegated task (if this is a subagent)
 * @returns {Object} Tool instance with execute function
 */
export function createTaskTool(options = {}) {
  const { taskManager, tracer, debug = false, delegationTask = null } = options;

  if (!taskManager) {
    throw new Error('TaskManager instance is required');
  }

  /**
   * Build the agent scope fields from the tracer's session hierarchy.
   * These fields are included in every emitted task event so consumers
   * can group events by agent/subagent without relying on span ancestry.
   * @returns {Object} Agent scope attributes
   */
  const getAgentScope = () => {
    if (!tracer) return {};
    return {
      'agent.session_id': tracer.sessionId || null,
      'agent.parent_session_id': tracer.parentSessionId || null,
      'agent.root_session_id': tracer.rootSessionId || null,
      'agent.kind': tracer.agentKind || 'main',
      ...(delegationTask ? { 'delegation.task': delegationTask } : {}),
    };
  };

  /**
   * Build global task-list context fields (total count, incomplete remaining).
   * @returns {Object}
   */
  const getListContext = () => {
    const all = taskManager.listTasks();
    const incomplete = taskManager.getIncompleteTasks();
    return {
      'task.total_count': all.length,
      'task.incomplete_remaining': incomplete.length,
    };
  };

  /**
   * Record task telemetry event with agent scope and monotonic sequence.
   * @param {string} eventType - Event type (created, updated, completed, deleted, listed, error)
   * @param {Object} data - Event data
   */
  const recordTaskEvent = (eventType, data = {}) => {
    if (tracer && typeof tracer.recordTaskEvent === 'function') {
      tracer.recordTaskEvent(eventType, {
        'task.sequence': ++_globalSequence,
        ...getAgentScope(),
        ...data
      });
    }
  };

  return {
    name: 'task',
    description: 'Manage tasks for tracking progress during code exploration',
    parameters: taskSchema,

    /**
     * Execute task action
     * @param {Object} params - Tool parameters
     * @returns {string} Result message
     */
    execute: async (params) => {
      try {
        const validation = taskSchema.safeParse(params);
        if (!validation.success) {
          recordTaskEvent('validation_error', {
            'task.error': validation.error.message
          });
          return `Error: Invalid task parameters - ${validation.error.message}`;
        }

        const { action, tasks: rawTasks, id, title, description, status, priority, dependencies, after } = validation.data;

        // Parse tasks if passed as JSON string (common AI model behavior)
        let tasks = rawTasks;
        if (typeof rawTasks === 'string') {
          try {
            tasks = JSON.parse(rawTasks);
          } catch (e) {
            return `Error: Invalid tasks JSON - ${e.message}`;
          }
        }

        switch (action) {
          case 'create': {
            if (tasks && Array.isArray(tasks)) {
              // Batch create
              const created = taskManager.createTasks(tasks);
              const allTasks = taskManager.listTasks();
              const taskIndex = new Map(allTasks.map((t, i) => [t.id, i]));
              recordTaskEvent('batch_created', {
                'task.action': 'create',
                'task.count': created.length,
                'task.items_json': JSON.stringify(created.map(t => serializeTask(t, taskIndex.get(t.id) ?? 0))),
                ...getListContext()
              });
              return `Created ${created.length} tasks: ${created.map(t => t.id).join(', ')}\n\n${taskManager.formatTasksForPrompt()}`;
            } else if (title) {
              // Single create
              const task = taskManager.createTask({ title, description, priority, dependencies, after });
              const allTasks = taskManager.listTasks();
              const order = allTasks.findIndex(t => t.id === task.id);
              recordTaskEvent('created', {
                'task.action': 'create',
                'task.id': task.id,
                'task.title': task.title,
                'task.status': task.status,
                'task.priority': task.priority || null,
                'task.dependencies': JSON.stringify(task.dependencies || []),
                'task.after': after || null,
                'task.order': order,
                ...getListContext()
              });
              return `Created task ${task.id}: ${task.title}\n\n${taskManager.formatTasksForPrompt()}`;
            } else {
              return 'Error: Create action requires either "tasks" array or "title" parameter';
            }
          }

          case 'update': {
            if (tasks && Array.isArray(tasks)) {
              // Batch update
              const updated = taskManager.updateTasks(tasks);
              const allTasks = taskManager.listTasks();
              const taskIndex = new Map(allTasks.map((t, i) => [t.id, i]));
              recordTaskEvent('batch_updated', {
                'task.action': 'update',
                'task.count': updated.length,
                'task.items_json': JSON.stringify(updated.map(t => serializeTask(t, taskIndex.get(t.id) ?? 0))),
                ...getListContext()
              });
              return `Updated ${updated.length} tasks: ${updated.map(t => t.id).join(', ')}\n\n${taskManager.formatTasksForPrompt()}`;
            } else if (id) {
              // Single update
              const updates = {};
              if (status) updates.status = status;
              if (title) updates.title = title;
              if (description) updates.description = description;
              if (priority) updates.priority = priority;
              if (dependencies) updates.dependencies = dependencies;

              const task = taskManager.updateTask(id, updates);
              const allTasks = taskManager.listTasks();
              const order = allTasks.findIndex(t => t.id === task.id);
              recordTaskEvent('updated', {
                'task.action': 'update',
                'task.id': task.id,
                'task.title': task.title,
                'task.status': task.status,
                'task.priority': task.priority || null,
                'task.dependencies': JSON.stringify(task.dependencies || []),
                'task.order': order,
                'task.fields_updated': Object.keys(updates).join(', '),
                ...getListContext()
              });
              return `Updated task ${task.id}\n\n${taskManager.formatTasksForPrompt()}`;
            } else {
              return 'Error: Update action requires either "tasks" array or "id" parameter';
            }
          }

          case 'complete': {
            if (tasks && Array.isArray(tasks)) {
              // Batch complete - validate each item has an id
              const ids = tasks.map((t, index) => {
                if (typeof t === 'string') return t;
                if (t && typeof t.id === 'string') return t.id;
                throw new Error(`Invalid task item at index ${index}: must be a string ID or object with 'id' property`);
              });
              const completed = taskManager.completeTasks(ids);
              const allTasks = taskManager.listTasks();
              const taskIndex = new Map(allTasks.map((t, i) => [t.id, i]));
              recordTaskEvent('batch_completed', {
                'task.action': 'complete',
                'task.count': completed.length,
                'task.items_json': JSON.stringify(completed.map(t => serializeTask(t, taskIndex.get(t.id) ?? 0))),
                ...getListContext()
              });
              return `Completed ${completed.length} tasks\n\n${taskManager.formatTasksForPrompt()}`;
            } else if (id) {
              // Single complete
              const task = taskManager.completeTask(id);
              const allTasks = taskManager.listTasks();
              const order = allTasks.findIndex(t => t.id === task.id);
              recordTaskEvent('completed', {
                'task.action': 'complete',
                'task.id': task.id,
                'task.title': task.title,
                'task.status': task.status,
                'task.priority': task.priority || null,
                'task.dependencies': JSON.stringify(task.dependencies || []),
                'task.order': order,
                ...getListContext()
              });
              return `Completed task ${task.id}: ${task.title}\n\n${taskManager.formatTasksForPrompt()}`;
            } else {
              return 'Error: Complete action requires either "tasks" array or "id" parameter';
            }
          }

          case 'delete': {
            if (tasks && Array.isArray(tasks)) {
              // Batch delete - validate each item has an id
              const ids = tasks.map((t, index) => {
                if (typeof t === 'string') return t;
                if (t && typeof t.id === 'string') return t.id;
                throw new Error(`Invalid task item at index ${index}: must be a string ID or object with 'id' property`);
              });
              // Capture task data before deletion for the event
              const tasksBefore = ids.map(tid => taskManager.getTask(tid)).filter(Boolean);
              const deleted = taskManager.deleteTasks(ids);
              recordTaskEvent('batch_deleted', {
                'task.action': 'delete',
                'task.count': deleted.length,
                'task.items_json': JSON.stringify(tasksBefore.map((t, i) => ({ id: t.id, title: t.title, status: t.status }))),
                ...getListContext()
              });
              return `Deleted ${deleted.length} tasks: ${deleted.join(', ')}\n\n${taskManager.formatTasksForPrompt()}`;
            } else if (id) {
              // Capture task data before deletion
              const taskBefore = taskManager.getTask(id);
              taskManager.deleteTask(id);
              recordTaskEvent('deleted', {
                'task.action': 'delete',
                'task.id': id,
                'task.title': taskBefore?.title || null,
                'task.status': taskBefore?.status || null,
                ...getListContext()
              });
              return `Deleted task ${id}\n\n${taskManager.formatTasksForPrompt()}`;
            } else {
              return 'Error: Delete action requires either "tasks" array or "id" parameter';
            }
          }

          case 'list': {
            const allTasks = taskManager.listTasks();
            const incomplete = taskManager.getIncompleteTasks();
            recordTaskEvent('listed', {
              'task.action': 'list',
              'task.total_count': allTasks.length,
              'task.incomplete_count': incomplete.length,
              'task.completed_count': allTasks.length - incomplete.length,
              'task.items_json': JSON.stringify(allTasks.map((t, i) => serializeTask(t, i)))
            });
            return taskManager.formatTasksForPrompt();
          }

          default:
            recordTaskEvent('unknown_action', {
              'task.action': action
            });
            return `Error: Unknown action "${action}". Valid actions: create, update, complete, delete, list`;
        }
      } catch (error) {
        recordTaskEvent('error', {
          'task.error': error.message,
          'task.action': params?.action || 'unknown'
        });
        if (debug) {
          console.error('[TaskTool] Error:', error);
        }
        return `Error: ${error.message}`;
      }
    }
  };
}

export default createTaskTool;
