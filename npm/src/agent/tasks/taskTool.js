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
- "Implement X, then add tests, then update docs" → three sequential tasks

SKIP tasks for single-goal requests, even complex ones:
- "How does ranking work?" — just investigate and answer
- "Explain the authentication flow" — just trace and explain
Multiple internal steps (search, read, analyze) for one goal ≠ multiple tasks.

## Granularity

Tasks = logical units of work, not files or steps.
- "Fix 8 similar test files" → ONE task (same fix repeated)
- "Update API + tests + docs" → THREE tasks (different work types)
- Max 3–4 tasks. More means you're too granular.

## Workflow

1. **Plan**: Call task tool with action="create" and a tasks array up front
2. **Execute**: Update status to "in_progress" / "completed" as you work. Add, split, or cancel tasks as you learn more.
3. **Finish**: All tasks must be "completed" or "cancelled" before calling attempt_completion.

## Rules

- Dependencies are enforced: a task cannot start until its dependencies are completed
- Circular dependencies are rejected
- attempt_completion is blocked while tasks remain unresolved
`;

/**
 * Task guidance to inject at start of request
 */
export const taskGuidancePrompt = `Does this request have MULTIPLE DISTINCT GOALS?
- "Do A AND B AND C" (multiple goals) → Create tasks for each goal
- "Investigate/explain/find X" (single goal) → Skip tasks, just answer directly
Multiple internal steps for ONE goal = NO tasks needed.
If creating tasks, use the task tool with action="create" first.`;

/**
 * Create task completion blocked message
 * @param {string} taskSummary - Summary of incomplete tasks
 * @returns {string} Formatted message
 */
export function createTaskCompletionBlockedMessage(taskSummary) {
  return `You cannot complete yet. The following tasks are still unresolved:

${taskSummary}

For each pending/in_progress task, either:
- Complete it: call task tool with action="complete", id="task-X"
- Cancel it: call task tool with action="update", id="task-X", status="cancelled"

After all tasks are resolved, call attempt_completion again.`;
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
