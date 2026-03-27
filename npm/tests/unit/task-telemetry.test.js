/**
 * Tests for enriched task telemetry events.
 * Verifies that task events include agent scope, full task state,
 * per-task batch payloads, and monotonic sequence numbers.
 * @module tests/unit/task-telemetry
 */

import { jest, describe, test, expect, beforeEach } from '@jest/globals';
import { TaskManager } from '../../src/agent/tasks/TaskManager.js';
import { createTaskTool } from '../../src/agent/tasks/taskTool.js';
import { SimpleAppTracer, SimpleTelemetry } from '../../src/agent/simpleTelemetry.js';

describe('Task telemetry enrichment', () => {
  let manager;
  let tracer;
  let events;

  beforeEach(() => {
    manager = new TaskManager({ debug: false });
    events = [];

    // Create a tracer that captures events for assertions
    const telemetry = new SimpleTelemetry({ enableConsole: false });
    tracer = new SimpleAppTracer(telemetry, 'test-session-main');
    // Override addEvent to capture emitted events
    tracer.addEvent = (name, attrs) => {
      events.push({ name, attrs });
    };
  });

  function createTool(opts = {}) {
    return createTaskTool({ taskManager: manager, tracer, debug: false, ...opts });
  }

  // ---- A. Agent scope in every event ----

  describe('agent scope fields', () => {
    test('single create event includes agent scope', async () => {
      const tool = createTool();
      await tool.execute({ action: 'create', title: 'Auth module' });

      const evt = events.find(e => e.name === 'task.created');
      expect(evt).toBeDefined();
      expect(evt.attrs['agent.session_id']).toBe('test-session-main');
      expect(evt.attrs['agent.parent_session_id']).toBeNull();
      expect(evt.attrs['agent.root_session_id']).toBe('test-session-main');
      expect(evt.attrs['agent.kind']).toBe('main');
    });

    test('child tracer scopes events to subagent', async () => {
      const childTracer = tracer.createChildTracer('sub-session-1', { agentKind: 'delegate' });
      childTracer.addEvent = (name, attrs) => {
        events.push({ name, attrs });
      };

      const tool = createTaskTool({
        taskManager: manager,
        tracer: childTracer,
        delegationTask: 'Fix the login bug',
      });
      await tool.execute({ action: 'create', title: 'Patch auth' });

      const evt = events.find(e => e.name === 'task.created');
      expect(evt.attrs['agent.session_id']).toBe('sub-session-1');
      expect(evt.attrs['agent.parent_session_id']).toBe('test-session-main');
      expect(evt.attrs['agent.root_session_id']).toBe('test-session-main');
      expect(evt.attrs['agent.kind']).toBe('delegate');
      expect(evt.attrs['delegation.task']).toBe('Fix the login bug');
    });

    test('nested child tracers preserve root session', () => {
      const child = tracer.createChildTracer('child-1');
      const grandchild = child.createChildTracer('grandchild-1');

      expect(grandchild.sessionId).toBe('grandchild-1');
      expect(grandchild.parentSessionId).toBe('child-1');
      expect(grandchild.rootSessionId).toBe('test-session-main');
    });

    test('all event types include agent scope', async () => {
      const tool = createTool();
      await tool.execute({ action: 'create', title: 'Task A' });
      await tool.execute({ action: 'update', id: 'task-1', status: 'in_progress' });
      await tool.execute({ action: 'complete', id: 'task-1' });
      await tool.execute({ action: 'create', title: 'Task B' });
      await tool.execute({ action: 'delete', id: 'task-2' });
      await tool.execute({ action: 'list' });

      const taskEvents = events.filter(e => e.name.startsWith('task.'));
      expect(taskEvents.length).toBeGreaterThanOrEqual(5);
      for (const evt of taskEvents) {
        expect(evt.attrs['agent.session_id']).toBeDefined();
        expect(evt.attrs['agent.root_session_id']).toBeDefined();
        expect(evt.attrs['agent.kind']).toBeDefined();
      }
    });
  });

  // ---- B. Enriched single-task events ----

  describe('enriched single-task events', () => {
    test('created event includes full task state', async () => {
      const tool = createTool();
      await tool.execute({ action: 'create', title: 'Setup DB', priority: 'high' });

      const evt = events.find(e => e.name === 'task.created');
      expect(evt.attrs['task.id']).toBe('task-1');
      expect(evt.attrs['task.title']).toBe('Setup DB');
      expect(evt.attrs['task.status']).toBe('pending');
      expect(evt.attrs['task.priority']).toBe('high');
      expect(evt.attrs['task.dependencies']).toBe('[]');
      expect(evt.attrs['task.order']).toBe(0);
      expect(evt.attrs['task.total_count']).toBe(1);
      expect(evt.attrs['task.incomplete_remaining']).toBe(1);
    });

    test('updated event includes full task state', async () => {
      const tool = createTool();
      await tool.execute({ action: 'create', title: 'Auth' });
      events.length = 0; // Clear

      await tool.execute({ action: 'update', id: 'task-1', status: 'in_progress', priority: 'critical' });

      const evt = events.find(e => e.name === 'task.updated');
      expect(evt.attrs['task.id']).toBe('task-1');
      expect(evt.attrs['task.title']).toBe('Auth');
      expect(evt.attrs['task.status']).toBe('in_progress');
      expect(evt.attrs['task.priority']).toBe('critical');
      expect(evt.attrs['task.order']).toBe(0);
      expect(evt.attrs['task.fields_updated']).toContain('status');
      expect(evt.attrs['task.total_count']).toBe(1);
      expect(evt.attrs['task.incomplete_remaining']).toBe(1);
    });

    test('completed event includes full task state', async () => {
      const tool = createTool();
      await tool.execute({ action: 'create', title: 'Deploy' });
      events.length = 0;

      await tool.execute({ action: 'complete', id: 'task-1' });

      const evt = events.find(e => e.name === 'task.completed');
      expect(evt.attrs['task.id']).toBe('task-1');
      expect(evt.attrs['task.title']).toBe('Deploy');
      expect(evt.attrs['task.status']).toBe('completed');
      expect(evt.attrs['task.order']).toBe(0);
      expect(evt.attrs['task.incomplete_remaining']).toBe(0);
    });

    test('deleted event includes task state before deletion', async () => {
      const tool = createTool();
      await tool.execute({ action: 'create', title: 'Cleanup' });
      events.length = 0;

      await tool.execute({ action: 'delete', id: 'task-1' });

      const evt = events.find(e => e.name === 'task.deleted');
      expect(evt.attrs['task.id']).toBe('task-1');
      expect(evt.attrs['task.title']).toBe('Cleanup');
      expect(evt.attrs['task.status']).toBe('pending');
      expect(evt.attrs['task.total_count']).toBe(0);
    });

    test('created event with dependencies', async () => {
      const tool = createTool();
      await tool.execute({ action: 'create', title: 'Setup' });
      await tool.execute({ action: 'create', title: 'Build', dependencies: ['task-1'] });

      const evt = events.filter(e => e.name === 'task.created')[1];
      expect(JSON.parse(evt.attrs['task.dependencies'])).toEqual(['task-1']);
    });
  });

  // ---- C. Enriched batch events with per-task payloads ----

  describe('enriched batch events', () => {
    test('batch_created includes items_json with per-task data', async () => {
      const tool = createTool();
      await tool.execute({
        action: 'create',
        tasks: [
          { id: 'auth', title: 'Auth module', priority: 'high' },
          { id: 'db', title: 'Database setup', priority: 'medium', dependencies: ['auth'] },
          { id: 'api', title: 'API routes' },
        ]
      });

      const evt = events.find(e => e.name === 'task.batch_created');
      expect(evt).toBeDefined();
      expect(evt.attrs['task.count']).toBe(3);
      expect(evt.attrs['task.total_count']).toBe(3);

      const items = JSON.parse(evt.attrs['task.items_json']);
      expect(items).toHaveLength(3);
      expect(items[0]).toMatchObject({ id: 'auth', title: 'Auth module', status: 'pending', priority: 'high' });
      expect(items[1]).toMatchObject({ id: 'db', title: 'Database setup', dependencies: ['auth'] });
      expect(items[2]).toMatchObject({ id: 'api', title: 'API routes' });
      // Verify ordering
      expect(items[0].order).toBe(0);
      expect(items[1].order).toBe(1);
      expect(items[2].order).toBe(2);
    });

    test('batch_updated includes items_json with current state', async () => {
      const tool = createTool();
      await tool.execute({
        action: 'create',
        tasks: [
          { id: 'a', title: 'Task A' },
          { id: 'b', title: 'Task B' },
        ]
      });
      events.length = 0;

      await tool.execute({
        action: 'update',
        tasks: [
          { id: 'a', status: 'in_progress' },
          { id: 'b', status: 'in_progress' },
        ]
      });

      const evt = events.find(e => e.name === 'task.batch_updated');
      const items = JSON.parse(evt.attrs['task.items_json']);
      expect(items).toHaveLength(2);
      expect(items[0]).toMatchObject({ id: 'a', status: 'in_progress' });
      expect(items[1]).toMatchObject({ id: 'b', status: 'in_progress' });
    });

    test('batch_completed includes items_json', async () => {
      const tool = createTool();
      await tool.execute({
        action: 'create',
        tasks: [
          { id: 'x', title: 'X' },
          { id: 'y', title: 'Y' },
        ]
      });
      events.length = 0;

      await tool.execute({ action: 'complete', tasks: ['x', 'y'] });

      const evt = events.find(e => e.name === 'task.batch_completed');
      const items = JSON.parse(evt.attrs['task.items_json']);
      expect(items).toHaveLength(2);
      expect(items[0]).toMatchObject({ id: 'x', status: 'completed' });
      expect(items[1]).toMatchObject({ id: 'y', status: 'completed' });
      expect(evt.attrs['task.incomplete_remaining']).toBe(0);
    });

    test('batch_deleted includes items_json with pre-deletion state', async () => {
      const tool = createTool();
      await tool.execute({
        action: 'create',
        tasks: [
          { id: 'p', title: 'P task' },
          { id: 'q', title: 'Q task' },
        ]
      });
      events.length = 0;

      await tool.execute({ action: 'delete', tasks: ['p', 'q'] });

      const evt = events.find(e => e.name === 'task.batch_deleted');
      const items = JSON.parse(evt.attrs['task.items_json']);
      expect(items).toHaveLength(2);
      expect(items[0]).toMatchObject({ id: 'p', title: 'P task', status: 'pending' });
      expect(items[1]).toMatchObject({ id: 'q', title: 'Q task', status: 'pending' });
    });

    test('list event includes items_json snapshot', async () => {
      const tool = createTool();
      await tool.execute({
        action: 'create',
        tasks: [
          { id: 'a', title: 'A' },
          { id: 'b', title: 'B' },
        ]
      });
      await tool.execute({ action: 'complete', id: 'a' });
      events.length = 0;

      await tool.execute({ action: 'list' });

      const evt = events.find(e => e.name === 'task.listed');
      const items = JSON.parse(evt.attrs['task.items_json']);
      expect(items).toHaveLength(2);
      expect(items[0]).toMatchObject({ id: 'a', status: 'completed', order: 0 });
      expect(items[1]).toMatchObject({ id: 'b', status: 'pending', order: 1 });
    });
  });

  // ---- D. Monotonic sequence ordering ----

  describe('monotonic sequence', () => {
    test('events have strictly increasing sequence numbers', async () => {
      const tool = createTool();
      await tool.execute({ action: 'create', title: 'Task 1' });
      await tool.execute({ action: 'create', title: 'Task 2' });
      await tool.execute({ action: 'update', id: 'task-1', status: 'in_progress' });
      await tool.execute({ action: 'complete', id: 'task-1' });

      const seqs = events
        .filter(e => e.name.startsWith('task.'))
        .map(e => e.attrs['task.sequence']);
      expect(seqs.length).toBeGreaterThanOrEqual(4);
      for (let i = 1; i < seqs.length; i++) {
        expect(seqs[i]).toBeGreaterThan(seqs[i - 1]);
      }
    });
  });

  // ---- E. list context (total_count, incomplete_remaining) ----

  describe('list context fields', () => {
    test('total_count and incomplete_remaining are accurate', async () => {
      const tool = createTool();
      await tool.execute({
        action: 'create',
        tasks: [
          { id: 'a', title: 'A' },
          { id: 'b', title: 'B' },
          { id: 'c', title: 'C' },
        ]
      });

      const createEvt = events.find(e => e.name === 'task.batch_created');
      expect(createEvt.attrs['task.total_count']).toBe(3);
      expect(createEvt.attrs['task.incomplete_remaining']).toBe(3);

      events.length = 0;
      await tool.execute({ action: 'complete', id: 'a' });
      const completeEvt = events.find(e => e.name === 'task.completed');
      expect(completeEvt.attrs['task.total_count']).toBe(3);
      expect(completeEvt.attrs['task.incomplete_remaining']).toBe(2);
    });
  });

  // ---- F. Child tracer creation ----

  describe('SimpleAppTracer.createChildTracer', () => {
    test('creates tracer with correct hierarchy', () => {
      const child = tracer.createChildTracer('child-sess');
      expect(child.sessionId).toBe('child-sess');
      expect(child.parentSessionId).toBe('test-session-main');
      expect(child.rootSessionId).toBe('test-session-main');
      expect(child.agentKind).toBe('delegate');
    });

    test('uses same telemetry backend', () => {
      const child = tracer.createChildTracer('child-sess');
      expect(child.telemetry).toBe(tracer.telemetry);
    });

    test('child isEnabled matches parent', () => {
      const child = tracer.createChildTracer('child-sess');
      expect(child.isEnabled()).toBe(tracer.isEnabled());
    });
  });
});
