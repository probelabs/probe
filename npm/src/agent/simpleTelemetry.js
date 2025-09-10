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

  setAttributes(attributes) {
    // For simplicity, just log attributes when no active span
    if (this.telemetry && this.telemetry.enableConsole) {
      console.log('[Attributes]', attributes);
    }
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