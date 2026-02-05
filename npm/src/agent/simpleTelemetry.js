import { existsSync, mkdirSync, createWriteStream } from 'fs';
import { dirname } from 'path';

/**
 * Simple telemetry implementation for probe-agent
 * This provides basic tracing functionality without complex OpenTelemetry dependencies
 */
export class SimpleTelemetry {
  constructor(options = {}) {
    this.serviceName = options.serviceName || 'probe-agent';
    this.enableFile = options.enableFile || false;
    this.enableConsole = options.enableConsole || false;
    this.filePath = options.filePath || './traces.jsonl';
    this.stream = null;
    
    if (this.enableFile) {
      this.initializeFileExporter();
    }
  }

  initializeFileExporter() {
    try {
      const dir = dirname(this.filePath);
      if (!existsSync(dir)) {
        mkdirSync(dir, { recursive: true });
      }
      
      this.stream = createWriteStream(this.filePath, { flags: 'a' });
      this.stream.on('error', (error) => {
        console.error(`[SimpleTelemetry] Stream error: ${error.message}`);
      });
      
      console.log(`[SimpleTelemetry] File exporter initialized: ${this.filePath}`);
    } catch (error) {
      console.error(`[SimpleTelemetry] Failed to initialize file exporter: ${error.message}`);
    }
  }

  createSpan(name, attributes = {}) {
    const span = {
      traceId: this.generateTraceId(),
      spanId: this.generateSpanId(),
      name,
      startTime: Date.now(),
      attributes: { ...attributes, service: this.serviceName },
      events: [],
      status: 'OK'
    };
    
    return {
      ...span,
      addEvent: (eventName, eventAttributes = {}) => {
        span.events.push({
          name: eventName,
          time: Date.now(),
          attributes: eventAttributes
        });
      },
      setAttributes: (attrs) => {
        Object.assign(span.attributes, attrs);
      },
      setStatus: (status) => {
        span.status = status;
      },
      end: () => {
        span.endTime = Date.now();
        span.duration = span.endTime - span.startTime;
        this.exportSpan(span);
      }
    };
  }

  exportSpan(span) {
    const spanData = {
      ...span,
      timestamp: new Date().toISOString()
    };

    if (this.enableConsole) {
      console.log('[Trace]', JSON.stringify(spanData, null, 2));
    }

    if (this.enableFile && this.stream) {
      this.stream.write(JSON.stringify(spanData) + '\n');
    }
  }

  generateTraceId() {
    return Math.random().toString(36).substring(2, 15) + Math.random().toString(36).substring(2, 15);
  }

  generateSpanId() {
    return Math.random().toString(36).substring(2, 10);
  }

  async flush() {
    if (this.stream) {
      return new Promise((resolve) => {
        this.stream.once('drain', resolve);
        if (!this.stream.writableNeedDrain) {
          resolve();
        }
      });
    }
  }

  async shutdown() {
    if (this.stream) {
      return new Promise((resolve) => {
        this.stream.end(() => {
          console.log(`[SimpleTelemetry] File stream closed: ${this.filePath}`);
          resolve();
        });
      });
    }
  }
}

/**
 * Simple tracer for application-level tracing
 */
export class SimpleAppTracer {
  constructor(telemetry, sessionId = null) {
    this.telemetry = telemetry;
    this.sessionId = sessionId || this.generateSessionId();
  }

  generateSessionId() {
    return Math.random().toString(36).substring(2, 15);
  }

  isEnabled() {
    return this.telemetry !== null;
  }

  createSessionSpan(attributes = {}) {
    if (!this.isEnabled()) return null;

    return this.telemetry.createSpan('agent.session', {
      'session.id': this.sessionId,
      ...attributes
    });
  }

  createAISpan(modelName, provider, attributes = {}) {
    if (!this.isEnabled()) return null;

    return this.telemetry.createSpan('ai.request', {
      'ai.model': modelName,
      'ai.provider': provider,
      'session.id': this.sessionId,
      ...attributes
    });
  }

  createToolSpan(toolName, attributes = {}) {
    if (!this.isEnabled()) return null;

    return this.telemetry.createSpan('tool.call', {
      'tool.name': toolName,
      'session.id': this.sessionId,
      ...attributes
    });
  }

  addEvent(name, attributes = {}) {
    // For simplicity, just log events when no active span
    if (this.telemetry && this.telemetry.enableConsole) {
      console.log('[Event]', name, attributes);
    }
  }

  /**
   * Record a generic event (used by completionPrompt and other features)
   */
  // visor-disable: SimpleAppTracer uses this.sessionId because it's a per-session instance. AppTracer extracts from attributes because it's a singleton managing multiple sessions. Different architectures require different approaches.
  recordEvent(name, attributes = {}) {
    if (!this.isEnabled()) return;

    this.addEvent(name, {
      'session.id': this.sessionId,
      ...attributes
    });
  }

  /**
   * Record delegation events
   */
  recordDelegationEvent(eventType, data = {}) {
    if (!this.isEnabled()) return;

    this.addEvent(`delegation.${eventType}`, {
      'session.id': this.sessionId,
      ...data
    });
  }

  /**
   * Record JSON validation events
   */
  recordJsonValidationEvent(eventType, data = {}) {
    if (!this.isEnabled()) return;

    this.addEvent(`json_validation.${eventType}`, {
      'session.id': this.sessionId,
      ...data
    });
  }

  /**
   * Record Mermaid validation events
   */
  recordMermaidValidationEvent(eventType, data = {}) {
    if (!this.isEnabled()) return;

    this.addEvent(`mermaid_validation.${eventType}`, {
      'session.id': this.sessionId,
      ...data
    });
  }

  /**
   * Record task management events
   */
  recordTaskEvent(eventType, data = {}) {
    if (!this.isEnabled()) return;

    this.addEvent(`task.${eventType}`, {
      'session.id': this.sessionId,
      ...data
    });
  }

  /**
   * Record MCP (Model Context Protocol) events
   * Tracks server connections, tool discovery, method filtering, and tool execution
   */
  recordMcpEvent(eventType, data = {}) {
    if (!this.isEnabled()) return;

    this.addEvent(`mcp.${eventType}`, {
      'session.id': this.sessionId,
      ...data
    });
  }

  /**
   * Record bash tool events
   * Tracks command permission checks, allowed/denied commands, and execution
   */
  recordBashEvent(eventType, data = {}) {
    if (!this.isEnabled()) return;

    this.addEvent(`bash.${eventType}`, {
      'session.id': this.sessionId,
      ...data
    });
  }

  setAttributes(attributes) {
    // For simplicity, just log attributes when no active span
    if (this.telemetry && this.telemetry.enableConsole) {
      console.log('[Attributes]', attributes);
    }
  }

  /**
   * Hash content for deduplication/comparison purposes
   * @param {string} content - The content to hash
   * @returns {string} - Hex string hash
   */
  hashContent(content) {
    let hash = 0;
    const len = Math.min(content.length, 1000);
    for (let i = 0; i < len; i++) {
      hash = ((hash << 5) - hash) + content.charCodeAt(i);
      hash |= 0; // Convert to 32-bit integer
    }
    return hash.toString(16);
  }

  /**
   * Record a conversation turn (assistant response or tool result)
   * @param {string} role - The role (assistant, tool_result)
   * @param {string} content - The turn content
   * @param {Object} metadata - Additional metadata
   */
  recordConversationTurn(role, content, metadata = {}) {
    if (!this.isEnabled()) return;

    this.addEvent(`conversation.turn.${role}`, {
      'session.id': this.sessionId,
      'conversation.role': role,
      'conversation.content': content.substring(0, 10000),
      'conversation.content.length': content.length,
      'conversation.content.hash': this.hashContent(content),
      ...metadata
    });
  }

  /**
   * Record error events with classification
   * @param {string} errorType - The type of error (wrapped_tool, unrecognized_tool, no_tool_call, circuit_breaker, etc.)
   * @param {Object} errorDetails - Error details including message, stack, context
   */
  recordErrorEvent(errorType, errorDetails = {}) {
    if (!this.isEnabled()) return;

    this.addEvent(`error.${errorType}`, {
      'session.id': this.sessionId,
      'error.type': errorType,
      'error.message': errorDetails.message?.substring(0, 1000) || null,
      'error.stack': errorDetails.stack?.substring(0, 2000) || null,
      'error.recoverable': errorDetails.recoverable ?? true,
      'error.context': JSON.stringify(errorDetails.context || {}).substring(0, 1000),
      ...Object.fromEntries(
        Object.entries(errorDetails)
          .filter(([k]) => !['message', 'stack', 'context', 'recoverable'].includes(k))
          .map(([k, v]) => [`error.${k}`, v])
      )
    });
  }

  /**
   * Record AI thinking/reasoning content
   * @param {string} thinkingContent - The thinking content from AI response
   * @param {Object} metadata - Additional metadata
   */
  recordThinkingContent(thinkingContent, metadata = {}) {
    if (!this.isEnabled() || !thinkingContent) return;

    this.addEvent('ai.thinking', {
      'session.id': this.sessionId,
      'ai.thinking.content': thinkingContent.substring(0, 50000),
      'ai.thinking.length': thinkingContent.length,
      'ai.thinking.hash': this.hashContent(thinkingContent),
      ...metadata
    });
  }

  /**
   * Record AI tool call decision
   * @param {string} toolName - The tool name AI decided to call
   * @param {Object} params - The parameters AI provided
   * @param {Object} metadata - Additional metadata
   */
  recordToolDecision(toolName, params, metadata = {}) {
    if (!this.isEnabled()) return;

    this.addEvent('ai.tool_decision', {
      'session.id': this.sessionId,
      'ai.tool_decision.name': toolName,
      'ai.tool_decision.params': JSON.stringify(params || {}).substring(0, 2000),
      ...metadata
    });
  }

  /**
   * Record tool result after execution
   * @param {string} toolName - The tool that was executed
   * @param {string|Object} result - The tool result
   * @param {boolean} success - Whether the tool succeeded
   * @param {number} durationMs - Execution duration in milliseconds
   * @param {Object} metadata - Additional metadata
   */
  recordToolResult(toolName, result, success, durationMs, metadata = {}) {
    if (!this.isEnabled()) return;

    const resultStr = typeof result === 'string' ? result : JSON.stringify(result);
    this.addEvent('tool.result', {
      'session.id': this.sessionId,
      'tool.name': toolName,
      'tool.result': resultStr.substring(0, 10000),
      'tool.result.length': resultStr.length,
      'tool.result.hash': this.hashContent(resultStr),
      'tool.duration_ms': durationMs,
      'tool.success': success,
      ...metadata
    });
  }

  /**
   * Record MCP tool execution start
   * @param {string} toolName - MCP tool name
   * @param {string} serverName - MCP server name
   * @param {Object} params - Tool parameters
   * @param {Object} metadata - Additional metadata
   */
  recordMcpToolStart(toolName, serverName, params, metadata = {}) {
    if (!this.isEnabled()) return;

    this.addEvent('mcp.tool.start', {
      'session.id': this.sessionId,
      'mcp.tool.name': toolName,
      'mcp.tool.server': serverName || 'unknown',
      'mcp.tool.params': JSON.stringify(params || {}).substring(0, 2000),
      ...metadata
    });
  }

  /**
   * Record MCP tool execution end
   * @param {string} toolName - MCP tool name
   * @param {string} serverName - MCP server name
   * @param {string|Object} result - Tool result
   * @param {boolean} success - Whether succeeded
   * @param {number} durationMs - Execution duration
   * @param {string} errorMessage - Error message if failed
   * @param {Object} metadata - Additional metadata
   */
  recordMcpToolEnd(toolName, serverName, result, success, durationMs, errorMessage = null, metadata = {}) {
    if (!this.isEnabled()) return;

    const resultStr = typeof result === 'string' ? result : JSON.stringify(result || '');
    this.addEvent('mcp.tool.end', {
      'session.id': this.sessionId,
      'mcp.tool.name': toolName,
      'mcp.tool.server': serverName || 'unknown',
      'mcp.tool.result': resultStr.substring(0, 10000),
      'mcp.tool.result.length': resultStr.length,
      'mcp.tool.duration_ms': durationMs,
      'mcp.tool.success': success,
      'mcp.tool.error': errorMessage,
      ...metadata
    });
  }

  /**
   * Record iteration lifecycle event
   * @param {string} eventType - start or end
   * @param {number} iteration - Iteration number
   * @param {Object} data - Additional data
   */
  recordIterationEvent(eventType, iteration, data = {}) {
    if (!this.isEnabled()) return;

    this.addEvent(`iteration.${eventType}`, {
      'session.id': this.sessionId,
      'iteration': iteration,
      ...data
    });
  }

  /**
   * Record per-turn token breakdown
   * @param {number} iteration - Iteration number
   * @param {Object} tokenData - Token metrics
   */
  recordTokenTurn(iteration, tokenData = {}) {
    if (!this.isEnabled()) return;

    this.addEvent('tokens.turn', {
      'session.id': this.sessionId,
      'iteration': iteration,
      'tokens.input': tokenData.inputTokens || 0,
      'tokens.output': tokenData.outputTokens || 0,
      'tokens.total': (tokenData.inputTokens || 0) + (tokenData.outputTokens || 0),
      'tokens.cache_read': tokenData.cacheReadTokens || 0,
      'tokens.cache_write': tokenData.cacheWriteTokens || 0,
      'tokens.context_used': tokenData.contextTokens || 0,
      'tokens.context_remaining': tokenData.maxContextTokens ? (tokenData.maxContextTokens - (tokenData.contextTokens || 0)) : null
    });
  }

  async withSpan(spanName, fn, attributes = {}) {
    if (!this.isEnabled()) {
      return fn();
    }

    const span = this.telemetry.createSpan(spanName, {
      'session.id': this.sessionId,
      ...attributes
    });

    try {
      const result = await fn();
      span.setStatus('OK');
      return result;
    } catch (error) {
      span.setStatus('ERROR');
      span.addEvent('exception', { 
        'exception.message': error.message,
        'exception.stack': error.stack 
      });
      throw error;
    } finally {
      span.end();
    }
  }

  async flush() {
    if (this.telemetry) {
      await this.telemetry.flush();
    }
  }

  async shutdown() {
    if (this.telemetry) {
      await this.telemetry.shutdown();
    }
  }
}

/**
 * Initialize simple telemetry from CLI options
 */
export function initializeSimpleTelemetryFromOptions(options) {
  const telemetry = new SimpleTelemetry({
    serviceName: 'probe-agent',
    enableFile: options.traceFile !== undefined,
    enableConsole: options.traceConsole,
    filePath: options.traceFile || './traces.jsonl'
  });

  return telemetry;
}