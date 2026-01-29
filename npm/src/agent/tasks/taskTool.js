/**
 * Task Tool - XML tool definition and executor for task management
 * @module agent/tasks/taskTool
 */

import { z } from 'zod';

/**
 * Schema for a single task item in batch operations
 */
export const taskItemSchema = z.object({
  id: z.string().optional(),
  title: z.string().optional(),
  description: z.string().optional(),
  status: z.enum(['pending', 'in_progress', 'completed', 'cancelled']).optional(),
  priority: z.enum(['low', 'medium', 'high', 'critical']).optional(),
  dependencies: z.array(z.string()).optional(),
  after: z.string().optional()
});

/**
 * Task schema for validation
 */
export const taskSchema = z.object({
  action: z.enum(['create', 'update', 'complete', 'delete', 'list']),
  tasks: z.array(z.union([z.string(), taskItemSchema])).optional(),
  id: z.string().optional(),
  title: z.string().optional(),
  description: z.string().optional(),
  status: z.enum(['pending', 'in_progress', 'completed', 'cancelled']).optional(),
  priority: z.enum(['low', 'medium', 'high', 'critical']).optional(),
  dependencies: z.array(z.string()).optional(),
  after: z.string().optional()
});

/**
 * Task tool XML definition for system prompt
 */
export const taskToolDefinition = `## task
Manage tasks for tracking progress during code exploration and problem-solving. Create tasks to break down complex problems, track dependencies, and ensure all work is completed.

Parameters:
- action: (required) The action to perform: create, update, complete, delete, list
- tasks: (optional) JSON array for batch operations - alternative to single-task params
- id: (optional) Task ID for single operations (e.g., "task-1")
- title: (optional) Task title for create/update
- description: (optional) Task description for create/update
- status: (optional) Task status for update: pending, in_progress, completed, cancelled
- priority: (optional) Task priority: low, medium, high, critical
- dependencies: (optional) JSON array of task IDs that must be completed first
- after: (optional) Task ID to insert the new task after (for ordering). By default, new tasks are appended to the end

Usage Examples:

Creating a single task:
<task>
<action>create</action>
<title>Analyze authentication module</title>
<description>Search and understand how authentication works</description>
<priority>high</priority>
</task>

Creating multiple tasks with dependencies:
<task>
<action>create</action>
<tasks>[
  {"title": "Search for user model", "priority": "high"},
  {"title": "Analyze authentication flow", "dependencies": ["task-1"]},
  {"title": "Review session management", "dependencies": ["task-2"]}
]</tasks>
</task>

Inserting a task after a specific task (instead of appending to end):
<task>
<action>create</action>
<title>Investigate error handling</title>
<after>task-2</after>
</task>

Updating a task status:
<task>
<action>update</action>
<id>task-1</id>
<status>in_progress</status>
</task>

Batch updating multiple tasks:
<task>
<action>update</action>
<tasks>[
  {"id": "task-1", "status": "completed"},
  {"id": "task-2", "status": "in_progress"}
]</tasks>
</task>

Completing a task:
<task>
<action>complete</action>
<id>task-1</id>
</task>

Cancelling a task:
<task>
<action>update</action>
<id>task-1</id>
<status>cancelled</status>
</task>

Deleting a task:
<task>
<action>delete</action>
<id>task-1</id>
</task>

Listing all tasks:
<task>
<action>list</action>
</task>
`;

/**
 * Task system prompt addition - comprehensive guidance for AI
 */
export const taskSystemPrompt = `[Task Management System]

You have access to a task tracking tool to organize your work on complex requests.

## When to Create Tasks

CREATE TASKS when the request has **multiple distinct deliverables or goals**:
- "Fix bug A AND add feature B" → Two separate tasks
- "Investigate auth, payments, AND notifications" → Three independent areas
- "Implement X, then add tests, then update docs" → Sequential phases with different outputs
- User explicitly asks for a plan or task breakdown

SKIP TASKS for single-goal requests, even if they require multiple searches:
- "How does ranking work?" → Just investigate and answer (one goal)
- "What does function X do?" → Just look it up (one goal)
- "Explain the authentication flow" → Just trace and explain (one goal)
- "Find where errors are logged" → Just search and report (one goal)

**Key insight**: Multiple *internal steps* (search, read, analyze) are NOT the same as multiple *goals*.
A single investigation with many steps is still ONE task, not many.

MODIFY TASKS when (during execution):
- You discover the problem is more complex than expected → Add new tasks
- A single task covers too much scope → Split into smaller tasks
- You find related work that needs attention → Add dependent tasks
- A task becomes irrelevant based on findings → Cancel it
- Task priorities change based on discoveries → Update priority
- You learn new context → Update task description

## Task Workflow

**STEP 1 - Plan (at start):**
Analyze the request and create tasks for each logical step:

<task>
<action>create</action>
<tasks>[
  {"title": "Search for authentication module", "priority": "high"},
  {"title": "Analyze login flow implementation", "dependencies": ["task-1"]},
  {"title": "Find session management code", "dependencies": ["task-1"]},
  {"title": "Summarize authentication architecture", "dependencies": ["task-2", "task-3"]}
]</tasks>
</task>

**STEP 2 - Execute (during work):**
Update task status as you work:

<task>
<action>update</action>
<id>task-1</id>
<status>in_progress</status>
</task>

... do the work (search, extract, etc.) ...

<task>
<action>complete</action>
<id>task-1</id>
</task>

**STEP 2b - Adapt (when you discover new work):**
As you work, you may discover that:
- A task is more complex than expected → Split it into subtasks
- New areas need investigation → Add new tasks
- Some tasks are no longer needed → Cancel them
- Task order should change → Update dependencies

*Adding a new task when you discover more work:*
<task>
<action>create</action>
<title>Investigate caching layer</title>
<description>Found references to Redis caching in auth module</description>
</task>

*Inserting a task after a specific task (to maintain logical order):*
<task>
<action>create</action>
<title>Check rate limiting</title>
<after>task-2</after>
</task>

*Cancelling and splitting a complex task:*
<task>
<action>update</action>
<id>task-3</id>
<status>cancelled</status>
</task>
<task>
<action>create</action>
<tasks>[
  {"title": "Review JWT token generation", "priority": "high"},
  {"title": "Review token refresh logic"}
]</tasks>
</task>

**STEP 3 - Finish (before completion):**
Before calling attempt_completion, ensure ALL tasks are either:
- \`completed\` - you finished the work
- \`cancelled\` - no longer needed

If you created tasks, you MUST resolve them all before completing.

## Key Rules

1. **Dependencies are enforced**: A task cannot start until its dependencies are completed
2. **Circular dependencies are rejected**: task-1 → task-2 → task-1 is invalid
3. **Completion is blocked**: attempt_completion will fail if tasks remain unresolved
4. **List to review**: Use <task><action>list</action></task> to see current task status
5. **Tasks are living documents**: Add, split, or cancel tasks as you learn more about the problem
`;

/**
 * Task guidance to inject at start of request
 */
export const taskGuidancePrompt = `<task_guidance>
Does this request have MULTIPLE DISTINCT GOALS?
- "Do A AND B AND C" (multiple goals) → Create tasks for each goal
- "Investigate/explain/find X" (single goal) → Skip tasks, just answer directly

Multiple internal steps (search, read, analyze) for ONE goal = NO tasks needed.
Only create tasks when there are separate deliverables the user is asking for.

If creating tasks, use the task tool with action="create" first.
</task_guidance>`;

/**
 * Create task completion blocked message
 * @param {string} taskSummary - Summary of incomplete tasks
 * @returns {string} Formatted message
 */
export function createTaskCompletionBlockedMessage(taskSummary) {
  return `<task_completion_blocked>
You cannot complete yet. The following tasks are still unresolved:

${taskSummary}

Required action:
1. For each "pending" or "in_progress" task, either:
   - Complete the work and mark it: <task><action>complete</action><id>task-X</id></task>
   - Or cancel if no longer needed: <task><action>update</action><id>task-X</id><status>cancelled</status></task>

2. After ALL tasks are resolved (completed or cancelled), call attempt_completion again.

Use <task><action>list</action></task> to review current status.
</task_completion_blocked>`;
}

/**
 * Create task tool instance
 * @param {Object} options - Configuration options
 * @param {import('./TaskManager.js').TaskManager} options.taskManager - TaskManager instance
 * @param {Object} [options.tracer] - Optional tracer for telemetry
 * @param {boolean} [options.debug=false] - Enable debug logging
 * @returns {Object} Tool instance with execute function
 */
export function createTaskTool(options = {}) {
  const { taskManager, tracer, debug = false } = options;

  if (!taskManager) {
    throw new Error('TaskManager instance is required');
  }

  /**
   * Record task telemetry event
   * @param {string} eventType - Event type (created, updated, completed, deleted, listed, error)
   * @param {Object} data - Event data
   */
  const recordTaskEvent = (eventType, data = {}) => {
    if (tracer && typeof tracer.recordTaskEvent === 'function') {
      tracer.recordTaskEvent(eventType, data);
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

        const { action, tasks, id, title, description, status, priority, dependencies, after } = validation.data;

        switch (action) {
          case 'create': {
            if (tasks && Array.isArray(tasks)) {
              // Batch create
              const created = taskManager.createTasks(tasks);
              const ids = created.map(t => t.id).join(', ');
              recordTaskEvent('batch_created', {
                'task.action': 'create',
                'task.count': created.length,
                'task.ids': ids,
                'task.total_count': taskManager.listTasks().length
              });
              return `Created ${created.length} tasks: ${ids}\n\n${taskManager.formatTasksForPrompt()}`;
            } else if (title) {
              // Single create
              const task = taskManager.createTask({ title, description, priority, dependencies, after });
              recordTaskEvent('created', {
                'task.action': 'create',
                'task.id': task.id,
                'task.title': title,
                'task.priority': priority || 'none',
                'task.has_dependencies': dependencies && dependencies.length > 0,
                'task.after': after || 'none',
                'task.total_count': taskManager.listTasks().length
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
              const ids = updated.map(t => t.id).join(', ');
              recordTaskEvent('batch_updated', {
                'task.action': 'update',
                'task.count': updated.length,
                'task.ids': ids
              });
              return `Updated ${updated.length} tasks: ${ids}\n\n${taskManager.formatTasksForPrompt()}`;
            } else if (id) {
              // Single update
              const updates = {};
              if (status) updates.status = status;
              if (title) updates.title = title;
              if (description) updates.description = description;
              if (priority) updates.priority = priority;
              if (dependencies) updates.dependencies = dependencies;

              const task = taskManager.updateTask(id, updates);
              recordTaskEvent('updated', {
                'task.action': 'update',
                'task.id': id,
                'task.new_status': status || 'unchanged',
                'task.fields_updated': Object.keys(updates).join(', ')
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
              recordTaskEvent('batch_completed', {
                'task.action': 'complete',
                'task.count': completed.length,
                'task.ids': ids.join(', '),
                'task.incomplete_remaining': taskManager.getIncompleteTasks().length
              });
              return `Completed ${completed.length} tasks\n\n${taskManager.formatTasksForPrompt()}`;
            } else if (id) {
              // Single complete
              const task = taskManager.completeTask(id);
              recordTaskEvent('completed', {
                'task.action': 'complete',
                'task.id': id,
                'task.title': task.title,
                'task.incomplete_remaining': taskManager.getIncompleteTasks().length
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
              const deleted = taskManager.deleteTasks(ids);
              recordTaskEvent('batch_deleted', {
                'task.action': 'delete',
                'task.count': deleted.length,
                'task.ids': deleted.join(', '),
                'task.total_count': taskManager.listTasks().length
              });
              return `Deleted ${deleted.length} tasks: ${deleted.join(', ')}\n\n${taskManager.formatTasksForPrompt()}`;
            } else if (id) {
              // Single delete
              taskManager.deleteTask(id);
              recordTaskEvent('deleted', {
                'task.action': 'delete',
                'task.id': id,
                'task.total_count': taskManager.listTasks().length
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
              'task.completed_count': allTasks.length - incomplete.length
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
