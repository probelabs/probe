import { trace, context, SpanStatusCode } from '@opentelemetry/api';

/**
 * Application-specific tracing layer for probe-agent
 * Provides higher-level tracing functions for AI operations and tool calls
 */
export class AppTracer {
  constructor(telemetryConfig, sessionId = null) {
    this.telemetryConfig = telemetryConfig;
    this.tracer = telemetryConfig?.getTracer();
    this.sessionId = sessionId || this.generateSessionId();
    this.traceId = this.generateTraceId();
  }

  /**
   * Generate a unique session ID
   */
  generateSessionId() {
    return Math.random().toString(36).substring(2, 15) + Math.random().toString(36).substring(2, 15);
  }

  /**
   * Generate trace ID from session ID for consistent tracing
   */
  generateTraceId() {
    if (!this.sessionId) return null;
    
    // Create a deterministic trace ID from session ID
    const hash = this.hashString(this.sessionId);
    return hash.padEnd(32, '0').substring(0, 32);
  }

  /**
   * Simple hash function for session ID
   */
  hashString(str) {
    let hash = 0;
    for (let i = 0; i < str.length; i++) {
      const char = str.charCodeAt(i);
      hash = ((hash << 5) - hash) + char;
      hash = hash & hash; // Convert to 32bit integer
    }
    return Math.abs(hash).toString(16);
  }

  /**
   * Check if tracing is enabled
   */
  isEnabled() {
    return this.tracer !== null;
  }

  /**
   * Create a root span for the agent session
   */
  createSessionSpan(attributes = {}) {
    if (!this.isEnabled()) return null;

    return this.tracer.startSpan('agent.session', {
      attributes: {
        'session.id': this.sessionId,
        'trace.id': this.traceId,
        ...attributes,
      },
    });
  }

  /**
   * Create a span for AI model requests
   */
  createAISpan(modelName, provider, attributes = {}) {
    if (!this.isEnabled()) return null;

    return this.tracer.startSpan('ai.request', {
      attributes: {
        'ai.model': modelName,
        'ai.provider': provider,
        'session.id': this.sessionId,
        ...attributes,
      },
    });
  }

  /**
   * Create a span for tool calls
   */
  createToolSpan(toolName, attributes = {}) {
    if (!this.isEnabled()) return null;

    return this.tracer.startSpan('tool.call', {
      attributes: {
        'tool.name': toolName,
        'session.id': this.sessionId,
        ...attributes,
      },
    });
  }

  /**
   * Create a span for code search operations
   */
  createSearchSpan(query, attributes = {}) {
    if (!this.isEnabled()) return null;

    return this.tracer.startSpan('search.query', {
      attributes: {
        'search.query': query,
        'session.id': this.sessionId,
        ...attributes,
      },
    });
  }

  /**
   * Create a span for code extraction operations
   */
  createExtractSpan(files, attributes = {}) {
    if (!this.isEnabled()) return null;

    return this.tracer.startSpan('extract.files', {
      attributes: {
        'extract.file_count': Array.isArray(files) ? files.length : 1,
        'extract.files': Array.isArray(files) ? files.join(',') : files,
        'session.id': this.sessionId,
        ...attributes,
      },
    });
  }

  /**
   * Create a span for agent iterations
   */
  createIterationSpan(iteration, attributes = {}) {
    if (!this.isEnabled()) return null;

    return this.tracer.startSpan('agent.iteration', {
      attributes: {
        'iteration.number': iteration,
        'session.id': this.sessionId,
        ...attributes,
      },
    });
  }

  /**
   * Wrap a function with automatic span creation
   */
  wrapFunction(spanName, fn, attributes = {}) {
    if (!this.isEnabled()) {
      return fn;
    }

    return async (...args) => {
      const span = this.tracer.startSpan(spanName, {
        attributes: {
          'session.id': this.sessionId,
          ...attributes,
        },
      });

      try {
        const result = await context.with(trace.setSpan(context.active(), span), () => fn(...args));
        span.setStatus({ code: SpanStatusCode.OK });
        return result;
      } catch (error) {
        span.setStatus({
          code: SpanStatusCode.ERROR,
          message: error.message,
        });
        span.recordException(error);
        throw error;
      } finally {
        span.end();
      }
    };
  }

  /**
   * Execute a function within a span context
   */
  async withSpan(spanName, fn, attributes = {}) {
    if (!this.isEnabled()) {
      return fn();
    }

    const span = this.tracer.startSpan(spanName, {
      attributes: {
        'session.id': this.sessionId,
        ...attributes,
      },
    });

    try {
      const result = await context.with(trace.setSpan(context.active(), span), () => fn());
      span.setStatus({ code: SpanStatusCode.OK });
      return result;
    } catch (error) {
      span.setStatus({
        code: SpanStatusCode.ERROR,
        message: error.message,
      });
      span.recordException(error);
      throw error;
    } finally {
      span.end();
    }
  }

  /**
   * Add event to current active span
   */
  addEvent(name, attributes = {}) {
    if (!this.isEnabled()) return;

    const activeSpan = trace.getActiveSpan();
    if (activeSpan) {
      activeSpan.addEvent(name, {
        'session.id': this.sessionId,
        ...attributes,
      });
    }
  }

  /**
   * Set attributes on current active span
   */
  setAttributes(attributes) {
    if (!this.isEnabled()) return;

    const activeSpan = trace.getActiveSpan();
    if (activeSpan) {
      activeSpan.setAttributes({
        'session.id': this.sessionId,
        ...attributes,
      });
    }
  }

  /**
   * Force flush all pending spans
   */
  async flush() {
    if (this.telemetryConfig) {
      await this.telemetryConfig.forceFlush();
    }
  }

  /**
   * Shutdown tracing
   */
  async shutdown() {
    if (this.telemetryConfig) {
      await this.telemetryConfig.shutdown();
    }
  }
}