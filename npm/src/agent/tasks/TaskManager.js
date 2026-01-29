/**
 * TaskManager - Manages tasks for tracking agent progress
 * @module agent/tasks/TaskManager
 */

/**
 * @typedef {Object} Task
 * @property {string} id - Unique task identifier (e.g., "task-1")
 * @property {string} title - Short task description
 * @property {string} [description] - Detailed task description
 * @property {'pending'|'in_progress'|'completed'|'cancelled'} status - Current task status
 * @property {'low'|'medium'|'high'|'critical'} [priority] - Task priority
 * @property {string[]} dependencies - Array of task IDs that must complete first
 * @property {string} createdAt - ISO timestamp of creation
 * @property {string} updatedAt - ISO timestamp of last update
 * @property {string} [completedAt] - ISO timestamp when completed
 */

/**
 * TaskManager class for managing tasks within a ProbeAgent session
 */
export class TaskManager {
  /**
   * Create a new TaskManager instance
   * @param {Object} [options] - Configuration options
   * @param {boolean} [options.debug=false] - Enable debug logging
   */
  constructor(options = {}) {
    /** @type {Map<string, Task>} */
    this.tasks = new Map();
    this.taskCounter = 0;
    this.debug = options.debug || false;
  }

  /**
   * Generate the next task ID
   * @returns {string} New task ID
   * @private
   */
  _generateId() {
    this.taskCounter++;
    return `task-${this.taskCounter}`;
  }

  /**
   * Get current timestamp in ISO format
   * @returns {string} ISO timestamp
   * @private
   */
  _now() {
    return new Date().toISOString();
  }

  /**
   * Create a single task
   * @param {Object} taskData - Task data
   * @param {string} taskData.title - Task title
   * @param {string} [taskData.description] - Task description
   * @param {'low'|'medium'|'high'|'critical'} [taskData.priority] - Task priority
   * @param {string[]} [taskData.dependencies] - Task IDs this task depends on
   * @param {string} [taskData.after] - Task ID to insert this task after (for ordering)
   * @returns {Task} Created task
   * @throws {Error} If dependencies are invalid or create a cycle
   */
  createTask(taskData) {
    const id = this._generateId();
    const now = this._now();

    // Validate dependencies exist
    const dependencies = taskData.dependencies || [];
    for (const depId of dependencies) {
      if (!this.tasks.has(depId)) {
        throw new Error(`Dependency "${depId}" does not exist. Available tasks: ${this._getAvailableTaskIds()}`);
      }
    }

    // Validate 'after' task exists if specified
    const afterTaskId = taskData.after;
    if (afterTaskId && !this.tasks.has(afterTaskId)) {
      throw new Error(`Task "${afterTaskId}" does not exist. Cannot insert after non-existent task. Available tasks: ${this._getAvailableTaskIds()}`);
    }

    // Check for circular dependencies
    if (dependencies.length > 0 && !this._validateNoCycle(id, dependencies)) {
      throw new Error(`Adding dependencies [${dependencies.join(', ')}] to "${id}" would create a circular dependency`);
    }

    const task = {
      id,
      title: taskData.title,
      description: taskData.description || null,
      status: 'pending',
      priority: taskData.priority || null,
      dependencies,
      createdAt: now,
      updatedAt: now,
      completedAt: null
    };

    // Insert task at the correct position
    if (afterTaskId) {
      this._insertAfter(afterTaskId, id, task);
    } else {
      this.tasks.set(id, task);
    }

    if (this.debug) {
      console.log(`[TaskManager] Created task: ${id} - ${task.title}${afterTaskId ? ` (after ${afterTaskId})` : ''}`);
    }

    return task;
  }

  /**
   * Insert a task after a specific task in the Map order
   * @param {string} afterId - Task ID to insert after
   * @param {string} newId - New task ID
   * @param {Task} newTask - New task object
   * @private
   */
  _insertAfter(afterId, newId, newTask) {
    const newTasks = new Map();

    for (const [id, task] of this.tasks) {
      newTasks.set(id, task);
      if (id === afterId) {
        newTasks.set(newId, newTask);
      }
    }

    this.tasks = newTasks;
  }

  /**
   * Create multiple tasks in batch
   * @param {Object[]} tasksData - Array of task data objects
   * @returns {Task[]} Created tasks
   */
  createTasks(tasksData) {
    const createdTasks = [];

    for (const taskData of tasksData) {
      const task = this.createTask(taskData);
      createdTasks.push(task);
    }

    return createdTasks;
  }

  /**
   * Get a task by ID
   * @param {string} id - Task ID
   * @returns {Task|null} Task or null if not found
   */
  getTask(id) {
    return this.tasks.get(id) || null;
  }

  /**
   * Update a task
   * @param {string} id - Task ID
   * @param {Object} updates - Fields to update
   * @returns {Task} Updated task
   * @throws {Error} If task not found or update is invalid
   */
  updateTask(id, updates) {
    const task = this.tasks.get(id);
    if (!task) {
      throw new Error(`Task "${id}" not found. Available tasks: ${this._getAvailableTaskIds()}`);
    }

    // Handle status updates
    if (updates.status) {
      if (updates.status === 'completed' && !task.completedAt) {
        updates.completedAt = this._now();
      }
    }

    // Handle dependency updates
    if (updates.dependencies) {
      // Validate new dependencies exist
      for (const depId of updates.dependencies) {
        if (!this.tasks.has(depId)) {
          throw new Error(`Dependency "${depId}" does not exist. Available tasks: ${this._getAvailableTaskIds()}`);
        }
      }

      // Check for circular dependencies
      if (!this._validateNoCycle(id, updates.dependencies)) {
        throw new Error(`Adding dependencies [${updates.dependencies.join(', ')}] to "${id}" would create a circular dependency`);
      }
    }

    // Apply updates
    const updatedTask = {
      ...task,
      ...updates,
      id, // Ensure ID cannot be changed
      createdAt: task.createdAt, // Ensure createdAt cannot be changed
      updatedAt: this._now()
    };

    this.tasks.set(id, updatedTask);

    if (this.debug) {
      console.log(`[TaskManager] Updated task: ${id}`, updates);
    }

    return updatedTask;
  }

  /**
   * Update multiple tasks in batch
   * @param {Object[]} updates - Array of {id, ...updates} objects
   * @returns {Task[]} Updated tasks
   */
  updateTasks(updates) {
    const updatedTasks = [];

    for (const update of updates) {
      const { id, ...taskUpdates } = update;
      const task = this.updateTask(id, taskUpdates);
      updatedTasks.push(task);
    }

    return updatedTasks;
  }

  /**
   * Delete a task
   * @param {string} id - Task ID
   * @returns {boolean} True if deleted
   * @throws {Error} If task has dependents
   */
  deleteTask(id) {
    const task = this.tasks.get(id);
    if (!task) {
      throw new Error(`Task "${id}" not found. Available tasks: ${this._getAvailableTaskIds()}`);
    }

    // Check if other tasks depend on this one
    const dependents = this._getDependents(id);
    if (dependents.length > 0) {
      throw new Error(`Cannot delete "${id}" - other tasks depend on it: ${dependents.join(', ')}`);
    }

    this.tasks.delete(id);

    if (this.debug) {
      console.log(`[TaskManager] Deleted task: ${id}`);
    }

    return true;
  }

  /**
   * Delete multiple tasks in batch
   * @param {string[]} ids - Task IDs to delete
   * @returns {string[]} Deleted task IDs
   */
  deleteTasks(ids) {
    const deletedIds = [];

    for (const id of ids) {
      this.deleteTask(id);
      deletedIds.push(id);
    }

    return deletedIds;
  }

  /**
   * Mark a task as completed
   * @param {string} id - Task ID
   * @returns {Task} Updated task
   */
  completeTask(id) {
    return this.updateTask(id, { status: 'completed' });
  }

  /**
   * Mark multiple tasks as completed
   * @param {string[]} ids - Task IDs
   * @returns {Task[]} Updated tasks
   */
  completeTasks(ids) {
    return ids.map(id => this.completeTask(id));
  }

  /**
   * List all tasks
   * @param {Object} [filter] - Optional filter
   * @param {'pending'|'in_progress'|'completed'|'cancelled'} [filter.status] - Filter by status
   * @returns {Task[]} Array of tasks
   */
  listTasks(filter = {}) {
    let tasks = Array.from(this.tasks.values());

    if (filter.status) {
      tasks = tasks.filter(t => t.status === filter.status);
    }

    return tasks;
  }

  /**
   * Check if there are any incomplete tasks (pending or in_progress)
   * @returns {boolean} True if there are incomplete tasks
   */
  hasIncompleteTasks() {
    for (const task of this.tasks.values()) {
      if (task.status === 'pending' || task.status === 'in_progress') {
        return true;
      }
    }
    return false;
  }

  /**
   * Get incomplete tasks (pending or in_progress)
   * @returns {Task[]} Array of incomplete tasks
   */
  getIncompleteTasks() {
    return Array.from(this.tasks.values()).filter(
      t => t.status === 'pending' || t.status === 'in_progress'
    );
  }

  /**
   * Get tasks that are ready to start (all dependencies completed)
   * @returns {Task[]} Array of ready tasks
   */
  getReadyTasks() {
    return Array.from(this.tasks.values()).filter(task => {
      if (task.status !== 'pending') return false;

      // Check all dependencies are completed or cancelled
      for (const depId of task.dependencies) {
        const dep = this.tasks.get(depId);
        if (dep && dep.status !== 'completed' && dep.status !== 'cancelled') {
          return false;
        }
      }
      return true;
    });
  }

  /**
   * Get blocked tasks (have incomplete dependencies)
   * @returns {Task[]} Array of blocked tasks
   */
  getBlockedTasks() {
    return Array.from(this.tasks.values()).filter(task => {
      if (task.status !== 'pending') return false;

      for (const depId of task.dependencies) {
        const dep = this.tasks.get(depId);
        if (dep && dep.status !== 'completed' && dep.status !== 'cancelled') {
          return true;
        }
      }
      return false;
    });
  }

  /**
   * Get human-readable task summary for checkpoint messages
   * @returns {string} Formatted task summary
   */
  getTaskSummary() {
    const tasks = this.listTasks();
    if (tasks.length === 0) {
      return 'No tasks created.';
    }

    const lines = ['Tasks:'];
    for (const task of tasks) {
      let line = `- [${task.status}] ${task.id}: ${task.title}`;

      // Add blocking info for pending tasks
      if (task.status === 'pending' && task.dependencies.length > 0) {
        const blockers = task.dependencies.filter(depId => {
          const dep = this.tasks.get(depId);
          return dep && dep.status !== 'completed' && dep.status !== 'cancelled';
        });
        if (blockers.length > 0) {
          line += ` (blocked by: ${blockers.join(', ')})`;
        }
      }

      lines.push(line);
    }

    return lines.join('\n');
  }

  /**
   * Format tasks for inclusion in AI prompts
   * @returns {string} XML-formatted task list
   */
  formatTasksForPrompt() {
    const tasks = this.listTasks();
    if (tasks.length === 0) {
      return '<task_status>No tasks created.</task_status>';
    }

    const taskLines = tasks.map(task => {
      const blockers = task.dependencies.filter(depId => {
        const dep = this.tasks.get(depId);
        return dep && dep.status !== 'completed' && dep.status !== 'cancelled';
      });

      let line = `  <task id="${task.id}" status="${task.status}"`;
      if (task.priority) line += ` priority="${task.priority}"`;
      if (blockers.length > 0) line += ` blocked_by="${blockers.join(',')}"`;
      line += `>${task.title}</task>`;

      return line;
    });

    return `<task_status>\n${taskLines.join('\n')}\n</task_status>`;
  }

  /**
   * Clear all tasks
   */
  clear() {
    this.tasks.clear();
    this.taskCounter = 0;

    if (this.debug) {
      console.log('[TaskManager] Cleared all tasks');
    }
  }

  /**
   * Export tasks for persistence
   * @returns {Object} Serializable task data
   */
  export() {
    return {
      tasks: Array.from(this.tasks.entries()),
      taskCounter: this.taskCounter
    };
  }

  /**
   * Import tasks from exported data
   * @param {Object} data - Exported task data
   */
  import(data) {
    this.tasks = new Map(data.tasks);
    this.taskCounter = data.taskCounter;
  }

  /**
   * Get list of available task IDs for error messages
   * @returns {string} Comma-separated list of task IDs
   * @private
   */
  _getAvailableTaskIds() {
    const ids = Array.from(this.tasks.keys());
    return ids.length > 0 ? ids.join(', ') : '(none)';
  }

  /**
   * Get tasks that depend on a given task
   * @param {string} taskId - Task ID
   * @returns {string[]} Array of dependent task IDs
   * @private
   */
  _getDependents(taskId) {
    const dependents = [];
    for (const [id, task] of this.tasks) {
      if (task.dependencies.includes(taskId)) {
        dependents.push(id);
      }
    }
    return dependents;
  }

  /**
   * Validate that adding dependencies won't create a cycle
   * Uses DFS to detect cycles
   * @param {string} taskId - Task being updated
   * @param {string[]} newDependencies - New dependencies to add
   * @returns {boolean} True if no cycle would be created
   * @private
   */
  _validateNoCycle(taskId, newDependencies) {
    // Build a temporary dependency graph including the new dependencies
    const graph = new Map();

    for (const [id, task] of this.tasks) {
      graph.set(id, [...task.dependencies]);
    }

    // Add or update the target task's dependencies
    graph.set(taskId, newDependencies);

    // DFS to detect cycles
    const visited = new Set();
    const recursionStack = new Set();

    const hasCycle = (nodeId) => {
      if (recursionStack.has(nodeId)) {
        return true; // Found a cycle
      }
      if (visited.has(nodeId)) {
        return false; // Already fully explored
      }

      visited.add(nodeId);
      recursionStack.add(nodeId);

      const deps = graph.get(nodeId) || [];
      for (const depId of deps) {
        if (hasCycle(depId)) {
          return true;
        }
      }

      recursionStack.delete(nodeId);
      return false;
    };

    // Check from the task being modified
    return !hasCycle(taskId);
  }
}

export default TaskManager;
