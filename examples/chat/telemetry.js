import { NodeSDK } from '@opentelemetry/sdk-node';
import { resourceFromAttributes } from '@opentelemetry/resources';
import { ATTR_SERVICE_NAME, ATTR_SERVICE_VERSION } from '@opentelemetry/semantic-conventions';
import { trace, context } from '@opentelemetry/api';
import { OTLPTraceExporter } from '@opentelemetry/exporter-trace-otlp-http';
import { BatchSpanProcessor, ConsoleSpanExporter } from '@opentelemetry/sdk-trace-base';
import { existsSync, mkdirSync } from 'fs';
import { dirname } from 'path';
import { FileSpanExporter } from './fileSpanExporter.js';

/**
 * Custom OpenTelemetry configuration for probe-chat
 */
export class TelemetryConfig {
  constructor(options = {}) {
    this.serviceName = options.serviceName || 'probe-chat';
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
      if (process.env.PROBE_NON_INTERACTIVE !== '1' || process.env.DEBUG_CHAT === '1') {
        console.warn('Telemetry already initialized');
      }
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
        // CRITICAL FIX: Configure BatchSpanProcessor with shorter delays for better span export
        spanProcessors.push(new BatchSpanProcessor(fileExporter, {
          // The maximum queue size. After the size is reached spans are dropped.
          maxQueueSize: 2048,
          // The maximum batch size of every export. It must be smaller or equal to maxQueueSize.
          maxExportBatchSize: 512,
          // The interval between two consecutive exports
          scheduledDelayMillis: 500, // Reduced from default 5000ms
          // How long the export can run before it is cancelled
          exportTimeoutMillis: 30000,
        }));
        if (process.env.PROBE_NON_INTERACTIVE !== '1' || process.env.DEBUG_CHAT === '1') {
          console.log(`[Telemetry] File exporter enabled, writing to: ${this.filePath}`);
        }
      } catch (error) {
        if (process.env.PROBE_NON_INTERACTIVE !== '1' || process.env.DEBUG_CHAT === '1') {
          console.error(`[Telemetry] Failed to initialize file exporter: ${error.message}`);
        }
      }
    }

    // Add remote exporter if enabled
    if (this.enableRemote) {
      try {
        const remoteExporter = new OTLPTraceExporter({
          url: this.remoteEndpoint,
        });
        // Configure BatchSpanProcessor with shorter delays for better span export
        spanProcessors.push(new BatchSpanProcessor(remoteExporter, {
          maxQueueSize: 2048,
          maxExportBatchSize: 512,
          scheduledDelayMillis: 500, // Reduced from default 5000ms
          exportTimeoutMillis: 30000,
        }));
        if (process.env.PROBE_NON_INTERACTIVE !== '1' || process.env.DEBUG_CHAT === '1') {
          console.log(`[Telemetry] Remote exporter enabled, endpoint: ${this.remoteEndpoint}`);
        }
      } catch (error) {
        if (process.env.PROBE_NON_INTERACTIVE !== '1' || process.env.DEBUG_CHAT === '1') {
          console.error(`[Telemetry] Failed to initialize remote exporter: ${error.message}`);
        }
      }
    }

    // Add console exporter if enabled (useful for debugging)
    if (this.enableConsole) {
      const consoleExporter = new ConsoleSpanExporter();
      // Configure BatchSpanProcessor with shorter delays for better span export
      spanProcessors.push(new BatchSpanProcessor(consoleExporter, {
        maxQueueSize: 2048,
        maxExportBatchSize: 512,
        scheduledDelayMillis: 500, // Reduced from default 5000ms
        exportTimeoutMillis: 30000,
      }));
      if (process.env.PROBE_NON_INTERACTIVE !== '1' || process.env.DEBUG_CHAT === '1') {
        console.log(`[Telemetry] Console exporter enabled`);
      }
    }

    if (spanProcessors.length === 0) {
      if (process.env.PROBE_NON_INTERACTIVE !== '1' || process.env.DEBUG_CHAT === '1') {
        console.log('[Telemetry] No exporters configured, telemetry will not be collected');
      }
      return;
    }

    this.sdk = new NodeSDK({
      resource,
      spanProcessors,
    });

    try {
      this.sdk.start();
      this.tracer = trace.getTracer(this.serviceName, this.serviceVersion);
      if (process.env.PROBE_NON_INTERACTIVE !== '1' || process.env.DEBUG_CHAT === '1') {
        console.log(`[Telemetry] OpenTelemetry SDK initialized successfully`);
      }
    } catch (error) {
      if (process.env.PROBE_NON_INTERACTIVE !== '1' || process.env.DEBUG_CHAT === '1') {
        console.error(`[Telemetry] Failed to start OpenTelemetry SDK: ${error.message}`);
      }
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
        span.setStatus({ code: trace.SpanStatusCode.OK });
        return result;
      } catch (error) {
        span.setStatus({
          code: trace.SpanStatusCode.ERROR,
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
        // Get the active tracer provider
        const tracerProvider = trace.getTracerProvider();
        
        if (tracerProvider && typeof tracerProvider.forceFlush === 'function') {
          // Call forceFlush on the tracer provider
          await tracerProvider.forceFlush();
          
          if (process.env.DEBUG_CHAT === '1') {
            console.log('[Telemetry] TracerProvider flushed successfully');
          }
        }
        
        // Also try to access registered span processors directly for better control
        if (tracerProvider._registeredSpanProcessors) {
          const flushPromises = [];
          
          for (const processor of tracerProvider._registeredSpanProcessors) {
            if (typeof processor.forceFlush === 'function') {
              flushPromises.push(processor.forceFlush());
            }
          }
          
          if (flushPromises.length > 0) {
            await Promise.all(flushPromises);
            
            if (process.env.DEBUG_CHAT === '1') {
              console.log(`[Telemetry] Directly flushed ${flushPromises.length} span processors`);
            }
          }
        }
        
        // Add a small delay to ensure file writes complete
        await new Promise(resolve => setTimeout(resolve, 100));
        
        if (process.env.DEBUG_CHAT === '1') {
          console.log('[Telemetry] OpenTelemetry spans flushed successfully');
        }
      } catch (error) {
        if (process.env.PROBE_NON_INTERACTIVE !== '1' || process.env.DEBUG_CHAT === '1') {
          console.error(`[Telemetry] Failed to flush OpenTelemetry spans: ${error.message}`);
        }
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
        if (process.env.PROBE_NON_INTERACTIVE !== '1' || process.env.DEBUG_CHAT === '1') {
          console.log('[Telemetry] OpenTelemetry SDK shutdown successfully');
        }
      } catch (error) {
        if (process.env.PROBE_NON_INTERACTIVE !== '1' || process.env.DEBUG_CHAT === '1') {
          console.error(`[Telemetry] Failed to shutdown OpenTelemetry SDK: ${error.message}`);
        }
      }
    }
  }
}

/**
 * Default telemetry configuration
 */
export const defaultTelemetryConfig = new TelemetryConfig();

/**
 * Initialize telemetry from environment variables
 */
export function initializeTelemetryFromEnv() {
  const config = new TelemetryConfig({
    serviceName: process.env.OTEL_SERVICE_NAME || 'probe-chat',
    serviceVersion: process.env.OTEL_SERVICE_VERSION || '1.0.0',
    enableFile: process.env.OTEL_ENABLE_FILE === 'true',
    enableRemote: process.env.OTEL_ENABLE_REMOTE === 'true',
    enableConsole: process.env.OTEL_ENABLE_CONSOLE === 'true',
    filePath: process.env.OTEL_FILE_PATH || './traces.jsonl',
    remoteEndpoint: process.env.OTEL_EXPORTER_OTLP_TRACES_ENDPOINT || 'http://localhost:4318/v1/traces',
  });

  config.initialize();
  return config;
}