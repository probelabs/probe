/**
 * Tests for TaskManager functionality
 * @module tests/unit/task-manager
 */

import { jest, describe, test, expect, beforeEach } from '@jest/globals';
import { TaskManager } from '../../src/agent/tasks/TaskManager.js';

describe('TaskManager', () => {
  let manager;

  beforeEach(() => {
    manager = new TaskManager({ debug: false });
  });

  describe('createTask', () => {
    test('should create a task with basic fields', () => {
      const task = manager.createTask({ title: 'Test task' });

      expect(task.id).toBe('task-1');
      expect(task.title).toBe('Test task');
      expect(task.status).toBe('pending');
      expect(task.dependencies).toEqual([]);
      expect(task.createdAt).toBeTruthy();
      expect(task.updatedAt).toBeTruthy();
    });

    test('should create tasks with incremental IDs', () => {
      const task1 = manager.createTask({ title: 'Task 1' });
      const task2 = manager.createTask({ title: 'Task 2' });
      const task3 = manager.createTask({ title: 'Task 3' });

      expect(task1.id).toBe('task-1');
      expect(task2.id).toBe('task-2');
      expect(task3.id).toBe('task-3');
    });

    test('should create task with all optional fields', () => {
      const task = manager.createTask({
        title: 'Full task',
        description: 'Detailed description',
        priority: 'high'
      });

      expect(task.title).toBe('Full task');
      expect(task.description).toBe('Detailed description');
      expect(task.priority).toBe('high');
    });

    test('should create task with valid dependencies', () => {
      const task1 = manager.createTask({ title: 'Task 1' });
      const task2 = manager.createTask({
        title: 'Task 2',
        dependencies: ['task-1']
      });

      expect(task2.dependencies).toEqual(['task-1']);
    });

    test('should throw error for non-existent dependency', () => {
      expect(() => {
        manager.createTask({
          title: 'Task 1',
          dependencies: ['non-existent']
        });
      }).toThrow(/does not exist/);
    });
  });

  describe('createTasks (batch)', () => {
    test('should create multiple tasks at once', () => {
      const tasks = manager.createTasks([
        { title: 'Task 1' },
        { title: 'Task 2' },
        { title: 'Task 3' }
      ]);

      expect(tasks).toHaveLength(3);
      expect(tasks[0].id).toBe('task-1');
      expect(tasks[1].id).toBe('task-2');
      expect(tasks[2].id).toBe('task-3');
    });

    test('should create tasks with dependencies to earlier tasks in batch', () => {
      const tasks = manager.createTasks([
        { id: 'first', title: 'Task 1' },
        { id: 'second', title: 'Task 2', dependencies: ['first'] },
        { id: 'third', title: 'Task 3', dependencies: ['first', 'second'] }
      ]);

      expect(tasks[1].dependencies).toEqual(['first']);
      expect(tasks[2].dependencies).toEqual(['first', 'second']);
    });

    test('should preserve user-provided IDs in batch creation', () => {
      const tasks = manager.createTasks([
        { id: 'auth', title: 'Authenticate with API' },
        { id: 'list-projects', title: 'List Projects', dependencies: ['auth'] },
        { id: 'list-clusters', title: 'List Clusters', dependencies: ['list-projects'] }
      ]);

      expect(tasks).toHaveLength(3);
      // User-provided IDs are preserved as-is
      expect(tasks[0].id).toBe('auth');
      expect(tasks[1].id).toBe('list-projects');
      expect(tasks[1].dependencies).toEqual(['auth']);
      expect(tasks[2].id).toBe('list-clusters');
      expect(tasks[2].dependencies).toEqual(['list-projects']);
    });

    test('should allow completing tasks by user-provided ID', () => {
      manager.createTasks([
        { id: 'tui-static-gen', title: 'Generate TUI statics' },
        { id: 'tui-render', title: 'Render TUI', dependencies: ['tui-static-gen'] }
      ]);

      // Should be able to complete using the user-provided ID
      const completed = manager.completeTask('tui-static-gen');
      expect(completed.id).toBe('tui-static-gen');
      expect(completed.status).toBe('completed');

      // Dependent task should now be unblocked
      const render = manager.getTask('tui-render');
      expect(render).toBeTruthy();
    });

    test('should preserve user-provided IDs with multiple dependencies', () => {
      const tasks = manager.createTasks([
        { id: 'setup', title: 'Setup' },
        { id: 'build', title: 'Build', dependencies: ['setup'] },
        { id: 'test', title: 'Test', dependencies: ['setup'] },
        { id: 'deploy', title: 'Deploy', dependencies: ['build', 'test'] }
      ]);

      expect(tasks).toHaveLength(4);
      expect(tasks[3].id).toBe('deploy');
      expect(tasks[3].dependencies).toEqual(['build', 'test']);
    });

    test('should preserve user-provided IDs in "after" parameter', () => {
      const tasks = manager.createTasks([
        { id: 'first', title: 'First task' },
        { id: 'third', title: 'Third task' },
        { id: 'second', title: 'Second task', after: 'first' }
      ]);

      expect(tasks).toHaveLength(3);
      const allTasks = manager.listTasks();
      const taskIds = allTasks.map(t => t.id);
      expect(taskIds).toEqual(['first', 'second', 'third']);
    });

    test('should reject duplicate IDs in batch', () => {
      expect(() => {
        manager.createTasks([
          { id: 'dup', title: 'First' },
          { id: 'dup', title: 'Second' }
        ]);
      }).toThrow(/Duplicate task ID "dup"/);
    });

    test('should reject IDs that collide with existing tasks', () => {
      manager.createTask({ title: 'Existing' }); // creates task-1
      expect(() => {
        manager.createTasks([
          { id: 'task-1', title: 'Collision' }
        ]);
      }).toThrow(/already exists/);
    });

    test('should not create any tasks if batch validation fails', () => {
      expect(() => {
        manager.createTasks([
          { id: 'a', title: 'Task A' },
          { id: 'b', title: 'Task B', dependencies: ['nonexistent'] }
        ]);
      }).toThrow();

      // No tasks should have been created (atomic batch)
      expect(manager.listTasks()).toHaveLength(0);
    });

    test('should require id on all items when batch has dependencies', () => {
      expect(() => {
        manager.createTasks([
          { id: 'setup', title: 'Setup' },
          { title: 'Build (no custom id)' },
          { id: 'deploy', title: 'Deploy', dependencies: ['setup'] }
        ]);
      }).toThrow(/missing required "id" field/);

      expect(manager.listTasks()).toHaveLength(0);
    });

    test('should allow missing IDs in batch without dependencies', () => {
      const tasks = manager.createTasks([
        { title: 'Task A' },
        { title: 'Task B' },
        { title: 'Task C' }
      ]);

      expect(tasks).toHaveLength(3);
    });

    test('should error when batch dependency references unknown user ID', () => {
      expect(() => {
        manager.createTasks([
          { id: 'a', title: 'Task A' },
          { id: 'b', title: 'Task B', dependencies: ['unknown'] }
        ]);
      }).toThrow(/does not exist/);
    });

    test('should error with clear message when dependencies use auto-generated IDs without providing batch IDs', () => {
      expect(() => {
        manager.createTasks([
          { id: 'first', title: 'First task' },
          { id: 'second', title: 'Second task', dependencies: ['task-0'] }
        ]);
      }).toThrow(/does not exist/);

      expect(manager.listTasks()).toHaveLength(0);
    });

    test('should include batch IDs in error message for failed dependency resolution', () => {
      try {
        manager.createTasks([
          { id: 'auth', title: 'Auth' },
          { id: 'fetch', title: 'Fetch', dependencies: ['wrong-id'] }
        ]);
        expect(true).toBe(false); // should not reach here
      } catch (e) {
        expect(e.message).toContain('batch IDs: auth, fetch');
        expect(e.message).toContain('wrong-id');
      }
    });
  });

  describe('getTask', () => {
    test('should return task by ID', () => {
      manager.createTask({ title: 'Test task' });
      const task = manager.getTask('task-1');

      expect(task).not.toBeNull();
      expect(task.title).toBe('Test task');
    });

    test('should return null for non-existent task', () => {
      const task = manager.getTask('non-existent');
      expect(task).toBeNull();
    });
  });

  describe('updateTask', () => {
    test('should update task status', () => {
      manager.createTask({ title: 'Test task' });
      const updated = manager.updateTask('task-1', { status: 'in_progress' });

      expect(updated.status).toBe('in_progress');
    });

    test('should set completedAt when completing task', () => {
      manager.createTask({ title: 'Test task' });
      const updated = manager.updateTask('task-1', { status: 'completed' });

      expect(updated.status).toBe('completed');
      expect(updated.completedAt).toBeTruthy();
    });

    test('should update multiple fields at once', () => {
      manager.createTask({ title: 'Test task' });
      const updated = manager.updateTask('task-1', {
        title: 'Updated title',
        description: 'New description',
        priority: 'critical'
      });

      expect(updated.title).toBe('Updated title');
      expect(updated.description).toBe('New description');
      expect(updated.priority).toBe('critical');
    });

    test('should throw error for non-existent task', () => {
      expect(() => {
        manager.updateTask('non-existent', { status: 'completed' });
      }).toThrow(/not found/);
    });

    test('should update dependencies', () => {
      manager.createTask({ title: 'Task 1' });
      manager.createTask({ title: 'Task 2' });
      manager.createTask({ title: 'Task 3' });

      const updated = manager.updateTask('task-3', {
        dependencies: ['task-1', 'task-2']
      });

      expect(updated.dependencies).toEqual(['task-1', 'task-2']);
    });
  });

  describe('updateTasks (batch)', () => {
    test('should update multiple tasks at once', () => {
      manager.createTask({ title: 'Task 1' });
      manager.createTask({ title: 'Task 2' });

      const updated = manager.updateTasks([
        { id: 'task-1', status: 'completed' },
        { id: 'task-2', status: 'in_progress' }
      ]);

      expect(updated[0].status).toBe('completed');
      expect(updated[1].status).toBe('in_progress');
    });
  });

  describe('completeTask', () => {
    test('should mark task as completed', () => {
      manager.createTask({ title: 'Test task' });
      const completed = manager.completeTask('task-1');

      expect(completed.status).toBe('completed');
      expect(completed.completedAt).toBeTruthy();
    });
  });

  describe('completeTasks (batch)', () => {
    test('should complete multiple tasks', () => {
      manager.createTask({ title: 'Task 1' });
      manager.createTask({ title: 'Task 2' });

      const completed = manager.completeTasks(['task-1', 'task-2']);

      expect(completed[0].status).toBe('completed');
      expect(completed[1].status).toBe('completed');
    });
  });

  describe('deleteTask', () => {
    test('should delete a task', () => {
      manager.createTask({ title: 'Test task' });
      const result = manager.deleteTask('task-1');

      expect(result).toBe(true);
      expect(manager.getTask('task-1')).toBeNull();
    });

    test('should throw error when deleting task with dependents', () => {
      manager.createTask({ title: 'Task 1' });
      manager.createTask({ title: 'Task 2', dependencies: ['task-1'] });

      expect(() => {
        manager.deleteTask('task-1');
      }).toThrow(/depend on it/);
    });

    test('should throw error for non-existent task', () => {
      expect(() => {
        manager.deleteTask('non-existent');
      }).toThrow(/not found/);
    });
  });

  describe('listTasks', () => {
    test('should list all tasks', () => {
      manager.createTask({ title: 'Task 1' });
      manager.createTask({ title: 'Task 2' });
      manager.createTask({ title: 'Task 3' });

      const tasks = manager.listTasks();
      expect(tasks).toHaveLength(3);
    });

    test('should filter by status', () => {
      manager.createTask({ title: 'Task 1' });
      manager.createTask({ title: 'Task 2' });
      manager.completeTask('task-1');

      const completed = manager.listTasks({ status: 'completed' });
      const pending = manager.listTasks({ status: 'pending' });

      expect(completed).toHaveLength(1);
      expect(pending).toHaveLength(1);
    });
  });

  describe('hasIncompleteTasks', () => {
    test('should return false when no tasks exist', () => {
      expect(manager.hasIncompleteTasks()).toBe(false);
    });

    test('should return true when pending tasks exist', () => {
      manager.createTask({ title: 'Test task' });
      expect(manager.hasIncompleteTasks()).toBe(true);
    });

    test('should return true when in_progress tasks exist', () => {
      manager.createTask({ title: 'Test task' });
      manager.updateTask('task-1', { status: 'in_progress' });
      expect(manager.hasIncompleteTasks()).toBe(true);
    });

    test('should return false when all tasks completed', () => {
      manager.createTask({ title: 'Task 1' });
      manager.createTask({ title: 'Task 2' });
      manager.completeTask('task-1');
      manager.completeTask('task-2');
      expect(manager.hasIncompleteTasks()).toBe(false);
    });

    test('should return false when all tasks cancelled', () => {
      manager.createTask({ title: 'Test task' });
      manager.updateTask('task-1', { status: 'cancelled' });
      expect(manager.hasIncompleteTasks()).toBe(false);
    });
  });

  describe('getIncompleteTasks', () => {
    test('should return only incomplete tasks', () => {
      manager.createTask({ title: 'Task 1' });
      manager.createTask({ title: 'Task 2' });
      manager.createTask({ title: 'Task 3' });
      manager.completeTask('task-1');

      const incomplete = manager.getIncompleteTasks();
      expect(incomplete).toHaveLength(2);
      expect(incomplete.map(t => t.id)).toEqual(['task-2', 'task-3']);
    });
  });

  describe('getReadyTasks', () => {
    test('should return tasks with no dependencies', () => {
      manager.createTask({ title: 'Task 1' });
      manager.createTask({ title: 'Task 2', dependencies: ['task-1'] });

      const ready = manager.getReadyTasks();
      expect(ready).toHaveLength(1);
      expect(ready[0].id).toBe('task-1');
    });

    test('should return tasks with completed dependencies', () => {
      manager.createTask({ title: 'Task 1' });
      manager.createTask({ title: 'Task 2', dependencies: ['task-1'] });
      manager.completeTask('task-1');

      const ready = manager.getReadyTasks();
      expect(ready).toHaveLength(1);
      expect(ready[0].id).toBe('task-2');
    });
  });

  describe('getBlockedTasks', () => {
    test('should return tasks with incomplete dependencies', () => {
      manager.createTask({ title: 'Task 1' });
      manager.createTask({ title: 'Task 2', dependencies: ['task-1'] });

      const blocked = manager.getBlockedTasks();
      expect(blocked).toHaveLength(1);
      expect(blocked[0].id).toBe('task-2');
    });

    test('should not return tasks with completed dependencies', () => {
      manager.createTask({ title: 'Task 1' });
      manager.createTask({ title: 'Task 2', dependencies: ['task-1'] });
      manager.completeTask('task-1');

      const blocked = manager.getBlockedTasks();
      expect(blocked).toHaveLength(0);
    });
  });

  describe('getTaskSummary', () => {
    test('should return formatted summary', () => {
      manager.createTask({ title: 'Task 1' });
      manager.createTask({ title: 'Task 2', dependencies: ['task-1'] });
      manager.updateTask('task-1', { status: 'in_progress' });

      const summary = manager.getTaskSummary();

      expect(summary).toContain('[in_progress] task-1');
      expect(summary).toContain('[pending] task-2');
      expect(summary).toContain('blocked by: task-1');
    });

    test('should return message when no tasks', () => {
      const summary = manager.getTaskSummary();
      expect(summary).toBe('No tasks created.');
    });
  });

  describe('formatTasksForPrompt', () => {
    test('should return XML formatted task list', () => {
      manager.createTask({ title: 'Task 1', priority: 'high' });

      const formatted = manager.formatTasksForPrompt();

      expect(formatted).toContain('<task_status>');
      expect(formatted).toContain('</task_status>');
      expect(formatted).toContain('id="task-1"');
      expect(formatted).toContain('status="pending"');
      expect(formatted).toContain('priority="high"');
    });
  });

  describe('clear', () => {
    test('should remove all tasks', () => {
      manager.createTask({ title: 'Task 1' });
      manager.createTask({ title: 'Task 2' });

      manager.clear();

      expect(manager.listTasks()).toHaveLength(0);
    });

    test('should reset task counter', () => {
      manager.createTask({ title: 'Task 1' });
      manager.clear();
      const task = manager.createTask({ title: 'New task' });

      expect(task.id).toBe('task-1');
    });
  });

  describe('export/import', () => {
    test('should export and import tasks', () => {
      manager.createTask({ title: 'Task 1' });
      manager.createTask({ title: 'Task 2' });

      const exported = manager.export();

      const newManager = new TaskManager();
      newManager.import(exported);

      expect(newManager.listTasks()).toHaveLength(2);
      expect(newManager.getTask('task-1').title).toBe('Task 1');
    });
  });

  describe('Circular Dependency Detection', () => {
    test('should prevent direct circular dependency', () => {
      manager.createTask({ title: 'Task 1' });

      // Try to add task-1 as dependency of task-2, then task-2 as dependency of task-1
      manager.createTask({ title: 'Task 2', dependencies: ['task-1'] });

      expect(() => {
        manager.updateTask('task-1', { dependencies: ['task-2'] });
      }).toThrow(/circular dependency/);
    });

    test('should prevent indirect circular dependency', () => {
      manager.createTask({ title: 'Task 1' });
      manager.createTask({ title: 'Task 2', dependencies: ['task-1'] });
      manager.createTask({ title: 'Task 3', dependencies: ['task-2'] });

      expect(() => {
        manager.updateTask('task-1', { dependencies: ['task-3'] });
      }).toThrow(/circular dependency/);
    });

    test('should prevent self-reference', () => {
      manager.createTask({ title: 'Task 1' });

      expect(() => {
        manager.updateTask('task-1', { dependencies: ['task-1'] });
      }).toThrow(/circular dependency/);
    });

    test('should allow valid dependency chains', () => {
      manager.createTask({ title: 'Task 1' });
      manager.createTask({ title: 'Task 2' });
      manager.createTask({ title: 'Task 3' });

      // Linear chain: 1 -> 2 -> 3
      manager.updateTask('task-2', { dependencies: ['task-1'] });
      manager.updateTask('task-3', { dependencies: ['task-2'] });

      expect(manager.getTask('task-3').dependencies).toEqual(['task-2']);
    });

    test('should allow diamond dependency pattern', () => {
      manager.createTask({ title: 'Task 1' });
      manager.createTask({ title: 'Task 2', dependencies: ['task-1'] });
      manager.createTask({ title: 'Task 3', dependencies: ['task-1'] });
      manager.createTask({ title: 'Task 4', dependencies: ['task-2', 'task-3'] });

      // Diamond: 1 -> 2 -> 4, 1 -> 3 -> 4
      expect(manager.getTask('task-4').dependencies).toEqual(['task-2', 'task-3']);
    });
  });

  describe('Task Ordering with "after" parameter', () => {
    test('should insert task after specified task', () => {
      manager.createTask({ title: 'Task 1' });
      manager.createTask({ title: 'Task 2' });
      manager.createTask({ title: 'Task 3' });

      // Insert task-4 after task-1
      manager.createTask({ title: 'Task 4', after: 'task-1' });

      const tasks = manager.listTasks();
      const taskIds = tasks.map(t => t.id);

      expect(taskIds).toEqual(['task-1', 'task-4', 'task-2', 'task-3']);
    });

    test('should append to end when after is not specified', () => {
      manager.createTask({ title: 'Task 1' });
      manager.createTask({ title: 'Task 2' });
      manager.createTask({ title: 'Task 3' });

      const tasks = manager.listTasks();
      const taskIds = tasks.map(t => t.id);

      expect(taskIds).toEqual(['task-1', 'task-2', 'task-3']);
    });

    test('should insert after last task correctly', () => {
      manager.createTask({ title: 'Task 1' });
      manager.createTask({ title: 'Task 2' });

      // Insert after last task (same as appending)
      manager.createTask({ title: 'Task 3', after: 'task-2' });

      const tasks = manager.listTasks();
      const taskIds = tasks.map(t => t.id);

      expect(taskIds).toEqual(['task-1', 'task-2', 'task-3']);
    });

    test('should throw error for non-existent after task', () => {
      manager.createTask({ title: 'Task 1' });

      expect(() => {
        manager.createTask({ title: 'Task 2', after: 'non-existent' });
      }).toThrow(/does not exist/);
    });

    test('should work with dependencies and after together', () => {
      manager.createTask({ title: 'Task 1' });
      manager.createTask({ title: 'Task 2' });
      manager.createTask({ title: 'Task 3' });

      // Insert task-4 after task-1, with dependency on task-1
      manager.createTask({
        title: 'Task 4',
        after: 'task-1',
        dependencies: ['task-1']
      });

      const tasks = manager.listTasks();
      const taskIds = tasks.map(t => t.id);
      const task4 = manager.getTask('task-4');

      expect(taskIds).toEqual(['task-1', 'task-4', 'task-2', 'task-3']);
      expect(task4.dependencies).toEqual(['task-1']);
    });

    test('should handle multiple insertions correctly', () => {
      manager.createTask({ title: 'Task 1' });
      manager.createTask({ title: 'Task 2' });

      // Insert task-3 after task-1
      manager.createTask({ title: 'Task 3', after: 'task-1' });
      // Insert task-4 after task-3
      manager.createTask({ title: 'Task 4', after: 'task-3' });

      const tasks = manager.listTasks();
      const taskIds = tasks.map(t => t.id);

      expect(taskIds).toEqual(['task-1', 'task-3', 'task-4', 'task-2']);
    });
  });

  describe('Input Validation', () => {
    test('should throw error for invalid status value', () => {
      manager.createTask({ title: 'Task 1' });

      expect(() => {
        manager.updateTask('task-1', { status: 'invalid_status' });
      }).toThrow(/Invalid status/);
    });

    test('should throw error for invalid priority value', () => {
      manager.createTask({ title: 'Task 1' });

      expect(() => {
        manager.updateTask('task-1', { priority: 'urgent' });
      }).toThrow(/Invalid priority/);
    });

    test('should throw error when dependencies is not an array', () => {
      manager.createTask({ title: 'Task 1' });
      manager.createTask({ title: 'Task 2' });

      expect(() => {
        manager.updateTask('task-2', { dependencies: 'task-1' });
      }).toThrow(/Dependencies must be an array/);
    });

    test('should accept valid status values', () => {
      manager.createTask({ title: 'Task 1' });

      // All valid statuses should work
      const validStatuses = ['pending', 'in_progress', 'completed', 'cancelled'];
      for (const status of validStatuses) {
        const updated = manager.updateTask('task-1', { status });
        expect(updated.status).toBe(status);
      }
    });

    test('should accept valid priority values', () => {
      manager.createTask({ title: 'Task 1' });

      // All valid priorities should work
      const validPriorities = ['low', 'medium', 'high', 'critical'];
      for (const priority of validPriorities) {
        const updated = manager.updateTask('task-1', { priority });
        expect(updated.priority).toBe(priority);
      }
    });

    test('should allow null priority to clear it', () => {
      manager.createTask({ title: 'Task 1', priority: 'high' });
      const updated = manager.updateTask('task-1', { priority: null });
      expect(updated.priority).toBeNull();
    });
  });

  describe('XML Escaping (Security)', () => {
    test('should escape XML special characters in task title', () => {
      manager.createTask({ title: 'Test <script>alert("xss")</script>' });
      const output = manager.formatTasksForPrompt();

      expect(output).toContain('&lt;script&gt;');
      expect(output).toContain('&quot;xss&quot;');
      expect(output).not.toContain('<script>');
    });

    test('should escape ampersands in task title', () => {
      manager.createTask({ title: 'Task & another task' });
      const output = manager.formatTasksForPrompt();

      expect(output).toContain('Task &amp; another task');
    });

    test('should escape quotes in attributes', () => {
      manager.createTask({ title: 'Normal task', priority: 'high' });
      // The priority attribute value should be properly escaped
      const output = manager.formatTasksForPrompt();
      expect(output).toContain('priority="high"');
    });

    test('should handle task with all special characters', () => {
      manager.createTask({
        title: '<tag attr="value">content & more</tag>',
        priority: 'high'
      });
      const output = manager.formatTasksForPrompt();

      expect(output).toContain('&lt;tag');
      expect(output).toContain('&gt;');
      expect(output).toContain('&amp;');
      expect(output).toContain('&quot;');
    });
  });

  describe('Import Security (Prototype Pollution Prevention)', () => {
    test('should reject null import data', () => {
      expect(() => manager.import(null)).toThrow(/Invalid import data/);
    });

    test('should reject non-object import data', () => {
      expect(() => manager.import('string')).toThrow(/Invalid import data/);
      expect(() => manager.import(123)).toThrow(/Invalid import data/);
    });

    test('should reject import with __proto__ property', () => {
      const maliciousData = {
        tasks: [],
        taskCounter: 0,
        __proto__: { malicious: true }
      };
      // Note: This is tricky because __proto__ is special in JS
      // We need to use Object.defineProperty or similar
      const data = Object.create(null);
      data.tasks = [];
      data.taskCounter = 0;
      Object.defineProperty(data, '__proto__', {
        value: { malicious: true },
        enumerable: true
      });

      expect(() => manager.import(data)).toThrow(/prototype pollution/);
    });

    test('should reject import with constructor property', () => {
      const maliciousData = {
        tasks: [],
        taskCounter: 0,
        constructor: function() {}
      };

      expect(() => manager.import(maliciousData)).toThrow(/prototype pollution/);
    });

    test('should reject import with non-array tasks', () => {
      expect(() => manager.import({ tasks: {}, taskCounter: 0 })).toThrow(/tasks must be an array/);
      expect(() => manager.import({ tasks: 'string', taskCounter: 0 })).toThrow(/tasks must be an array/);
    });

    test('should reject import with invalid taskCounter', () => {
      expect(() => manager.import({ tasks: [], taskCounter: 'invalid' })).toThrow(/taskCounter must be/);
      expect(() => manager.import({ tasks: [], taskCounter: -1 })).toThrow(/taskCounter must be/);
      expect(() => manager.import({ tasks: [], taskCounter: 1.5 })).toThrow(/taskCounter must be/);
    });

    test('should reject import with invalid task entry format', () => {
      expect(() => manager.import({
        tasks: [['task-1']], // missing task object
        taskCounter: 1
      })).toThrow(/task entry must be/);

      expect(() => manager.import({
        tasks: [[123, { title: 'test' }]], // id not a string
        taskCounter: 1
      })).toThrow(/task id must be a string/);
    });

    test('should reject task with __proto__ in import', () => {
      const task = Object.create(null);
      task.id = 'task-1';
      task.title = 'Test';
      task.status = 'pending';
      task.dependencies = [];
      Object.defineProperty(task, '__proto__', {
        value: { malicious: true },
        enumerable: true
      });

      expect(() => manager.import({
        tasks: [['task-1', task]],
        taskCounter: 1
      })).toThrow(/prototype pollution.*task/);
    });

    test('should accept valid import data', () => {
      manager.createTask({ title: 'Task 1' });
      manager.createTask({ title: 'Task 2', dependencies: ['task-1'] });

      const exported = manager.export();

      const newManager = new TaskManager();
      newManager.import(exported);

      expect(newManager.listTasks().length).toBe(2);
      expect(newManager.getTask('task-1').title).toBe('Task 1');
      expect(newManager.getTask('task-2').dependencies).toEqual(['task-1']);
    });
  });
});
