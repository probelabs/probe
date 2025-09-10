import nodeSDKPkg from '@opentelemetry/sdk-node';
import resourcesPkg from '@opentelemetry/resources';
import semanticConventionsPkg from '@opentelemetry/semantic-conventions';
import { trace, context, SpanStatusCode } from '@opentelemetry/api';
import otlpPkg from '@opentelemetry/exporter-trace-otlp-http';
import spanPkg from '@opentelemetry/sdk-trace-base';

import { existsSync, mkdirSync } from 'fs';
import { dirname } from 'path';
import { FileSpanExporter } from './fileSpanExporter.js';

const { NodeSDK } = nodeSDKPkg;
const { resourceFromAttributes } = resourcesPkg;
const { ATTR_SERVICE_NAME, ATTR_SERVICE_VERSION } = semanticConventionsPkg;
const { OTLPTraceExporter } = otlpPkg;
const { BatchSpanProcessor, ConsoleSpanExporter } = spanPkg;

/**
 * Custom OpenTelemetry configuration for probe-agent
 */
export class TelemetryConfig {
  constructor(options = {}) {
    this.serviceName = options.serviceName || 'probe-agent';
    this.serviceVersion = options.serviceVersion || '1.0.0';
    this.enableFile = options.enableFile || false;
    this.enableRemote = options.enableRemote || false;
    this.enableConsole = options.enableConsole || false;
    this.filePath = options.filePath || './traces.jsonl';
    this.remoteEndpoint = options.remoteEndpoint || 'http://localhost:4318/v1/traces';
    this.sdk = null;
    this.tracer = null;
  }

  /**
   * Initialize OpenTelemetry SDK
   */
  initialize() {
    if (this.sdk) {
      console.warn('Telemetry already initialized');
      return;
    }

    const resource = resourceFromAttributes({
      [ATTR_SERVICE_NAME]: this.serviceName,
      [ATTR_SERVICE_VERSION]: this.serviceVersion,
    });

    const spanProcessors = [];

    // Add file exporter if enabled
    if (this.enableFile) {
      try {
        // Ensure the directory exists
        const dir = dirname(this.filePath);
        if (!existsSync(dir)) {
          mkdirSync(dir, { recursive: true });
        }
        
        const fileExporter = new FileSpanExporter(this.filePath);
        spanProcessors.push(new BatchSpanProcessor(fileExporter, {
          maxQueueSize: 2048,
          maxExportBatchSize: 512,
          scheduledDelayMillis: 500,
          exportTimeoutMillis: 30000,
        }));
        console.log(`[Telemetry] File exporter enabled, writing to: ${this.filePath}`);
      } catch (error) {
        console.error(`[Telemetry] Failed to initialize file exporter: ${error.message}`);
      }
    }

    // Add remote exporter if enabled
    if (this.enableRemote) {
      try {
        const remoteExporter = new OTLPTraceExporter({
          url: this.remoteEndpoint,
        });
        spanProcessors.push(new BatchSpanProcessor(remoteExporter, {
          maxQueueSize: 2048,
          maxExportBatchSize: 512,
          scheduledDelayMillis: 500,
          exportTimeoutMillis: 30000,
        }));
        console.log(`[Telemetry] Remote exporter enabled, endpoint: ${this.remoteEndpoint}`);
      } catch (error) {
        console.error(`[Telemetry] Failed to initialize remote exporter: ${error.message}`);
      }
    }

    // Add console exporter if enabled (useful for debugging)
    if (this.enableConsole) {
      const consoleExporter = new ConsoleSpanExporter();
      spanProcessors.push(new BatchSpanProcessor(consoleExporter, {
        maxQueueSize: 2048,
        maxExportBatchSize: 512,
        scheduledDelayMillis: 500,
        exportTimeoutMillis: 30000,
      }));
      console.log(`[Telemetry] Console exporter enabled`);
    }

    if (spanProcessors.length === 0) {
      console.log('[Telemetry] No exporters configured, telemetry will not be collected');
      return;
    }

    this.sdk = new NodeSDK({
      resource,
      spanProcessors,
    });

    try {
      this.sdk.start();
      this.tracer = trace.getTracer(this.serviceName, this.serviceVersion);
      console.log(`[Telemetry] OpenTelemetry SDK initialized successfully`);
    } catch (error) {
      console.error(`[Telemetry] Failed to start OpenTelemetry SDK: ${error.message}`);
    }
  }

  /**
   * Get the tracer instance
   */
  getTracer() {
    return this.tracer;
  }

  /**
   * Create a span with the given name and attributes
   */
  createSpan(name, attributes = {}) {
    if (!this.tracer) {
      return null;
    }

    return this.tracer.startSpan(name, {
      attributes,
    });
  }

  /**
   * Wrap a function to automatically create spans
   */
  wrapFunction(name, fn, attributes = {}) {
    if (!this.tracer) {
      return fn;
    }

    return async (...args) => {
      const span = this.createSpan(name, attributes);
      if (!span) {
        return fn(...args);
      }

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
   * Force flush all pending spans
   */
  async forceFlush() {
    if (this.sdk) {
      try {
        const tracerProvider = trace.getTracerProvider();
        
        if (tracerProvider && typeof tracerProvider.forceFlush === 'function') {
          await tracerProvider.forceFlush();
        }
        
        // Add a small delay to ensure file writes complete
        await new Promise(resolve => setTimeout(resolve, 100));
        
        console.log('[Telemetry] OpenTelemetry spans flushed successfully');
      } catch (error) {
        console.error(`[Telemetry] Failed to flush OpenTelemetry spans: ${error.message}`);
      }
    }
  }

  /**
   * Shutdown telemetry
   */
  async shutdown() {
    if (this.sdk) {
      try {
        await this.sdk.shutdown();
        console.log('[Telemetry] OpenTelemetry SDK shutdown successfully');
      } catch (error) {
        console.error(`[Telemetry] Failed to shutdown OpenTelemetry SDK: ${error.message}`);
      }
    }
  }
}

/**
 * Initialize telemetry from CLI options
 */
export function initializeTelemetryFromOptions(options) {
  const config = new TelemetryConfig({
    serviceName: 'probe-agent',
    serviceVersion: '1.0.0',
    enableFile: options.traceFile !== undefined,
    enableRemote: options.traceRemote !== undefined,
    enableConsole: options.traceConsole,
    filePath: options.traceFile || './traces.jsonl',
    remoteEndpoint: options.traceRemote || 'http://localhost:4318/v1/traces',
  });

  config.initialize();
  return config;
}