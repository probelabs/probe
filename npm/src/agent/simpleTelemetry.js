import { existsSync, mkdirSync, createWriteStream } from 'fs';
import { AsyncLocalStorage } from 'async_hooks';
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

  createSpan(name, attributes = {}, options = {}) {
    const traceId = options.traceId || this.generateTraceId();
    const parentSpanId = options.parentSpanId || null;
    const spanKind = options.spanKind || 'INTERNAL';
    const span = {
      traceId,
      spanId: this.generateSpanId(),
      parentSpanId,
      spanKind,
      name,
      startTime: Date.now(),
      attributes: { ...attributes, service: this.serviceName },
      events: [],
      status: 'OK',
      statusMessage: null
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
        if (typeof status === 'string') {
          span.status = status;
          return;
        }
        if (status && typeof status === 'object') {
          const code = status.code ?? status.statusCode;
          if (code === 2) {
            span.status = 'ERROR';
          } else if (code === 1) {
            span.status = 'OK';
          } else if (code === 0) {
            span.status = 'UNSET';
          }
          if (status.message) {
            span.statusMessage = status.message;
          }
        }
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
    this.contextStore = new AsyncLocalStorage();
  }

  generateSessionId() {
    return Math.random().toString(36).substring(2, 15);
  }

  isEnabled() {
    return this.telemetry !== null;
  }

  createSessionSpan(attributes = {}) {
    if (!this.isEnabled()) return null;

    return this.createSpanWithContext('agent.session', {
      'session.id': this.sessionId,
      ...attributes
    });
  }

  createAISpan(modelName, provider, attributes = {}) {
    if (!this.isEnabled()) return null;

    return this.createSpanWithContext('ai.request', {
      'ai.model': modelName,
      'ai.provider': provider,
      'session.id': this.sessionId,
      ...attributes
    }, { spanKind: 'CLIENT' });
  }

  createToolSpan(toolName, attributes = {}) {
    if (!this.isEnabled()) return null;

    return this.createSpanWithContext('tool.call', {
      'tool.name': toolName,
      'session.id': this.sessionId,
      ...attributes
    });
  }

  createDelegationSpan(sessionId, task, attributes = {}) {
    if (!this.isEnabled()) return null;

    return this.createSpanWithContext('delegation.session', {
      'delegation.session_id': sessionId,
      'delegation.task': task,
      'session.id': this.sessionId,
      ...attributes
    });
  }

  addEvent(name, attributes = {}) {
    const store = this.contextStore.getStore();
    if (store?.span?.addEvent) {
      store.span.addEvent(name, attributes);
      return;
    }
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

  setAttributes(attributes) {
    const store = this.contextStore.getStore();
    if (store?.span?.setAttributes) {
      store.span.setAttributes(attributes);
      return;
    }
    if (this.telemetry && this.telemetry.enableConsole) {
      console.log('[Attributes]', attributes);
    }
  }

  createSpanWithContext(name, attributes = {}, options = {}) {
    if (!this.isEnabled()) return null;
    const parent = this.contextStore.getStore();
    const spanOptions = {
      traceId: options.traceId || parent?.traceId,
      parentSpanId: options.parentSpanId || parent?.spanId,
      spanKind: options.spanKind
    };
    return this.telemetry.createSpan(name, {
      'session.id': this.sessionId,
      ...attributes
    }, spanOptions);
  }

  async withSpan(spanName, fn, attributes = {}, options = {}) {
    if (!this.isEnabled()) {
      return fn();
    }

    const span = this.createSpanWithContext(spanName, attributes, options);
    const context = { traceId: span.traceId, spanId: span.spanId, span };

    return this.contextStore.run(context, async () => {
      try {
        const result = await fn(span);
        span.setStatus('OK');
        return result;
      } catch (error) {
        span.setStatus({ code: 2, message: error.message });
        span.addEvent('exception', { 
          'exception.message': error.message,
          'exception.stack': error.stack 
        });
        throw error;
      } finally {
        span.end();
      }
    });
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
