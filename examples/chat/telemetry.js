import nodeSDKPkg from '@opentelemetry/sdk-node';
import resourcesPkg from '@opentelemetry/resources';
import semanticConventionsPkg from '@opentelemetry/semantic-conventions';
import { trace, context } from '@opentelemetry/api';
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
        spanProcessors.push(new BatchSpanProcessor(fileExporter));
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
        spanProcessors.push(new BatchSpanProcessor(remoteExporter));
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
      spanProcessors.push(new BatchSpanProcessor(consoleExporter));
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