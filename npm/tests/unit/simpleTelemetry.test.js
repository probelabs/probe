/**
 * Unit tests for SimpleTelemetry and SimpleAppTracer
 * @module tests/unit/simpleTelemetry
 */

import { jest, describe, test, expect, beforeEach, afterEach } from '@jest/globals';
import { SimpleTelemetry, SimpleAppTracer } from '../../src/agent/simpleTelemetry.js';

describe('SimpleTelemetry', () => {
  let telemetry;

  beforeEach(() => {
    telemetry = new SimpleTelemetry({
      serviceName: 'test-service',
      enableConsole: false,
      enableFile: false
    });
  });

  afterEach(async () => {
    await telemetry.shutdown();
  });

  describe('createSpan', () => {
    test('should create a span with name and attributes', () => {
      const span = telemetry.createSpan('test-span', { 'test.attr': 'value' });

      expect(span.name).toBe('test-span');
      expect(span.attributes).toMatchObject({
        'test.attr': 'value',
        service: 'test-service'
      });
      expect(span.traceId).toBeDefined();
      expect(span.spanId).toBeDefined();
    });

    test('should allow adding events to spans', () => {
      const span = telemetry.createSpan('test-span');

      span.addEvent('test-event', { 'event.data': 'test' });

      expect(span.events).toHaveLength(1);
      expect(span.events[0].name).toBe('test-event');
      expect(span.events[0].attributes).toMatchObject({ 'event.data': 'test' });
    });

    test('should allow setting attributes', () => {
      const span = telemetry.createSpan('test-span');

      span.setAttributes({ 'new.attr': 'new-value' });

      expect(span.attributes['new.attr']).toBe('new-value');
    });

    test('should allow setting status', () => {
      const span = telemetry.createSpan('test-span');

      // setStatus modifies the internal span state used during export
      // The function should not throw and should be callable
      expect(() => span.setStatus('ERROR')).not.toThrow();
    });
  });
});

describe('SimpleAppTracer', () => {
  let telemetry;
  let tracer;

  beforeEach(() => {
    telemetry = new SimpleTelemetry({
      serviceName: 'test-service',
      enableConsole: false,
      enableFile: false
    });
    tracer = new SimpleAppTracer(telemetry, 'test-session-123');
  });

  afterEach(async () => {
    await tracer.shutdown();
  });

  describe('constructor', () => {
    test('should initialize with provided session ID', () => {
      expect(tracer.sessionId).toBe('test-session-123');
    });

    test('should generate session ID if not provided', () => {
      const tracerWithoutSession = new SimpleAppTracer(telemetry);
      expect(tracerWithoutSession.sessionId).toBeDefined();
      expect(tracerWithoutSession.sessionId.length).toBeGreaterThan(0);
    });
  });

  describe('isEnabled', () => {
    test('should return true when telemetry is provided', () => {
      expect(tracer.isEnabled()).toBe(true);
    });

    test('should return false when telemetry is null', () => {
      const disabledTracer = new SimpleAppTracer(null, 'session-123');
      expect(disabledTracer.isEnabled()).toBe(false);
    });
  });

  describe('recordEvent', () => {
    test('should record event with name and attributes', () => {
      // Mock addEvent
      const addEventSpy = jest.spyOn(tracer, 'addEvent');

      tracer.recordEvent('test.event', { 'custom.attr': 'value' });

      expect(addEventSpy).toHaveBeenCalledWith('test.event', {
        'session.id': 'test-session-123',
        'custom.attr': 'value'
      });
    });

    test('should not record event when tracer is disabled', () => {
      const disabledTracer = new SimpleAppTracer(null, 'session-123');
      const addEventSpy = jest.spyOn(disabledTracer, 'addEvent');

      disabledTracer.recordEvent('test.event', { 'custom.attr': 'value' });

      expect(addEventSpy).not.toHaveBeenCalled();
    });

    test('should support completion_prompt.started event', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');

      tracer.recordEvent('completion_prompt.started', {
        'completion_prompt.original_result_length': 100
      });

      expect(addEventSpy).toHaveBeenCalledWith('completion_prompt.started', {
        'session.id': 'test-session-123',
        'completion_prompt.original_result_length': 100
      });
    });

    test('should support completion_prompt.completed event', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');

      tracer.recordEvent('completion_prompt.completed', {
        'completion_prompt.final_result_length': 200
      });

      expect(addEventSpy).toHaveBeenCalledWith('completion_prompt.completed', {
        'session.id': 'test-session-123',
        'completion_prompt.final_result_length': 200
      });
    });

    test('should support completion_prompt.error event', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');

      tracer.recordEvent('completion_prompt.error', {
        'completion_prompt.error': 'Test error message'
      });

      expect(addEventSpy).toHaveBeenCalledWith('completion_prompt.error', {
        'session.id': 'test-session-123',
        'completion_prompt.error': 'Test error message'
      });
    });
  });

  describe('recordDelegationEvent', () => {
    test('should record delegation event with type and data', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');

      tracer.recordDelegationEvent('started', { task: 'analyze code' });

      expect(addEventSpy).toHaveBeenCalledWith('delegation.started', {
        'session.id': 'test-session-123',
        task: 'analyze code'
      });
    });
  });

  describe('recordJsonValidationEvent', () => {
    test('should record JSON validation event with type and data', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');

      tracer.recordJsonValidationEvent('validation_started', { schema: 'user' });

      expect(addEventSpy).toHaveBeenCalledWith('json_validation.validation_started', {
        'session.id': 'test-session-123',
        schema: 'user'
      });
    });
  });

  describe('recordMermaidValidationEvent', () => {
    test('should record Mermaid validation event with type and data', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');

      tracer.recordMermaidValidationEvent('validation_complete', { valid: true });

      expect(addEventSpy).toHaveBeenCalledWith('mermaid_validation.validation_complete', {
        'session.id': 'test-session-123',
        valid: true
      });
    });
  });

  describe('createSessionSpan', () => {
    test('should create session span with attributes', () => {
      const span = tracer.createSessionSpan({ 'custom.attr': 'value' });

      expect(span).not.toBeNull();
      expect(span.name).toBe('agent.session');
      expect(span.attributes).toMatchObject({
        'session.id': 'test-session-123',
        'custom.attr': 'value'
      });
    });

    test('should return null when tracer is disabled', () => {
      const disabledTracer = new SimpleAppTracer(null);
      const span = disabledTracer.createSessionSpan();

      expect(span).toBeNull();
    });
  });

  describe('createAISpan', () => {
    test('should create AI span with model and provider', () => {
      const span = tracer.createAISpan('gpt-4', 'openai', { 'custom.attr': 'value' });

      expect(span).not.toBeNull();
      expect(span.name).toBe('ai.request');
      expect(span.attributes).toMatchObject({
        'ai.model': 'gpt-4',
        'ai.provider': 'openai',
        'session.id': 'test-session-123',
        'custom.attr': 'value'
      });
    });
  });

  describe('createToolSpan', () => {
    test('should create tool span with tool name', () => {
      const span = tracer.createToolSpan('search', { query: 'test' });

      expect(span).not.toBeNull();
      expect(span.name).toBe('tool.call');
      expect(span.attributes).toMatchObject({
        'tool.name': 'search',
        'session.id': 'test-session-123',
        query: 'test'
      });
    });
  });

  describe('withSpan', () => {
    test('should execute function within span context', async () => {
      let result;

      result = await tracer.withSpan('test.operation', async () => {
        return 'success';
      }, { 'operation.type': 'test' });

      expect(result).toBe('success');
    });

    test('should handle errors within span', async () => {
      await expect(tracer.withSpan('test.operation', async () => {
        throw new Error('Test error');
      })).rejects.toThrow('Test error');
    });

    test('should execute function directly when tracer is disabled', async () => {
      const disabledTracer = new SimpleAppTracer(null);

      const result = await disabledTracer.withSpan('test.operation', async () => {
        return 'executed';
      });

      expect(result).toBe('executed');
    });
  });
});
