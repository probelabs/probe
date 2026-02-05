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

  describe('hashContent', () => {
    test('should return a hex string hash', () => {
      const hash = tracer.hashContent('test content');
      expect(typeof hash).toBe('string');
      expect(hash).toMatch(/^-?[0-9a-f]+$/);
    });

    test('should return different hashes for different content', () => {
      const hash1 = tracer.hashContent('content 1');
      const hash2 = tracer.hashContent('content 2');
      expect(hash1).not.toBe(hash2);
    });

    test('should return same hash for same content', () => {
      const hash1 = tracer.hashContent('same content');
      const hash2 = tracer.hashContent('same content');
      expect(hash1).toBe(hash2);
    });

    test('should handle empty string', () => {
      const hash = tracer.hashContent('');
      expect(hash).toBe('0');
    });

    test('should handle long content (uses first 1000 chars)', () => {
      const longContent = 'a'.repeat(5000);
      const hash = tracer.hashContent(longContent);
      expect(typeof hash).toBe('string');
    });
  });

  describe('recordConversationTurn', () => {
    test('should record assistant conversation turn', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');

      tracer.recordConversationTurn('assistant', 'Hello, how can I help?', {
        iteration: 1,
        has_tool_call: false
      });

      expect(addEventSpy).toHaveBeenCalledWith('conversation.turn.assistant', expect.objectContaining({
        'session.id': 'test-session-123',
        'conversation.role': 'assistant',
        'conversation.content': 'Hello, how can I help?',
        'conversation.content.length': 22,
        iteration: 1,
        has_tool_call: false
      }));
    });

    test('should record tool_result conversation turn', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');

      tracer.recordConversationTurn('tool_result', 'Search results: file.js', {
        iteration: 2,
        tool_name: 'search',
        tool_success: true
      });

      expect(addEventSpy).toHaveBeenCalledWith('conversation.turn.tool_result', expect.objectContaining({
        'conversation.role': 'tool_result',
        'conversation.content': 'Search results: file.js',
        tool_name: 'search',
        tool_success: true
      }));
    });

    test('should truncate long content to 10000 chars', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');
      const longContent = 'a'.repeat(15000);

      tracer.recordConversationTurn('assistant', longContent, {});

      expect(addEventSpy).toHaveBeenCalledWith('conversation.turn.assistant', expect.objectContaining({
        'conversation.content': 'a'.repeat(10000),
        'conversation.content.length': 15000
      }));
    });

    test('should not record when tracer is disabled', () => {
      const disabledTracer = new SimpleAppTracer(null, 'session-123');
      const addEventSpy = jest.spyOn(disabledTracer, 'addEvent');

      disabledTracer.recordConversationTurn('assistant', 'test', {});

      expect(addEventSpy).not.toHaveBeenCalled();
    });
  });

  describe('recordErrorEvent', () => {
    test('should record wrapped_tool error', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');

      tracer.recordErrorEvent('wrapped_tool', {
        message: 'Tool call wrapped in markdown',
        context: { toolName: 'search', iteration: 1 }
      });

      expect(addEventSpy).toHaveBeenCalledWith('error.wrapped_tool', expect.objectContaining({
        'session.id': 'test-session-123',
        'error.type': 'wrapped_tool',
        'error.message': 'Tool call wrapped in markdown',
        'error.recoverable': true
      }));
    });

    test('should record unrecognized_tool error', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');

      tracer.recordErrorEvent('unrecognized_tool', {
        message: 'Unknown tool: foo',
        context: { toolName: 'foo', validTools: ['search', 'query'] }
      });

      expect(addEventSpy).toHaveBeenCalledWith('error.unrecognized_tool', expect.objectContaining({
        'error.type': 'unrecognized_tool',
        'error.message': 'Unknown tool: foo'
      }));
    });

    test('should record circuit_breaker error as non-recoverable', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');

      tracer.recordErrorEvent('circuit_breaker', {
        message: 'Format error limit exceeded',
        recoverable: false,
        context: { formatErrorCount: 3 }
      });

      expect(addEventSpy).toHaveBeenCalledWith('error.circuit_breaker', expect.objectContaining({
        'error.type': 'circuit_breaker',
        'error.recoverable': false
      }));
    });

    test('should truncate long error messages', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');
      const longMessage = 'error '.repeat(500);

      tracer.recordErrorEvent('test_error', { message: longMessage });

      const call = addEventSpy.mock.calls[0][1];
      expect(call['error.message'].length).toBeLessThanOrEqual(1000);
    });
  });

  describe('recordThinkingContent', () => {
    test('should record AI thinking content', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');
      const thinkingContent = 'Let me analyze this code...';

      tracer.recordThinkingContent(thinkingContent, { iteration: 1 });

      expect(addEventSpy).toHaveBeenCalledWith('ai.thinking', expect.objectContaining({
        'session.id': 'test-session-123',
        'ai.thinking.content': thinkingContent,
        'ai.thinking.length': thinkingContent.length,
        iteration: 1
      }));
    });

    test('should not record when thinkingContent is empty', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');

      tracer.recordThinkingContent('', { iteration: 1 });

      expect(addEventSpy).not.toHaveBeenCalled();
    });

    test('should not record when thinkingContent is null', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');

      tracer.recordThinkingContent(null, { iteration: 1 });

      expect(addEventSpy).not.toHaveBeenCalled();
    });

    test('should truncate very long thinking content', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');
      const longThinking = 'think '.repeat(20000);

      tracer.recordThinkingContent(longThinking, {});

      const call = addEventSpy.mock.calls[0][1];
      expect(call['ai.thinking.content'].length).toBeLessThanOrEqual(50000);
      expect(call['ai.thinking.length']).toBe(longThinking.length);
    });
  });

  describe('recordToolDecision', () => {
    test('should record tool decision with name and params', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');

      tracer.recordToolDecision('search', { query: 'test', path: './src' }, { iteration: 1 });

      expect(addEventSpy).toHaveBeenCalledWith('ai.tool_decision', expect.objectContaining({
        'session.id': 'test-session-123',
        'ai.tool_decision.name': 'search',
        'ai.tool_decision.params': '{"query":"test","path":"./src"}',
        iteration: 1
      }));
    });

    test('should handle null params', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');

      tracer.recordToolDecision('attempt_completion', null, {});

      expect(addEventSpy).toHaveBeenCalledWith('ai.tool_decision', expect.objectContaining({
        'ai.tool_decision.params': '{}'
      }));
    });
  });

  describe('recordToolResult', () => {
    test('should record successful tool result', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');
      const resultContent = 'Found 5 results';

      tracer.recordToolResult('search', resultContent, true, 150, { iteration: 1 });

      expect(addEventSpy).toHaveBeenCalledWith('tool.result', expect.objectContaining({
        'session.id': 'test-session-123',
        'tool.name': 'search',
        'tool.result': resultContent,
        'tool.result.length': resultContent.length,
        'tool.duration_ms': 150,
        'tool.success': true,
        iteration: 1
      }));
    });

    test('should record failed tool result', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');

      tracer.recordToolResult('bash', 'Error: command not found', false, 50, { iteration: 2 });

      expect(addEventSpy).toHaveBeenCalledWith('tool.result', expect.objectContaining({
        'tool.name': 'bash',
        'tool.success': false,
        'tool.duration_ms': 50
      }));
    });

    test('should handle object results', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');
      const result = { matches: 3, files: ['a.js', 'b.js'] };

      tracer.recordToolResult('search', result, true, 200, {});

      expect(addEventSpy).toHaveBeenCalledWith('tool.result', expect.objectContaining({
        'tool.result': JSON.stringify(result)
      }));
    });
  });

  describe('recordMcpToolStart', () => {
    test('should record MCP tool start event', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');

      tracer.recordMcpToolStart('fetch', 'http-server', { url: 'http://example.com' }, { iteration: 1 });

      expect(addEventSpy).toHaveBeenCalledWith('mcp.tool.start', expect.objectContaining({
        'session.id': 'test-session-123',
        'mcp.tool.name': 'fetch',
        'mcp.tool.server': 'http-server',
        'mcp.tool.params': '{"url":"http://example.com"}',
        iteration: 1
      }));
    });
  });

  describe('recordMcpToolEnd', () => {
    test('should record MCP tool success', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');

      tracer.recordMcpToolEnd('fetch', 'http-server', 'Response data', true, 300, null, { iteration: 1 });

      expect(addEventSpy).toHaveBeenCalledWith('mcp.tool.end', expect.objectContaining({
        'mcp.tool.name': 'fetch',
        'mcp.tool.server': 'http-server',
        'mcp.tool.result': 'Response data',
        'mcp.tool.success': true,
        'mcp.tool.duration_ms': 300,
        'mcp.tool.error': null
      }));
    });

    test('should record MCP tool failure', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');

      tracer.recordMcpToolEnd('fetch', 'http-server', null, false, 100, 'Connection timeout', { iteration: 1 });

      expect(addEventSpy).toHaveBeenCalledWith('mcp.tool.end', expect.objectContaining({
        'mcp.tool.success': false,
        'mcp.tool.error': 'Connection timeout'
      }));
    });
  });

  describe('recordIterationEvent', () => {
    test('should record iteration start event', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');

      tracer.recordIterationEvent('start', 1, { max_iterations: 10, message_count: 5 });

      expect(addEventSpy).toHaveBeenCalledWith('iteration.start', expect.objectContaining({
        'session.id': 'test-session-123',
        'iteration': 1,
        max_iterations: 10,
        message_count: 5
      }));
    });

    test('should record iteration end event', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');

      tracer.recordIterationEvent('end', 3, { completed: false, message_count: 8 });

      expect(addEventSpy).toHaveBeenCalledWith('iteration.end', expect.objectContaining({
        'iteration': 3,
        completed: false,
        message_count: 8
      }));
    });
  });

  describe('recordTokenTurn', () => {
    test('should record token metrics', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');

      tracer.recordTokenTurn(1, {
        inputTokens: 1000,
        outputTokens: 500,
        cacheReadTokens: 200,
        cacheWriteTokens: 100,
        contextTokens: 1500,
        maxContextTokens: 8000
      });

      expect(addEventSpy).toHaveBeenCalledWith('tokens.turn', expect.objectContaining({
        'session.id': 'test-session-123',
        'iteration': 1,
        'tokens.input': 1000,
        'tokens.output': 500,
        'tokens.total': 1500,
        'tokens.cache_read': 200,
        'tokens.cache_write': 100,
        'tokens.context_used': 1500,
        'tokens.context_remaining': 6500
      }));
    });

    test('should handle missing token data', () => {
      const addEventSpy = jest.spyOn(tracer, 'addEvent');

      tracer.recordTokenTurn(1, {});

      expect(addEventSpy).toHaveBeenCalledWith('tokens.turn', expect.objectContaining({
        'tokens.input': 0,
        'tokens.output': 0,
        'tokens.total': 0
      }));
    });
  });
});
