/**
 * OTEL Log Bridge — patches console.log/info/warn/error to:
 * 1. Append trace context [trace_id=... span_id=...] to output (like visor2)
 * 2. Emit each log as an OTEL Log Record via @opentelemetry/api-logs
 *
 * Lazy-loads @opentelemetry/api and @opentelemetry/api-logs.
 * If packages are not installed, patching is a no-op.
 *
 * Usage:
 *   import { patchConsole, unpatchConsole } from './otelLogBridge.js';
 *   patchConsole();  // Call once at startup
 */

import { createRequire } from 'module';

// createRequire for loading optional @opentelemetry packages in ESM context
const _require = createRequire(import.meta.url);

// OTel severity mapping (OpenTelemetry SeverityNumber values)
const OTEL_SEVERITY = {
  log: 9,     // INFO
  info: 9,    // INFO
  warn: 13,   // WARN
  error: 17,  // ERROR
  debug: 5,   // DEBUG
};

// Track patch state
let patched = false;
const originals = {};

// Lazy-loaded OTel references
let otelApi = null;
let otelApiAttempted = false;
let otelLogger = null;
let otelLoggerAttempted = false;

/**
 * Try to load @opentelemetry/api lazily.
 * Returns { trace, context } or null if not available.
 */
function getOtelApi() {
  if (otelApiAttempted) return otelApi;
  otelApiAttempted = true;
  try {
    // Dynamic require wrapped in IIFE to prevent bundler from resolving
    otelApi = (function(name) { return _require(name); })('@opentelemetry/api');
  } catch {
    // @opentelemetry/api not installed
  }
  return otelApi;
}

/**
 * Try to get an OTEL Logger from @opentelemetry/api-logs lazily.
 * Returns a logger instance or null if not available.
 */
function getOtelLogger() {
  if (otelLoggerAttempted) return otelLogger;
  otelLoggerAttempted = true;
  try {
    const { logs } = (function(name) { return _require(name); })('@opentelemetry/api-logs');
    otelLogger = logs.getLogger('probe-agent');
  } catch {
    // @opentelemetry/api-logs not installed
  }
  return otelLogger;
}

/**
 * Extract trace context suffix from the active OTel span.
 * Returns '' if no active span or OTel is not available.
 */
function getTraceSuffix() {
  try {
    const api = getOtelApi();
    if (!api) return '';
    const span = api.trace.getSpan(api.context.active());
    const ctx = span?.spanContext?.();
    if (!ctx?.traceId) return '';
    return ` [trace_id=${ctx.traceId} span_id=${ctx.spanId}]`;
  } catch {
    return '';
  }
}

/**
 * Emit a log record to the OTEL Logs pipeline.
 * Non-blocking, best-effort — errors are silently ignored.
 */
function emitOtelLog(msg, level) {
  try {
    const logger = getOtelLogger();
    if (!logger) return;

    const api = getOtelApi();
    let traceId, spanId;
    if (api) {
      const span = api.trace.getSpan(api.context.active());
      const ctx = span?.spanContext?.();
      if (ctx?.traceId) {
        traceId = ctx.traceId;
        spanId = ctx.spanId;
      }
    }

    logger.emit({
      severityNumber: OTEL_SEVERITY[level] || 9,
      severityText: level.toUpperCase(),
      body: msg,
      attributes: {
        'probe.logger': true,
        ...(traceId ? { trace_id: traceId, span_id: spanId } : {}),
      },
    });
  } catch {
    // OTel logs not available; ignore
  }
}

/**
 * Patch console.log/info/warn/error to:
 * - Append trace context to output
 * - Emit OTEL log records
 *
 * Safe to call multiple times — only patches once.
 */
export function patchConsole() {
  if (patched) return;

  const methods = ['log', 'info', 'warn', 'error'];
  const c = globalThis.console;

  for (const m of methods) {
    const orig = c[m].bind(c);
    originals[m] = orig;

    c[m] = (...args) => {
      // Build the message string for OTEL log emission
      const msgParts = args.map(a =>
        typeof a === 'string' ? a : (a instanceof Error ? a.message : JSON.stringify(a))
      );
      const msg = msgParts.join(' ');

      // Emit to OTEL Logs pipeline (non-blocking, best-effort)
      emitOtelLog(msg, m === 'log' ? 'log' : m);

      // Append trace context suffix to console output
      const suffix = getTraceSuffix();
      if (suffix) {
        if (typeof args[0] === 'string') {
          args[0] = args[0] + suffix;
        } else {
          args.push(suffix);
        }
      }

      return orig(...args);
    };
  }

  patched = true;
}

/**
 * Restore original console methods.
 * Useful for testing or cleanup.
 */
export function unpatchConsole() {
  if (!patched) return;

  const c = globalThis.console;
  for (const [m, orig] of Object.entries(originals)) {
    c[m] = orig;
  }
  patched = false;
}

/**
 * Check if console is currently patched.
 */
export function isConsolePatched() {
  return patched;
}
