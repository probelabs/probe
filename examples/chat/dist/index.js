#!/usr/bin/env node
var __defProp = Object.defineProperty;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __name = (target, value) => __defProp(target, "name", { value, configurable: true });
var __esm = (fn, res) => function __init() {
  return fn && (res = (0, fn[__getOwnPropNames(fn)[0]])(fn = 0)), res;
};
var __export = (target, all) => {
  for (var name in all)
    __defProp(target, name, { get: all[name], enumerable: true });
};

// tokenCounter.js
import { get_encoding } from "tiktoken";
var TokenCounter;
var init_tokenCounter = __esm({
  "tokenCounter.js"() {
    TokenCounter = class {
      static {
        __name(this, "TokenCounter");
      }
      constructor() {
        try {
          this.tokenizer = get_encoding("cl100k_base");
          this.contextSize = 0;
          this.history = [];
          this.requestTokens = 0;
          this.responseTokens = 0;
          this.currentRequestTokens = 0;
          this.currentResponseTokens = 0;
          this.cacheCreationTokens = 0;
          this.cacheReadTokens = 0;
          this.currentCacheCreationTokens = 0;
          this.currentCacheReadTokens = 0;
          this.cachedPromptTokens = 0;
          this.currentCachedPromptTokens = 0;
        } catch (error) {
          console.error("Error initializing tokenizer:", error);
          this.tokenizer = null;
          this.contextSize = 0;
          this.requestTokens = 0;
          this.responseTokens = 0;
          this.currentRequestTokens = 0;
          this.currentResponseTokens = 0;
          this.cacheCreationTokens = 0;
          this.cacheReadTokens = 0;
          this.currentCacheCreationTokens = 0;
          this.currentCacheReadTokens = 0;
          this.cachedPromptTokens = 0;
          this.currentCachedPromptTokens = 0;
          this.history = [];
        }
        this.debug = process.env.DEBUG_CHAT === "1";
      }
      /**
       * Count tokens in a string using tiktoken or fallback method
       * @param {string} text - The text to count tokens for
       * @returns {number} - The number of tokens
       */
      countTokens(text) {
        if (typeof text !== "string") {
          text = String(text);
        }
        if (this.tokenizer) {
          try {
            const tokens = this.tokenizer.encode(text);
            return tokens.length;
          } catch (error) {
            return Math.ceil(text.length / 4);
          }
        } else {
          return Math.ceil(text.length / 4);
        }
      }
      /**
       * Add to request token count (manual counting, less used now with recordUsage)
       * @param {string|number} input - The text to count tokens for or the token count directly
       */
      addRequestTokens(input) {
        let tokenCount = 0;
        if (typeof input === "number") {
          tokenCount = input;
        } else if (typeof input === "string") {
          tokenCount = this.countTokens(input);
        } else {
          console.warn("[WARN] Invalid input type for addRequestTokens:", typeof input);
          return;
        }
        this.requestTokens += tokenCount;
        this.currentRequestTokens = tokenCount;
        if (this.debug) {
          console.log(`[DEBUG] (Manual) Added ${tokenCount} request tokens. Total: ${this.requestTokens}, Current: ${this.currentRequestTokens}`);
        }
      }
      /**
       * Add to response token count (manual counting, less used now with recordUsage)
       * @param {string|number} input - The text to count tokens for or the token count directly
       */
      addResponseTokens(input) {
        let tokenCount = 0;
        if (typeof input === "number") {
          tokenCount = input;
        } else if (typeof input === "string") {
          tokenCount = this.countTokens(input);
        } else {
          console.warn("[WARN] Invalid input type for addResponseTokens:", typeof input);
          return;
        }
        this.responseTokens += tokenCount;
        this.currentResponseTokens = tokenCount;
        if (this.debug) {
          console.log(`[DEBUG] (Manual) Added ${tokenCount} response tokens. Total: ${this.responseTokens}, Current: ${this.currentResponseTokens}`);
        }
      }
      /**
       * Record token usage from the AI SDK's result for a single LLM call.
       * This resets 'current' counters and updates totals.
       * @param {Object} usage - The usage object { promptTokens, completionTokens, totalTokens }
       * @param {Object} providerMetadata - Metadata possibly containing cache info
       */
      recordUsage(usage, providerMetadata) {
        if (!usage) {
          console.warn("[WARN] No usage information provided to recordUsage");
          return;
        }
        this.currentRequestTokens = 0;
        this.currentResponseTokens = 0;
        this.currentCacheCreationTokens = 0;
        this.currentCacheReadTokens = 0;
        this.currentCachedPromptTokens = 0;
        const promptTokens = Number(usage.promptTokens) || 0;
        const completionTokens = Number(usage.completionTokens) || 0;
        this.currentRequestTokens = promptTokens;
        this.currentResponseTokens = completionTokens;
        this.requestTokens += promptTokens;
        this.responseTokens += completionTokens;
        if (providerMetadata?.anthropic) {
          const cacheCreation = Number(providerMetadata.anthropic.cacheCreationInputTokens) || 0;
          const cacheRead = Number(providerMetadata.anthropic.cacheReadInputTokens) || 0;
          this.currentCacheCreationTokens = cacheCreation;
          this.currentCacheReadTokens = cacheRead;
          this.cacheCreationTokens += cacheCreation;
          this.cacheReadTokens += cacheRead;
          if (this.debug) {
            console.log(`[DEBUG] Anthropic cache tokens (current): creation=${cacheCreation}, read=${cacheRead}`);
          }
        }
        if (providerMetadata?.openai) {
          const cachedPrompt = Number(providerMetadata.openai.cachedPromptTokens) || 0;
          this.currentCachedPromptTokens = cachedPrompt;
          this.cachedPromptTokens += cachedPrompt;
          if (this.debug) {
            console.log(`[DEBUG] OpenAI cached prompt tokens (current): ${cachedPrompt}`);
          }
        }
        if (this.debug) {
          console.log(
            `[DEBUG] Recorded usage: current(req=${this.currentRequestTokens}, resp=${this.currentResponseTokens}), total(req=${this.requestTokens}, resp=${this.responseTokens})`
          );
          console.log(`[DEBUG] Total cache tokens: Anthropic(create=${this.cacheCreationTokens}, read=${this.cacheReadTokens}), OpenAI(prompt=${this.cachedPromptTokens})`);
        }
      }
      /**
       * Calculate the current context window size based on provided messages or internal history.
       * @param {Array|null} messages - Optional messages array to use for calculation. If null, uses internal this.history.
       * @returns {number} - Total tokens estimated in the context window.
       */
      calculateContextSize(messages = null) {
        const msgsToCount = messages !== null ? messages : this.history;
        let totalTokens = 0;
        if (this.debug && messages === null) {
          console.log(`[DEBUG] Calculating context size from internal history (${this.history.length} messages)`);
        }
        for (const msg of msgsToCount) {
          let messageTokens = 0;
          messageTokens += 4;
          if (typeof msg.content === "string") {
            messageTokens += this.countTokens(msg.content);
          } else if (Array.isArray(msg.content)) {
            for (const item of msg.content) {
              if (item.type === "text" && typeof item.text === "string") {
                messageTokens += this.countTokens(item.text);
              } else if (item.type === "image" && item.image) {
                if (item.image.startsWith("data:image/")) {
                  const base64Length = item.image.length;
                  const estimatedImageTokens = Math.min(Math.max(Math.floor(base64Length / 1e3), 500), 2e3);
                  messageTokens += estimatedImageTokens;
                } else {
                  messageTokens += 1e3;
                }
              } else {
                messageTokens += this.countTokens(JSON.stringify(item));
              }
            }
          } else if (msg.content) {
            messageTokens += this.countTokens(JSON.stringify(msg.content));
          }
          if (msg.toolCalls) {
            messageTokens += this.countTokens(JSON.stringify(msg.toolCalls));
            messageTokens += 5;
          }
          if (msg.role === "tool" && msg.toolCallId) {
            messageTokens += this.countTokens(msg.toolCallId);
            messageTokens += 5;
          }
          if (msg.toolCallResults) {
            messageTokens += this.countTokens(JSON.stringify(msg.toolCallResults));
            messageTokens += 5;
          }
          totalTokens += messageTokens;
        }
        if (messages === null) {
          this.contextSize = totalTokens;
          if (this.debug) {
            console.log(`[DEBUG] Updated internal context size: ${this.contextSize} tokens`);
          }
        }
        return totalTokens;
      }
      /**
       * Update internal history and recalculate internal context window size.
       * @param {Array} messages - New message history array.
       */
      updateHistory(messages) {
        if (!Array.isArray(messages)) {
          console.warn("[WARN] updateHistory called with non-array:", messages);
          this.history = [];
        } else {
          this.history = [...messages];
        }
        this.calculateContextSize();
        if (this.debug) {
          console.log(`[DEBUG] History updated (${this.history.length} messages). Recalculated context size: ${this.contextSize}`);
        }
      }
      /**
       * Clear all counters and internal history. Reset context size.
       */
      clear() {
        this.requestTokens = 0;
        this.responseTokens = 0;
        this.currentRequestTokens = 0;
        this.currentResponseTokens = 0;
        this.cacheCreationTokens = 0;
        this.cacheReadTokens = 0;
        this.currentCacheCreationTokens = 0;
        this.currentCacheReadTokens = 0;
        this.cachedPromptTokens = 0;
        this.currentCachedPromptTokens = 0;
        this.history = [];
        this.contextSize = 0;
        if (this.debug) {
          console.log("[DEBUG] TokenCounter cleared: usage, history, and context size reset.");
        }
      }
      /**
       * Start a new conversation turn - reset CURRENT token counters.
       * Calculates context size based on history *before* the new turn.
       */
      startNewTurn() {
        this.currentRequestTokens = 0;
        this.currentResponseTokens = 0;
        this.currentCacheCreationTokens = 0;
        this.currentCacheReadTokens = 0;
        this.currentCachedPromptTokens = 0;
        this.calculateContextSize();
        if (this.debug) {
          console.log("[DEBUG] TokenCounter: New turn started. Current counters reset.");
          console.log(`[DEBUG] Context size at start of turn: ${this.contextSize} tokens`);
        }
      }
      /**
       * Get the current token usage state including context size.
       * Recalculates context size from internal history before returning.
       * @returns {Object} - Object containing current turn, total session, and context window usage.
       */
      getTokenUsage() {
        const currentContextSize = this.calculateContextSize();
        const currentCacheRead = this.currentCacheReadTokens + this.currentCachedPromptTokens;
        const currentCacheWrite = this.currentCacheCreationTokens;
        const totalCacheRead = this.cacheReadTokens + this.cachedPromptTokens;
        const totalCacheWrite = this.cacheCreationTokens;
        const usageData = {
          contextWindow: currentContextSize,
          // Use the freshly calculated value
          current: {
            // Usage for the *last* LLM call recorded
            request: this.currentRequestTokens,
            response: this.currentResponseTokens,
            total: this.currentRequestTokens + this.currentResponseTokens,
            cacheRead: currentCacheRead,
            cacheWrite: currentCacheWrite,
            cacheTotal: currentCacheRead + currentCacheWrite,
            // Keep detailed breakdown if needed
            anthropic: {
              cacheCreation: this.currentCacheCreationTokens,
              cacheRead: this.currentCacheReadTokens
            },
            openai: {
              cachedPrompt: this.currentCachedPromptTokens
            }
          },
          total: {
            // Accumulated usage over the session
            request: this.requestTokens,
            response: this.responseTokens,
            total: this.requestTokens + this.responseTokens,
            cacheRead: totalCacheRead,
            cacheWrite: totalCacheWrite,
            cacheTotal: totalCacheRead + totalCacheWrite,
            // Keep detailed breakdown if needed
            anthropic: {
              cacheCreation: this.cacheCreationTokens,
              cacheRead: this.cacheReadTokens
            },
            openai: {
              cachedPrompt: this.cachedPromptTokens
            }
          }
        };
        if (this.debug) {
        }
        return usageData;
      }
    };
  }
});

// tokenUsageDisplay.js
import chalk from "chalk";
var TokenUsageDisplay;
var init_tokenUsageDisplay = __esm({
  "tokenUsageDisplay.js"() {
    TokenUsageDisplay = class {
      static {
        __name(this, "TokenUsageDisplay");
      }
      /**
       * Format a number with commas
       * @param {number} num Number to format
       * @returns {string} Formatted number
       */
      formatNumber(num) {
        return num.toLocaleString();
      }
      /**
       * Format cache tokens
       * @param {Object} tokens Token data
       * @returns {Object} Formatted cache data
       */
      formatCacheTokens(tokens = {}) {
        const totalCacheRead = tokens.cacheRead !== void 0 ? tokens.cacheRead : ((tokens.anthropic || {}).cacheRead || 0) + ((tokens.openai || {}).cachedPrompt || 0);
        const totalCacheWrite = tokens.cacheWrite !== void 0 ? tokens.cacheWrite : (tokens.anthropic || {}).cacheCreation || 0;
        const totalCache = tokens.cacheTotal !== void 0 ? tokens.cacheTotal : totalCacheRead + totalCacheWrite;
        return {
          read: this.formatNumber(totalCacheRead),
          write: this.formatNumber(totalCacheWrite),
          total: this.formatNumber(totalCache)
        };
      }
      /**
       * Format usage data for UI display
       * @param {Object} usage Token usage data
       * @returns {Object} Formatted usage data
       */
      format(usage) {
        const contextWindow = usage.contextWindow || 100;
        const current = usage.current || {};
        const formatted = {
          contextWindow: this.formatNumber(contextWindow),
          current: {
            request: this.formatNumber(current.request || 0),
            response: this.formatNumber(current.response || 0),
            total: this.formatNumber(current.total || 0),
            cacheRead: this.formatNumber(current.cacheRead || 0),
            cacheWrite: this.formatNumber(current.cacheWrite || 0),
            cache: this.formatCacheTokens(current)
          },
          total: {
            request: this.formatNumber((usage.total || {}).request || 0),
            response: this.formatNumber((usage.total || {}).response || 0),
            total: this.formatNumber((usage.total || {}).total || 0),
            cacheRead: this.formatNumber((usage.total || {}).cacheRead || 0),
            cacheWrite: this.formatNumber((usage.total || {}).cacheWrite || 0),
            cache: this.formatCacheTokens(usage.total || {})
          }
        };
        return formatted;
      }
    };
  }
});

// fileSpanExporter.js
import { createWriteStream } from "fs";
import corePkg from "@opentelemetry/core";
var ExportResultCode, FileSpanExporter;
var init_fileSpanExporter = __esm({
  "fileSpanExporter.js"() {
    ({ ExportResultCode } = corePkg);
    FileSpanExporter = class {
      static {
        __name(this, "FileSpanExporter");
      }
      constructor(filePath = "./traces.jsonl") {
        this.filePath = filePath;
        this.stream = createWriteStream(filePath, { flags: "a" });
        this.stream.on("error", (error) => {
          console.error(`[FileSpanExporter] Stream error: ${error.message}`);
        });
      }
      /**
       * Export spans to file
       * @param {ReadableSpan[]} spans - Array of spans to export
       * @param {function} resultCallback - Callback to call with the export result
       */
      export(spans, resultCallback) {
        if (!spans || spans.length === 0) {
          resultCallback({ code: ExportResultCode.SUCCESS });
          return;
        }
        try {
          const timestamp = Date.now();
          spans.forEach((span, index) => {
            if (index === 0 && process.env.DEBUG_CHAT === "1") {
              console.log("[FileSpanExporter] First span properties:");
              const keys = Object.getOwnPropertyNames(span);
              keys.forEach((key) => {
                if (key.toLowerCase().includes("parent") || key === "_spanContext" || key === "parentContext") {
                  console.log(`  ${key}:`, span[key]);
                }
              });
            }
            let parentSpanId = void 0;
            if (span.parentSpanContext) {
              parentSpanId = span.parentSpanContext.spanId;
            } else if (span._parentSpanContext) {
              parentSpanId = span._parentSpanContext.spanId;
            } else if (span.parent) {
              parentSpanId = span.parent.spanId;
            } else if (span._parent) {
              parentSpanId = span._parent.spanId;
            } else if (span._parentId) {
              parentSpanId = span._parentId;
            } else if (span.parentSpanId) {
              parentSpanId = span.parentSpanId;
            }
            const spanData = {
              traceId: span.spanContext().traceId,
              spanId: span.spanContext().spanId,
              parentSpanId,
              name: span.name,
              kind: span.kind,
              startTimeUnixNano: span.startTime[0] * 1e9 + span.startTime[1],
              endTimeUnixNano: span.endTime[0] * 1e9 + span.endTime[1],
              attributes: this.convertAttributes(span.attributes),
              status: span.status,
              events: span.events?.map((event) => ({
                timeUnixNano: event.time[0] * 1e9 + event.time[1],
                name: event.name,
                attributes: this.convertAttributes(event.attributes)
              })) || [],
              links: span.links?.map((link) => ({
                traceId: link.context.traceId,
                spanId: link.context.spanId,
                attributes: this.convertAttributes(link.attributes)
              })) || [],
              resource: {
                attributes: this.convertAttributes(span.resource?.attributes || {})
              },
              instrumentationLibrary: {
                name: span.instrumentationLibrary?.name || "unknown",
                version: span.instrumentationLibrary?.version || "unknown"
              },
              timestamp
            };
            this.stream.write(JSON.stringify(spanData) + "\n");
          });
          resultCallback({ code: ExportResultCode.SUCCESS });
        } catch (error) {
          console.error(`[FileSpanExporter] Export error: ${error.message}`);
          resultCallback({
            code: ExportResultCode.FAILED,
            error
          });
        }
      }
      /**
       * Convert OpenTelemetry attributes to plain object
       * @param {Object} attributes - OpenTelemetry attributes
       * @returns {Object} Plain object with string values
       */
      convertAttributes(attributes) {
        if (!attributes) return {};
        const result = {};
        for (const [key, value] of Object.entries(attributes)) {
          if (typeof value === "object" && value !== null) {
            result[key] = JSON.stringify(value);
          } else {
            result[key] = String(value);
          }
        }
        return result;
      }
      /**
       * Shutdown the exporter
       * @returns {Promise<void>}
       */
      async shutdown() {
        return new Promise((resolve3) => {
          if (this.stream) {
            this.stream.end(() => {
              console.log(`[FileSpanExporter] File stream closed: ${this.filePath}`);
              resolve3();
            });
          } else {
            resolve3();
          }
        });
      }
      /**
       * Force flush any pending spans
       * @returns {Promise<void>}
       */
      async forceFlush() {
        return new Promise((resolve3, reject) => {
          if (this.stream) {
            const flushTimeout = setTimeout(() => {
              console.warn("[FileSpanExporter] Flush timeout after 5 seconds");
              resolve3();
            }, 5e3);
            if (this.stream.writableCorked) {
              this.stream.uncork();
            }
            if (this.stream.writableNeedDrain) {
              this.stream.once("drain", () => {
                clearTimeout(flushTimeout);
                resolve3();
              });
            } else {
              setImmediate(() => {
                clearTimeout(flushTimeout);
                resolve3();
              });
            }
          } else {
            resolve3();
          }
        });
      }
    };
  }
});

// telemetry.js
import nodeSDKPkg from "@opentelemetry/sdk-node";
import resourcesPkg from "@opentelemetry/resources";
import semanticConventionsPkg from "@opentelemetry/semantic-conventions";
import { trace, context } from "@opentelemetry/api";
import otlpPkg from "@opentelemetry/exporter-trace-otlp-http";
import spanPkg from "@opentelemetry/sdk-trace-base";
import { existsSync, mkdirSync } from "fs";
import { dirname } from "path";
var NodeSDK, resourceFromAttributes, ATTR_SERVICE_NAME, ATTR_SERVICE_VERSION, OTLPTraceExporter, BatchSpanProcessor, ConsoleSpanExporter, TelemetryConfig, defaultTelemetryConfig;
var init_telemetry = __esm({
  "telemetry.js"() {
    init_fileSpanExporter();
    ({ NodeSDK } = nodeSDKPkg);
    ({ resourceFromAttributes } = resourcesPkg);
    ({ ATTR_SERVICE_NAME, ATTR_SERVICE_VERSION } = semanticConventionsPkg);
    ({ OTLPTraceExporter } = otlpPkg);
    ({ BatchSpanProcessor, ConsoleSpanExporter } = spanPkg);
    TelemetryConfig = class {
      static {
        __name(this, "TelemetryConfig");
      }
      constructor(options = {}) {
        this.serviceName = options.serviceName || "probe-chat";
        this.serviceVersion = options.serviceVersion || "1.0.0";
        this.enableFile = options.enableFile || false;
        this.enableRemote = options.enableRemote || false;
        this.enableConsole = options.enableConsole || false;
        this.filePath = options.filePath || "./traces.jsonl";
        this.remoteEndpoint = options.remoteEndpoint || "http://localhost:4318/v1/traces";
        this.sdk = null;
        this.tracer = null;
      }
      /**
       * Initialize OpenTelemetry SDK
       */
      initialize() {
        if (this.sdk) {
          if (process.env.PROBE_NON_INTERACTIVE !== "1" || process.env.DEBUG_CHAT === "1") {
            console.warn("Telemetry already initialized");
          }
          return;
        }
        const resource = resourceFromAttributes({
          [ATTR_SERVICE_NAME]: this.serviceName,
          [ATTR_SERVICE_VERSION]: this.serviceVersion
        });
        const spanProcessors = [];
        if (this.enableFile) {
          try {
            const dir = dirname(this.filePath);
            if (!existsSync(dir)) {
              mkdirSync(dir, { recursive: true });
            }
            const fileExporter = new FileSpanExporter(this.filePath);
            spanProcessors.push(new BatchSpanProcessor(fileExporter, {
              // The maximum queue size. After the size is reached spans are dropped.
              maxQueueSize: 2048,
              // The maximum batch size of every export. It must be smaller or equal to maxQueueSize.
              maxExportBatchSize: 512,
              // The interval between two consecutive exports
              scheduledDelayMillis: 500,
              // Reduced from default 5000ms
              // How long the export can run before it is cancelled
              exportTimeoutMillis: 3e4
            }));
            if (process.env.PROBE_NON_INTERACTIVE !== "1" || process.env.DEBUG_CHAT === "1") {
              console.log(`[Telemetry] File exporter enabled, writing to: ${this.filePath}`);
            }
          } catch (error) {
            if (process.env.PROBE_NON_INTERACTIVE !== "1" || process.env.DEBUG_CHAT === "1") {
              console.error(`[Telemetry] Failed to initialize file exporter: ${error.message}`);
            }
          }
        }
        if (this.enableRemote) {
          try {
            const remoteExporter = new OTLPTraceExporter({
              url: this.remoteEndpoint
            });
            spanProcessors.push(new BatchSpanProcessor(remoteExporter, {
              maxQueueSize: 2048,
              maxExportBatchSize: 512,
              scheduledDelayMillis: 500,
              // Reduced from default 5000ms
              exportTimeoutMillis: 3e4
            }));
            if (process.env.PROBE_NON_INTERACTIVE !== "1" || process.env.DEBUG_CHAT === "1") {
              console.log(`[Telemetry] Remote exporter enabled, endpoint: ${this.remoteEndpoint}`);
            }
          } catch (error) {
            if (process.env.PROBE_NON_INTERACTIVE !== "1" || process.env.DEBUG_CHAT === "1") {
              console.error(`[Telemetry] Failed to initialize remote exporter: ${error.message}`);
            }
          }
        }
        if (this.enableConsole) {
          const consoleExporter = new ConsoleSpanExporter();
          spanProcessors.push(new BatchSpanProcessor(consoleExporter, {
            maxQueueSize: 2048,
            maxExportBatchSize: 512,
            scheduledDelayMillis: 500,
            // Reduced from default 5000ms
            exportTimeoutMillis: 3e4
          }));
          if (process.env.PROBE_NON_INTERACTIVE !== "1" || process.env.DEBUG_CHAT === "1") {
            console.log(`[Telemetry] Console exporter enabled`);
          }
        }
        if (spanProcessors.length === 0) {
          if (process.env.PROBE_NON_INTERACTIVE !== "1" || process.env.DEBUG_CHAT === "1") {
            console.log("[Telemetry] No exporters configured, telemetry will not be collected");
          }
          return;
        }
        this.sdk = new NodeSDK({
          resource,
          spanProcessors
        });
        try {
          this.sdk.start();
          this.tracer = trace.getTracer(this.serviceName, this.serviceVersion);
          if (process.env.PROBE_NON_INTERACTIVE !== "1" || process.env.DEBUG_CHAT === "1") {
            console.log(`[Telemetry] OpenTelemetry SDK initialized successfully`);
          }
        } catch (error) {
          if (process.env.PROBE_NON_INTERACTIVE !== "1" || process.env.DEBUG_CHAT === "1") {
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
          attributes
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
              message: error.message
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
            if (tracerProvider && typeof tracerProvider.forceFlush === "function") {
              await tracerProvider.forceFlush();
              if (process.env.DEBUG_CHAT === "1") {
                console.log("[Telemetry] TracerProvider flushed successfully");
              }
            }
            if (tracerProvider._registeredSpanProcessors) {
              const flushPromises = [];
              for (const processor of tracerProvider._registeredSpanProcessors) {
                if (typeof processor.forceFlush === "function") {
                  flushPromises.push(processor.forceFlush());
                }
              }
              if (flushPromises.length > 0) {
                await Promise.all(flushPromises);
                if (process.env.DEBUG_CHAT === "1") {
                  console.log(`[Telemetry] Directly flushed ${flushPromises.length} span processors`);
                }
              }
            }
            await new Promise((resolve3) => setTimeout(resolve3, 100));
            if (process.env.DEBUG_CHAT === "1") {
              console.log("[Telemetry] OpenTelemetry spans flushed successfully");
            }
          } catch (error) {
            if (process.env.PROBE_NON_INTERACTIVE !== "1" || process.env.DEBUG_CHAT === "1") {
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
            if (process.env.PROBE_NON_INTERACTIVE !== "1" || process.env.DEBUG_CHAT === "1") {
              console.log("[Telemetry] OpenTelemetry SDK shutdown successfully");
            }
          } catch (error) {
            if (process.env.PROBE_NON_INTERACTIVE !== "1" || process.env.DEBUG_CHAT === "1") {
              console.error(`[Telemetry] Failed to shutdown OpenTelemetry SDK: ${error.message}`);
            }
          }
        }
      }
    };
    defaultTelemetryConfig = new TelemetryConfig();
  }
});

// appTracer.js
import { trace as trace2, SpanStatusCode, SpanKind, context as context2, TraceFlags } from "@opentelemetry/api";
import { randomUUID, createHash } from "crypto";
function sessionIdToTraceId(sessionId2) {
  const hash = createHash("sha256").update(sessionId2).digest("hex");
  return hash.substring(0, 32);
}
var OTEL_ATTRS, AppTracer, appTracer;
var init_appTracer = __esm({
  "appTracer.js"() {
    __name(sessionIdToTraceId, "sessionIdToTraceId");
    OTEL_ATTRS = {
      // Standard semantic conventions
      SERVICE_NAME: "service.name",
      SERVICE_VERSION: "service.version",
      HTTP_METHOD: "http.method",
      HTTP_STATUS_CODE: "http.status_code",
      ERROR_TYPE: "error.type",
      ERROR_MESSAGE: "error.message",
      // Custom application attributes following OpenTelemetry naming conventions
      APP_SESSION_ID: "app.session.id",
      APP_MESSAGE_TYPE: "app.message.type",
      APP_MESSAGE_CONTENT: "app.message.content",
      APP_MESSAGE_LENGTH: "app.message.length",
      APP_MESSAGE_HASH: "app.message.hash",
      APP_AI_PROVIDER: "app.ai.provider",
      APP_AI_MODEL: "app.ai.model",
      APP_AI_TEMPERATURE: "app.ai.temperature",
      APP_AI_MAX_TOKENS: "app.ai.max_tokens",
      APP_AI_RESPONSE_CONTENT: "app.ai.response.content",
      APP_AI_RESPONSE_LENGTH: "app.ai.response.length",
      APP_AI_RESPONSE_HASH: "app.ai.response.hash",
      APP_AI_COMPLETION_TOKENS: "app.ai.completion_tokens",
      APP_AI_PROMPT_TOKENS: "app.ai.prompt_tokens",
      APP_AI_FINISH_REASON: "app.ai.finish_reason",
      APP_TOOL_NAME: "app.tool.name",
      APP_TOOL_PARAMS: "app.tool.params",
      APP_TOOL_RESULT: "app.tool.result",
      APP_TOOL_SUCCESS: "app.tool.success",
      APP_ITERATION_NUMBER: "app.iteration.number"
    };
    AppTracer = class {
      static {
        __name(this, "AppTracer");
      }
      constructor() {
        this.tracer = trace2.getTracer("probe-chat", "1.0.0");
        this.activeSpans = /* @__PURE__ */ new Map();
        this.sessionSpans = /* @__PURE__ */ new Map();
        this.sessionContexts = /* @__PURE__ */ new Map();
      }
      /**
       * Get the shared tracer instance
       */
      getTracer() {
        return this.tracer;
      }
      /**
       * Hash a string for deduplication purposes
       */
      _hashString(str) {
        let hash = 0;
        if (str.length === 0) return hash;
        for (let i = 0; i < str.length; i++) {
          const char = str.charCodeAt(i);
          hash = (hash << 5) - hash + char;
          hash = hash & hash;
        }
        return hash.toString();
      }
      /**
       * Get the active context for a session, creating spans within the session trace
       */
      _getSessionContext(sessionId2) {
        return this.sessionContexts.get(sessionId2) || context2.active();
      }
      /**
       * Start a chat session span with custom trace ID based on session ID
       */
      startChatSession(sessionId2, userMessage, provider, model) {
        if (process.env.DEBUG_CHAT === "1") {
          console.log(`[DEBUG] AppTracer: Starting chat session span for ${sessionId2}`);
        }
        const traceId = sessionIdToTraceId(sessionId2);
        const spanId = randomUUID().replace(/-/g, "").substring(0, 16);
        const spanContext = {
          traceId,
          spanId,
          traceFlags: TraceFlags.SAMPLED,
          isRemote: false
        };
        const activeContext = trace2.setSpanContext(context2.active(), spanContext);
        const span = context2.with(activeContext, () => {
          return this.tracer.startSpan("messaging.process", {
            kind: SpanKind.SERVER,
            attributes: {
              [OTEL_ATTRS.APP_SESSION_ID]: sessionId2,
              [OTEL_ATTRS.APP_MESSAGE_CONTENT]: userMessage.substring(0, 500),
              // Capture more message content
              [OTEL_ATTRS.APP_MESSAGE_LENGTH]: userMessage.length,
              [OTEL_ATTRS.APP_MESSAGE_HASH]: this._hashString(userMessage),
              // Add hash for deduplication
              [OTEL_ATTRS.APP_AI_PROVIDER]: provider,
              [OTEL_ATTRS.APP_AI_MODEL]: model,
              "app.session.start_time": Date.now(),
              "app.trace.custom_id": true
              // Mark that we're using custom trace ID
            }
          });
        });
        if (process.env.DEBUG_CHAT === "1") {
          console.log(`[DEBUG] AppTracer: Created chat session span ${span.spanContext().spanId} in trace ${span.spanContext().traceId}`);
        }
        const sessionContext = trace2.setSpan(context2.active(), span);
        this.sessionContexts.set(sessionId2, sessionContext);
        this.sessionSpans.set(sessionId2, span);
        if (process.env.DEBUG_CHAT === "1") {
          console.log(`[DEBUG] AppTracer: Session context established for ${sessionId2}`);
        }
        return span;
      }
      /**
       * Execute a function within the session context to ensure proper trace correlation
       */
      withSessionContext(sessionId2, fn) {
        const sessionContext = this._getSessionContext(sessionId2);
        return context2.with(sessionContext, fn);
      }
      /**
       * Get the trace ID for a session (derived from session ID)
       */
      getTraceIdForSession(sessionId2) {
        return sessionIdToTraceId(sessionId2);
      }
      /**
       * Start processing a user message
       */
      startUserMessageProcessing(sessionId2, messageId, message, imageUrlsFound = 0) {
        if (process.env.DEBUG_CHAT === "1") {
          console.log(`[DEBUG] AppTracer: Starting user message processing span for ${sessionId2}`);
        }
        const sessionContext = this._getSessionContext(sessionId2);
        return context2.with(sessionContext, () => {
          const parentSpan = trace2.getActiveSpan();
          const spanOptions = {
            kind: SpanKind.INTERNAL,
            attributes: {
              [OTEL_ATTRS.APP_SESSION_ID]: sessionId2,
              "app.message.id": messageId,
              [OTEL_ATTRS.APP_MESSAGE_TYPE]: "user",
              [OTEL_ATTRS.APP_MESSAGE_CONTENT]: message.substring(0, 1e3),
              // Include actual message content
              [OTEL_ATTRS.APP_MESSAGE_LENGTH]: message.length,
              [OTEL_ATTRS.APP_MESSAGE_HASH]: this._hashString(message),
              "app.message.image_urls_found": imageUrlsFound,
              "app.processing.start_time": Date.now()
            }
          };
          if (parentSpan) {
            spanOptions.parent = parentSpan.spanContext();
          }
          const span = this.tracer.startSpan("messaging.message.process", spanOptions);
          if (process.env.DEBUG_CHAT === "1") {
            console.log(`[DEBUG] AppTracer: Created user message processing span ${span.spanContext().spanId} with parent ${parentSpan?.spanContext().spanId}`);
          }
          this.activeSpans.set(`${sessionId2}_user_processing`, span);
          const messageContext = trace2.setSpan(sessionContext, span);
          this.sessionContexts.set(`${sessionId2}_message_processing`, messageContext);
          return span;
        });
      }
      /**
       * Execute a function within the context of user message processing span
       */
      withUserProcessingContext(sessionId2, fn) {
        const span = this.activeSpans.get(`${sessionId2}_user_processing`);
        if (span) {
          return context2.with(trace2.setSpan(context2.active(), span), fn);
        }
        return fn();
      }
      /**
       * Start the agent loop
       */
      startAgentLoop(sessionId2, maxIterations) {
        const sessionContext = this._getSessionContext(sessionId2);
        return context2.with(sessionContext, () => {
          const parentSpan = trace2.getActiveSpan();
          const spanOptions = {
            kind: SpanKind.INTERNAL,
            attributes: {
              "app.session.id": sessionId2,
              "app.loop.max_iterations": maxIterations,
              "app.loop.start_time": Date.now()
            }
          };
          if (parentSpan) {
            spanOptions.parent = parentSpan.spanContext();
          }
          const span = this.tracer.startSpan("agent.loop.start", spanOptions);
          this.activeSpans.set(`${sessionId2}_agent_loop`, span);
          const agentLoopContext = trace2.setSpan(sessionContext, span);
          this.sessionContexts.set(`${sessionId2}_agent_loop`, agentLoopContext);
          return span;
        });
      }
      /**
       * Execute a function within the context of agent loop span
       */
      withAgentLoopContext(sessionId2, fn) {
        const span = this.activeSpans.get(`${sessionId2}_agent_loop`);
        if (span) {
          return context2.with(trace2.setSpan(context2.active(), span), fn);
        }
        return fn();
      }
      /**
       * Start a single iteration of the agent loop
       */
      startAgentIteration(sessionId2, iterationNumber, messagesCount, contextTokens) {
        const sessionContext = this._getSessionContext(sessionId2);
        return context2.with(sessionContext, () => {
          const span = this.tracer.startSpan("agent.loop.iteration", {
            kind: SpanKind.INTERNAL,
            attributes: {
              "app.session.id": sessionId2,
              "app.iteration.number": iterationNumber,
              "app.iteration.messages_count": messagesCount,
              "app.iteration.context_tokens": contextTokens,
              "app.iteration.start_time": Date.now()
            }
          });
          this.activeSpans.set(`${sessionId2}_iteration_${iterationNumber}`, span);
          const iterationContext = trace2.setSpan(sessionContext, span);
          this.sessionContexts.set(`${sessionId2}_iteration_${iterationNumber}`, iterationContext);
          return span;
        });
      }
      /**
       * Execute a function within the context of agent iteration span
       */
      withIterationContext(sessionId2, iterationNumber, fn) {
        const span = this.activeSpans.get(`${sessionId2}_iteration_${iterationNumber}`);
        if (span) {
          return context2.with(trace2.setSpan(context2.active(), span), fn);
        }
        return fn();
      }
      /**
       * Start an AI generation request
       */
      startAiGenerationRequest(sessionId2, iterationNumber, model, provider, settings = {}, messagesContext = []) {
        if (process.env.DEBUG_CHAT === "1") {
          console.log(`[DEBUG] AppTracer: Starting AI generation request span for session ${sessionId2}, iteration ${iterationNumber}`);
        }
        const iterationContext = this.sessionContexts.get(`${sessionId2}_iteration_${iterationNumber}`);
        const sessionContext = iterationContext || this._getSessionContext(sessionId2);
        return context2.with(sessionContext, () => {
          const span = this.tracer.startSpan("ai.generation.request", {
            kind: SpanKind.CLIENT,
            attributes: {
              [OTEL_ATTRS.APP_SESSION_ID]: sessionId2,
              [OTEL_ATTRS.APP_ITERATION_NUMBER]: iterationNumber,
              [OTEL_ATTRS.APP_AI_MODEL]: model,
              [OTEL_ATTRS.APP_AI_PROVIDER]: provider,
              [OTEL_ATTRS.APP_AI_TEMPERATURE]: settings.temperature || 0,
              [OTEL_ATTRS.APP_AI_MAX_TOKENS]: settings.maxTokens || 0,
              "app.ai.max_retries": settings.maxRetries || 0,
              "app.ai.messages_count": messagesContext.length,
              "app.ai.request_start_time": Date.now()
            }
          });
          if (process.env.DEBUG_CHAT === "1") {
            console.log(`[DEBUG] AppTracer: Created AI generation span ${span.spanContext().spanId}`);
          }
          this.activeSpans.set(`${sessionId2}_ai_request_${iterationNumber}`, span);
          const aiRequestContext = trace2.setSpan(sessionContext, span);
          this.sessionContexts.set(`${sessionId2}_ai_request_${iterationNumber}`, aiRequestContext);
          return span;
        });
      }
      /**
       * Record AI response received
       */
      recordAiResponse(sessionId2, iterationNumber, responseData) {
        const sessionContext = this._getSessionContext(sessionId2);
        return context2.with(sessionContext, () => {
          const span = this.tracer.startSpan("ai.generation.response", {
            kind: SpanKind.INTERNAL,
            attributes: {
              [OTEL_ATTRS.APP_SESSION_ID]: sessionId2,
              [OTEL_ATTRS.APP_ITERATION_NUMBER]: iterationNumber,
              [OTEL_ATTRS.APP_AI_RESPONSE_CONTENT]: responseData.response ? responseData.response.substring(0, 2e3) : "",
              // Include actual response content
              [OTEL_ATTRS.APP_AI_RESPONSE_LENGTH]: responseData.responseLength || (responseData.response ? responseData.response.length : 0),
              [OTEL_ATTRS.APP_AI_RESPONSE_HASH]: responseData.response ? this._hashString(responseData.response) : "",
              [OTEL_ATTRS.APP_AI_COMPLETION_TOKENS]: responseData.completionTokens || 0,
              [OTEL_ATTRS.APP_AI_PROMPT_TOKENS]: responseData.promptTokens || 0,
              [OTEL_ATTRS.APP_AI_FINISH_REASON]: responseData.finishReason || "unknown",
              "app.ai.response.time_to_first_chunk_ms": responseData.timeToFirstChunk || 0,
              "app.ai.response.time_to_finish_ms": responseData.timeToFinish || 0,
              "app.ai.response.received_time": Date.now()
            }
          });
          span.setStatus({ code: SpanStatusCode.OK });
          span.end();
          return span;
        });
      }
      /**
       * Record a parsed tool call
       */
      recordToolCallParsed(sessionId2, iterationNumber, toolName, toolParams) {
        const aiRequestSpan = this.activeSpans.get(`${sessionId2}_ai_request_${iterationNumber}`);
        const spanOptions = {
          kind: SpanKind.INTERNAL,
          attributes: {
            "app.session.id": sessionId2,
            "app.tool.name": toolName,
            "app.tool.params": JSON.stringify(toolParams).substring(0, 500),
            // Truncate large params
            "app.tool.parsed_time": Date.now()
          }
        };
        if (aiRequestSpan) {
          spanOptions.parent = aiRequestSpan.spanContext();
        }
        const span = this.tracer.startSpan("tool.call.parse", spanOptions);
        span.setStatus({ code: SpanStatusCode.OK });
        span.end();
        return span;
      }
      /**
       * Start tool execution
       */
      startToolExecution(sessionId2, iterationNumber, toolName, toolParams) {
        const aiRequestContext = this.sessionContexts.get(`${sessionId2}_ai_request_${iterationNumber}`);
        const sessionContext = aiRequestContext || this._getSessionContext(sessionId2);
        return context2.with(sessionContext, () => {
          const span = this.tracer.startSpan("tool.call", {
            kind: SpanKind.INTERNAL,
            attributes: {
              [OTEL_ATTRS.APP_SESSION_ID]: sessionId2,
              [OTEL_ATTRS.APP_ITERATION_NUMBER]: iterationNumber,
              [OTEL_ATTRS.APP_TOOL_NAME]: toolName,
              [OTEL_ATTRS.APP_TOOL_PARAMS]: JSON.stringify(toolParams).substring(0, 1e3),
              // Include actual tool parameters
              "app.tool.params.hash": this._hashString(JSON.stringify(toolParams)),
              "app.tool.execution_start_time": Date.now(),
              // Add specific attributes based on tool type
              ...toolName === "search" && toolParams.query ? { "app.tool.search.query": toolParams.query } : {},
              ...toolName === "extract" && toolParams.file_path ? { "app.tool.extract.file_path": toolParams.file_path } : {},
              ...toolName === "query" && toolParams.pattern ? { "app.tool.query.pattern": toolParams.pattern } : {}
            }
          });
          this.activeSpans.set(`${sessionId2}_tool_execution_${iterationNumber}`, span);
          const toolExecutionContext = trace2.setSpan(sessionContext, span);
          this.sessionContexts.set(`${sessionId2}_tool_execution_${iterationNumber}`, toolExecutionContext);
          return span;
        });
      }
      /**
       * End tool execution with results
       */
      endToolExecution(sessionId2, iterationNumber, success, resultLength = 0, errorMessage = null, result = null) {
        const span = this.activeSpans.get(`${sessionId2}_tool_execution_${iterationNumber}`);
        if (!span) return;
        const attributes = {
          [OTEL_ATTRS.APP_TOOL_SUCCESS]: success,
          "app.tool.result_length": resultLength,
          "app.tool.execution_end_time": Date.now(),
          ...errorMessage ? { [OTEL_ATTRS.ERROR_MESSAGE]: errorMessage } : {},
          ...result ? {
            [OTEL_ATTRS.APP_TOOL_RESULT]: typeof result === "string" ? result.substring(0, 2e3) : JSON.stringify(result).substring(0, 2e3),
            "app.tool.result.hash": this._hashString(typeof result === "string" ? result : JSON.stringify(result))
          } : {}
        };
        span.setAttributes(attributes);
        span.setStatus({
          code: success ? SpanStatusCode.OK : SpanStatusCode.ERROR,
          message: errorMessage
        });
        span.end();
        this.activeSpans.delete(`${sessionId2}_tool_execution_${iterationNumber}`);
      }
      /**
       * End an iteration
       */
      endIteration(sessionId2, iterationNumber, success = true, completedAction = null) {
        const span = this.activeSpans.get(`${sessionId2}_iteration_${iterationNumber}`);
        if (!span) return;
        span.setAttributes({
          "app.iteration.success": success,
          "app.iteration.end_time": Date.now(),
          ...completedAction ? { "app.iteration.completed_action": completedAction } : {}
        });
        span.setStatus({ code: success ? SpanStatusCode.OK : SpanStatusCode.ERROR });
        span.end();
        this.activeSpans.delete(`${sessionId2}_iteration_${iterationNumber}`);
      }
      /**
       * End the agent loop
       */
      endAgentLoop(sessionId2, totalIterations, success = true, completionReason = null) {
        const span = this.activeSpans.get(`${sessionId2}_agent_loop`);
        if (!span) return;
        span.setAttributes({
          "app.loop.total_iterations": totalIterations,
          "app.loop.success": success,
          "app.loop.end_time": Date.now(),
          ...completionReason ? { "app.loop.completion_reason": completionReason } : {}
        });
        span.setStatus({ code: success ? SpanStatusCode.OK : SpanStatusCode.ERROR });
        span.end();
        this.activeSpans.delete(`${sessionId2}_agent_loop`);
      }
      /**
       * End user message processing
       */
      endUserMessageProcessing(sessionId2, success = true) {
        const span = this.activeSpans.get(`${sessionId2}_user_processing`);
        if (!span) {
          if (process.env.DEBUG_CHAT === "1") {
            console.log(`[DEBUG] AppTracer: No user message processing span found for ${sessionId2}`);
          }
          return;
        }
        if (process.env.DEBUG_CHAT === "1") {
          console.log(`[DEBUG] AppTracer: Ending user message processing span ${span.spanContext().spanId} for ${sessionId2}`);
        }
        span.setAttributes({
          "app.processing.success": success,
          "app.processing.end_time": Date.now()
        });
        span.setStatus({ code: success ? SpanStatusCode.OK : SpanStatusCode.ERROR });
        span.end();
        this.activeSpans.delete(`${sessionId2}_user_processing`);
        this.sessionContexts.delete(`${sessionId2}_message_processing`);
      }
      /**
       * End the chat session
       */
      endChatSession(sessionId2, success = true, totalTokensUsed = 0) {
        const span = this.sessionSpans.get(sessionId2);
        if (!span) {
          if (process.env.DEBUG_CHAT === "1") {
            console.log(`[DEBUG] AppTracer: No chat session span found for ${sessionId2}`);
          }
          return;
        }
        if (process.env.DEBUG_CHAT === "1") {
          console.log(`[DEBUG] AppTracer: Ending chat session span ${span.spanContext().spanId} for ${sessionId2}`);
        }
        span.setAttributes({
          "app.session.success": success,
          "app.session.total_tokens_used": totalTokensUsed,
          "app.session.end_time": Date.now()
        });
        span.setStatus({ code: success ? SpanStatusCode.OK : SpanStatusCode.ERROR });
        span.end();
        this.sessionSpans.delete(sessionId2);
        this.sessionContexts.delete(sessionId2);
      }
      /**
       * End AI request span
       */
      endAiRequest(sessionId2, iterationNumber, success = true) {
        const span = this.activeSpans.get(`${sessionId2}_ai_request_${iterationNumber}`);
        if (!span) {
          if (process.env.DEBUG_CHAT === "1") {
            console.log(`[DEBUG] AppTracer: No AI request span found for ${sessionId2}_ai_request_${iterationNumber}`);
          }
          return;
        }
        if (process.env.DEBUG_CHAT === "1") {
          console.log(`[DEBUG] AppTracer: Ending AI request span ${span.spanContext().spanId} for ${sessionId2}, iteration ${iterationNumber}`);
        }
        span.setAttributes({
          "app.ai.request_success": success,
          "app.ai.request_end_time": Date.now()
        });
        span.setStatus({ code: success ? SpanStatusCode.OK : SpanStatusCode.ERROR });
        span.end();
        this.activeSpans.delete(`${sessionId2}_ai_request_${iterationNumber}`);
      }
      /**
       * Record a completion attempt
       */
      recordCompletionAttempt(sessionId2, success = true, finalResult = null) {
        const sessionSpan = this.sessionSpans.get(sessionId2);
        const spanOptions = {
          kind: SpanKind.INTERNAL,
          attributes: {
            "app.session.id": sessionId2,
            "app.completion.success": success,
            "app.completion.result_length": finalResult ? finalResult.length : 0,
            "app.completion.attempt_time": Date.now()
          }
        };
        if (sessionSpan) {
          spanOptions.parent = sessionSpan.spanContext();
        }
        const span = this.tracer.startSpan("agent.completion.attempt", spanOptions);
        span.setStatus({ code: success ? SpanStatusCode.OK : SpanStatusCode.ERROR });
        span.end();
        return span;
      }
      /**
       * Start image URL processing
       */
      startImageProcessing(sessionId2, messageId, imageUrls = [], cleanedMessageLength = 0) {
        const userProcessingSpan = this.activeSpans.get(`${sessionId2}_user_processing`);
        const spanOptions = {
          kind: SpanKind.INTERNAL,
          attributes: {
            "app.session.id": sessionId2,
            "app.message.id": messageId,
            "app.image.urls_found": imageUrls.length,
            "app.image.message_cleaned_length": cleanedMessageLength,
            "app.image.processing_start_time": Date.now(),
            "app.image.urls_list": JSON.stringify(imageUrls).substring(0, 500)
          }
        };
        if (userProcessingSpan) {
          spanOptions.parent = userProcessingSpan.spanContext();
        }
        const span = this.tracer.startSpan("content.image.processing", spanOptions);
        this.activeSpans.set(`${sessionId2}_image_processing`, span);
        return span;
      }
      /**
       * Record image URL validation results
       */
      recordImageValidation(sessionId2, validationResults) {
        const imageProcessingSpan = this.activeSpans.get(`${sessionId2}_image_processing`);
        const spanOptions = {
          kind: SpanKind.INTERNAL,
          attributes: {
            "app.session.id": sessionId2,
            "app.image.validation.total_urls": validationResults.totalUrls || 0,
            "app.image.validation.valid_urls": validationResults.validUrls || 0,
            "app.image.validation.invalid_urls": validationResults.invalidUrls || 0,
            "app.image.validation.redirected_urls": validationResults.redirectedUrls || 0,
            "app.image.validation.timeout_urls": validationResults.timeoutUrls || 0,
            "app.image.validation.network_errors": validationResults.networkErrors || 0,
            "app.image.validation.duration_ms": validationResults.durationMs || 0,
            "app.image.validation_time": Date.now()
          }
        };
        if (imageProcessingSpan) {
          spanOptions.parent = imageProcessingSpan.spanContext();
        }
        const span = this.tracer.startSpan("content.image.validation", spanOptions);
        span.setStatus({
          code: validationResults.validUrls > 0 ? SpanStatusCode.OK : SpanStatusCode.ERROR,
          message: `${validationResults.validUrls}/${validationResults.totalUrls} URLs validated successfully`
        });
        span.end();
        return span;
      }
      /**
       * End image processing
       */
      endImageProcessing(sessionId2, success = true, finalValidUrls = 0) {
        const span = this.activeSpans.get(`${sessionId2}_image_processing`);
        if (!span) return;
        span.setAttributes({
          "app.image.processing_success": success,
          "app.image.final_valid_urls": finalValidUrls,
          "app.image.processing_end_time": Date.now()
        });
        span.setStatus({ code: success ? SpanStatusCode.OK : SpanStatusCode.ERROR });
        span.end();
        this.activeSpans.delete(`${sessionId2}_image_processing`);
      }
      /**
       * Record AI model errors
       */
      recordAiModelError(sessionId2, iterationNumber, errorDetails) {
        const aiRequestSpan = this.activeSpans.get(`${sessionId2}_ai_request_${iterationNumber}`);
        const spanOptions = {
          kind: SpanKind.INTERNAL,
          attributes: {
            "app.session.id": sessionId2,
            "app.error.type": "ai_model_error",
            "app.error.category": errorDetails.category || "unknown",
            // timeout, api_limit, network, etc.
            "app.error.message": errorDetails.message?.substring(0, 500) || "",
            "app.error.model": errorDetails.model || "",
            "app.error.provider": errorDetails.provider || "",
            "app.error.status_code": errorDetails.statusCode || 0,
            "app.error.retry_attempt": errorDetails.retryAttempt || 0,
            "app.error.timestamp": Date.now()
          }
        };
        if (aiRequestSpan) {
          spanOptions.parent = aiRequestSpan.spanContext();
        }
        const span = this.tracer.startSpan("ai.generation.error", spanOptions);
        span.setStatus({ code: SpanStatusCode.ERROR, message: errorDetails.message });
        span.end();
        return span;
      }
      /**
       * Record tool execution errors
       */
      recordToolError(sessionId2, iterationNumber, toolName, errorDetails) {
        const toolExecutionSpan = this.activeSpans.get(`${sessionId2}_tool_execution_${iterationNumber}`);
        const spanOptions = {
          kind: SpanKind.INTERNAL,
          attributes: {
            "app.session.id": sessionId2,
            "app.error.type": "tool_execution_error",
            "app.error.tool_name": toolName,
            "app.error.category": errorDetails.category || "unknown",
            // validation, execution, network, filesystem
            "app.error.message": errorDetails.message?.substring(0, 500) || "",
            "app.error.exit_code": errorDetails.exitCode || 0,
            "app.error.signal": errorDetails.signal || "",
            "app.error.params": JSON.stringify(errorDetails.params || {}).substring(0, 300),
            "app.error.timestamp": Date.now()
          }
        };
        if (toolExecutionSpan) {
          spanOptions.parent = toolExecutionSpan.spanContext();
        }
        const span = this.tracer.startSpan("tool.call.error", spanOptions);
        span.setStatus({ code: SpanStatusCode.ERROR, message: errorDetails.message });
        span.end();
        return span;
      }
      /**
       * Record session cancellation
       */
      recordSessionCancellation(sessionId2, reason = "user_request", context3 = {}) {
        const sessionSpan = this.sessionSpans.get(sessionId2);
        const spanOptions = {
          kind: SpanKind.INTERNAL,
          attributes: {
            "app.session.id": sessionId2,
            "app.cancellation.reason": reason,
            // user_request, timeout, error, signal
            "app.cancellation.context": JSON.stringify(context3).substring(0, 300),
            "app.cancellation.current_iteration": context3.currentIteration || 0,
            "app.cancellation.active_tool": context3.activeTool || "",
            "app.cancellation.timestamp": Date.now()
          }
        };
        if (sessionSpan) {
          spanOptions.parent = sessionSpan.spanContext();
        }
        const span = this.tracer.startSpan("messaging.session.cancel", spanOptions);
        span.setStatus({ code: SpanStatusCode.ERROR, message: `Session cancelled: ${reason}` });
        span.end();
        return span;
      }
      /**
       * Record token management metrics
       */
      recordTokenMetrics(sessionId2, tokenData) {
        const sessionSpan = this.sessionSpans.get(sessionId2);
        const spanOptions = {
          kind: SpanKind.INTERNAL,
          attributes: {
            "app.session.id": sessionId2,
            "app.tokens.context_window": tokenData.contextWindow || 0,
            "app.tokens.current_total": tokenData.currentTotal || 0,
            "app.tokens.request_tokens": tokenData.requestTokens || 0,
            "app.tokens.response_tokens": tokenData.responseTokens || 0,
            "app.tokens.cache_read": tokenData.cacheRead || 0,
            "app.tokens.cache_write": tokenData.cacheWrite || 0,
            "app.tokens.utilization_percent": tokenData.contextWindow ? Math.round(tokenData.currentTotal / tokenData.contextWindow * 100) : 0,
            "app.tokens.measurement_time": Date.now()
          }
        };
        if (sessionSpan) {
          spanOptions.parent = sessionSpan.spanContext();
        }
        const span = this.tracer.startSpan("ai.token.metrics", spanOptions);
        span.setStatus({ code: SpanStatusCode.OK });
        span.end();
        return span;
      }
      /**
       * Record history management operations
       */
      recordHistoryOperation(sessionId2, operation, details = {}) {
        const sessionSpan = this.sessionSpans.get(sessionId2);
        const spanOptions = {
          kind: SpanKind.INTERNAL,
          attributes: {
            "app.session.id": sessionId2,
            "app.history.operation": operation,
            // trim, update, clear, save
            "app.history.messages_before": details.messagesBefore || 0,
            "app.history.messages_after": details.messagesAfter || 0,
            "app.history.messages_removed": details.messagesRemoved || 0,
            "app.history.reason": details.reason || "",
            // max_length, memory_limit, session_reset
            "app.history.operation_time": Date.now()
          }
        };
        if (sessionSpan) {
          spanOptions.parent = sessionSpan.spanContext();
        }
        const span = this.tracer.startSpan("messaging.history.manage", spanOptions);
        span.setStatus({ code: SpanStatusCode.OK });
        span.end();
        return span;
      }
      /**
       * Record system prompt generation metrics
       */
      recordSystemPromptGeneration(sessionId2, promptData) {
        const sessionSpan = this.sessionSpans.get(sessionId2);
        const spanOptions = {
          kind: SpanKind.INTERNAL,
          attributes: {
            "app.session.id": sessionId2,
            "app.prompt.base_length": promptData.baseLength || 0,
            "app.prompt.final_length": promptData.finalLength || 0,
            "app.prompt.files_added": promptData.filesAdded || 0,
            "app.prompt.generation_duration_ms": promptData.generationDurationMs || 0,
            "app.prompt.type": promptData.promptType || "default",
            "app.prompt.estimated_tokens": promptData.estimatedTokens || 0,
            "app.prompt.generation_time": Date.now()
          }
        };
        if (sessionSpan) {
          spanOptions.parent = sessionSpan.spanContext();
        }
        const span = this.tracer.startSpan("ai.prompt.generate", spanOptions);
        span.setStatus({ code: SpanStatusCode.OK });
        span.end();
        return span;
      }
      /**
       * Record file system operations
       */
      recordFileSystemOperation(sessionId2, operation, details = {}) {
        const activeSpan = this.activeSpans.get(`${sessionId2}_tool_execution_${details.iterationNumber}`) || this.sessionSpans.get(sessionId2);
        const spanOptions = {
          kind: SpanKind.INTERNAL,
          attributes: {
            "app.session.id": sessionId2,
            "app.fs.operation": operation,
            // read, write, create_temp, delete, mkdir
            "app.fs.path": details.path?.substring(0, 200) || "",
            "app.fs.size_bytes": details.sizeBytes || 0,
            "app.fs.duration_ms": details.durationMs || 0,
            "app.fs.success": details.success !== false,
            "app.fs.error_code": details.errorCode || "",
            "app.fs.operation_time": Date.now()
          }
        };
        if (activeSpan) {
          spanOptions.parent = activeSpan.spanContext();
        }
        const span = this.tracer.startSpan("fs.operation", spanOptions);
        span.setStatus({
          code: details.success !== false ? SpanStatusCode.OK : SpanStatusCode.ERROR,
          message: details.errorMessage
        });
        span.end();
        return span;
      }
      /**
       * Clean up any remaining active spans for a session
       */
      cleanup(sessionId2) {
        if (process.env.DEBUG_CHAT === "1") {
          console.log(`[DEBUG] AppTracer: Cleaning up session ${sessionId2}`);
        }
        const keysToDelete = [];
        for (const [key, span] of this.activeSpans.entries()) {
          if (key.includes(sessionId2)) {
            if (process.env.DEBUG_CHAT === "1") {
              console.log(`[DEBUG] AppTracer: Cleaning up active span ${key}`);
            }
            span.setStatus({ code: SpanStatusCode.ERROR, message: "Session cleanup" });
            span.end();
            keysToDelete.push(key);
          }
        }
        keysToDelete.forEach((key) => this.activeSpans.delete(key));
        const sessionSpan = this.sessionSpans.get(sessionId2);
        if (sessionSpan) {
          if (process.env.DEBUG_CHAT === "1") {
            console.log(`[DEBUG] AppTracer: Cleaning up orphaned session span for ${sessionId2}`);
          }
          sessionSpan.setStatus({ code: SpanStatusCode.ERROR, message: "Session cleanup - orphaned span" });
          sessionSpan.end();
          this.sessionSpans.delete(sessionId2);
        }
        const contextKeysToDelete = [];
        for (const [key] of this.sessionContexts.entries()) {
          if (key.includes(sessionId2)) {
            contextKeysToDelete.push(key);
          }
        }
        contextKeysToDelete.forEach((key) => this.sessionContexts.delete(key));
        if (process.env.DEBUG_CHAT === "1") {
          console.log(`[DEBUG] AppTracer: Session cleanup completed for ${sessionId2}, cleaned ${contextKeysToDelete.length} contexts`);
        }
      }
    };
    appTracer = new AppTracer();
  }
});

// tools.js
import {
  searchTool,
  queryTool,
  extractTool,
  DEFAULT_SYSTEM_MESSAGE,
  attemptCompletionSchema,
  attemptCompletionToolDefinition,
  searchSchema,
  querySchema,
  extractSchema,
  searchToolDefinition,
  queryToolDefinition,
  extractToolDefinition
} from "@buger/probe";
import { randomUUID as randomUUID2 } from "crypto";
import { parseXmlToolCall } from "@buger/probe";
function parseXmlToolCallWithThinking(xmlString) {
  const thinkingMatch = xmlString.match(/<thinking>([\s\S]*?)<\/thinking>/);
  const thinkingContent = thinkingMatch ? thinkingMatch[1].trim() : null;
  const cleanedXmlString = xmlString.replace(/<thinking>[\s\S]*?<\/thinking>/g, "").trim();
  const parsedTool = parseXmlToolCall(cleanedXmlString);
  if (process.env.DEBUG_CHAT === "1" && thinkingContent) {
    console.log(`[DEBUG] AI Thinking Process:
${thinkingContent}`);
  }
  return parsedTool;
}
var sessionId, debug, configOptions, tools, searchToolInstance, queryToolInstance, extractToolInstance, implementToolDefinition, listFilesToolDefinition, searchFilesToolDefinition;
var init_tools = __esm({
  "tools.js"() {
    sessionId = process.env.PROBE_SESSION_ID || randomUUID2();
    console.error(`Generated session ID for search caching: ${sessionId}`);
    debug = process.env.DEBUG_CHAT === "1";
    configOptions = {
      sessionId,
      debug
    };
    tools = {
      searchTool: searchTool(configOptions),
      queryTool: queryTool(configOptions),
      extractTool: extractTool(configOptions)
      // Note: The actual implement tool *instance* comes from probeTool.js
      // This file primarily deals with definitions for the system prompt.
    };
    ({ searchTool: searchToolInstance, queryTool: queryToolInstance, extractTool: extractToolInstance } = tools);
    implementToolDefinition = `
## implement
Description: Implement a given task. Can modify files. Can be used ONLY if task explicitly stated that something requires modification or implementation.

Parameters:
- task: (required) The task description. Should be as detailed as possible, ideally pointing to exact files which needs be modified or created.
- autoCommits: (optional) Whether to enable auto-commits in aider. Default is false.

Usage Example:

<examples>

User: Can you implement a function to calculate Fibonacci numbers in main.js?
<implement>
<task>Implement a recursive function to calculate the nth Fibonacci number in main.js</task>
</implement>

User: Can you implement a function to calculate Fibonacci numbers in main.js with auto-commits?
<implement>
<task>Implement a recursive function to calculate the nth Fibonacci number in main.js</task>
<autoCommits>true</autoCommits>
</implement>

</examples>
`;
    listFilesToolDefinition = `
## listFiles
Description: List files and directories in a specified location.

Parameters:
- directory: (optional) The directory path to list files from. Defaults to current directory if not specified.

Usage Example:

<examples>

User: Can you list the files in the src directory?
<listFiles>
<directory>src</directory>
</listFiles>

User: What files are in the current directory?
<listFiles>
</listFiles>

</examples>
`;
    searchFilesToolDefinition = `
## searchFiles
Description: Find files with name matching a glob pattern with recursive search capability.

Parameters:
- pattern: (required) The glob pattern to search for (e.g., "**/*.js", "*.md").
- directory: (optional) The directory to search in. Defaults to current directory if not specified.
- recursive: (optional) Whether to search recursively. Defaults to true.

Usage Example:

<examples>

User: Can you find all JavaScript files in the project?
<searchFiles>
<pattern>**/*.js</pattern>
</searchFiles>

User: Find all markdown files in the docs directory, but only at the top level.
<searchFiles>
<pattern>*.md</pattern>
<directory>docs</directory>
<recursive>false</recursive>
</searchFiles>

</examples>
`;
    __name(parseXmlToolCallWithThinking, "parseXmlToolCallWithThinking");
  }
});

// implement/core/utils.js
var ErrorTypes, BackendError, ErrorHandler, RetryHandler, ProgressTracker, FileChangeParser, TokenEstimator;
var init_utils = __esm({
  "implement/core/utils.js"() {
    ErrorTypes = {
      INITIALIZATION_FAILED: "initialization_failed",
      DEPENDENCY_MISSING: "dependency_missing",
      CONFIGURATION_INVALID: "configuration_invalid",
      EXECUTION_FAILED: "execution_failed",
      TIMEOUT: "timeout",
      CANCELLATION: "cancellation",
      NETWORK_ERROR: "network_error",
      API_ERROR: "api_error",
      AUTHENTICATION: "authentication",
      FILE_ACCESS_ERROR: "file_access_error",
      VALIDATION_ERROR: "validation_error",
      BACKEND_NOT_FOUND: "backend_not_found",
      SESSION_NOT_FOUND: "session_not_found",
      QUOTA_EXCEEDED: "quota_exceeded"
    };
    BackendError = class _BackendError extends Error {
      static {
        __name(this, "BackendError");
      }
      /**
       * @param {string} message - Error message
       * @param {string} type - Error type from ErrorTypes
       * @param {string} [code] - Error code
       * @param {Object} [details] - Additional error details
       */
      constructor(message, type, code = null, details = {}) {
        super(message);
        this.name = "BackendError";
        this.type = type;
        this.code = code;
        this.details = details;
        this.timestamp = (/* @__PURE__ */ new Date()).toISOString();
        if (Error.captureStackTrace) {
          Error.captureStackTrace(this, _BackendError);
        }
      }
      /**
       * Convert error to JSON representation
       * @returns {Object}
       */
      toJSON() {
        return {
          name: this.name,
          message: this.message,
          type: this.type,
          code: this.code,
          details: this.details,
          timestamp: this.timestamp,
          stack: this.stack
        };
      }
    };
    ErrorHandler = class {
      static {
        __name(this, "ErrorHandler");
      }
      /**
       * Create a new BackendError
       * @param {string} type - Error type
       * @param {string} message - Error message
       * @param {string} [code] - Error code
       * @param {Object} [details] - Additional details
       * @returns {BackendError}
       */
      static createError(type, message, code = null, details = {}) {
        return new BackendError(message, type, code, details);
      }
      /**
       * Check if an error is retryable
       * @param {Error|BackendError} error - Error to check
       * @returns {boolean}
       */
      static isRetryable(error) {
        if (error instanceof BackendError) {
          const retryableTypes = [
            ErrorTypes.NETWORK_ERROR,
            ErrorTypes.TIMEOUT,
            ErrorTypes.API_ERROR
          ];
          return retryableTypes.includes(error.type);
        }
        const message = error.message.toLowerCase();
        return message.includes("timeout") || message.includes("network") || message.includes("connection");
      }
      /**
       * Get recovery strategy for an error
       * @param {Error|BackendError} error - Error to analyze
       * @returns {string} Recovery strategy
       */
      static getRecoveryStrategy(error) {
        if (!(error instanceof BackendError)) {
          return "manual_intervention";
        }
        switch (error.type) {
          case ErrorTypes.DEPENDENCY_MISSING:
            return "install_dependencies";
          case ErrorTypes.CONFIGURATION_INVALID:
            return "fix_configuration";
          case ErrorTypes.TIMEOUT:
            return "retry_with_longer_timeout";
          case ErrorTypes.AUTHENTICATION:
            return "check_api_credentials";
          case ErrorTypes.NETWORK_ERROR:
            return "retry_with_backoff";
          case ErrorTypes.QUOTA_EXCEEDED:
            return "wait_or_upgrade";
          case ErrorTypes.API_ERROR:
            return "check_api_key";
          default:
            return "manual_intervention";
        }
      }
      /**
       * Format error for user display
       * @param {Error|BackendError} error - Error to format
       * @returns {string}
       */
      static formatForDisplay(error) {
        if (error instanceof BackendError) {
          let message = `${error.message}`;
          if (error.code) {
            message += ` (${error.code})`;
          }
          const strategy = this.getRecoveryStrategy(error);
          switch (strategy) {
            case "install_dependencies":
              message += "\n\u{1F4A1} Try installing missing dependencies";
              break;
            case "fix_configuration":
              message += "\n\u{1F4A1} Check your configuration settings";
              break;
            case "retry_with_longer_timeout":
              message += "\n\u{1F4A1} Consider increasing the timeout value";
              break;
            case "check_api_credentials":
              message += "\n\u{1F4A1} Check your API key and authentication settings";
              break;
            case "check_api_key":
              message += "\n\u{1F4A1} Verify your API key is valid";
              break;
          }
          return message;
        }
        return error.message;
      }
    };
    RetryHandler = class {
      static {
        __name(this, "RetryHandler");
      }
      /**
       * Execute a function with retry logic
       * @param {Function} fn - Function to execute
       * @param {Object} [options] - Retry options
       * @param {number} [options.maxAttempts=3] - Maximum retry attempts
       * @param {number} [options.initialDelay=1000] - Initial delay in ms
       * @param {number} [options.maxDelay=30000] - Maximum delay in ms
       * @param {number} [options.backoffFactor=2] - Backoff multiplier
       * @param {Function} [options.shouldRetry] - Custom retry predicate
       * @returns {Promise<*>}
       */
      static async withRetry(fn, options = {}) {
        const {
          maxAttempts = 3,
          initialDelay = 1e3,
          maxDelay = 3e4,
          backoffFactor = 2,
          shouldRetry = ErrorHandler.isRetryable
        } = options;
        let lastError;
        let delay = initialDelay;
        for (let attempt = 1; attempt <= maxAttempts; attempt++) {
          try {
            return await fn();
          } catch (error) {
            lastError = error;
            if (attempt === maxAttempts || !shouldRetry(error)) {
              throw error;
            }
            console.error(`[ERROR] ========================================`);
            console.error(`[ERROR] Attempt ${attempt} failed, retrying in ${delay}ms...`);
            console.error(`[ERROR] Error message: ${error.message}`);
            console.error(`[ERROR] Error type: ${error.type || "unknown"}`);
            console.error(`[ERROR] Error code: ${error.code || "unknown"}`);
            if (error.metadata) {
              console.error(`[ERROR] Error metadata:`, JSON.stringify(error.metadata, null, 2));
            }
            console.error(`[ERROR] ========================================`);
            await this.sleep(delay);
            delay = Math.min(delay * backoffFactor, maxDelay);
          }
        }
        throw lastError;
      }
      /**
       * Sleep for specified milliseconds
       * @param {number} ms - Milliseconds to sleep
       * @returns {Promise<void>}
       */
      static sleep(ms) {
        return new Promise((resolve3) => setTimeout(resolve3, ms));
      }
    };
    ProgressTracker = class {
      static {
        __name(this, "ProgressTracker");
      }
      /**
       * @param {string} sessionId - Session ID
       * @param {Function} [onProgress] - Progress callback
       */
      constructor(sessionId2, onProgress = null) {
        this.sessionId = sessionId2;
        this.onProgress = onProgress;
        this.startTime = Date.now();
        this.steps = [];
        this.currentStep = null;
      }
      /**
       * Start a new step
       * @param {string} name - Step name
       * @param {string} [message] - Step message
       */
      startStep(name, message = null) {
        if (this.currentStep) {
          this.endStep();
        }
        this.currentStep = {
          name,
          message,
          startTime: Date.now()
        };
        this.reportProgress({
          type: "step_start",
          step: name,
          message
        });
      }
      /**
       * End the current step
       * @param {string} [result] - Step result
       */
      endStep(result = "completed") {
        if (!this.currentStep) return;
        const duration = Date.now() - this.currentStep.startTime;
        this.currentStep.duration = duration;
        this.currentStep.result = result;
        this.steps.push(this.currentStep);
        this.reportProgress({
          type: "step_end",
          step: this.currentStep.name,
          result,
          duration
        });
        this.currentStep = null;
      }
      /**
       * Report progress update
       * @param {Object} update - Progress update
       */
      reportProgress(update) {
        if (!this.onProgress) return;
        const progress = {
          sessionId: this.sessionId,
          timestamp: Date.now(),
          elapsed: Date.now() - this.startTime,
          ...update
        };
        try {
          this.onProgress(progress);
        } catch (error) {
          console.error("Progress callback error:", error);
        }
      }
      /**
       * Report a message
       * @param {string} message - Message to report
       * @param {string} [type='info'] - Message type
       */
      reportMessage(message, type = "info") {
        this.reportProgress({
          type: "message",
          messageType: type,
          message
        });
      }
      /**
       * Get execution summary
       * @returns {Object}
       */
      getSummary() {
        return {
          sessionId: this.sessionId,
          totalDuration: Date.now() - this.startTime,
          steps: this.steps,
          currentStep: this.currentStep
        };
      }
    };
    FileChangeParser = class {
      static {
        __name(this, "FileChangeParser");
      }
      /**
       * Parse file changes from command output
       * @param {string} output - Command output
       * @param {string} [workingDir] - Working directory
       * @returns {import('../types/BackendTypes').FileChange[]}
       */
      static parseChanges(output, workingDir = process.cwd()) {
        const changes = [];
        const patterns = {
          created: /(?:created?|new file|added?):?\s+(.+)/gi,
          modified: /(?:modified?|changed?|updated?):?\s+(.+)/gi,
          deleted: /(?:deleted?|removed?):?\s+(.+)/gi,
          diff: /^[-+]{3}\s+(.+)$/gm
        };
        for (const [type, pattern] of Object.entries(patterns)) {
          if (type === "diff") continue;
          let match2;
          while ((match2 = pattern.exec(output)) !== null) {
            const filePath = match2[1].trim();
            if (filePath && !changes.some((c) => c.path === filePath)) {
              changes.push({
                path: filePath,
                type,
                description: `File ${filePath} was ${type}`
              });
            }
          }
        }
        const gitStatusPattern = /^([AMD])\s+(.+)$/gm;
        let match;
        while ((match = gitStatusPattern.exec(output)) !== null) {
          const status = match[1];
          const filePath = match[2];
          const typeMap = {
            "A": "created",
            "M": "modified",
            "D": "deleted"
          };
          if (typeMap[status] && !changes.some((c) => c.path === filePath)) {
            changes.push({
              path: filePath,
              type: typeMap[status],
              description: `File ${filePath} was ${typeMap[status]}`
            });
          }
        }
        return changes;
      }
      /**
       * Extract diff statistics from output
       * @param {string} output - Command output
       * @returns {Object}
       */
      static extractDiffStats(output) {
        const stats = {
          filesChanged: 0,
          insertions: 0,
          deletions: 0
        };
        const statPattern = /(\d+)\s+files?\s+changed(?:,\s+(\d+)\s+insertions?)?(?:,\s+(\d+)\s+deletions?)?/;
        const match = output.match(statPattern);
        if (match) {
          stats.filesChanged = parseInt(match[1], 10);
          stats.insertions = match[2] ? parseInt(match[2], 10) : 0;
          stats.deletions = match[3] ? parseInt(match[3], 10) : 0;
        }
        return stats;
      }
    };
    TokenEstimator = class {
      static {
        __name(this, "TokenEstimator");
      }
      /**
       * Estimate token count for text
       * @param {string} text - Text to estimate
       * @returns {number} Estimated token count
       */
      static estimate(text) {
        return Math.ceil(text.length / 4);
      }
      /**
       * Check if text exceeds token limit
       * @param {string} text - Text to check
       * @param {number} limit - Token limit
       * @returns {boolean}
       */
      static exceedsLimit(text, limit) {
        return this.estimate(text) > limit;
      }
      /**
       * Truncate text to fit within token limit
       * @param {string} text - Text to truncate
       * @param {number} limit - Token limit
       * @param {string} [suffix='...'] - Suffix to add
       * @returns {string}
       */
      static truncate(text, limit, suffix = "...") {
        const estimatedChars = limit * 4;
        if (text.length <= estimatedChars) {
          return text;
        }
        return text.substring(0, estimatedChars - suffix.length) + suffix;
      }
    };
  }
});

// implement/core/timeouts.js
function secondsToMs(seconds) {
  return seconds * 1e3;
}
function isValidTimeout(seconds) {
  return seconds >= TIMEOUTS.IMPLEMENT_MINIMUM && seconds <= TIMEOUTS.IMPLEMENT_MAXIMUM;
}
function getDefaultTimeoutMs() {
  return secondsToMs(TIMEOUTS.IMPLEMENT_DEFAULT);
}
var TIMEOUTS;
var init_timeouts = __esm({
  "implement/core/timeouts.js"() {
    TIMEOUTS = {
      // Main implementation timeouts (in seconds - user-friendly)
      IMPLEMENT_DEFAULT: 1200,
      // 20 minutes - default for Claude Code/Aider execution
      IMPLEMENT_MINIMUM: 60,
      // 1 minute - minimum allowed timeout
      IMPLEMENT_MAXIMUM: 3600,
      // 1 hour - maximum allowed timeout
      // Quick verification checks (in milliseconds)
      VERSION_CHECK: 5e3,
      // 5 seconds - claude --version, aider --version
      PATH_CHECK: 2e3,
      // 2 seconds - command existence checks
      NPM_CHECK: 5e3,
      // 5 seconds - npm operations
      WSL_CHECK: 2e3,
      // 2 seconds - WSL availability checks
      // Network operations (in milliseconds) 
      HTTP_REQUEST: 1e4,
      // 10 seconds - GitHub URLs, remote requests
      FILE_FLUSH: 5e3
      // 5 seconds - file operations and flushing
    };
    __name(secondsToMs, "secondsToMs");
    __name(isValidTimeout, "isValidTimeout");
    __name(getDefaultTimeoutMs, "getDefaultTimeoutMs");
  }
});

// implement/core/BackendManager.js
var BackendManager, BackendManager_default;
var init_BackendManager = __esm({
  "implement/core/BackendManager.js"() {
    init_utils();
    init_timeouts();
    BackendManager = class {
      static {
        __name(this, "BackendManager");
      }
      /**
       * @param {Object} config - Backend manager configuration
       * @param {string} config.defaultBackend - Default backend name
       * @param {string[]} [config.fallbackBackends] - Fallback backend names
       * @param {string} [config.selectionStrategy='auto'] - Backend selection strategy
       * @param {number} [config.maxConcurrentSessions=3] - Maximum concurrent sessions
       * @param {number} [config.timeout=300000] - Default timeout in milliseconds
       * @param {number} [config.retryAttempts=2] - Number of retry attempts
       */
      constructor(config) {
        this.config = {
          defaultBackend: config.defaultBackend || "aider",
          selectionStrategy: config.selectionStrategy || "auto",
          maxConcurrentSessions: config.maxConcurrentSessions || 3,
          timeout: config.timeout || getDefaultTimeoutMs(),
          // Use centralized default (20 minutes)
          retryAttempts: config.retryAttempts || 2,
          ...config
        };
        this.backends = /* @__PURE__ */ new Map();
        this.activeSessionCount = 0;
        this.sessionBackendMap = /* @__PURE__ */ new Map();
        this.initialized = false;
      }
      /**
       * Initialize the backend manager
       * @returns {Promise<void>}
       */
      async initialize() {
        if (this.initialized) return;
        for (const [name, backend] of this.backends) {
          try {
            if (!backend.initialized) {
              await backend.initialize(this.config.backends?.[name] || {});
            }
          } catch (error) {
            console.warn(`Failed to initialize backend '${name}':`, error.message);
          }
        }
        this.initialized = true;
      }
      /**
       * Register a new backend
       * @param {import('../backends/BaseBackend')} backend - Backend instance
       * @returns {Promise<void>}
       */
      async registerBackend(backend) {
        if (!backend || !backend.name) {
          throw new Error("Invalid backend: must have a name property");
        }
        if (this.backends.has(backend.name)) {
          console.warn(`Backend '${backend.name}' is already registered, replacing...`);
        }
        this.backends.set(backend.name, backend);
        if (this.initialized && !backend.initialized) {
          try {
            await backend.initialize(this.config.backends?.[backend.name] || {});
          } catch (error) {
            console.warn(`Failed to initialize backend '${backend.name}':`, error.message);
          }
        }
      }
      /**
       * Unregister a backend
       * @param {string} name - Backend name
       * @returns {Promise<void>}
       */
      async unregisterBackend(name) {
        const backend = this.backends.get(name);
        if (backend) {
          await backend.cleanup();
          this.backends.delete(name);
        }
      }
      /**
       * Get list of available backend names
       * @returns {string[]}
       */
      getAvailableBackends() {
        return Array.from(this.backends.keys());
      }
      /**
       * Get backend instance by name
       * @param {string} name - Backend name
       * @returns {import('../backends/BaseBackend')|null}
       */
      getBackend(name) {
        return this.backends.get(name) || null;
      }
      /**
       * Get backend information
       * @param {string} name - Backend name
       * @returns {import('../types/BackendTypes').BackendInfo|null}
       */
      async getBackendInfo(name) {
        const backend = this.backends.get(name);
        if (!backend) return null;
        const info = backend.getInfo();
        info.available = await backend.isAvailable();
        return info;
      }
      /**
       * Select appropriate backend for request
       * @param {import('../types/BackendTypes').ImplementRequest} request - Implementation request
       * @returns {Promise<string>} Selected backend name
       */
      async selectBackend(request) {
        switch (this.config.selectionStrategy) {
          case "preference":
            return this.selectByPreference(request);
          case "capability":
            return this.selectByCapability(request);
          case "auto":
          default:
            return this.selectAuto(request);
        }
      }
      /**
       * Select backend using auto strategy
       * @param {import('../types/BackendTypes').ImplementRequest} request - Implementation request
       * @returns {Promise<string>}
       * @private
       */
      async selectAuto(request) {
        console.error("[BackendManager] Starting backend selection with strategy: auto");
        console.error(`[BackendManager] Default backend: ${this.config.defaultBackend}`);
        console.error(`[BackendManager] Available backends: ${Array.from(this.backends.keys()).join(", ")}`);
        if (request.options?.backend && this.backends.has(request.options.backend)) {
          console.error(`[BackendManager] Checking explicit backend: ${request.options.backend}`);
          const backend = this.backends.get(request.options.backend);
          if (await backend.isAvailable()) {
            console.error(`[BackendManager] Selected explicit backend: ${request.options.backend}`);
            return request.options.backend;
          } else {
            console.error(`[BackendManager] Explicit backend ${request.options.backend} is not available`);
            throw new BackendError(
              `Requested backend '${request.options.backend}' is not available`,
              ErrorTypes.BACKEND_NOT_FOUND,
              "BACKEND_NOT_AVAILABLE"
            );
          }
        }
        if (this.backends.has(this.config.defaultBackend)) {
          console.error(`[BackendManager] Checking default backend: ${this.config.defaultBackend}`);
          const backend = this.backends.get(this.config.defaultBackend);
          const isAvailable = await backend.isAvailable();
          console.error(`[BackendManager] Default backend ${this.config.defaultBackend} available: ${isAvailable}`);
          if (isAvailable) {
            console.error(`[BackendManager] Selected default backend: ${this.config.defaultBackend}`);
            return this.config.defaultBackend;
          }
        }
        console.error("[BackendManager] No available backends found!");
        throw new BackendError(
          `Default backend '${this.config.defaultBackend}' is not available`,
          ErrorTypes.BACKEND_NOT_FOUND,
          "NO_AVAILABLE_BACKENDS"
        );
      }
      /**
       * Select backend by user preference
       * @param {import('../types/BackendTypes').ImplementRequest} request - Implementation request
       * @returns {Promise<string>}
       * @private
       */
      async selectByPreference(request) {
        const preference = request.options?.backend || this.config.defaultBackend;
        if (!this.backends.has(preference)) {
          throw new BackendError(
            `Preferred backend '${preference}' not found`,
            ErrorTypes.BACKEND_NOT_FOUND,
            "PREFERRED_BACKEND_NOT_FOUND"
          );
        }
        const backend = this.backends.get(preference);
        if (!await backend.isAvailable()) {
          throw new BackendError(
            `Preferred backend '${preference}' is not available`,
            ErrorTypes.BACKEND_NOT_FOUND,
            "PREFERRED_BACKEND_UNAVAILABLE"
          );
        }
        return preference;
      }
      /**
       * Select backend by capability matching
       * @param {import('../types/BackendTypes').ImplementRequest} request - Implementation request
       * @returns {Promise<string>}
       * @private
       */
      async selectByCapability(request) {
        const candidates = [];
        for (const [name, backend] of this.backends) {
          if (!await backend.isAvailable()) continue;
          const capabilities = backend.getCapabilities();
          let score = 0;
          if (request.context?.language) {
            if (capabilities.supportsLanguages.includes(request.context.language)) {
              score += 10;
            } else if (capabilities.supportsLanguages.includes("all")) {
              score += 5;
            }
          }
          if (request.options?.generateTests && capabilities.supportsTestGeneration) {
            score += 5;
          }
          if (request.options?.streaming && capabilities.supportsStreaming) {
            score += 3;
          }
          score += Math.min(capabilities.maxConcurrentSessions, 5);
          candidates.push({ name, score });
        }
        if (candidates.length === 0) {
          throw new BackendError(
            "No capable backends found",
            ErrorTypes.BACKEND_NOT_FOUND,
            "NO_CAPABLE_BACKENDS"
          );
        }
        candidates.sort((a, b) => b.score - a.score);
        return candidates[0].name;
      }
      /**
       * Execute implementation request
       * @param {import('../types/BackendTypes').ImplementRequest} request - Implementation request
       * @returns {Promise<import('../types/BackendTypes').ImplementResult>}
       */
      async executeImplementation(request) {
        await this.initialize();
        if (this.activeSessionCount >= this.config.maxConcurrentSessions) {
          throw new BackendError(
            "Maximum concurrent sessions reached",
            ErrorTypes.QUOTA_EXCEEDED,
            "MAX_SESSIONS_REACHED",
            { limit: this.config.maxConcurrentSessions, current: this.activeSessionCount }
          );
        }
        console.error(`[BackendManager] Selecting backend for request ${request.sessionId}`);
        const backendName = await this.selectBackend(request);
        console.error(`[BackendManager] Selected backend: ${backendName}`);
        const backend = this.backends.get(backendName);
        if (!backend) {
          throw new BackendError(
            `Backend '${backendName}' not found`,
            ErrorTypes.BACKEND_NOT_FOUND,
            "BACKEND_NOT_FOUND"
          );
        }
        this.activeSessionCount++;
        this.sessionBackendMap.set(request.sessionId, backendName);
        try {
          if (!request.options?.timeout) {
            request.options = request.options || {};
            request.options.timeout = this.config.timeout;
          }
          const result = await RetryHandler.withRetry(
            () => backend.execute(request),
            {
              maxAttempts: this.config.retryAttempts + 1,
              shouldRetry: /* @__PURE__ */ __name((error) => {
                if (error instanceof BackendError) {
                  if (error.type === ErrorTypes.CANCELLATION || error.type === ErrorTypes.VALIDATION_ERROR) {
                    return false;
                  }
                }
                return ErrorHandler.isRetryable(error);
              }, "shouldRetry")
            }
          );
          result.backend = backendName;
          return result;
        } catch (error) {
          if (this.config.fallbackBackends.length > 0) {
            console.warn(`Backend '${backendName}' failed, trying fallbacks...`);
            for (const fallbackName of this.config.fallbackBackends) {
              if (fallbackName === backendName) continue;
              const fallbackBackend = this.backends.get(fallbackName);
              if (!fallbackBackend || !await fallbackBackend.isAvailable()) {
                continue;
              }
              try {
                console.log(`Trying fallback backend: ${fallbackName}`);
                this.sessionBackendMap.set(request.sessionId, fallbackName);
                const result = await fallbackBackend.execute(request);
                result.backend = fallbackName;
                result.fallback = true;
                return result;
              } catch (fallbackError) {
                console.warn(`Fallback backend '${fallbackName}' also failed:`, fallbackError.message);
              }
            }
          }
          throw error;
        } finally {
          this.activeSessionCount--;
          this.sessionBackendMap.delete(request.sessionId);
        }
      }
      /**
       * Cancel an implementation session
       * @param {string} sessionId - Session ID
       * @returns {Promise<void>}
       */
      async cancelImplementation(sessionId2) {
        const backendName = this.sessionBackendMap.get(sessionId2);
        if (!backendName) {
          throw new BackendError(
            `Session '${sessionId2}' not found`,
            ErrorTypes.SESSION_NOT_FOUND,
            "SESSION_NOT_FOUND"
          );
        }
        const backend = this.backends.get(backendName);
        if (backend) {
          await backend.cancel(sessionId2);
        }
        this.sessionBackendMap.delete(sessionId2);
        this.activeSessionCount = Math.max(0, this.activeSessionCount - 1);
      }
      /**
       * Get session status
       * @param {string} sessionId - Session ID
       * @returns {Promise<import('../types/BackendTypes').BackendStatus>}
       */
      async getSessionStatus(sessionId2) {
        const backendName = this.sessionBackendMap.get(sessionId2);
        if (!backendName) {
          return {
            status: "unknown",
            message: "Session not found"
          };
        }
        const backend = this.backends.get(backendName);
        if (!backend) {
          return {
            status: "error",
            message: "Backend not found"
          };
        }
        const status = await backend.getStatus(sessionId2);
        status.backend = backendName;
        return status;
      }
      /**
       * Validate configuration
       * @returns {Promise<import('../types/BackendTypes').ValidationResult>}
       */
      async validateConfiguration() {
        const errors = [];
        const warnings = [];
        if (!this.backends.has(this.config.defaultBackend)) {
          errors.push(`Default backend '${this.config.defaultBackend}' not registered`);
        }
        let hasAvailable = false;
        for (const [name, backend] of this.backends) {
          if (await backend.isAvailable()) {
            hasAvailable = true;
            break;
          }
        }
        if (!hasAvailable) {
          errors.push("No backends are available");
        }
        return {
          valid: errors.length === 0,
          errors,
          warnings
        };
      }
      /**
       * Check health of all backends
       * @param {string} [name] - Specific backend to check, or all if not specified
       * @returns {Promise<Object>}
       */
      async checkBackendHealth(name = null) {
        const results = {};
        const backendsToCheck = name ? [name] : Array.from(this.backends.keys());
        for (const backendName of backendsToCheck) {
          const backend = this.backends.get(backendName);
          if (!backend) continue;
          try {
            const available = await backend.isAvailable();
            const info = backend.getInfo();
            results[backendName] = {
              status: available ? "healthy" : "unavailable",
              available,
              version: info.version,
              capabilities: info.capabilities,
              dependencies: info.dependencies
            };
          } catch (error) {
            results[backendName] = {
              status: "error",
              available: false,
              error: error.message
            };
          }
        }
        return results;
      }
      /**
       * Get recommended backend for a given context
       * @param {Object} context - Request context
       * @returns {string|null}
       */
      getRecommendedBackend(context3) {
        if (context3.language) {
          for (const [name, backend] of this.backends) {
            const capabilities = backend.getCapabilities();
            if (capabilities.supportsLanguages.includes(context3.language)) {
              return name;
            }
          }
        }
        return this.config.defaultBackend;
      }
      /**
       * Check if a backend can handle a request
       * @param {string} backendName - Backend name
       * @param {import('../types/BackendTypes').ImplementRequest} request - Request to check
       * @returns {boolean}
       */
      canHandleRequest(backendName, request) {
        const backend = this.backends.get(backendName);
        if (!backend) return false;
        const validation = backend.validateRequest(request);
        return validation.valid;
      }
      /**
       * Clean up all backends
       * @returns {Promise<void>}
       */
      async cleanup() {
        const cleanupPromises = [];
        for (const [name, backend] of this.backends) {
          cleanupPromises.push(
            backend.cleanup().catch((error) => {
              console.error(`Error cleaning up backend '${name}':`, error);
            })
          );
        }
        await Promise.all(cleanupPromises);
        this.backends.clear();
        this.sessionBackendMap.clear();
        this.activeSessionCount = 0;
        this.initialized = false;
      }
    };
    BackendManager_default = BackendManager;
  }
});

// implement/backends/BaseBackend.js
var BaseBackend, BaseBackend_default;
var init_BaseBackend = __esm({
  "implement/backends/BaseBackend.js"() {
    init_utils();
    BaseBackend = class _BaseBackend {
      static {
        __name(this, "BaseBackend");
      }
      /**
       * @param {string} name - Backend name
       * @param {string} version - Backend version
       */
      constructor(name, version) {
        if (new.target === _BaseBackend) {
          throw new Error("BaseBackend is an abstract class and cannot be instantiated directly");
        }
        this.name = name;
        this.version = version;
        this.initialized = false;
        this.activeSessions = /* @__PURE__ */ new Map();
      }
      /**
       * Initialize the backend with configuration
       * @param {import('../types/BackendTypes').BackendConfig} config - Backend-specific configuration
       * @returns {Promise<void>}
       * @abstract
       */
      async initialize(config) {
        throw new Error("initialize() must be implemented by subclass");
      }
      /**
       * Check if backend is available and properly configured
       * @returns {Promise<boolean>}
       * @abstract
       */
      async isAvailable() {
        throw new Error("isAvailable() must be implemented by subclass");
      }
      /**
       * Get required dependencies for this backend
       * @returns {import('../types/BackendTypes').Dependency[]}
       * @abstract
       */
      getRequiredDependencies() {
        throw new Error("getRequiredDependencies() must be implemented by subclass");
      }
      /**
       * Execute implementation task
       * @param {import('../types/BackendTypes').ImplementRequest} request - Implementation request
       * @returns {Promise<import('../types/BackendTypes').ImplementResult>}
       * @abstract
       */
      async execute(request) {
        throw new Error("execute() must be implemented by subclass");
      }
      /**
       * Cancel an active implementation session
       * @param {string} sessionId - Session to cancel
       * @returns {Promise<void>}
       */
      async cancel(sessionId2) {
        const session = this.activeSessions.get(sessionId2);
        if (session && session.cancel) {
          await session.cancel();
        }
        this.activeSessions.delete(sessionId2);
      }
      /**
       * Get status of an implementation session
       * @param {string} sessionId - Session ID
       * @returns {Promise<import('../types/BackendTypes').BackendStatus>}
       */
      async getStatus(sessionId2) {
        const session = this.activeSessions.get(sessionId2);
        if (!session) {
          return {
            status: "unknown",
            message: "Session not found"
          };
        }
        return {
          status: session.status || "running",
          progress: session.progress,
          message: session.message,
          details: session.details
        };
      }
      /**
       * Clean up backend resources
       * @returns {Promise<void>}
       */
      async cleanup() {
        const sessionIds = Array.from(this.activeSessions.keys());
        await Promise.all(sessionIds.map((id) => this.cancel(id)));
        this.activeSessions.clear();
        this.initialized = false;
      }
      /**
       * Get backend capabilities
       * @returns {import('../types/BackendTypes').BackendCapabilities}
       */
      getCapabilities() {
        return {
          supportsLanguages: [],
          supportsStreaming: false,
          supportsRollback: false,
          supportsDirectFileEdit: false,
          supportsPlanGeneration: false,
          supportsTestGeneration: false,
          maxConcurrentSessions: 1
        };
      }
      /**
       * Get backend information
       * @returns {import('../types/BackendTypes').BackendInfo}
       */
      getInfo() {
        return {
          name: this.name,
          version: this.version,
          description: this.getDescription(),
          available: false,
          capabilities: this.getCapabilities(),
          dependencies: this.getRequiredDependencies()
        };
      }
      /**
       * Get backend description
       * @returns {string}
       */
      getDescription() {
        return "Implementation backend";
      }
      /**
       * Validate implementation request
       * @param {import('../types/BackendTypes').ImplementRequest} request - Request to validate
       * @returns {import('../types/BackendTypes').ValidationResult}
       */
      validateRequest(request) {
        const errors = [];
        const warnings = [];
        if (!request.sessionId) {
          errors.push("sessionId is required");
        }
        if (!request.task || request.task.trim().length === 0) {
          errors.push("task description is required");
        }
        if (this.activeSessions.size >= this.getCapabilities().maxConcurrentSessions) {
          errors.push(`Maximum concurrent sessions (${this.getCapabilities().maxConcurrentSessions}) reached`);
        }
        if (request.context?.language) {
          const supportedLanguages = this.getCapabilities().supportsLanguages;
          if (supportedLanguages.length > 0 && !supportedLanguages.includes(request.context.language)) {
            warnings.push(`Language '${request.context.language}' may not be fully supported`);
          }
        }
        if (request.options?.generateTests && !this.getCapabilities().supportsTestGeneration) {
          warnings.push("Test generation requested but not supported by this backend");
        }
        return {
          valid: errors.length === 0,
          errors,
          warnings
        };
      }
      /**
       * Create a session info object
       * @param {string} sessionId - Session ID
       * @returns {Object}
       * @protected
       */
      createSessionInfo(sessionId2) {
        return {
          sessionId: sessionId2,
          startTime: Date.now(),
          status: "pending",
          progress: 0,
          message: "Initializing",
          cancel: null,
          details: {}
        };
      }
      /**
       * Update session status
       * @param {string} sessionId - Session ID
       * @param {Partial<import('../types/BackendTypes').BackendStatus>} update - Status update
       * @protected
       */
      updateSessionStatus(sessionId2, update) {
        const session = this.activeSessions.get(sessionId2);
        if (session) {
          Object.assign(session, update);
        }
      }
      /**
       * Check if backend is initialized
       * @throws {Error} If backend is not initialized
       * @protected
       */
      checkInitialized() {
        if (!this.initialized) {
          throw new BackendError(
            `Backend '${this.name}' is not initialized`,
            ErrorTypes.INITIALIZATION_FAILED,
            "BACKEND_NOT_INITIALIZED"
          );
        }
      }
      /**
       * Log message with backend context
       * @param {string} level - Log level
       * @param {string} message - Log message
       * @param {Object} [data] - Additional data
       * @protected
       */
      log(level, message, data = {}) {
        const logMessage = `[${this.name}] ${message}`;
        const logData = { backend: this.name, ...data };
        switch (level) {
          case "debug":
            if (process.env.DEBUG) {
              console.debug(logMessage, logData);
            }
            break;
          case "info":
            console.error(logMessage, logData);
            break;
          case "warn":
            console.warn(logMessage, logData);
            break;
          case "error":
            console.error(logMessage, logData);
            break;
          default:
            console.error(logMessage, logData);
        }
      }
    };
    BaseBackend_default = BaseBackend;
  }
});

// implement/backends/AiderBackend.js
import { spawn, exec } from "child_process";
import { promisify } from "util";
import { promises as fsPromises } from "fs";
import path from "path";
import os from "os";
var execPromise, AiderBackend, AiderBackend_default;
var init_AiderBackend = __esm({
  "implement/backends/AiderBackend.js"() {
    init_BaseBackend();
    init_utils();
    init_timeouts();
    execPromise = promisify(exec);
    AiderBackend = class extends BaseBackend_default {
      static {
        __name(this, "AiderBackend");
      }
      constructor() {
        super("aider", "1.0.0");
        this.config = null;
        this.aiderVersion = null;
      }
      /**
       * @override
       */
      async initialize(config) {
        this.config = {
          command: "aider",
          timeout: getDefaultTimeoutMs(),
          // Use centralized default (20 minutes)
          maxOutputSize: 10 * 1024 * 1024,
          // 10MB
          additionalArgs: [],
          environment: {},
          autoCommit: false,
          modelSelection: "auto",
          ...config
        };
        const available = await this.isAvailable();
        if (!available) {
          throw new BackendError(
            "Aider command not found or not accessible. Please install aider with: pip install aider-chat",
            ErrorTypes.DEPENDENCY_MISSING,
            "AIDER_NOT_FOUND"
          );
        }
        try {
          const { stdout } = await execPromise("aider --version", { timeout: TIMEOUTS.VERSION_CHECK });
          this.aiderVersion = stdout.trim();
          this.log("info", `Initialized with aider version: ${this.aiderVersion}`);
        } catch (error) {
          this.log("warn", "Could not determine aider version", { error: error.message });
        }
        this.initialized = true;
      }
      /**
       * @override
       */
      async isAvailable() {
        try {
          await execPromise("which aider", { timeout: TIMEOUTS.VERSION_CHECK });
          const hasApiKey = !!(process.env.ANTHROPIC_API_KEY || process.env.OPENAI_API_KEY || process.env.GOOGLE_API_KEY || process.env.GEMINI_API_KEY);
          if (!hasApiKey) {
            this.log("warn", "No API key found. Aider requires ANTHROPIC_API_KEY, OPENAI_API_KEY, or GOOGLE_API_KEY");
            return false;
          }
          return true;
        } catch (error) {
          return false;
        }
      }
      /**
       * @override
       */
      getRequiredDependencies() {
        return [
          {
            name: "aider-chat",
            type: "pip",
            version: ">=0.20.0",
            installCommand: "pip install aider-chat",
            description: "AI pair programming tool"
          },
          {
            name: "API Key",
            type: "environment",
            description: "One of: ANTHROPIC_API_KEY, OPENAI_API_KEY, GOOGLE_API_KEY, or GEMINI_API_KEY"
          }
        ];
      }
      /**
       * @override
       */
      getCapabilities() {
        return {
          supportsLanguages: ["python", "javascript", "typescript", "go", "rust", "java", "cpp", "c", "csharp", "ruby", "php", "swift"],
          supportsStreaming: true,
          supportsRollback: true,
          supportsDirectFileEdit: true,
          supportsPlanGeneration: false,
          supportsTestGeneration: false,
          maxConcurrentSessions: 3
        };
      }
      /**
       * @override
       */
      getDescription() {
        return "Aider - AI pair programming in your terminal";
      }
      /**
       * @override
       */
      async execute(request) {
        this.checkInitialized();
        const validation = this.validateRequest(request);
        if (!validation.valid) {
          throw new BackendError(
            `Invalid request: ${validation.errors.join(", ")}`,
            ErrorTypes.VALIDATION_ERROR,
            "INVALID_REQUEST"
          );
        }
        const sessionInfo = this.createSessionInfo(request.sessionId);
        const progressTracker = new ProgressTracker(request.sessionId, request.callbacks?.onProgress);
        this.activeSessions.set(request.sessionId, sessionInfo);
        try {
          progressTracker.startStep("prepare", "Preparing aider execution");
          const tempDir = os.tmpdir();
          const tempFileName = `aider-task-${request.sessionId}-${Date.now()}.txt`;
          const tempFilePath = path.join(tempDir, tempFileName);
          await fsPromises.writeFile(tempFilePath, request.task, "utf8");
          sessionInfo.tempFile = tempFilePath;
          this.log("debug", "Created temporary task file", { path: tempFilePath });
          progressTracker.endStep();
          progressTracker.startStep("execute", "Executing aider");
          const workingDir = this.validateWorkingDirectory(request.context?.workingDirectory || process.cwd());
          this.updateSessionStatus(request.sessionId, {
            status: "running",
            progress: 25,
            message: "Aider is processing your request"
          });
          const result = await this.executeCommand(workingDir, request, sessionInfo, progressTracker);
          progressTracker.endStep();
          try {
            await fsPromises.unlink(tempFilePath);
          } catch (error) {
            this.log("warn", "Failed to clean up temp file", { path: tempFilePath, error: error.message });
          }
          this.updateSessionStatus(request.sessionId, {
            status: "completed",
            progress: 100,
            message: "Implementation completed successfully"
          });
          return result;
        } catch (error) {
          if (sessionInfo.tempFile) {
            try {
              await fsPromises.unlink(sessionInfo.tempFile);
            } catch (cleanupError) {
              this.log("warn", "Failed to clean up temp file on error", { error: cleanupError.message });
            }
          }
          this.updateSessionStatus(request.sessionId, {
            status: "failed",
            message: error.message
          });
          if (error instanceof BackendError) {
            throw error;
          }
          throw new BackendError(
            `Aider execution failed: ${error.message}`,
            ErrorTypes.EXECUTION_FAILED,
            "AIDER_EXECUTION_FAILED",
            { originalError: error, sessionId: request.sessionId }
          );
        } finally {
          this.activeSessions.delete(request.sessionId);
        }
      }
      /**
       * Build aider command arguments
       * @param {import('../types/BackendTypes').ImplementRequest} request - Implementation request
       * @param {string} tempFilePath - Path to temporary file with task
       * @returns {Array<string>} Command arguments array for secure execution
       * @private
       */
      buildCommandArgs(request, tempFilePath) {
        if (!tempFilePath || typeof tempFilePath !== "string") {
          throw new BackendError(
            "Invalid temporary file path",
            ErrorTypes.VALIDATION_ERROR,
            "INVALID_TEMP_FILE_PATH"
          );
        }
        const args = [
          "--yes",
          "--no-check-update",
          "--no-analytics",
          "--message-file",
          tempFilePath
          // Separate argument to prevent injection
        ];
        if (!request.options?.autoCommit && !this.config.autoCommit) {
          args.push("--no-auto-commits");
        }
        const model = this.selectModel(request);
        if (model) {
          if (this.isValidModelName(model)) {
            args.push("--model");
            args.push(model);
          } else {
            this.log("warn", `Invalid model name ignored: ${model}`);
          }
        }
        if (request.options?.timeout || this.config.timeout) {
          const timeoutSeconds = Math.floor((request.options?.timeout || this.config.timeout) / 1e3);
        }
        if (this.config.additionalArgs && this.config.additionalArgs.length > 0) {
          const validatedArgs = this.validateAdditionalArgs(this.config.additionalArgs);
          args.push(...validatedArgs);
        }
        if (request.options?.additionalArgs) {
          const validatedArgs = this.validateAdditionalArgs(request.options.additionalArgs);
          args.push(...validatedArgs);
        }
        return args;
      }
      /**
       * Select the appropriate model based on configuration and environment
       * @param {import('../types/BackendTypes').ImplementRequest} request - Implementation request
       * @returns {string|null} Model identifier or null
       * @private
       */
      selectModel(request) {
        if (request.options?.model) {
          return request.options.model;
        }
        if (this.config.model) {
          return this.config.model;
        }
        if (this.config.modelSelection === "auto") {
          const geminiApiKey = process.env.GEMINI_API_KEY || process.env.GOOGLE_API_KEY;
          const anthropicApiKey = process.env.ANTHROPIC_API_KEY;
          const openaiApiKey = process.env.OPENAI_API_KEY;
          if (geminiApiKey) {
            return "gemini/gemini-2.5-pro";
          } else if (anthropicApiKey) {
            return "claude-3-5-sonnet-20241022";
          } else if (openaiApiKey) {
            return "gpt-4";
          }
        }
        return null;
      }
      /**
       * Validate model name to prevent command injection
       * @param {string} model - Model name to validate
       * @returns {boolean} True if valid, false otherwise
       * @private
       */
      isValidModelName(model) {
        return model && typeof model === "string" && model.trim().length > 0;
      }
      /**
       * Validate additional arguments to prevent command injection
       * @param {Array<string>} args - Arguments to validate
       * @returns {Array<string>} Validated arguments
       * @private
       */
      validateAdditionalArgs(args) {
        if (!Array.isArray(args)) {
          this.log("warn", "additionalArgs must be an array, ignoring");
          return [];
        }
        const validatedArgs = [];
        const maxArgLength = 500;
        for (const arg of args) {
          if (typeof arg !== "string") {
            this.log("warn", `Skipping non-string argument: ${typeof arg}`);
            continue;
          }
          if (arg.length > maxArgLength) {
            this.log("warn", `Skipping overly long argument (${arg.length} chars)`);
            continue;
          }
          if (this.containsShellMetacharacters(arg)) {
            this.log("warn", `Skipping argument with shell metacharacters: ${arg.substring(0, 50)}`);
            continue;
          }
          if (this.isValidAiderArgument(arg)) {
            validatedArgs.push(arg);
          } else {
            this.log("warn", `Skipping potentially unsafe argument: ${arg.substring(0, 50)}`);
          }
        }
        return validatedArgs;
      }
      /**
       * Check if string contains shell metacharacters
       * @param {string} str - String to check
       * @returns {boolean} True if contains metacharacters
       * @private
       */
      containsShellMetacharacters(str) {
        const shellMetacharacters = /[;&|`$(){}[\]<>*?'"\\]/;
        const controlChars = /[\x00-\x1f\x7f]/;
        return shellMetacharacters.test(str) || controlChars.test(str);
      }
      /**
       * Validate if argument is a known safe aider argument
       * @param {string} arg - Argument to validate
       * @returns {boolean} True if valid aider argument
       * @private
       */
      isValidAiderArgument(arg) {
        const safeAiderFlags = [
          "--yes",
          "--no-check-update",
          "--no-analytics",
          "--no-auto-commits",
          "--model",
          "--message-file",
          "--dry-run",
          "--map-tokens",
          "--show-model-warnings",
          "--no-show-model-warnings",
          "--edit-format",
          "--architect",
          "--weak-model",
          "--cache-prompts",
          "--no-cache-prompts",
          "--map-refresh",
          "--restore-chat-history",
          "--encoding",
          "--config"
        ];
        if (safeAiderFlags.includes(arg)) {
          return true;
        }
        for (const flag of safeAiderFlags) {
          if (arg.startsWith(flag + "=")) {
            const value = arg.substring(flag.length + 1);
            return !this.containsShellMetacharacters(value) && value.length <= 100;
          }
        }
        if (!arg.startsWith("-") && !this.containsShellMetacharacters(arg) && arg.length <= 100) {
          return true;
        }
        return false;
      }
      /**
       * Validate command path to prevent command injection
       * @param {string} command - Command to validate
       * @returns {boolean} True if valid
       * @private
       */
      isValidCommand(command) {
        if (!command || typeof command !== "string") {
          return false;
        }
        const validCommandPattern = /^[a-zA-Z0-9._/-]+$/;
        const maxLength = 200;
        return validCommandPattern.test(command) && command.length <= maxLength && !this.containsShellMetacharacters(command);
      }
      /**
       * Validate working directory path
       * @param {string} dir - Directory path to validate
       * @returns {string} Validated directory path
       * @private
       */
      validateWorkingDirectory(dir) {
        if (!dir || typeof dir !== "string") {
          throw new BackendError(
            "Invalid working directory",
            ErrorTypes.VALIDATION_ERROR,
            "INVALID_WORKING_DIRECTORY"
          );
        }
        const resolvedPath = path.resolve(dir);
        if (this.containsShellMetacharacters(resolvedPath)) {
          throw new BackendError(
            "Working directory contains unsafe characters",
            ErrorTypes.VALIDATION_ERROR,
            "UNSAFE_WORKING_DIRECTORY"
          );
        }
        return resolvedPath;
      }
      /**
       * Validate environment variables
       * @param {Object} env - Environment variables to validate
       * @returns {Object} Validated environment variables
       * @private
       */
      validateEnvironment(env) {
        if (!env || typeof env !== "object") {
          return {};
        }
        const validatedEnv = {};
        const maxValueLength = 1e3;
        for (const [key, value] of Object.entries(env)) {
          if (typeof key !== "string" || !/^[A-Z_][A-Z0-9_]*$/i.test(key)) {
            this.log("warn", `Skipping invalid environment variable key: ${key}`);
            continue;
          }
          if (typeof value !== "string") {
            this.log("warn", `Skipping non-string environment variable value for: ${key}`);
            continue;
          }
          if (value.length > maxValueLength) {
            this.log("warn", `Skipping overly long environment variable value for: ${key}`);
            continue;
          }
          if (/[\x00-\x1f\x7f]/.test(value)) {
            this.log("warn", `Skipping environment variable with control characters: ${key}`);
            continue;
          }
          validatedEnv[key] = value;
        }
        return validatedEnv;
      }
      /**
       * Execute aider command
       * @param {string} workingDir - Working directory
       * @param {import('../types/BackendTypes').ImplementRequest} request - Implementation request
       * @param {Object} sessionInfo - Session information
       * @param {ProgressTracker} progressTracker - Progress tracker
       * @returns {Promise<import('../types/BackendTypes').ImplementResult>}
       * @private
       */
      async executeCommand(workingDir, request, sessionInfo, progressTracker) {
        return new Promise((resolve3, reject) => {
          const startTime = Date.now();
          const commandArgs = this.buildCommandArgs(request, sessionInfo.tempFile);
          const commandPath = this.config.command || "aider";
          this.log("info", "Executing aider command", {
            command: commandPath,
            args: commandArgs.slice(0, 5),
            // Log first few args only for security
            workingDir
          });
          if (!this.isValidCommand(commandPath)) {
            throw new BackendError(
              "Invalid or unsafe command path",
              ErrorTypes.VALIDATION_ERROR,
              "INVALID_COMMAND_PATH"
            );
          }
          const childProcess = spawn(commandPath, commandArgs, {
            cwd: workingDir,
            env: { ...process.env, ...this.validateEnvironment(this.config.environment) }
          });
          sessionInfo.childProcess = childProcess;
          sessionInfo.cancel = () => {
            if (childProcess && !childProcess.killed) {
              this.log("info", "Cancelling aider process", { sessionId: request.sessionId });
              childProcess.kill("SIGTERM");
              setTimeout(() => {
                if (!childProcess.killed) {
                  childProcess.kill("SIGKILL");
                }
              }, 5e3);
            }
          };
          let stdoutData = "";
          let stderrData = "";
          let outputSize = 0;
          let lastProgressUpdate = Date.now();
          childProcess.stdout.on("data", (data) => {
            const output = data.toString();
            outputSize += output.length;
            if (outputSize > this.config.maxOutputSize) {
              childProcess.kill("SIGTERM");
              reject(new BackendError(
                "Output size exceeded maximum limit",
                ErrorTypes.EXECUTION_FAILED,
                "OUTPUT_TOO_LARGE",
                { limit: this.config.maxOutputSize, actual: outputSize }
              ));
              return;
            }
            stdoutData += output;
            process.stderr.write(output);
            const now = Date.now();
            if (now - lastProgressUpdate > 1e3) {
              progressTracker.reportMessage(output.trim(), "stdout");
              lastProgressUpdate = now;
              const elapsedSeconds = Math.floor((now - startTime) / 1e3);
              const estimatedProgress = Math.min(25 + elapsedSeconds * 2, 90);
              this.updateSessionStatus(request.sessionId, {
                progress: estimatedProgress
              });
            }
          });
          childProcess.stderr.on("data", (data) => {
            const output = data.toString();
            stderrData += output;
            process.stderr.write(output);
            if (output.toLowerCase().includes("warning") || output.toLowerCase().includes("error")) {
              progressTracker.reportMessage(output.trim(), "stderr");
            }
          });
          childProcess.on("close", (code) => {
            const executionTime = Date.now() - startTime;
            clearTimeout(timeoutId);
            this.log("info", `Aider process exited`, {
              code,
              executionTime,
              outputSize: stdoutData.length
            });
            const changes = FileChangeParser.parseChanges(stdoutData + stderrData, workingDir);
            const diffStats = FileChangeParser.extractDiffStats(stdoutData + stderrData);
            if (code === 0) {
              const combinedOutput = stdoutData + stderrData;
              const hasAuthError = /AuthenticationError|Invalid API key|insufficient permissions|not able to authenticate/i.test(combinedOutput);
              const hasOtherErrors = /Error:|Exception:|Failed:|fatal:/i.test(combinedOutput);
              const hasChanges = changes.length > 0;
              const isActualSuccess = !hasAuthError && !hasOtherErrors && (hasChanges || !request.requiresChanges);
              if (isActualSuccess) {
                resolve3({
                  success: true,
                  sessionId: request.sessionId,
                  output: stdoutData,
                  changes,
                  metrics: {
                    executionTime,
                    filesModified: changes.length,
                    linesChanged: diffStats.insertions + diffStats.deletions,
                    tokensUsed: TokenEstimator.estimate(request.task + stdoutData),
                    exitCode: code
                  },
                  metadata: {
                    command: commandPath,
                    args: commandArgs.slice(0, 5),
                    // Limited args for security
                    workingDirectory: workingDir,
                    aiderVersion: this.aiderVersion
                  }
                });
              } else {
                const errorType = hasAuthError ? "AUTHENTICATION_ERROR" : "EXECUTION_FAILED";
                const errorMessage = hasAuthError ? "Authentication failed - check API key and permissions" : `Aider completed but encountered errors: ${combinedOutput.substring(0, 200)}...`;
                reject(new BackendError(
                  errorMessage,
                  hasAuthError ? ErrorTypes.AUTHENTICATION : ErrorTypes.EXECUTION_FAILED,
                  errorType,
                  {
                    exitCode: code,
                    hasChanges,
                    hasAuthError,
                    hasOtherErrors,
                    stdout: stdoutData.substring(0, 1e3),
                    stderr: stderrData.substring(0, 1e3)
                  }
                ));
              }
            } else {
              reject(new BackendError(
                `Aider process exited with code ${code}`,
                ErrorTypes.EXECUTION_FAILED,
                "AIDER_PROCESS_FAILED",
                {
                  exitCode: code,
                  stdout: stdoutData.substring(0, 1e3),
                  stderr: stderrData.substring(0, 1e3)
                }
              ));
            }
          });
          childProcess.on("error", (error) => {
            clearTimeout(timeoutId);
            this.log("error", "Failed to spawn aider process", { error: error.message });
            reject(new BackendError(
              `Failed to spawn aider process: ${error.message}`,
              ErrorTypes.EXECUTION_FAILED,
              "AIDER_SPAWN_FAILED",
              { originalError: error }
            ));
          });
          const timeout = request.options?.timeout || this.config.timeout;
          const timeoutId = setTimeout(() => {
            if (!childProcess.killed) {
              this.log("warn", "Aider execution timed out", { timeout });
              childProcess.kill("SIGTERM");
              reject(new BackendError(
                `Aider execution timed out after ${timeout}ms`,
                ErrorTypes.TIMEOUT,
                "AIDER_TIMEOUT",
                { timeout }
              ));
            }
          }, timeout);
        });
      }
    };
    AiderBackend_default = AiderBackend;
  }
});

// implement/backends/ClaudeCodeBackend.js
import { exec as exec2, spawn as spawn2 } from "child_process";
import { promisify as promisify2 } from "util";
import path2 from "path";
var execPromise2, ClaudeCodeBackend, ClaudeCodeBackend_default;
var init_ClaudeCodeBackend = __esm({
  "implement/backends/ClaudeCodeBackend.js"() {
    init_BaseBackend();
    init_utils();
    init_timeouts();
    execPromise2 = promisify2(exec2);
    ClaudeCodeBackend = class extends BaseBackend_default {
      static {
        __name(this, "ClaudeCodeBackend");
      }
      constructor() {
        super("claude-code", "1.0.0");
        this.config = null;
      }
      /**
       * @override
       */
      async initialize(config) {
        this.config = {
          apiKey: config.apiKey || process.env.ANTHROPIC_API_KEY,
          model: config.model || "claude-3-5-sonnet-20241022",
          baseUrl: config.baseUrl,
          timeout: config.timeout || getDefaultTimeoutMs(),
          // Use centralized default (20 minutes)
          maxTokens: config.maxTokens || 8e3,
          temperature: config.temperature || 0.3,
          systemPrompt: config.systemPrompt,
          tools: config.tools || ["edit", "search", "bash"],
          maxTurns: config.maxTurns || 100,
          ...config
        };
        try {
          this.log("debug", "Using Claude Code CLI interface");
          await this.validateConfiguration();
          const available = await this.isAvailable();
          if (!available) {
            throw new Error("Claude Code is not available");
          }
          this.initialized = true;
        } catch (error) {
          throw new BackendError(
            `Failed to initialize Claude Code backend: ${error.message}`,
            ErrorTypes.INITIALIZATION_FAILED,
            "CLAUDE_CODE_INIT_FAILED",
            { originalError: error }
          );
        }
      }
      /**
       * @override
       */
      async isAvailable() {
        if (!this.config.apiKey) {
          this.log("warn", "No API key configured");
          return false;
        }
        try {
          let claudeCommand = null;
          try {
            await execPromise2("claude --version", { timeout: TIMEOUTS.VERSION_CHECK });
            claudeCommand = "claude";
            this.log("debug", "Claude found in PATH via direct execution");
          } catch (directError) {
            this.log("debug", "Claude not directly executable from PATH", { error: directError.message });
          }
          if (!claudeCommand) {
            try {
              const { stdout } = await execPromise2("npm list -g @anthropic-ai/claude-code --depth=0", { timeout: TIMEOUTS.VERSION_CHECK });
              if (stdout.includes("@anthropic-ai/claude-code")) {
                const { stdout: binPath } = await execPromise2("npm bin -g", { timeout: TIMEOUTS.VERSION_CHECK });
                const npmBinDir = binPath.trim();
                const isWindows = process.platform === "win32";
                const claudeBinary = isWindows ? "claude.cmd" : "claude";
                const claudePath = path2.join(npmBinDir, claudeBinary);
                try {
                  await execPromise2(`"${claudePath}" --version`, { timeout: TIMEOUTS.VERSION_CHECK });
                  claudeCommand = claudePath;
                  const pathSeparator = isWindows ? ";" : ":";
                  process.env.PATH = `${npmBinDir}${pathSeparator}${process.env.PATH}`;
                  this.log("debug", `Claude found at ${claudePath}, added ${npmBinDir} to PATH`);
                } catch (execError) {
                  this.log("debug", `Failed to execute claude at ${claudePath}`, { error: execError.message });
                }
              }
            } catch (npmError) {
              this.log("debug", "Failed to check npm global packages", { error: npmError.message });
            }
          }
          if (!claudeCommand && process.platform === "win32") {
            try {
              const { stdout: wslCheck } = await execPromise2("wsl --list", { timeout: TIMEOUTS.WSL_CHECK });
              if (wslCheck) {
                this.log("debug", "WSL detected, checking for claude in WSL");
                try {
                  await execPromise2("wsl claude --version", { timeout: TIMEOUTS.VERSION_CHECK });
                  claudeCommand = "wsl claude";
                  this.log("debug", "Claude found in WSL");
                } catch (wslClaudeError) {
                  this.log("debug", "Claude not found in WSL", { error: wslClaudeError.message });
                  const wslPaths = [
                    "wsl /usr/local/bin/claude",
                    "wsl ~/.npm-global/bin/claude",
                    "wsl ~/.local/bin/claude",
                    "wsl ~/node_modules/.bin/claude"
                  ];
                  for (const wslPath of wslPaths) {
                    try {
                      await execPromise2(`${wslPath} --version`, { timeout: TIMEOUTS.WSL_CHECK });
                      claudeCommand = wslPath;
                      this.log("debug", `Claude found in WSL at: ${wslPath}`);
                      break;
                    } catch (e) {
                    }
                  }
                }
              }
            } catch (wslError) {
              this.log("debug", "WSL not available or accessible", { error: wslError.message });
            }
          }
          if (!claudeCommand) {
            const isWindows = process.platform === "win32";
            const homeDir = process.env[isWindows ? "USERPROFILE" : "HOME"];
            const claudeBinary = isWindows ? "claude.cmd" : "claude";
            const commonPaths = [
              // Windows paths
              isWindows && path2.join(process.env.APPDATA || "", "npm", claudeBinary),
              isWindows && path2.join("C:", "Program Files", "nodejs", claudeBinary),
              // Unix-like paths
              !isWindows && path2.join("/usr/local/bin", claudeBinary),
              !isWindows && path2.join(homeDir, ".npm-global", "bin", claudeBinary),
              !isWindows && path2.join(homeDir, ".local", "bin", claudeBinary),
              // Cross-platform home directory paths
              path2.join(homeDir, "node_modules", ".bin", claudeBinary)
            ].filter(Boolean);
            for (const claudePath of commonPaths) {
              try {
                await execPromise2(`"${claudePath}" --version`, { timeout: TIMEOUTS.WSL_CHECK });
                claudeCommand = claudePath;
                this.log("debug", `Claude found at ${claudePath}`);
                break;
              } catch (e) {
              }
            }
          }
          if (!claudeCommand) {
            this.log("warn", "Claude Code CLI not found. Please install with: npm install -g @anthropic-ai/claude-code (or in WSL on Windows)");
            return false;
          }
          this.claudeCommand = claudeCommand;
          if (!this.config.apiKey || this.config.apiKey.trim() === "") {
            this.log("warn", "API key is not configured");
            return false;
          }
          return true;
        } catch (error) {
          this.log("debug", "Availability check failed", { error: error.message });
          return false;
        }
      }
      /**
       * @override
       */
      getRequiredDependencies() {
        return [
          {
            name: "claude-code",
            type: "cli",
            installCommand: "npm install -g @anthropic-ai/claude-code",
            description: "Claude Code CLI tool"
          },
          {
            name: "ANTHROPIC_API_KEY",
            type: "environment",
            description: "Anthropic API key for Claude Code"
          }
        ];
      }
      /**
       * @override
       */
      getCapabilities() {
        return {
          supportsLanguages: ["javascript", "typescript", "python", "rust", "go", "java", "c++", "c#", "ruby", "php", "swift"],
          supportsStreaming: true,
          supportsRollback: false,
          supportsDirectFileEdit: true,
          supportsPlanGeneration: true,
          supportsTestGeneration: true,
          maxConcurrentSessions: 5
        };
      }
      /**
       * @override
       */
      getDescription() {
        return "Claude Code CLI - Advanced AI coding assistant powered by Claude";
      }
      /**
       * @override
       */
      async execute(request) {
        this.checkInitialized();
        const validation = this.validateRequest(request);
        if (!validation.valid) {
          throw new BackendError(
            `Invalid request: ${validation.errors.join(", ")}`,
            ErrorTypes.VALIDATION_ERROR,
            "INVALID_REQUEST"
          );
        }
        const sessionInfo = this.createSessionInfo(request.sessionId);
        const progressTracker = new ProgressTracker(request.sessionId, request.callbacks?.onProgress);
        this.activeSessions.set(request.sessionId, sessionInfo);
        try {
          progressTracker.startStep("prepare", "Preparing Claude Code execution");
          const prompt = this.buildPrompt(request);
          const workingDir = request.context?.workingDirectory || process.cwd();
          this.updateSessionStatus(request.sessionId, {
            status: "running",
            progress: 25,
            message: "Claude Code is processing your request"
          });
          progressTracker.endStep();
          progressTracker.startStep("execute", "Executing with Claude Code");
          const result = await this.executeWithCLI(prompt, workingDir, request, sessionInfo, progressTracker);
          progressTracker.endStep();
          this.updateSessionStatus(request.sessionId, {
            status: "completed",
            progress: 100,
            message: "Implementation completed successfully"
          });
          return result;
        } catch (error) {
          this.updateSessionStatus(request.sessionId, {
            status: "failed",
            message: error.message
          });
          if (error instanceof BackendError) {
            throw error;
          }
          throw new BackendError(
            `Claude Code execution failed: ${error.message}`,
            ErrorTypes.EXECUTION_FAILED,
            "CLAUDE_CODE_EXECUTION_FAILED",
            { originalError: error, sessionId: request.sessionId }
          );
        } finally {
          this.activeSessions.delete(request.sessionId);
        }
      }
      /**
       * Validate configuration
       * @private
       */
      async validateConfiguration() {
        if (!this.config.apiKey) {
          throw new Error("API key is required. Set ANTHROPIC_API_KEY environment variable or provide apiKey in config");
        }
      }
      /**
       * Build prompt for Claude Code
       * @param {import('../types/BackendTypes').ImplementRequest} request - Implementation request
       * @returns {string} Formatted prompt
       * @private
       */
      buildPrompt(request) {
        let prompt = "";
        if (request.context?.additionalContext) {
          prompt += `Context:
${request.context.additionalContext}

`;
        }
        prompt += `Task:
${request.task}
`;
        if (request.context?.allowedFiles && request.context.allowedFiles.length > 0) {
          prompt += `
Only modify these files: ${request.context.allowedFiles.join(", ")}
`;
        }
        if (request.context?.language) {
          prompt += `
Primary language: ${request.context.language}
`;
        }
        if (request.options?.generateTests) {
          prompt += "\nAlso generate appropriate tests for the implemented functionality.\n";
        }
        if (request.options?.dryRun) {
          prompt += "\nThis is a dry run - describe what changes would be made without actually implementing them.\n";
        }
        return prompt.trim();
      }
      /**
       * Build system prompt for Claude Code
       * @param {import('../types/BackendTypes').ImplementRequest} request - Implementation request
       * @returns {string} System prompt
       * @private
       */
      buildSystemPrompt(request) {
        if (this.config.systemPrompt) {
          return this.config.systemPrompt;
        }
        return `You are an expert software developer assistant using Claude Code. Your task is to implement code changes based on user requirements.

Key guidelines:
- Follow best practices for the detected programming language
- Write clean, maintainable, and well-documented code
- Include error handling where appropriate
- Consider edge cases and potential issues
- Generate tests when requested or when it would be beneficial
- Make minimal, focused changes that achieve the requested functionality
- Preserve existing code style and conventions

Working directory: ${request.context?.workingDirectory || process.cwd()}
${request.context?.allowedFiles ? `Allowed files: ${request.context.allowedFiles.join(", ")}` : ""}
${request.context?.language ? `Primary language: ${request.context.language}` : ""}`;
      }
      /**
       * Execute using CLI interface
       * @private
       */
      async executeWithCLI(prompt, workingDir, request, sessionInfo, progressTracker) {
        const startTime = Date.now();
        const args = this.buildSecureCommandArgs(request);
        const validatedPrompt = this.validatePrompt(prompt);
        args.unshift("-p", validatedPrompt);
        this.log("debug", "Executing Claude Code CLI", {
          command: "claude",
          args: args.slice(0, 5),
          // Log first few args only for security
          workingDir
        });
        console.error(`[INFO] Claude Code execution details:`);
        console.error(`[INFO] Working directory: ${workingDir}`);
        console.error(`[INFO] Environment: ANTHROPIC_API_KEY=${this.config.apiKey ? "***set***" : "***not set***"}`);
        console.error(`[INFO] Prompt length: ${validatedPrompt.length} characters`);
        return new Promise(async (resolve3, reject) => {
          let claudeCommand = this.claudeCommand || "claude";
          if (!this.claudeCommand) {
            try {
              await execPromise2("claude --version", { timeout: TIMEOUTS.PATH_CHECK });
              claudeCommand = "claude";
            } catch (e) {
              const isWindows = process.platform === "win32";
              if (isWindows) {
                try {
                  await execPromise2("wsl claude --version", { timeout: TIMEOUTS.WSL_CHECK });
                  claudeCommand = "wsl claude";
                  this.log("debug", "Using claude from WSL");
                } catch (wslError) {
                }
              }
              if (claudeCommand === "claude") {
                try {
                  const { stdout: binPath } = await execPromise2("npm bin -g", { timeout: TIMEOUTS.PATH_CHECK });
                  const claudeBinary = isWindows ? "claude.cmd" : "claude";
                  const potentialClaudePath = path2.join(binPath.trim(), claudeBinary);
                  await execPromise2(`"${potentialClaudePath}" --version`, { timeout: TIMEOUTS.PATH_CHECK });
                  claudeCommand = potentialClaudePath;
                  this.log("debug", `Using claude from npm global: ${claudeCommand}`);
                } catch (npmError) {
                  this.log("warn", "Could not find claude in npm global bin or WSL, attempting direct execution");
                }
              }
            }
          }
          let spawnCommand = claudeCommand;
          let spawnArgs = args;
          if (claudeCommand.startsWith("wsl ")) {
            const wslParts = claudeCommand.split(" ");
            spawnCommand = wslParts[0];
            spawnArgs = [...wslParts.slice(1), ...args];
          }
          console.error(`[INFO] Executing command: ${spawnCommand} ${spawnArgs.join(" ")}`);
          console.error(`[INFO] Shell mode: ${process.platform === "win32"}`);
          const child = spawn2(spawnCommand, spawnArgs, {
            cwd: workingDir,
            env: this.buildSecureEnvironment(),
            stdio: ["pipe", "pipe", "pipe"],
            shell: process.platform === "win32"
            // Use shell on Windows for .cmd files
          });
          sessionInfo.childProcess = child;
          sessionInfo.cancel = () => {
            if (child && !child.killed) {
              child.kill("SIGTERM");
            }
          };
          let output = "";
          let errorOutput = "";
          if (child.stdin) {
            child.stdin.end();
          }
          if (child.stdout) {
            child.stdout.on("data", (data) => {
              const chunk = data.toString();
              output += chunk;
              process.stderr.write(chunk);
              progressTracker.reportMessage(chunk.trim(), "stdout");
            });
          }
          if (child.stderr) {
            child.stderr.on("data", (data) => {
              const chunk = data.toString();
              errorOutput += chunk;
              process.stderr.write(chunk);
              if (chunk.toLowerCase().includes("error")) {
                progressTracker.reportMessage(chunk.trim(), "stderr");
              }
            });
          }
          child.on("close", (code) => {
            const executionTime = Date.now() - startTime;
            clearTimeout(timeoutId);
            if (code === 0) {
              const changes = FileChangeParser.parseChanges(output, workingDir);
              resolve3({
                success: true,
                sessionId: request.sessionId,
                output,
                changes,
                metrics: {
                  executionTime,
                  tokensUsed: TokenEstimator.estimate(prompt + output),
                  filesModified: changes.length,
                  linesChanged: 0,
                  exitCode: code
                },
                metadata: {
                  command: "claude",
                  args: args.slice(0, 5),
                  // Limited args for security
                  model: this.config.model
                }
              });
            } else {
              console.error(`[ERROR] Claude Code CLI failed with exit code: ${code}`);
              console.error(`[ERROR] Full command: ${claudeCommand} ${args.join(" ")}`);
              console.error(`[ERROR] Working directory: ${workingDir}`);
              console.error(`[ERROR] Full stdout output:`);
              console.error(output || "(no stdout)");
              console.error(`[ERROR] Full stderr output:`);
              console.error(errorOutput || "(no stderr)");
              console.error(`[ERROR] Execution time: ${Date.now() - startTime}ms`);
              reject(new BackendError(
                `Claude Code CLI exited with code ${code}`,
                ErrorTypes.EXECUTION_FAILED,
                "CLI_EXECUTION_FAILED",
                {
                  exitCode: code,
                  stdout: output.substring(0, 1e3),
                  stderr: errorOutput.substring(0, 1e3)
                }
              ));
            }
          });
          child.on("error", (error) => {
            clearTimeout(timeoutId);
            console.error(`[ERROR] Failed to spawn Claude Code CLI process:`);
            console.error(`[ERROR] Command: ${spawnCommand}`);
            console.error(`[ERROR] Args: ${spawnArgs.join(" ")}`);
            console.error(`[ERROR] Working directory: ${workingDir}`);
            console.error(`[ERROR] Error message: ${error.message}`);
            console.error(`[ERROR] Error code: ${error.code || "unknown"}`);
            console.error(`[ERROR] Error signal: ${error.signal || "none"}`);
            console.error(`[ERROR] Full error:`, error);
            reject(new BackendError(
              `Failed to execute Claude Code CLI: ${error.message}`,
              ErrorTypes.EXECUTION_FAILED,
              "CLI_SPAWN_FAILED",
              { originalError: error }
            ));
          });
          const timeout = request.options?.timeout || this.config.timeout;
          const timeoutId = setTimeout(() => {
            if (!child.killed) {
              console.error(`[ERROR] Claude Code CLI timed out after ${timeout}ms`);
              console.error(`[ERROR] Command: ${spawnCommand} ${spawnArgs.join(" ")}`);
              console.error(`[ERROR] Working directory: ${workingDir}`);
              console.error(`[ERROR] Partial stdout output:`);
              console.error(output || "(no stdout)");
              console.error(`[ERROR] Partial stderr output:`);
              console.error(errorOutput || "(no stderr)");
              child.kill("SIGTERM");
              reject(new BackendError(
                `Claude Code execution timed out after ${timeout}ms`,
                ErrorTypes.TIMEOUT,
                "CLAUDE_CODE_TIMEOUT",
                { timeout }
              ));
            }
          }, timeout);
        });
      }
      /**
       * Build secure command arguments
       * @param {import('../types/BackendTypes').ImplementRequest} request - Implementation request
       * @returns {Array<string>} Secure command arguments
       * @private
       */
      buildSecureCommandArgs(request) {
        const args = [];
        const maxTurns = this.validateMaxTurns(request.options?.maxTurns || this.config.maxTurns);
        if (process.env.DEBUG) {
          this.log("debug", "Max turns check", {
            requestMaxTurns: request.options?.maxTurns,
            configMaxTurns: this.config.maxTurns,
            validatedMaxTurns: maxTurns
          });
        }
        args.push("--max-turns", maxTurns.toString());
        args.push("--dangerously-skip-permissions");
        if (process.env.DEBUG) {
          this.log("debug", "Final args constructed", { args });
        }
        return args;
      }
      /**
       * Build secure environment variables
       * @returns {Object} Secure environment variables
       * @private
       */
      buildSecureEnvironment() {
        const env = { ...process.env };
        if (this.config.apiKey && this.isValidApiKey(this.config.apiKey)) {
          env.ANTHROPIC_API_KEY = this.config.apiKey;
        }
        return env;
      }
      /**
       * Validate API key format
       * @param {string} apiKey - API key to validate
       * @returns {boolean} True if valid format
       * @private
       */
      isValidApiKey(apiKey) {
        return apiKey && typeof apiKey === "string" && apiKey.trim().length > 0;
      }
      /**
       * Validate max turns value
       * @param {number} maxTurns - Max turns to validate
       * @returns {number} Validated max turns value
       * @private
       */
      validateMaxTurns(maxTurns) {
        if (typeof maxTurns !== "number" || isNaN(maxTurns) || maxTurns < 1) {
          return 100;
        }
        return Math.min(Math.max(Math.floor(maxTurns), 1), 1e3);
      }
      /**
       * Validate prompt content
       * @param {string} prompt - Prompt to validate
       * @returns {string} Validated prompt
       * @private
       */
      validatePrompt(prompt) {
        if (!prompt || typeof prompt !== "string") {
          throw new BackendError(
            "Invalid prompt content",
            ErrorTypes.VALIDATION_ERROR,
            "INVALID_PROMPT"
          );
        }
        const maxPromptLength = 1e5;
        if (prompt.length > maxPromptLength) {
          throw new BackendError(
            `Prompt too long (${prompt.length} chars, max: ${maxPromptLength})`,
            ErrorTypes.VALIDATION_ERROR,
            "PROMPT_TOO_LONG"
          );
        }
        if (this.containsControlCharacters(prompt)) {
          this.log("warn", "Prompt contains control characters, they will be filtered");
          return prompt.replace(/[\x00-\x08\x0B\x0C\x0E-\x1F\x7F]/g, "");
        }
        return prompt;
      }
      /**
       * Check if string contains problematic control characters
       * @param {string} str - String to check
       * @returns {boolean} True if contains control characters
       * @private
       */
      containsControlCharacters(str) {
        return /[\x00-\x08\x0B\x0C\x0E-\x1F\x7F]/.test(str);
      }
    };
    ClaudeCodeBackend_default = ClaudeCodeBackend;
  }
});

// implement/backends/registry.js
var registry_exports = {};
__export(registry_exports, {
  AiderBackend: () => AiderBackend_default,
  ClaudeCodeBackend: () => ClaudeCodeBackend_default,
  createBackend: () => createBackend,
  getAvailableBackends: () => getAvailableBackends,
  getBackendMetadata: () => getBackendMetadata,
  listBackendNames: () => listBackendNames,
  registerBackend: () => registerBackend
});
function getAvailableBackends() {
  return { ...AVAILABLE_BACKENDS };
}
function createBackend(name) {
  const BackendClass = AVAILABLE_BACKENDS[name];
  if (!BackendClass) {
    return null;
  }
  return new BackendClass();
}
function registerBackend(name, BackendClass) {
  AVAILABLE_BACKENDS[name] = BackendClass;
}
function getBackendMetadata(name) {
  const backend = createBackend(name);
  if (!backend) {
    return null;
  }
  return {
    name: backend.name,
    version: backend.version,
    description: backend.getDescription(),
    capabilities: backend.getCapabilities(),
    dependencies: backend.getRequiredDependencies()
  };
}
function listBackendNames() {
  return Object.keys(AVAILABLE_BACKENDS);
}
var AVAILABLE_BACKENDS;
var init_registry = __esm({
  "implement/backends/registry.js"() {
    init_AiderBackend();
    init_ClaudeCodeBackend();
    AVAILABLE_BACKENDS = {
      aider: AiderBackend_default,
      "claude-code": ClaudeCodeBackend_default
    };
    __name(getAvailableBackends, "getAvailableBackends");
    __name(createBackend, "createBackend");
    __name(registerBackend, "registerBackend");
    __name(getBackendMetadata, "getBackendMetadata");
    __name(listBackendNames, "listBackendNames");
  }
});

// implement/core/config.js
import fs from "fs";
import path3 from "path";
import { promisify as promisify3 } from "util";
import { fileURLToPath } from "url";
var __dirname, readFile, writeFile, exists, ConfigManager, configManager;
var init_config = __esm({
  "implement/core/config.js"() {
    init_timeouts();
    __dirname = path3.dirname(fileURLToPath(import.meta.url));
    readFile = promisify3(fs.readFile);
    writeFile = promisify3(fs.writeFile);
    exists = promisify3(fs.exists);
    ConfigManager = class {
      static {
        __name(this, "ConfigManager");
      }
      constructor() {
        this.config = null;
        this.configPath = null;
        this.watchers = /* @__PURE__ */ new Map();
        this.changeCallbacks = [];
      }
      /**
       * Initialize configuration
       * @param {string} [configPath] - Path to configuration file
       * @returns {Promise<void>}
       */
      async initialize(configPath = null) {
        this.configPath = this.resolveConfigPath(configPath);
        await this.loadConfig();
        this.applyEnvironmentOverrides();
        if (this.configPath && fs.existsSync(this.configPath)) {
          this.setupWatcher();
        }
      }
      /**
       * Resolve configuration file path
       * @param {string} [providedPath] - User-provided path
       * @returns {string|null}
       * @private
       */
      resolveConfigPath(providedPath) {
        if (providedPath && fs.existsSync(providedPath)) {
          return providedPath;
        }
        if (process.env.IMPLEMENT_TOOL_CONFIG_PATH) {
          const envPath = process.env.IMPLEMENT_TOOL_CONFIG_PATH;
          if (fs.existsSync(envPath)) {
            return envPath;
          }
        }
        const localConfig = path3.join(process.cwd(), "implement-config.json");
        if (fs.existsSync(localConfig)) {
          return localConfig;
        }
        const defaultConfig = path3.join(__dirname, "..", "config", "default.json");
        if (fs.existsSync(defaultConfig)) {
          return defaultConfig;
        }
        return null;
      }
      /**
       * Load configuration from file
       * @returns {Promise<void>}
       * @private
       */
      async loadConfig() {
        if (this.configPath && fs.existsSync(this.configPath)) {
          try {
            const configData = await readFile(this.configPath, "utf8");
            this.config = JSON.parse(configData);
            console.error(`Loaded configuration from: ${this.configPath}`);
          } catch (error) {
            console.error(`Failed to load configuration from ${this.configPath}:`, error.message);
            this.config = this.getDefaultConfig();
          }
        } else {
          console.error("Using default configuration");
          this.config = this.getDefaultConfig();
        }
      }
      /**
       * Get default configuration
       * @returns {Object}
       * @private
       */
      getDefaultConfig() {
        return {
          implement: {
            defaultBackend: "aider",
            selectionStrategy: "auto",
            maxConcurrentSessions: 3,
            timeout: getDefaultTimeoutMs(),
            // Use centralized default (20 minutes)
            retryAttempts: 2,
            retryDelay: 5e3
          },
          backends: {
            aider: {
              command: "aider",
              timeout: getDefaultTimeoutMs(),
              // Use centralized default (20 minutes)
              maxOutputSize: 10485760,
              additionalArgs: [],
              environment: {},
              autoCommit: false,
              modelSelection: "auto"
            },
            "claude-code": {
              timeout: getDefaultTimeoutMs(),
              // Use centralized default (20 minutes)
              maxTokens: 8e3,
              temperature: 0.3,
              model: "claude-3-5-sonnet-20241022",
              systemPrompt: null,
              tools: ["edit", "search", "bash"],
              maxTurns: 100
            }
          }
        };
      }
      /**
       * Apply environment variable overrides
       * @private
       */
      applyEnvironmentOverrides() {
        if (process.env.IMPLEMENT_TOOL_BACKEND) {
          this.config.implement.defaultBackend = process.env.IMPLEMENT_TOOL_BACKEND;
          console.error(`[ImplementConfig] Setting default backend from env: ${process.env.IMPLEMENT_TOOL_BACKEND}`);
        }
        if (process.env.IMPLEMENT_TOOL_FALLBACKS) {
          this.config.implement.fallbackBackends = process.env.IMPLEMENT_TOOL_FALLBACKS.split(",").map((s) => s.trim()).filter(Boolean);
        }
        if (process.env.IMPLEMENT_TOOL_SELECTION_STRATEGY) {
          this.config.implement.selectionStrategy = process.env.IMPLEMENT_TOOL_SELECTION_STRATEGY;
        }
        if (process.env.IMPLEMENT_TOOL_TIMEOUT) {
          const timeoutSeconds = parseInt(process.env.IMPLEMENT_TOOL_TIMEOUT, 10);
          if (isNaN(timeoutSeconds)) {
            console.warn(`[Config] Invalid IMPLEMENT_TOOL_TIMEOUT value: ${process.env.IMPLEMENT_TOOL_TIMEOUT}. Using default: ${TIMEOUTS.IMPLEMENT_DEFAULT}s`);
          } else if (!isValidTimeout(timeoutSeconds)) {
            console.warn(`[Config] IMPLEMENT_TOOL_TIMEOUT ${timeoutSeconds}s outside valid range ${TIMEOUTS.IMPLEMENT_MINIMUM}-${TIMEOUTS.IMPLEMENT_MAXIMUM}s. Using default: ${TIMEOUTS.IMPLEMENT_DEFAULT}s`);
          } else {
            this.config.implement.timeout = secondsToMs(timeoutSeconds);
          }
        }
        if (process.env.AIDER_MODEL) {
          this.config.backends.aider = this.config.backends.aider || {};
          this.config.backends.aider.model = process.env.AIDER_MODEL;
        }
        if (process.env.AIDER_TIMEOUT) {
          this.config.backends.aider = this.config.backends.aider || {};
          this.config.backends.aider.timeout = parseInt(process.env.AIDER_TIMEOUT, 10);
        }
        if (process.env.AIDER_AUTO_COMMIT) {
          this.config.backends.aider = this.config.backends.aider || {};
          this.config.backends.aider.autoCommit = process.env.AIDER_AUTO_COMMIT === "true";
        }
        if (process.env.AIDER_ADDITIONAL_ARGS) {
          this.config.backends.aider = this.config.backends.aider || {};
          this.config.backends.aider.additionalArgs = process.env.AIDER_ADDITIONAL_ARGS.split(",").map((s) => s.trim()).filter(Boolean);
        }
        if (process.env.CLAUDE_CODE_MODEL) {
          this.config.backends["claude-code"] = this.config.backends["claude-code"] || {};
          this.config.backends["claude-code"].model = process.env.CLAUDE_CODE_MODEL;
        }
        if (process.env.CLAUDE_CODE_MAX_TOKENS) {
          this.config.backends["claude-code"] = this.config.backends["claude-code"] || {};
          this.config.backends["claude-code"].maxTokens = parseInt(process.env.CLAUDE_CODE_MAX_TOKENS, 10);
        }
        if (process.env.CLAUDE_CODE_TEMPERATURE) {
          this.config.backends["claude-code"] = this.config.backends["claude-code"] || {};
          this.config.backends["claude-code"].temperature = parseFloat(process.env.CLAUDE_CODE_TEMPERATURE);
        }
        if (process.env.CLAUDE_CODE_MAX_TURNS) {
          this.config.backends["claude-code"] = this.config.backends["claude-code"] || {};
          this.config.backends["claude-code"].maxTurns = parseInt(process.env.CLAUDE_CODE_MAX_TURNS, 10);
        }
      }
      /**
       * Set up file watcher for configuration changes
       * @private
       */
      setupWatcher() {
        if (!this.configPath) return;
        fs.watchFile(this.configPath, { interval: 2e3 }, async (curr, prev) => {
          if (curr.mtime !== prev.mtime) {
            console.error("Configuration file changed, reloading...");
            await this.reloadConfig();
          }
        });
      }
      /**
       * Reload configuration from file
       * @returns {Promise<void>}
       */
      async reloadConfig() {
        try {
          const oldConfig = JSON.stringify(this.config);
          await this.loadConfig();
          this.applyEnvironmentOverrides();
          const newConfig = JSON.stringify(this.config);
          if (oldConfig !== newConfig) {
            this.notifyChangeCallbacks();
          }
        } catch (error) {
          console.error("Failed to reload configuration:", error);
        }
      }
      /**
       * Register a callback for configuration changes
       * @param {Function} callback - Callback function
       */
      onChange(callback) {
        this.changeCallbacks.push(callback);
      }
      /**
       * Notify all change callbacks
       * @private
       */
      notifyChangeCallbacks() {
        for (const callback of this.changeCallbacks) {
          try {
            callback(this.config);
          } catch (error) {
            console.error("Error in configuration change callback:", error);
          }
        }
      }
      /**
       * Get configuration value by path
       * @param {string} [path] - Dot-separated path (e.g., 'implement.defaultBackend')
       * @returns {*}
       */
      get(path5 = null) {
        if (!path5) {
          return this.config;
        }
        const parts = path5.split(".");
        let value = this.config;
        for (const part of parts) {
          if (value && typeof value === "object" && part in value) {
            value = value[part];
          } else {
            return void 0;
          }
        }
        return value;
      }
      /**
       * Set configuration value by path
       * @param {string} path - Dot-separated path
       * @param {*} value - Value to set
       */
      set(path5, value) {
        const parts = path5.split(".");
        const lastPart = parts.pop();
        let target = this.config;
        for (const part of parts) {
          if (!(part in target) || typeof target[part] !== "object") {
            target[part] = {};
          }
          target = target[part];
        }
        target[lastPart] = value;
      }
      /**
       * Save configuration to file
       * @param {string} [path] - Path to save to (defaults to current config path)
       * @returns {Promise<void>}
       */
      async save(path5 = null) {
        const savePath = path5 || this.configPath;
        if (!savePath) {
          throw new Error("No configuration file path specified");
        }
        try {
          const configData = JSON.stringify(this.config, null, 2);
          await writeFile(savePath, configData, "utf8");
          console.error(`Configuration saved to: ${savePath}`);
        } catch (error) {
          throw new Error(`Failed to save configuration: ${error.message}`);
        }
      }
      /**
       * Get backend-specific configuration
       * @param {string} backendName - Backend name
       * @returns {Object}
       */
      getBackendConfig(backendName) {
        return this.config.backends?.[backendName] || {};
      }
      /**
       * Get implementation tool configuration
       * @returns {Object}
       */
      getImplementConfig() {
        return this.config.implement || {};
      }
      /**
       * Validate configuration
       * @returns {Object} Validation result
       */
      validate() {
        const errors = [];
        const warnings = [];
        if (!this.config.implement?.defaultBackend) {
          errors.push("implement.defaultBackend is required");
        }
        const defaultBackend = this.config.implement?.defaultBackend;
        if (defaultBackend && !this.config.backends?.[defaultBackend]) {
          warnings.push(`Configuration for default backend '${defaultBackend}' not found`);
        }
        const fallbackBackends = this.config.implement?.fallbackBackends || [];
        for (const backend of fallbackBackends) {
          if (!this.config.backends?.[backend]) {
            warnings.push(`Configuration for fallback backend '${backend}' not found`);
          }
        }
        const validStrategies = ["auto", "preference", "capability"];
        const strategy = this.config.implement?.selectionStrategy;
        if (strategy && !validStrategies.includes(strategy)) {
          errors.push(`Invalid selection strategy: ${strategy}. Must be one of: ${validStrategies.join(", ")}`);
        }
        return {
          valid: errors.length === 0,
          errors,
          warnings
        };
      }
      /**
       * Clean up resources
       */
      cleanup() {
        if (this.configPath) {
          fs.unwatchFile(this.configPath);
        }
        this.changeCallbacks = [];
        this.watchers.clear();
      }
    };
    configManager = new ConfigManager();
  }
});

// implement/core/ImplementTool.js
function createImplementTool(config = {}) {
  const tool = new ImplementTool(config);
  return {
    ...tool.getToolDefinition(),
    execute: /* @__PURE__ */ __name(async (params) => {
      return await tool.execute(params);
    }, "execute"),
    cancel: /* @__PURE__ */ __name(async (sessionId2) => {
      return await tool.cancel(sessionId2);
    }, "cancel"),
    getInfo: /* @__PURE__ */ __name(async () => {
      return await tool.getBackendInfo();
    }, "getInfo"),
    cleanup: /* @__PURE__ */ __name(async () => {
      return await tool.cleanup();
    }, "cleanup"),
    // Expose the tool instance for advanced usage
    instance: tool
  };
}
var ImplementTool;
var init_ImplementTool = __esm({
  "implement/core/ImplementTool.js"() {
    init_BackendManager();
    init_registry();
    init_utils();
    init_config();
    ImplementTool = class {
      static {
        __name(this, "ImplementTool");
      }
      /**
       * @param {Object} config - Tool configuration
       * @param {boolean} [config.enabled=false] - Whether the tool is enabled
       * @param {Object} [config.backendConfig] - Backend manager configuration
       */
      constructor(config = {}) {
        this.enabled = config.enabled || false;
        this.backendManager = null;
        this.config = config;
        this.initialized = false;
      }
      /**
       * Initialize the implementation tool
       * @returns {Promise<void>}
       */
      async initialize() {
        if (this.initialized) return;
        if (!this.enabled) {
          throw new Error("Implementation tool is not enabled. Use --allow-edit flag to enable.");
        }
        await configManager.initialize(this.config.configPath);
        const implementConfig = configManager.getImplementConfig();
        const backendConfigs = configManager.get("backends") || {};
        const backendManagerConfig = {
          ...implementConfig,
          backends: backendConfigs,
          ...this.config.backendConfig
        };
        this.backendManager = new BackendManager_default(backendManagerConfig);
        await this.registerBackends();
        await this.backendManager.initialize();
        const configValidation = configManager.validate();
        if (!configValidation.valid) {
          console.error("Configuration errors:", configValidation.errors.join(", "));
          if (configValidation.warnings.length > 0) {
            console.warn("Configuration warnings:", configValidation.warnings.join(", "));
          }
        }
        const backendValidation = await this.backendManager.validateConfiguration();
        if (!backendValidation.valid) {
          console.warn("Backend configuration warnings:", backendValidation.errors.join(", "));
        }
        configManager.onChange(async (newConfig) => {
          console.error("Configuration changed, reinitializing backends...");
          await this.reinitialize(newConfig);
        });
        this.initialized = true;
      }
      /**
       * Register all available backends
       * @private
       */
      async registerBackends() {
        const backendNames = listBackendNames();
        for (const name of backendNames) {
          try {
            const backend = createBackend(name);
            if (backend) {
              await this.backendManager.registerBackend(backend);
              console.error(`Registered backend: ${name}`);
            }
          } catch (error) {
            console.warn(`Failed to register backend '${name}':`, error.message);
          }
        }
      }
      /**
       * Reinitialize with new configuration
       * @param {Object} newConfig - New configuration
       * @private
       */
      async reinitialize(newConfig) {
        try {
          if (this.backendManager) {
            await this.backendManager.cleanup();
          }
          const implementConfig = newConfig.implement || {};
          const backendConfigs = newConfig.backends || {};
          const backendManagerConfig = {
            ...implementConfig,
            backends: backendConfigs,
            ...this.config.backendConfig
          };
          this.backendManager = new BackendManager_default(backendManagerConfig);
          await this.registerBackends();
          await this.backendManager.initialize();
          console.error("Backend reinitialization completed");
        } catch (error) {
          console.error("Failed to reinitialize backends:", error);
        }
      }
      /**
       * Get tool definition for AI models
       * @returns {Object}
       */
      getToolDefinition() {
        return {
          name: "implement",
          description: "Implement a feature or fix a bug using AI-powered code generation. Only available when --allow-edit is enabled.",
          parameters: {
            type: "object",
            properties: {
              task: {
                type: "string",
                description: "The task description for implementation"
              },
              backend: {
                type: "string",
                description: "Optional: Specific backend to use (aider, claude-code)",
                enum: listBackendNames()
              },
              autoCommit: {
                type: "boolean",
                description: "Whether to auto-commit changes (default: false)"
              },
              generateTests: {
                type: "boolean",
                description: "Whether to generate tests for the implementation"
              },
              dryRun: {
                type: "boolean",
                description: "Perform a dry run without making actual changes"
              }
            },
            required: ["task"]
          }
        };
      }
      /**
       * Execute implementation task
       * @param {Object} params - Execution parameters
       * @param {string} params.task - Task description
       * @param {string} [params.backend] - Specific backend to use
       * @param {boolean} [params.autoCommit] - Auto-commit changes
       * @param {boolean} [params.generateTests] - Generate tests
       * @param {boolean} [params.dryRun] - Dry run mode
       * @param {string} [params.sessionId] - Session ID
       * @returns {Promise<Object>}
       */
      async execute(params) {
        if (!this.enabled) {
          throw new Error("Implementation tool is not enabled. Use --allow-edit flag to enable.");
        }
        if (!this.initialized) {
          await this.initialize();
        }
        const { task, backend, autoCommit, generateTests, dryRun, sessionId: sessionId2, ...rest } = params;
        const request = {
          sessionId: sessionId2 || `implement-${Date.now()}`,
          task,
          context: {
            workingDirectory: process.cwd(),
            ...rest.context
          },
          options: {
            backend,
            autoCommit: autoCommit || false,
            generateTests: generateTests || false,
            dryRun: dryRun || false,
            ...rest.options
          },
          callbacks: {
            onProgress: /* @__PURE__ */ __name((update) => {
              if (update.message) {
                const prefix = update.type === "stderr" ? "[STDERR]" : "[INFO]";
                console.error(`${prefix} ${update.message}`);
              }
            }, "onProgress"),
            onError: /* @__PURE__ */ __name((error) => {
              console.error("[ERROR]", error.message);
            }, "onError")
          }
        };
        try {
          console.error(`Executing implementation task: ${task.substring(0, 100)}${task.length > 100 ? "..." : ""}`);
          console.error(`Using backend selection strategy: ${this.backendManager.config.selectionStrategy}`);
          if (backend) {
            console.error(`Requested backend: ${backend}`);
          }
          const result = await this.backendManager.executeImplementation(request);
          console.error(`Implementation completed using backend: ${result.backend}`);
          if (result.fallback) {
            console.error("Note: Used fallback backend due to primary backend failure");
          }
          return {
            success: result.success,
            output: result.output,
            error: result.error?.message || null,
            command: `[${result.backend}] ${task}`,
            timestamp: (/* @__PURE__ */ new Date()).toISOString(),
            prompt: task,
            backend: result.backend,
            metrics: result.metrics,
            changes: result.changes
          };
        } catch (error) {
          console.error(`Implementation failed:`, error.message);
          return {
            success: false,
            output: null,
            error: error.message,
            command: `[failed] ${task}`,
            timestamp: (/* @__PURE__ */ new Date()).toISOString(),
            prompt: task,
            backend: null,
            errorDetails: error instanceof BackendError ? error.toJSON() : { message: error.message }
          };
        }
      }
      /**
       * Cancel an implementation session
       * @param {string} sessionId - Session ID to cancel
       * @returns {Promise<void>}
       */
      async cancel(sessionId2) {
        if (!this.backendManager) {
          throw new Error("Implementation tool not initialized");
        }
        await this.backendManager.cancelImplementation(sessionId2);
      }
      /**
       * Get backend information
       * @returns {Promise<Object>}
       */
      async getBackendInfo() {
        if (!this.initialized) {
          await this.initialize();
        }
        const health = await this.backendManager.checkBackendHealth();
        const availableBackends = this.backendManager.getAvailableBackends();
        return {
          enabled: this.enabled,
          defaultBackend: this.backendManager.config.defaultBackend,
          fallbackBackends: this.backendManager.config.fallbackBackends,
          availableBackends,
          health
        };
      }
      /**
       * Clean up resources
       * @returns {Promise<void>}
       */
      async cleanup() {
        if (this.backendManager) {
          await this.backendManager.cleanup();
        }
        configManager.cleanup();
        this.initialized = false;
      }
    };
    __name(createImplementTool, "createImplementTool");
  }
});

// probeTool.js
import { searchTool as searchTool2, queryTool as queryTool2, extractTool as extractTool2, DEFAULT_SYSTEM_MESSAGE as DEFAULT_SYSTEM_MESSAGE2, listFilesByLevel } from "@buger/probe";
import { randomUUID as randomUUID3 } from "crypto";
import { EventEmitter } from "events";
import fs2 from "fs";
import { promises as fsPromises2 } from "fs";
import path4 from "path";
import { glob } from "glob";
function isSessionCancelled(sessionId2) {
  return activeToolExecutions.get(sessionId2)?.cancelled || false;
}
function cancelToolExecutions(sessionId2) {
  if (process.env.PROBE_NON_INTERACTIVE !== "1" || process.env.DEBUG_CHAT === "1") {
    console.log(`Cancelling tool executions for session: ${sessionId2}`);
  }
  const sessionData = activeToolExecutions.get(sessionId2);
  if (sessionData) {
    sessionData.cancelled = true;
    if (process.env.PROBE_NON_INTERACTIVE !== "1" || process.env.DEBUG_CHAT === "1") {
      console.log(`Session ${sessionId2} marked as cancelled`);
    }
    return true;
  }
  return false;
}
function registerToolExecution(sessionId2) {
  if (!sessionId2) return;
  if (!activeToolExecutions.has(sessionId2)) {
    activeToolExecutions.set(sessionId2, { cancelled: false });
  } else {
    activeToolExecutions.get(sessionId2).cancelled = false;
  }
}
function clearToolExecutionData(sessionId2) {
  if (!sessionId2) return;
  if (activeToolExecutions.has(sessionId2)) {
    activeToolExecutions.delete(sessionId2);
    if (process.env.PROBE_NON_INTERACTIVE !== "1" || process.env.DEBUG_CHAT === "1") {
      console.log(`Cleared tool execution data for session: ${sessionId2}`);
    }
  }
}
var toolCallEmitter, activeToolExecutions, defaultSessionId, configOptions2, baseSearchTool, baseQueryTool, baseExtractTool, wrapToolWithEmitter, implementToolConfig, pluggableImplementTool, baseImplementTool, baseListFilesTool, baseSearchFilesTool, searchToolInstance2, queryToolInstance2, extractToolInstance2, implementToolInstance, listFilesToolInstance, searchFilesToolInstance, probeTool;
var init_probeTool = __esm({
  "probeTool.js"() {
    init_ImplementTool();
    toolCallEmitter = new EventEmitter();
    activeToolExecutions = /* @__PURE__ */ new Map();
    __name(isSessionCancelled, "isSessionCancelled");
    __name(cancelToolExecutions, "cancelToolExecutions");
    __name(registerToolExecution, "registerToolExecution");
    __name(clearToolExecutionData, "clearToolExecutionData");
    defaultSessionId = randomUUID3();
    if (process.env.DEBUG_CHAT === "1") {
      console.log(`Generated default session ID (probeTool.js): ${defaultSessionId}`);
    }
    configOptions2 = {
      sessionId: defaultSessionId,
      debug: process.env.DEBUG_CHAT === "1"
    };
    baseSearchTool = searchTool2(configOptions2);
    baseQueryTool = queryTool2(configOptions2);
    baseExtractTool = extractTool2(configOptions2);
    wrapToolWithEmitter = /* @__PURE__ */ __name((tool, toolName, baseExecute) => {
      return {
        ...tool,
        // Spread schema, description etc.
        execute: /* @__PURE__ */ __name(async (params) => {
          const debug2 = process.env.DEBUG_CHAT === "1";
          const toolSessionId = params.sessionId || defaultSessionId;
          if (debug2) {
            console.log(`[DEBUG] probeTool: Executing ${toolName} for session ${toolSessionId}`);
            console.log(`[DEBUG] probeTool: Received params:`, params);
          }
          registerToolExecution(toolSessionId);
          if (isSessionCancelled(toolSessionId)) {
            console.error(`Tool execution cancelled BEFORE starting for session ${toolSessionId}`);
            throw new Error(`Tool execution cancelled for session ${toolSessionId}`);
          }
          console.error(`Executing ${toolName} for session ${toolSessionId}`);
          const { sessionId: sessionId2, ...toolParams } = params;
          try {
            const toolCallStartData = {
              timestamp: (/* @__PURE__ */ new Date()).toISOString(),
              name: toolName,
              args: toolParams,
              // Log schema params
              status: "started"
            };
            if (debug2) {
              console.log(`[DEBUG] probeTool: Emitting toolCallStart:${toolSessionId}`);
            }
            toolCallEmitter.emit(`toolCall:${toolSessionId}`, toolCallStartData);
            let result = null;
            let executionError = null;
            const executionPromise = baseExecute(toolParams).catch((err) => {
              executionError = err;
            });
            const checkInterval = 50;
            while (result === null && executionError === null) {
              if (isSessionCancelled(toolSessionId)) {
                console.error(`Tool execution cancelled DURING execution for session ${toolSessionId}`);
                throw new Error(`Tool execution cancelled for session ${toolSessionId}`);
              }
              const status = await Promise.race([
                executionPromise.then(() => "resolved").catch(() => "rejected"),
                new Promise((resolve3) => setTimeout(() => resolve3("pending"), checkInterval))
              ]);
              if (status === "resolved") {
                result = await executionPromise;
              } else if (status === "rejected") {
                break;
              }
            }
            if (executionError) {
              throw executionError;
            }
            if (isSessionCancelled(toolSessionId)) {
              if (process.env.PROBE_NON_INTERACTIVE !== "1" || process.env.DEBUG_CHAT === "1") {
                console.log(`Tool execution finished but session was cancelled for ${toolSessionId}`);
              }
              throw new Error(`Tool execution cancelled for session ${toolSessionId}`);
            }
            const toolCallData = {
              timestamp: (/* @__PURE__ */ new Date()).toISOString(),
              name: toolName,
              args: toolParams,
              // Safely preview result
              resultPreview: typeof result === "string" ? result.length > 200 ? result.substring(0, 200) + "..." : result : result ? JSON.stringify(result).substring(0, 200) + "..." : "No Result",
              status: "completed"
            };
            if (debug2) {
              console.log(`[DEBUG] probeTool: Emitting toolCall:${toolSessionId} (completed)`);
            }
            toolCallEmitter.emit(`toolCall:${toolSessionId}`, toolCallData);
            return result;
          } catch (error) {
            if (error.message.includes("cancelled for session")) {
              if (process.env.PROBE_NON_INTERACTIVE !== "1" || process.env.DEBUG_CHAT === "1") {
                console.log(`Caught cancellation error for ${toolName} in session ${toolSessionId}`);
              }
              throw error;
            }
            if (debug2) {
              console.error(`[DEBUG] probeTool: Error executing ${toolName}:`, error);
            }
            const toolCallErrorData = {
              timestamp: (/* @__PURE__ */ new Date()).toISOString(),
              name: toolName,
              args: toolParams,
              error: error.message || "Unknown error",
              status: "error"
            };
            if (debug2) {
              console.log(`[DEBUG] probeTool: Emitting toolCall:${toolSessionId} (error)`);
            }
            toolCallEmitter.emit(`toolCall:${toolSessionId}`, toolCallErrorData);
            throw error;
          }
        }, "execute")
      };
    }, "wrapToolWithEmitter");
    implementToolConfig = {
      enabled: process.env.ALLOW_EDIT === "1" || process.argv.includes("--allow-edit"),
      backendConfig: {
        // Configuration can be extended here
      }
    };
    pluggableImplementTool = createImplementTool(implementToolConfig);
    baseImplementTool = {
      name: "implement",
      description: pluggableImplementTool.description,
      parameters: pluggableImplementTool.parameters,
      execute: /* @__PURE__ */ __name(async ({ task, autoCommits = false, prompt, sessionId: sessionId2 }) => {
        const debug2 = process.env.DEBUG_CHAT === "1";
        if (debug2) {
          console.log(`[DEBUG] Executing implementation with task: ${task}`);
          console.log(`[DEBUG] Auto-commits: ${autoCommits}`);
          console.log(`[DEBUG] Session ID: ${sessionId2}`);
          if (prompt) console.log(`[DEBUG] Custom prompt: ${prompt}`);
        }
        if (!implementToolConfig.enabled) {
          return {
            success: false,
            output: null,
            error: "Implementation tool is not enabled. Use --allow-edit flag to enable.",
            command: null,
            timestamp: (/* @__PURE__ */ new Date()).toISOString(),
            prompt: prompt || task
          };
        }
        try {
          const result = await pluggableImplementTool.execute({
            task: prompt || task,
            // Use prompt if provided, otherwise use task
            autoCommit: autoCommits,
            sessionId: sessionId2,
            // Pass through any additional options that might be useful
            context: {
              workingDirectory: process.cwd()
            }
          });
          return result;
        } catch (error) {
          console.error(`Error in implement tool:`, error);
          return {
            success: false,
            output: null,
            error: error.message || "Unknown error in implementation tool",
            command: null,
            timestamp: (/* @__PURE__ */ new Date()).toISOString(),
            prompt: prompt || task
          };
        }
      }, "execute")
    };
    baseListFilesTool = {
      name: "listFiles",
      description: "List files in a specified directory",
      parameters: {
        type: "object",
        properties: {
          directory: {
            type: "string",
            description: "The directory path to list files from. Defaults to current directory if not specified."
          }
        },
        required: []
      },
      execute: /* @__PURE__ */ __name(async ({ directory = ".", sessionId: sessionId2 }) => {
        const debug2 = process.env.DEBUG_CHAT === "1";
        const currentWorkingDir = process.cwd();
        const allowedFoldersEnv = process.env.ALLOWED_FOLDERS;
        let allowedFolders2 = [];
        if (allowedFoldersEnv) {
          allowedFolders2 = allowedFoldersEnv.split(",").map((folder) => folder.trim()).filter((folder) => folder.length > 0);
        }
        let targetDirectory = directory;
        if (allowedFolders2.length > 0 && (directory === "." || directory === "./")) {
          targetDirectory = allowedFolders2[0];
          if (debug2) {
            console.log(`[DEBUG] Redirecting from '${directory}' to first allowed folder: ${targetDirectory}`);
          }
        }
        const targetDir = path4.resolve(currentWorkingDir, targetDirectory);
        if (allowedFolders2.length > 0) {
          const isAllowed = allowedFolders2.some((allowedFolder) => {
            const resolvedAllowedFolder = path4.resolve(currentWorkingDir, allowedFolder);
            return targetDir === resolvedAllowedFolder || targetDir.startsWith(resolvedAllowedFolder + path4.sep);
          });
          if (!isAllowed) {
            const error = `Access denied: Directory '${targetDirectory}' is not within allowed folders: ${allowedFolders2.join(", ")}`;
            if (debug2) {
              console.log(`[DEBUG] ${error}`);
            }
            return {
              success: false,
              directory: targetDir,
              error,
              timestamp: (/* @__PURE__ */ new Date()).toISOString()
            };
          }
        }
        if (debug2) {
          console.log(`[DEBUG] Listing files in directory: ${targetDir}`);
        }
        try {
          const files = await fs2.promises.readdir(targetDir, { withFileTypes: true });
          const result = files.map((file) => {
            const isDirectory = file.isDirectory();
            return {
              name: file.name,
              type: isDirectory ? "directory" : "file",
              path: path4.join(targetDirectory, file.name)
            };
          });
          if (debug2) {
            console.log(`[DEBUG] Found ${result.length} files/directories in ${targetDir}`);
          }
          return {
            success: true,
            directory: targetDir,
            files: result,
            timestamp: (/* @__PURE__ */ new Date()).toISOString()
          };
        } catch (error) {
          console.error(`Error listing files in ${targetDir}:`, error);
          return {
            success: false,
            directory: targetDir,
            error: error.message || "Unknown error listing files",
            timestamp: (/* @__PURE__ */ new Date()).toISOString()
          };
        }
      }, "execute")
    };
    baseSearchFilesTool = {
      name: "searchFiles",
      description: "Search for files using a glob pattern, recursively by default",
      parameters: {
        type: "object",
        properties: {
          pattern: {
            type: "string",
            description: 'The glob pattern to search for (e.g., "**/*.js", "*.md")'
          },
          directory: {
            type: "string",
            description: "The directory to search in. Defaults to current directory if not specified."
          },
          recursive: {
            type: "boolean",
            description: "Whether to search recursively. Defaults to true."
          }
        },
        required: ["pattern"]
      },
      execute: /* @__PURE__ */ __name(async ({ pattern, directory, recursive = true, sessionId: sessionId2 }) => {
        directory = directory || ".";
        const debug2 = process.env.DEBUG_CHAT === "1";
        const currentWorkingDir = process.cwd();
        const allowedFoldersEnv = process.env.ALLOWED_FOLDERS;
        let allowedFolders2 = [];
        if (allowedFoldersEnv) {
          allowedFolders2 = allowedFoldersEnv.split(",").map((folder) => folder.trim()).filter((folder) => folder.length > 0);
        }
        let targetDirectory = directory;
        if (allowedFolders2.length > 0 && (directory === "." || directory === "./")) {
          targetDirectory = allowedFolders2[0];
          if (debug2) {
            console.log(`[DEBUG] Redirecting from '${directory}' to first allowed folder: ${targetDirectory}`);
          }
        }
        const targetDir = path4.resolve(currentWorkingDir, targetDirectory);
        if (allowedFolders2.length > 0) {
          const isAllowed = allowedFolders2.some((allowedFolder) => {
            const resolvedAllowedFolder = path4.resolve(currentWorkingDir, allowedFolder);
            return targetDir === resolvedAllowedFolder || targetDir.startsWith(resolvedAllowedFolder + path4.sep);
          });
          if (!isAllowed) {
            const error = `Access denied: Directory '${targetDirectory}' is not within allowed folders: ${allowedFolders2.join(", ")}`;
            if (debug2) {
              console.log(`[DEBUG] ${error}`);
            }
            return {
              success: false,
              directory: targetDir,
              pattern,
              error,
              timestamp: (/* @__PURE__ */ new Date()).toISOString()
            };
          }
        }
        console.error(`Executing searchFiles with params: pattern="${pattern}", directory="${targetDirectory}", recursive=${recursive}`);
        console.error(`Resolved target directory: ${targetDir}`);
        console.error(`Current working directory: ${currentWorkingDir}`);
        if (debug2) {
          console.log(`[DEBUG] Searching for files with pattern: ${pattern}`);
          console.log(`[DEBUG] In directory: ${targetDir}`);
          console.log(`[DEBUG] Recursive: ${recursive}`);
        }
        if (pattern.includes("**/**") || pattern.split("*").length > 10) {
          console.error(`Pattern too complex: ${pattern}`);
          return {
            success: false,
            directory: targetDir,
            pattern,
            error: "Pattern too complex. Please use a simpler glob pattern.",
            timestamp: (/* @__PURE__ */ new Date()).toISOString()
          };
        }
        try {
          const options = {
            cwd: targetDir,
            dot: true,
            // Include dotfiles
            nodir: true,
            // Only return files, not directories
            absolute: false,
            // Return paths relative to the search directory
            timeout: 1e4,
            // 10 second timeout
            maxDepth: recursive ? 10 : 1
            // Limit recursion depth
          };
          const searchPattern = recursive ? pattern : pattern.replace(/^\*\*\//, "");
          console.error(`Starting glob search with pattern: ${searchPattern} in ${targetDir}`);
          console.error(`Glob options: ${JSON.stringify(options)}`);
          let files = [];
          if (pattern.includes("*") && !pattern.includes("**") && pattern.split("/").length <= 2) {
            console.error(`Using direct file search for simple pattern: ${pattern}`);
            try {
              const parts = pattern.split("/");
              let searchDir = targetDir;
              let filePattern;
              if (parts.length === 2) {
                searchDir = path4.join(targetDir, parts[0]);
                filePattern = parts[1];
              } else {
                filePattern = parts[0];
              }
              console.error(`Searching in directory: ${searchDir} for files matching: ${filePattern}`);
              try {
                await fsPromises2.access(searchDir);
              } catch (err) {
                console.error(`Directory does not exist: ${searchDir}`);
                return {
                  success: true,
                  directory: targetDir,
                  pattern,
                  recursive,
                  files: [],
                  count: 0,
                  timestamp: (/* @__PURE__ */ new Date()).toISOString()
                };
              }
              const dirEntries = await fsPromises2.readdir(searchDir, { withFileTypes: true });
              const regexPattern = filePattern.replace(/\./g, "\\.").replace(/\*/g, ".*");
              const regex = new RegExp(`^${regexPattern}$`);
              files = dirEntries.filter((entry) => entry.isFile() && regex.test(entry.name)).map((entry) => {
                const relativePath = parts.length === 2 ? path4.join(parts[0], entry.name) : entry.name;
                return relativePath;
              });
              console.error(`Direct search found ${files.length} files matching ${filePattern}`);
            } catch (err) {
              console.error(`Error in direct file search: ${err.message}`);
              console.error(`Falling back to glob search`);
              const timeoutPromise = new Promise((_, reject) => {
                setTimeout(() => reject(new Error("Search operation timed out after 10 seconds")), 1e4);
              });
              files = await Promise.race([
                glob(searchPattern, options),
                timeoutPromise
              ]);
            }
          } else {
            console.error(`Using glob for complex pattern: ${pattern}`);
            const timeoutPromise = new Promise((_, reject) => {
              setTimeout(() => reject(new Error("Search operation timed out after 10 seconds")), 1e4);
            });
            files = await Promise.race([
              glob(searchPattern, options),
              timeoutPromise
            ]);
          }
          console.error(`Search completed, found ${files.length} files in ${targetDir}`);
          console.error(`Pattern: ${pattern}, Recursive: ${recursive}`);
          if (debug2) {
            console.log(`[DEBUG] Found ${files.length} files matching pattern ${pattern}`);
          }
          const maxResults = 1e3;
          const limitedFiles = files.length > maxResults ? files.slice(0, maxResults) : files;
          if (files.length > maxResults) {
            console.warn(`Warning: Limited results to ${maxResults} files out of ${files.length} total matches`);
          }
          return {
            success: true,
            directory: targetDir,
            pattern,
            recursive,
            files: limitedFiles.map((file) => path4.join(targetDirectory, file)),
            count: limitedFiles.length,
            totalMatches: files.length,
            limited: files.length > maxResults,
            timestamp: (/* @__PURE__ */ new Date()).toISOString()
          };
        } catch (error) {
          console.error(`Error searching files with pattern "${pattern}" in ${targetDir}:`, error);
          console.error(`Search parameters: directory="${targetDirectory}", recursive=${recursive}, sessionId=${sessionId2}`);
          return {
            success: false,
            directory: targetDir,
            pattern,
            error: error.message || "Unknown error searching files",
            timestamp: (/* @__PURE__ */ new Date()).toISOString()
          };
        }
      }, "execute")
    };
    searchToolInstance2 = wrapToolWithEmitter(baseSearchTool, "search", baseSearchTool.execute);
    queryToolInstance2 = wrapToolWithEmitter(baseQueryTool, "query", baseQueryTool.execute);
    extractToolInstance2 = wrapToolWithEmitter(baseExtractTool, "extract", baseExtractTool.execute);
    implementToolInstance = wrapToolWithEmitter(baseImplementTool, "implement", baseImplementTool.execute);
    listFilesToolInstance = wrapToolWithEmitter(baseListFilesTool, "listFiles", baseListFilesTool.execute);
    searchFilesToolInstance = wrapToolWithEmitter(baseSearchFilesTool, "searchFiles", baseSearchFilesTool.execute);
    probeTool = {
      ...searchToolInstance2,
      // Inherit schema description etc. from the wrapped search tool
      name: "search",
      // Explicitly set name
      description: "DEPRECATED: Use <search> tool instead. Search code using keywords.",
      // parameters: searchSchema, // Use the imported schema
      execute: /* @__PURE__ */ __name(async (params) => {
        const debug2 = process.env.DEBUG_CHAT === "1";
        if (debug2) {
          console.log(`[DEBUG] probeTool (Compatibility Layer) executing for session ${params.sessionId}`);
        }
        const { keywords, folder, sessionId: sessionId2, ...rest } = params;
        const mappedParams = {
          query: keywords,
          path: folder || ".",
          // Default path if folder is missing
          sessionId: sessionId2,
          // Pass session ID through
          ...rest
          // Pass other params like allow_tests, maxResults etc.
        };
        if (debug2) {
          console.log("[DEBUG] probeTool mapped params: ", mappedParams);
        }
        try {
          const result = await searchToolInstance2.execute(mappedParams);
          const formattedResult = {
            results: result,
            // Assuming result is the direct data
            command: `probe search --query "${keywords}" --path "${folder || "."}"`,
            // Reconstruct approx command
            timestamp: (/* @__PURE__ */ new Date()).toISOString()
          };
          if (debug2) {
            console.log("[DEBUG] probeTool compatibility layer returning formatted result.");
          }
          return formattedResult;
        } catch (error) {
          if (debug2) {
            console.error(`[DEBUG] Error in probeTool compatibility layer:`, error);
          }
          throw error;
        }
      }, "execute")
    };
  }
});

// probeChat.js
import "dotenv/config";
import { createAnthropic } from "@ai-sdk/anthropic";
import { createOpenAI } from "@ai-sdk/openai";
import { createGoogleGenerativeAI } from "@ai-sdk/google";
import { streamText } from "ai";
import { randomUUID as randomUUID4 } from "crypto";
import { writeFileSync, existsSync as existsSync2 } from "fs";
import { join } from "path";
import { trace as trace3 } from "@opentelemetry/api";
import { listFilesByLevel as listFilesByLevel2 } from "@buger/probe";
function extractImageUrls(message, debug2 = false) {
  const tracer = trace3.getTracer("probe-chat", "1.0.0");
  return tracer.startActiveSpan("content.image.extract", (span) => {
    try {
      const imageUrlPattern = /(?:data:image\/[a-zA-Z]*;base64,[A-Za-z0-9+/=]+|https?:\/\/(?:(?:private-user-images\.githubusercontent\.com|github\.com\/user-attachments\/assets)\/[^\s"'<>]+|[^\s"'<>]+\.(?:png|jpg|jpeg|webp|gif)(?:\?[^\s"'<>]*)?))/gi;
      span.setAttributes({
        "message.length": message.length,
        "debug.enabled": debug2
      });
      if (debug2) {
        console.log(`[DEBUG] Scanning message for image URLs. Message length: ${message.length}`);
        console.log(`[DEBUG] Image URL pattern: ${imageUrlPattern.toString()}`);
      }
      const urls = [];
      let match;
      while ((match = imageUrlPattern.exec(message)) !== null) {
        urls.push(match[0]);
        if (debug2) {
          console.log(`[DEBUG] Found image URL: ${match[0]}`);
        }
      }
      const cleanedMessage = message.replace(imageUrlPattern, "").trim();
      span.setAttributes({
        "images.found": urls.length,
        "message.cleaned_length": cleanedMessage.length
      });
      if (debug2) {
        console.log(`[DEBUG] Total image URLs found: ${urls.length}`);
        if (urls.length > 0) {
          console.log(`[DEBUG] Original message length: ${message.length}, cleaned message length: ${cleanedMessage.length}`);
        }
      }
      const result = {
        imageUrls: urls,
        cleanedMessage
      };
      span.setStatus({ code: 1 });
      return result;
    } catch (error) {
      span.recordException(error);
      span.setStatus({ code: 2, message: error.message });
      throw error;
    } finally {
      span.end();
    }
  });
}
async function validateImageUrls(imageUrls, debug2 = false) {
  const validUrls = [];
  for (const url of imageUrls) {
    try {
      if (url.startsWith("data:image/")) {
        const dataUrlMatch = url.match(/^data:image\/([a-zA-Z]*);base64,([A-Za-z0-9+/=]+)$/);
        if (dataUrlMatch) {
          const [, imageType, base64Data] = dataUrlMatch;
          if (base64Data.length > 0 && imageType) {
            const estimatedSize = base64Data.length * 3 / 4;
            if (estimatedSize <= 10 * 1024 * 1024) {
              validUrls.push(url);
              if (debug2) {
                console.log(`[DEBUG] Valid base64 image: ${imageType} (~${(estimatedSize / 1024).toFixed(1)}KB)`);
              }
            } else {
              if (debug2) {
                console.log(`[DEBUG] Base64 image too large: ~${(estimatedSize / 1024 / 1024).toFixed(1)}MB (max 10MB)`);
              }
            }
          } else {
            if (debug2) {
              console.log(`[DEBUG] Invalid base64 data URL format: ${url.substring(0, 50)}...`);
            }
          }
        } else {
          if (debug2) {
            console.log(`[DEBUG] Invalid data URL format: ${url.substring(0, 50)}...`);
          }
        }
      } else {
        const response = await fetch(url, {
          method: "GET",
          headers: {
            "Range": "bytes=0-1023"
            // Only fetch first 1KB to check content type and minimize data transfer
          },
          timeout: 1e4,
          // TIMEOUTS.HTTP_REQUEST - 10 second timeout for GitHub URLs which can be slower
          redirect: "follow"
        });
        if (response.ok || response.status === 206) {
          const contentType = response.headers.get("content-type");
          if (contentType && contentType.startsWith("image/")) {
            const finalUrl = response.url;
            validUrls.push(finalUrl);
            if (debug2) {
              if (finalUrl !== url) {
                console.log(`[DEBUG] Valid image URL after redirect: ${url} -> ${finalUrl} (${contentType})`);
              } else {
                console.log(`[DEBUG] Valid image URL: ${finalUrl} (${contentType})`);
              }
            }
          } else {
            if (debug2) {
              console.log(`[DEBUG] URL not an image: ${url} (${contentType || "unknown type"})`);
            }
          }
        } else {
          if (debug2) {
            console.log(`[DEBUG] URL not accessible: ${url} (status: ${response.status})`);
          }
        }
      }
    } catch (error) {
      if (debug2) {
        console.log(`[DEBUG] Error validating image URL ${url}: ${error.message}`);
      }
    }
  }
  return validUrls;
}
var MAX_HISTORY_MESSAGES, MAX_TOOL_ITERATIONS, allowedFolders, validateFolders, ProbeChat;
var init_probeChat = __esm({
  "probeChat.js"() {
    init_tokenCounter();
    init_tokenUsageDisplay();
    init_telemetry();
    init_appTracer();
    init_tools();
    init_probeTool();
    MAX_HISTORY_MESSAGES = 100;
    MAX_TOOL_ITERATIONS = parseInt(process.env.MAX_TOOL_ITERATIONS || "30", 10);
    allowedFolders = process.env.ALLOWED_FOLDERS ? process.env.ALLOWED_FOLDERS.split(",").map((folder) => folder.trim()).filter(Boolean) : [];
    validateFolders = /* @__PURE__ */ __name(() => {
      if (allowedFolders.length > 0) {
        for (const folder of allowedFolders) {
          const exists2 = existsSync2(folder);
          if (process.env.PROBE_NON_INTERACTIVE !== "1" || process.env.DEBUG_CHAT === "1") {
            console.log(`- ${folder} ${exists2 ? "\u2713" : "\u2717 (not found)"}`);
            if (!exists2) {
              console.warn(`Warning: Folder "${folder}" does not exist or is not accessible`);
            }
          }
        }
      } else {
        if (process.env.PROBE_NON_INTERACTIVE !== "1" || process.env.DEBUG_CHAT === "1") {
          console.warn("No folders configured via ALLOWED_FOLDERS. Tools might default to current directory or require explicit paths.");
        }
      }
    }, "validateFolders");
    if (typeof process !== "undefined" && !process.env.PROBE_CHAT_SKIP_FOLDER_VALIDATION) {
      validateFolders();
    }
    __name(extractImageUrls, "extractImageUrls");
    __name(validateImageUrls, "validateImageUrls");
    ProbeChat = class {
      static {
        __name(this, "ProbeChat");
      }
      /**
       * Create a new ProbeChat instance
       * @param {Object} options - Configuration options
       * @param {string} [options.sessionId] - Optional session ID
       * @param {boolean} [options.isNonInteractive=false] - Suppress internal logs if true
       * @param {Function} [options.toolCallCallback] - Callback function for tool calls (sessionId, toolCallData) - *Note: Callback may need adjustment for XML flow*
       * @param {string} [options.customPrompt] - Custom prompt to replace the default system message
       * @param {string} [options.promptType] - Predefined prompt type (architect, code-review, support)
       * @param {boolean} [options.allowEdit=false] - Allow the use of the 'implement' tool
       */
      constructor(options = {}) {
        this.isNonInteractive = !!options.isNonInteractive;
        this.cancelled = false;
        this.abortController = null;
        this.allowedFolders = allowedFolders;
        this.customPrompt = options.customPrompt || process.env.CUSTOM_PROMPT || null;
        this.promptType = options.promptType || process.env.PROMPT_TYPE || null;
        this.allowEdit = !!options.allowEdit || process.env.ALLOW_EDIT === "1" || process.env.ALLOW_SUGGESTIONS === "1";
        this.clientApiProvider = options.apiProvider;
        this.clientApiKey = options.apiKey;
        this.clientApiUrl = options.apiUrl;
        this.tokenCounter = new TokenCounter();
        this.tokenDisplay = new TokenUsageDisplay({
          maxTokens: 8192
          // Will be updated based on model
        });
        this.sessionId = options.sessionId || randomUUID4();
        this.debug = process.env.DEBUG_CHAT === "1";
        if (this.debug) {
          console.log(`[DEBUG] Generated session ID for chat: ${this.sessionId}`);
          console.log(`[DEBUG] Maximum tool iterations configured: ${MAX_TOOL_ITERATIONS}`);
          console.log(`[DEBUG] Allow Edit (implement tool): ${this.allowEdit}`);
        }
        this.toolImplementations = {
          search: searchToolInstance2,
          query: queryToolInstance2,
          extract: extractToolInstance2,
          listFiles: listFilesToolInstance,
          searchFiles: searchFilesToolInstance
          // attempt_completion is handled specially in the loop, no direct implementation needed here
        };
        if (this.allowEdit) {
          this.toolImplementations.implement = implementToolInstance;
        }
        this.initializeModel();
        this.initializeTelemetry();
        this.history = [];
        this.displayHistory = [];
        this.storage = options.storage || null;
      }
      /**
       * Initialize the AI model based on available API keys and forced provider setting
       */
      initializeModel() {
        const anthropicApiKey = this.clientApiKey && this.clientApiProvider === "anthropic" ? this.clientApiKey : process.env.ANTHROPIC_API_KEY;
        const openaiApiKey = this.clientApiKey && this.clientApiProvider === "openai" ? this.clientApiKey : process.env.OPENAI_API_KEY;
        const googleApiKey = this.clientApiKey && this.clientApiProvider === "google" ? this.clientApiKey : process.env.GOOGLE_API_KEY;
        const llmBaseUrl = process.env.LLM_BASE_URL;
        const anthropicApiUrl = this.clientApiUrl && this.clientApiProvider === "anthropic" ? this.clientApiUrl : process.env.ANTHROPIC_API_URL || llmBaseUrl;
        const openaiApiUrl = this.clientApiUrl && this.clientApiProvider === "openai" ? this.clientApiUrl : process.env.OPENAI_API_URL || llmBaseUrl;
        const googleApiUrl = this.clientApiUrl && this.clientApiProvider === "google" ? this.clientApiUrl : process.env.GOOGLE_API_URL || llmBaseUrl;
        const modelName = process.env.MODEL_NAME;
        const clientForceProvider = this.clientApiProvider && this.clientApiKey ? this.clientApiProvider : null;
        const forceProvider = clientForceProvider || (process.env.FORCE_PROVIDER ? process.env.FORCE_PROVIDER.toLowerCase() : null);
        if (this.debug) {
          console.log(`[DEBUG] Available API keys: Anthropic=${!!anthropicApiKey}, OpenAI=${!!openaiApiKey}, Google=${!!googleApiKey}`);
          console.log(`[DEBUG] Force provider: ${forceProvider || "(not set)"}`);
          if (llmBaseUrl) console.log(`[DEBUG] Generic LLM Base URL: ${llmBaseUrl}`);
          if (process.env.ANTHROPIC_API_URL) console.log(`[DEBUG] Custom Anthropic URL: ${anthropicApiUrl}`);
          if (process.env.OPENAI_API_URL) console.log(`[DEBUG] Custom OpenAI URL: ${openaiApiUrl}`);
          if (process.env.GOOGLE_API_URL) console.log(`[DEBUG] Custom Google URL: ${googleApiUrl}`);
          if (modelName) console.log(`[DEBUG] Model override: ${modelName}`);
        }
        if (forceProvider) {
          if (!this.isNonInteractive || this.debug) {
            console.log(`Provider forced to: ${forceProvider}`);
          }
          if (forceProvider === "anthropic" && anthropicApiKey) {
            this.initializeAnthropicModel(anthropicApiKey, anthropicApiUrl, modelName);
            return;
          } else if (forceProvider === "openai" && openaiApiKey) {
            this.initializeOpenAIModel(openaiApiKey, openaiApiUrl, modelName);
            return;
          } else if (forceProvider === "google" && googleApiKey) {
            this.initializeGoogleModel(googleApiKey, googleApiUrl, modelName);
            return;
          }
          console.warn(`WARNING: Forced provider "${forceProvider}" selected but required API key is missing or invalid! Falling back to auto-detection.`);
        }
        if (anthropicApiKey) {
          this.initializeAnthropicModel(anthropicApiKey, anthropicApiUrl, modelName);
        } else if (openaiApiKey) {
          this.initializeOpenAIModel(openaiApiKey, openaiApiUrl, modelName);
        } else if (googleApiKey) {
          this.initializeGoogleModel(googleApiKey, googleApiUrl, modelName);
        } else {
          console.error("FATAL: No API key provided. Please set ANTHROPIC_API_KEY, OPENAI_API_KEY, or GOOGLE_API_KEY environment variable.");
          this.noApiKeysMode = true;
          this.model = "none";
          this.apiType = "none";
          console.log("ProbeChat cannot function without an API key.");
        }
      }
      /**
       * Initialize Anthropic model
       * @param {string} apiKey - Anthropic API key
       * @param {string} [apiUrl] - Optional Anthropic API URL override
       * @param {string} [modelName] - Optional model name override
       */
      initializeAnthropicModel(apiKey, apiUrl, modelName) {
        this.provider = createAnthropic({
          apiKey,
          ...apiUrl && { baseURL: apiUrl }
          // Conditionally add baseURL
        });
        this.model = modelName || "claude-3-7-sonnet-20250219";
        this.apiType = "anthropic";
        if (!this.isNonInteractive || this.debug) {
          const urlSource = process.env.ANTHROPIC_API_URL ? "ANTHROPIC_API_URL" : process.env.LLM_BASE_URL ? "LLM_BASE_URL" : "default";
          console.log(`Using Anthropic API with model: ${this.model}${apiUrl ? ` (URL: ${apiUrl}, from: ${urlSource})` : ""}`);
        }
      }
      /**
       * Initialize OpenAI model
       * @param {string} apiKey - OpenAI API key
       * @param {string} [apiUrl] - Optional OpenAI API URL override
       * @param {string} [modelName] - Optional model name override
       */
      initializeOpenAIModel(apiKey, apiUrl, modelName) {
        this.provider = createOpenAI({
          compatibility: "strict",
          apiKey,
          ...apiUrl && { baseURL: apiUrl }
          // Conditionally add baseURL
        });
        this.model = modelName || "gpt-4o";
        this.apiType = "openai";
        if (!this.isNonInteractive || this.debug) {
          const urlSource = process.env.OPENAI_API_URL ? "OPENAI_API_URL" : process.env.LLM_BASE_URL ? "LLM_BASE_URL" : "default";
          console.log(`Using OpenAI API with model: ${this.model}${apiUrl ? ` (URL: ${apiUrl}, from: ${urlSource})` : ""}`);
        }
      }
      /**
       * Initialize Google model
       * @param {string} apiKey - Google API key
       * @param {string} [apiUrl] - Optional Google API URL override
       * @param {string} [modelName] - Optional model name override
       */
      initializeGoogleModel(apiKey, apiUrl, modelName) {
        this.provider = createGoogleGenerativeAI({
          apiKey,
          ...apiUrl && { baseURL: apiUrl }
          // Conditionally add baseURL
        });
        this.model = modelName || "gemini-2.0-flash";
        this.apiType = "google";
        if (!this.isNonInteractive || this.debug) {
          const urlSource = process.env.GOOGLE_API_URL ? "GOOGLE_API_URL" : process.env.LLM_BASE_URL ? "LLM_BASE_URL" : "default";
          console.log(`Using Google API with model: ${this.model}${apiUrl ? ` (URL: ${apiUrl}, from: ${urlSource})` : ""}`);
        }
      }
      /**
       * Initialize telemetry configuration
       */
      initializeTelemetry() {
        try {
          const fileEnabled = process.env.OTEL_ENABLE_FILE === "true";
          const remoteEnabled = process.env.OTEL_ENABLE_REMOTE === "true";
          const consoleEnabled = process.env.OTEL_ENABLE_CONSOLE === "true";
          if (fileEnabled || remoteEnabled || consoleEnabled) {
            this.telemetryConfig = new TelemetryConfig({
              enableFile: fileEnabled,
              enableRemote: remoteEnabled,
              enableConsole: consoleEnabled,
              filePath: process.env.OTEL_FILE_PATH || "./traces.jsonl",
              remoteEndpoint: process.env.OTEL_EXPORTER_OTLP_TRACES_ENDPOINT || "http://localhost:4318/v1/traces"
            });
            this.telemetryConfig.initialize();
            if (this.debug) {
              console.log("[DEBUG] Telemetry initialized successfully");
            }
          } else {
            if (this.debug) {
              console.log("[DEBUG] Telemetry disabled - no exporters configured");
            }
          }
        } catch (error) {
          console.error("Failed to initialize telemetry:", error.message);
          this.telemetryConfig = null;
        }
      }
      /**
        * Get the system message with instructions for the AI (XML Tool Format)
        * @returns {Promise<string>} - The system message
        */
      async getSystemMessage() {
        let toolDefinitions = `
${searchToolDefinition}
${queryToolDefinition}
${extractToolDefinition}
${listFilesToolDefinition}
${searchFilesToolDefinition}
${attemptCompletionToolDefinition}
`;
        if (this.allowEdit) {
          toolDefinitions += `${implementToolDefinition}
`;
        }
        let xmlToolGuidelines = `
# Tool Use Formatting

Tool use MUST be formatted using XML-style tags. The tool name is enclosed in opening and closing tags, and each parameter is similarly enclosed within its own set of tags. You MUST use exactly ONE tool call per message until you are ready to complete the task.

Structure:
<tool_name>
<parameter1_name>value1</parameter1_name>
<parameter2_name>value2</parameter2_name>
...
</tool_name>

Example:
<search>
<query>error handling</query>
<path>src/search</path>
</search>

# Thinking Process

Before using a tool, analyze the situation within <thinking></thinking> tags. This helps you organize your thoughts and make better decisions. Your thinking process should include:

1. Analyze what information you already have and what information you need to proceed with the task.
2. Determine which of the available tools would be most effective for gathering this information or accomplishing the current step.
3. Check if all required parameters for the tool are available or can be inferred from the context.
4. If all parameters are available, proceed with the tool use.
5. If parameters are missing, explain what's missing and why it's needed.

Example:
<thinking>
I need to find code related to error handling in the search module. The most appropriate tool for this is the search tool, which requires a query parameter and a path parameter. I have both the query ("error handling") and the path ("src/search"), so I can proceed with the search.
</thinking>

# Tool Use Guidelines

1.  Think step-by-step about how to achieve the user's goal.
2.  Use <thinking></thinking> tags to analyze the situation and determine the appropriate tool.
3.  Choose **one** tool that helps achieve the current step.
4.  Format the tool call using the specified XML format. Ensure all required parameters are included.
5.  **You MUST respond with exactly one tool call in the specified XML format in each turn.**
6.  Wait for the tool execution result, which will be provided in the next message (within a <tool_result> block).
7.  Analyze the tool result and decide the next step. If more tool calls are needed, repeat steps 2-6.
8.  If the task is fully complete and all previous steps were successful, use the \`<attempt_completion>\` tool to provide the final answer. This is the ONLY way to finish the task.
9.  If you cannot proceed (e.g., missing information, invalid request), explain the issue clearly before using \`<attempt_completion>\` with an appropriate message in the \`<result>\` tag.
10. Do not be lazy and dig to the topic as deep as possible, until you see full picture.

Available Tools:
- search: Search code using keyword queries.
- query: Search code using structural AST patterns.
- extract: Extract specific code blocks or lines from files.
- listFiles: List files and directories in a specified location.
- searchFiles: Find files matching a glob pattern with recursive search capability.
${this.allowEdit ? "- implement: Implement a feature or fix a bug using aider.\n" : ""}
- attempt_completion: Finalize the task and provide the result to the user.
`;
        const commonInstructions = `<instructions>
Follow these instructions carefully:
1.  Analyze the user's request.
2.  Use <thinking></thinking> tags to analyze the situation and determine the appropriate tool for each step.
3.  Use the available tools step-by-step to fulfill the request.
4.  You should always prefer the \`search\` tool for code-related questions. Read full files only if really necessary.
4.  Ensure to get really deep and understand the full picture before answering. Ensure to check dependencies where required.
5.  You MUST respond with exactly ONE tool call per message, using the specified XML format, until the task is complete.
6.  Wait for the tool execution result (provided in the next user message in a <tool_result> block) before proceeding to the next step.
7.  Once the task is fully completed, and you have confirmed the success of all steps, use the '<attempt_completion>' tool to provide the final result. This is the ONLY way to signal completion.
8.  Prefer concise and focused search queries. Use specific keywords and phrases to narrow down results. Avoid reading files in full, only when absolutely necessary.
9.  Show mermaid diagrams to illustrate complex code structures or workflows. In diagrams, content inside ["..."] always should be in quotes.</instructions>
`;
        const predefinedPrompts = {
          "code-explorer": `You are ProbeChat Code Explorer, a specialized AI assistant focused on helping developers, product managers, and QAs understand and navigate codebases. Your primary function is to answer questions based on code, explain how systems work, and provide insights into code functionality using the provided code analysis tools.

When exploring code:
- Provide clear, concise explanations based on user request
- Find and highlight the most relevant code snippets, if required
- Trace function calls and data flow through the system
- Use diagrams to illustrate code structure and relationships when helpful
- Try to understand the user's intent and provide relevant information
- Understand high level picture
- Balance detail with clarity in your explanations`,
          "architect": `You are ProbeChat Architect, a specialized AI assistant focused on software architecture and design. Your primary function is to help users understand, analyze, and design software systems using the provided code analysis tools. You excel at identifying architectural patterns, suggesting improvements, and creating high-level design documentation. You provide detailed and accurate responses to user queries about system architecture, component relationships, and code organization.

When analyzing code:
- Focus on high-level design patterns and system organization
- Identify architectural patterns and component relationships
- Evaluate system structure and suggest architectural improvements
- Create diagrams to illustrate system architecture and workflows
- Consider scalability, maintainability, and extensibility in your analysis`,
          "code-review": `You are ProbeChat Code Reviewer, a specialized AI assistant focused on code quality and best practices. Your primary function is to help users identify issues, suggest improvements, and ensure code follows best practices using the provided code analysis tools. You excel at spotting bugs, performance issues, security vulnerabilities, and style inconsistencies. You provide detailed and constructive feedback on code quality.

When reviewing code:
- Look for bugs, edge cases, and potential issues
- Identify performance bottlenecks and optimization opportunities
- Check for security vulnerabilities and best practices
- Evaluate code style and consistency
- Is the backward compatibility can be broken?
- Organize feedback by severity (critical, major, minor) and type (bug, performance, security, style)
- Provide specific, actionable suggestions with code examples where appropriate

## Failure Detection

If you detect critical issues that should prevent the code from being merged, include <fail> in your response:
- Security vulnerabilities that could be exploited
- Breaking changes without proper documentation or migration path
- Critical bugs that would cause system failures
- Severe violations of project standards that must be addressed

The <fail> tag will cause the GitHub check to fail, drawing immediate attention to these critical issues.`,
          "engineer": `You are senior engineer focused on software architecture and design.
Before jumping on the task you first, in details analyse user request, and try to provide elegant and concise solution.
If solution is clear, you can jump to implementation right away, if not, you can ask user a clarification question, by calling attempt_completion tool, with required details.
You are allowed to use search tool with allow_tests argument, in order to find the tests.

Before jumping to implementation:
- Focus on high-level design patterns and system organization
- Identify architectural patterns and component relationships
- Evaluate system structure and suggest architectural improvements
- Focus on backward compatibility.
- Respond with diagrams to illustrate system architecture and workflows, if required.
- Consider scalability, maintainability, and extensibility in your analysis

During the implementation:
- Avoid implementing special cases
- Do not forget to add the tests`,
          "support": `You are ProbeChat Support, a specialized AI assistant focused on helping developers troubleshoot issues and solve problems. Your primary function is to help users diagnose errors, understand unexpected behaviors, and find solutions using the provided code analysis tools. You excel at debugging, explaining complex concepts, and providing step-by-step guidance. You provide detailed and patient support to help users overcome technical challenges.

When troubleshooting:
- Focus on finding root causes, not just symptoms
- Explain concepts clearly with appropriate context
- Provide step-by-step guidance to solve problems
- Suggest diagnostic steps to verify solutions
- Consider edge cases and potential complications
- Be empathetic and patient in your explanations`
        };
        let systemMessage = "";
        if (this.customPrompt) {
          systemMessage = "<role>" + this.customPrompt + "</role>";
          if (this.debug) {
            console.log(`[DEBUG] Using custom prompt`);
          }
        } else if (this.promptType && predefinedPrompts[this.promptType]) {
          systemMessage = "<role>" + predefinedPrompts[this.promptType] + "</role>";
          if (this.debug) {
            console.log(`[DEBUG] Using predefined prompt: ${this.promptType}`);
          }
          systemMessage += commonInstructions;
        } else {
          systemMessage = "<role>" + predefinedPrompts["code-explorer"] + "</role>";
          if (this.debug) {
            console.log(`[DEBUG] Using default prompt: code explorer`);
          }
          systemMessage += commonInstructions;
        }
        systemMessage += `
${xmlToolGuidelines}
`;
        systemMessage += `
# Tools Available
${toolDefinitions}
`;
        systemMessage += `
# CRITICAL: XML Tool Format Required

Even when processing images or visual content, you MUST respond using the XML tool format. Do not provide direct answers about images - instead use the appropriate tool (usually <attempt_completion>) with your analysis inside the <result> tag.

Example when analyzing an image:
<attempt_completion>
<result>
I can see this is a promotional image from Tyk showing... [your analysis here]
</result>
</attempt_completion>
`;
        const searchDirectory = this.allowedFolders.length > 0 ? this.allowedFolders[0] : process.cwd();
        if (this.debug) {
          console.log(`[DEBUG] Generating file list for base directory: ${searchDirectory}...`);
        }
        if (this.allowedFolders.length > 0) {
          const folderList = this.allowedFolders.map((f) => `"${f}"`).join(", ");
          systemMessage += `

You are configured to primarily operate within these folders: ${folderList}. When using tools like 'search' or 'query', the 'path' parameter should generally refer to these folders or subpaths within them. The root for relative paths is considered the project base.`;
        } else {
          systemMessage += `

Current path: ${searchDirectory}. When using tools, specify paths like '.' for the current directory, 'src/utils', etc., within the 'path' parameter. Dependencies are located in /dep folder: "/dep/go/github.com/user/repo", "/dep/js/<package>", "/dep/rust/crate_name".`;
        }
        systemMessage += `

# Capabilities & Rules
- Search given folder using keywords (\`search\`) or structural patterns (\`query\`).
- Extract specific code blocks or full files using (\`extract\`).
- File paths are relative to the project base unless using dependency syntax.
- Always wait for tool results (\`<tool_result>...\`) before proceeding.
- Use \`attempt_completion\` ONLY when the entire task is finished.
- Be direct and technical. Use exactly ONE tool call per response in the specified XML format. Prefer using search tool.
`;
        if (this.debug) {
          console.log(`[DEBUG] Base system message length (pre-file list): ${systemMessage.length}`);
        }
        try {
          let files = await listFilesByLevel2({
            directory: searchDirectory,
            // Use the determined search directory
            maxFiles: 100,
            // Keep it reasonable
            respectGitignore: true
          });
          files = files.filter((file) => {
            const lower = file.toLowerCase();
            return !lower.includes("probe-debug.txt") && !lower.includes("node_modules") && !lower.includes("/.git/");
          });
          if (files.length > 0) {
            const fileListHeader = `

# Project Files (Sample of up to ${files.length} files in ${searchDirectory}):
`;
            const fileListContent = files.map((file) => `- ${file}`).join("\n");
            systemMessage += fileListHeader + fileListContent;
            if (this.debug) {
              console.log(`[DEBUG] Added ${files.length} files to system message. Total length: ${systemMessage.length}`);
            }
          } else {
            if (this.debug) {
              console.log(`[DEBUG] No files found or listed for the project directory: ${searchDirectory}.`);
            }
            systemMessage += `

# Project Files
No files listed for the primary directory (${searchDirectory}). You may need to use tools like 'search' or 'query' with broad paths initially if the user's request requires file exploration.`;
          }
        } catch (error) {
          console.warn(`Warning: Could not generate file list for directory "${searchDirectory}": ${error.message}`);
          systemMessage += `

# Project Files
Could not retrieve file listing. Proceed based on user instructions and tool capabilities.`;
        }
        if (this.debug) {
          console.log(`[DEBUG] Final system message length: ${systemMessage.length}`);
          const debugFilePath = join(process.cwd(), "probe-debug-system-prompt.txt");
          try {
            writeFileSync(debugFilePath, systemMessage);
            console.log(`[DEBUG] Full system prompt saved to ${debugFilePath}`);
          } catch (e) {
            console.error(`[DEBUG] Failed to write full system prompt: ${e.message}`);
            console.log(`[DEBUG] System message START:
${systemMessage.substring(0, 300)}...`);
            console.log(`[DEBUG] System message END:
...${systemMessage.substring(systemMessage.length - 300)}`);
          }
        }
        return systemMessage;
      }
      /**
       * Abort the current chat request
       */
      abort() {
        if (!this.isNonInteractive || this.debug) {
          console.log(`Aborting chat for session: ${this.sessionId}`);
        }
        this.cancelled = true;
        if (this.abortController) {
          try {
            this.abortController.abort("User cancelled request");
          } catch (error) {
            if (error.name !== "AbortError") {
              console.error("Error aborting fetch request:", error);
            }
          }
        }
      }
      /**
       * Process a user message and get a response
       * @param {string} message - The user message
       * @param {string} [sessionId] - Optional session ID to use for this chat (overrides the default)
       * @param {Object} [apiCredentials] - Optional API credentials for this call
       * @param {string[]} [images] - Optional array of base64 image URLs
       * @returns {Promise<string>} - The AI response
       */
      async chat(message, sessionId2, apiCredentials = null, images = []) {
        const effectiveSessionId = sessionId2 || this.sessionId;
        const chatSessionSpan = appTracer.startChatSession(effectiveSessionId, message, this.apiType, this.model);
        return await appTracer.withSessionContext(effectiveSessionId, async () => {
          try {
            if (apiCredentials) {
              this.clientApiProvider = apiCredentials.apiProvider || this.clientApiProvider;
              this.clientApiKey = apiCredentials.apiKey || this.clientApiKey;
              this.clientApiUrl = apiCredentials.apiUrl || this.clientApiUrl;
              if (apiCredentials.apiKey && apiCredentials.apiProvider) {
                this.initializeModel();
              }
            }
            if (this.noApiKeysMode) {
              console.error("Cannot process chat: No API keys configured.");
              appTracer.endChatSession(effectiveSessionId, false, 0);
              return {
                response: "Error: ProbeChat is not configured with an AI provider API key. Please set the appropriate environment variable (e.g., ANTHROPIC_API_KEY, OPENAI_API_KEY) or provide an API key in the browser.",
                tokenUsage: { contextWindow: 0, current: {}, total: {} }
              };
            }
            this.cancelled = false;
            this.abortController = new AbortController();
            if (sessionId2 && sessionId2 !== this.sessionId) {
              if (this.debug) {
                console.log(`[DEBUG] Switching session ID from ${this.sessionId} to ${sessionId2}`);
              }
              this.sessionId = sessionId2;
            }
            const result = await this._processChat(message, effectiveSessionId, images);
            appTracer.endChatSession(effectiveSessionId, true, result.tokenUsage?.total?.total || 0);
            if (this.telemetryConfig) {
              try {
                await appTracer.withSessionContext(effectiveSessionId, async () => {
                  await new Promise((resolve3) => setTimeout(resolve3, 50));
                });
                await new Promise((resolve3) => setTimeout(resolve3, 600));
                await this.telemetryConfig.forceFlush();
                await new Promise((resolve3) => setTimeout(resolve3, 100));
              } catch (flushError) {
                if (this.debug) console.log("[DEBUG] Telemetry flush warning:", flushError.message);
              }
            }
            return result;
          } catch (error) {
            appTracer.endChatSession(effectiveSessionId, false, 0);
            if (this.telemetryConfig) {
              try {
                await appTracer.withSessionContext(effectiveSessionId, async () => {
                  await new Promise((resolve3) => setTimeout(resolve3, 50));
                });
                await new Promise((resolve3) => setTimeout(resolve3, 600));
                await this.telemetryConfig.forceFlush();
                await new Promise((resolve3) => setTimeout(resolve3, 100));
              } catch (flushError) {
                if (this.debug) console.log("[DEBUG] Telemetry flush warning:", flushError.message);
              }
            }
            throw error;
          }
        });
      }
      /**
       * Internal method to process a chat message using the XML tool loop
       * @param {string} message - The user message
       * @param {string} sessionId - The session ID for tracing
       * @param {string[]} images - Array of base64 image URLs
       * @returns {Promise<string>} - The final AI response after loop completion
       * @private
       */
      async _processChat(message, sessionId2, images = []) {
        let currentIteration = 0;
        let completionAttempted = false;
        let finalResult = `Error: Max tool iterations (${MAX_TOOL_ITERATIONS}) reached without completion. You can increase this limit using the MAX_TOOL_ITERATIONS environment variable or --max-iterations flag.`;
        this.abortController = new AbortController();
        const debugFilePath = join(process.cwd(), "probe-debug.txt");
        try {
          if (this.debug) {
            console.log(`[DEBUG] ===== Starting XML Tool Chat Loop (Session: ${this.sessionId}) =====`);
            console.log(`[DEBUG] Received user message: ${message}`);
            console.log(`[DEBUG] Initial history length: ${this.history.length}`);
          }
          this.tokenCounter.startNewTurn();
          this.tokenCounter.addRequestTokens(this.tokenCounter.countTokens(message));
          if (this.history.length > MAX_HISTORY_MESSAGES) {
            const removedCount = this.history.length - MAX_HISTORY_MESSAGES;
            this.history = this.history.slice(removedCount);
            if (this.debug) console.log(`[DEBUG] Trimmed history to ${this.history.length} messages (removed ${removedCount}).`);
          }
          const isFirstMessage = this.history.length === 0;
          const messageId = `msg_${Date.now()}`;
          appTracer.startUserMessageProcessing(sessionId2, messageId, message);
          const { imageUrls, cleanedMessage } = appTracer.withUserProcessingContext(
            sessionId2,
            () => extractImageUrls(message, this.debug)
          );
          if (imageUrls.length > 0) {
            appTracer.startImageProcessing(sessionId2, messageId, imageUrls, cleanedMessage.length);
            if (this.debug) console.log(`[DEBUG] Found ${imageUrls.length} image URLs in message`);
          }
          if (imageUrls.length > 0) {
            if (process.env.PROBE_NON_INTERACTIVE !== "1" || process.env.DEBUG_CHAT === "1") {
              console.log(`Detected ${imageUrls.length} image URL(s) in message.`);
            }
            if (this.debug) {
              console.log(`[DEBUG] Extracted image URLs:`, imageUrls);
            }
          }
          let validImageUrls = [];
          let validationResults = null;
          if (imageUrls.length > 0) {
            const validationStartTime = Date.now();
            validImageUrls = await validateImageUrls(imageUrls, this.debug);
            const validationEndTime = Date.now();
            validationResults = {
              totalUrls: imageUrls.length,
              validUrls: validImageUrls.length,
              invalidUrls: imageUrls.length - validImageUrls.length,
              redirectedUrls: 0,
              // TODO: capture from validateImageUrls if needed
              timeoutUrls: 0,
              // TODO: capture from validateImageUrls if needed  
              networkErrors: 0,
              // TODO: capture from validateImageUrls if needed
              durationMs: validationEndTime - validationStartTime
            };
            appTracer.recordImageValidation(sessionId2, validationResults);
            appTracer.endImageProcessing(sessionId2, validImageUrls.length > 0, validImageUrls.length);
          } else {
            validImageUrls = await validateImageUrls(imageUrls, this.debug);
          }
          appTracer.withUserProcessingContext(sessionId2, () => {
            appTracer.startAgentLoop(sessionId2, MAX_TOOL_ITERATIONS);
          });
          if (imageUrls.length > 0) {
            const invalidCount = imageUrls.length - validImageUrls.length;
            if (process.env.PROBE_NON_INTERACTIVE !== "1" || process.env.DEBUG_CHAT === "1") {
              if (validImageUrls.length > 0) {
                console.log(`Image validation: ${validImageUrls.length} valid, ${invalidCount} invalid/inaccessible.`);
              } else {
                console.log(`Image validation: All ${imageUrls.length} image URLs failed validation.`);
              }
            }
            if (this.debug && validImageUrls.length > 0) {
              console.log(`[DEBUG] Valid image URLs:`, validImageUrls);
            }
          }
          const wrappedMessage = isFirstMessage ? `<task>
${cleanedMessage}
</task>` : cleanedMessage;
          const allImages = [...validImageUrls, ...images];
          const userMessage = { role: "user", content: wrappedMessage };
          const displayUserMessage = {
            role: "user",
            content: message,
            // Store original unwrapped message
            visible: true,
            displayType: "user",
            timestamp: (/* @__PURE__ */ new Date()).toISOString()
          };
          if (allImages.length > 0) {
            userMessage.content = [
              { type: "text", text: wrappedMessage },
              ...allImages.map((imageUrl) => ({
                type: "image",
                image: imageUrl
              }))
            ];
            displayUserMessage.images = allImages;
            if (this.debug) {
              console.log(`[DEBUG] Created message with ${allImages.length} images (${validImageUrls.length} from URLs, ${images.length} uploaded)`);
            }
          }
          if (!this.displayHistory) {
            this.displayHistory = [];
          }
          this.displayHistory.push(displayUserMessage);
          if (this.storage) {
            this.storage.saveMessage(this.sessionId, {
              role: "user",
              content: message,
              // Original message
              timestamp: Date.now(),
              displayType: "user",
              visible: 1,
              images: allImages,
              metadata: {}
            }).catch((err) => {
              console.error("Failed to save user message to persistent storage:", err);
            });
          }
          let currentMessages = [
            ...this.history,
            userMessage
          ];
          const promptGenerationStart = Date.now();
          const systemPrompt = await this.getSystemMessage();
          const promptGenerationEnd = Date.now();
          if (this.debug) {
            const systemTokens = this.tokenCounter.countTokens(systemPrompt);
            this.tokenCounter.addRequestTokens(systemTokens);
            console.log(`[DEBUG] System prompt estimated tokens: ${systemTokens}`);
            appTracer.recordSystemPromptGeneration(sessionId2, {
              baseLength: 11747,
              // Approximate base system message length
              finalLength: systemPrompt.length,
              filesAdded: this.history.length > 0 ? 35 : 36,
              // Approximate from logs
              generationDurationMs: promptGenerationEnd - promptGenerationStart,
              promptType: this.promptType || "default",
              estimatedTokens: systemTokens
            });
          }
          while (currentIteration < MAX_TOOL_ITERATIONS && !completionAttempted) {
            currentIteration++;
            if (this.cancelled) throw new Error("Request was cancelled by the user");
            appTracer.withAgentLoopContext(sessionId2, () => {
              appTracer.startAgentIteration(sessionId2, currentIteration, currentMessages.length, this.tokenCounter.contextSize || 0);
            });
            if (this.debug) {
              console.log(`
[DEBUG] --- Tool Loop Iteration ${currentIteration}/${MAX_TOOL_ITERATIONS} ---`);
              console.log(`[DEBUG] Current messages count for AI call: ${currentMessages.length}`);
              currentMessages.slice(-3).forEach((msg, idx) => {
                const contentPreview = (typeof msg.content === "string" ? msg.content : JSON.stringify(msg.content)).substring(0, 80).replace(/\n/g, " ");
                console.log(`[DEBUG]   Msg[${currentMessages.length - 3 + idx}]: ${msg.role}: ${contentPreview}...`);
              });
            }
            this.tokenCounter.calculateContextSize(currentMessages);
            if (this.debug) console.log(`[DEBUG] Estimated context tokens BEFORE LLM call (Iter ${currentIteration}): ${this.tokenCounter.contextSize}`);
            let maxResponseTokens = 4e3;
            if (this.model.includes("claude-3-opus") || this.model.startsWith("gpt-4-")) {
              maxResponseTokens = 4096;
            } else if (this.model.includes("claude-3-5-sonnet") || this.model.startsWith("gpt-4o")) {
              maxResponseTokens = 8e3;
            } else if (this.model.includes("gemini-2.5")) {
              maxResponseTokens = 6e4;
            } else if (this.model.startsWith("gemini")) {
              maxResponseTokens = 8e3;
            }
            this.tokenDisplay = new TokenUsageDisplay({ maxTokens: maxResponseTokens });
            const userMsgIndices = currentMessages.reduce(
              (acc, msg, index) => msg.role === "user" ? [...acc, index] : acc,
              []
            );
            const lastUserMsgIndex = userMsgIndices[userMsgIndices.length - 1] ?? -1;
            const secondLastUserMsgIndex = userMsgIndices[userMsgIndices.length - 2] ?? -1;
            let transformedMessages = currentMessages;
            if (this.apiType === "anthropic") {
              transformedMessages = currentMessages.map((message2, index) => {
                if (message2.role === "user" && (index === lastUserMsgIndex || index === secondLastUserMsgIndex)) {
                  return {
                    ...message2,
                    content: typeof message2.content === "string" ? [{ type: "text", text: message2.content, providerOptions: { anthropic: { cacheControl: { type: "ephemeral" } } } }] : message2.content.map((content, contentIndex) => {
                      if (content.type === "text" && contentIndex === 0) {
                        return {
                          ...content,
                          providerOptions: { anthropic: { cacheControl: { type: "ephemeral" } } }
                        };
                      }
                      return content;
                    })
                  };
                }
                return message2;
              });
            }
            let streamError;
            const generateOptions = {
              model: this.provider(this.model),
              messages: transformedMessages,
              system: systemPrompt,
              temperature: 0.3,
              maxTokens: maxResponseTokens,
              signal: this.abortController.signal,
              onError({ error }) {
                streamError = error;
                console.error(error);
              },
              providerOptions: {
                openai: {
                  streamOptions: {
                    include_usage: true
                  }
                }
              },
              experimental_telemetry: {
                isEnabled: false,
                // Disable built-in telemetry in favor of our custom tracing
                functionId: this.sessionId,
                metadata: {
                  sessionId: this.sessionId,
                  iteration: currentIteration,
                  model: this.model,
                  apiType: this.apiType,
                  allowEdit: this.allowEdit,
                  promptType: this.promptType || "default"
                }
              }
            };
            const aiRequestSpan = appTracer.withIterationContext(sessionId2, currentIteration, () => {
              return appTracer.startAiGenerationRequest(sessionId2, currentIteration, this.model, this.apiType, {
                temperature: 0.3,
                maxTokens: maxResponseTokens,
                maxRetries: 2
              });
            });
            let assistantResponseContent = "";
            let startTime = Date.now();
            let firstChunkTime = null;
            try {
              if (this.debug) console.log(`[DEBUG] Calling streamText with model ${this.model}...`);
              if (streamError) {
                throw streamError;
              }
              const { textStream } = streamText(generateOptions);
              for await (const chunk of textStream) {
                if (this.cancelled) throw new Error("Request was cancelled by the user");
                if (firstChunkTime === null) {
                  firstChunkTime = Date.now();
                }
                assistantResponseContent += chunk;
              }
              if (this.debug) {
                console.log(`[DEBUG] Streamed AI response (Iter ${currentIteration}). Length: ${assistantResponseContent.length}`);
              }
              if (assistantResponseContent.length == 0) {
                console.warn(`[WARN] Empty response from AI model (Iter ${currentIteration}).`);
                throw new Error("Empty response from AI model");
              }
              currentMessages.push({ role: "assistant", content: assistantResponseContent });
              const responseTokenCount = this.tokenCounter.countTokens(assistantResponseContent);
              if (this.debug) console.log(`[DEBUG] Estimated response tokens (Iter ${currentIteration}): ${responseTokenCount}`);
              this.tokenCounter.addResponseTokens(responseTokenCount);
              this.tokenCounter.calculateContextSize(currentMessages);
              if (this.debug) console.log(`[DEBUG] Context size AFTER LLM response (Iter ${currentIteration}): ${this.tokenCounter.contextSize}`);
              const endTime = Date.now();
              appTracer.recordAiResponse(sessionId2, currentIteration, {
                response: assistantResponseContent,
                // Include actual response content
                responseLength: assistantResponseContent.length,
                completionTokens: responseTokenCount,
                promptTokens: this.tokenCounter.contextSize || 0,
                finishReason: "stop",
                timeToFirstChunk: firstChunkTime ? firstChunkTime - startTime : 0,
                timeToFinish: endTime - startTime
              });
              appTracer.endAiRequest(sessionId2, currentIteration, true);
            } catch (error) {
              let errorCategory = "unknown";
              if (this.cancelled || error.name === "AbortError" || error.message && error.message.includes("cancelled")) {
                errorCategory = "cancellation";
              } else if (error.message?.includes("timeout")) {
                errorCategory = "timeout";
              } else if (error.message?.includes("rate limit") || error.message?.includes("quota")) {
                errorCategory = "api_limit";
              } else if (error.message?.includes("network") || error.message?.includes("fetch")) {
                errorCategory = "network";
              } else if (error.status >= 400 && error.status < 500) {
                errorCategory = "client_error";
              } else if (error.status >= 500) {
                errorCategory = "server_error";
              }
              appTracer.recordAiModelError(sessionId2, currentIteration, {
                category: errorCategory,
                message: error.message,
                model: this.model,
                provider: this.apiType,
                statusCode: error.status || 0,
                retryAttempt: 0
              });
              appTracer.endAiRequest(sessionId2, currentIteration, false);
              if (this.cancelled || error.name === "AbortError" || error.message && error.message.includes("cancelled")) {
                console.log(`Chat request cancelled during LLM call (Iter ${currentIteration})`);
                this.cancelled = true;
                appTracer.recordSessionCancellation(sessionId2, "ai_request_cancelled", {
                  currentIteration,
                  activeTool: "ai_generation"
                });
                throw new Error("Request was cancelled by the user");
              }
              console.error(`Error during streamText (Iter ${currentIteration}):`, error);
              finalResult = `Error: Failed to get response from AI model during iteration ${currentIteration}. ${error.message}`;
              throw new Error(finalResult);
            }
            const parsedTool = parseXmlToolCallWithThinking(assistantResponseContent);
            if (parsedTool) {
              const { toolName, params } = parsedTool;
              if (this.debug) console.log(`[DEBUG] Parsed tool call: ${toolName} with params:`, params);
              appTracer.recordToolCallParsed(sessionId2, currentIteration, toolName, params);
              if (toolName === "attempt_completion") {
                completionAttempted = true;
                const validation = attemptCompletionSchema.safeParse(params);
                if (!validation.success) {
                  finalResult = `Error: AI attempted completion with invalid parameters: ${JSON.stringify(validation.error.issues)}`;
                  console.warn(`[WARN] Invalid attempt_completion parameters:`, validation.error.issues);
                  appTracer.recordCompletionAttempt(sessionId2, false);
                } else {
                  finalResult = validation.data.result;
                  const displayAssistantMessage = {
                    role: "assistant",
                    content: finalResult,
                    visible: true,
                    displayType: "final",
                    timestamp: (/* @__PURE__ */ new Date()).toISOString()
                  };
                  this.displayHistory.push(displayAssistantMessage);
                  if (this.storage) {
                    this.storage.saveMessage(this.sessionId, {
                      role: "assistant",
                      content: finalResult,
                      timestamp: Date.now(),
                      displayType: "final",
                      visible: 1,
                      images: [],
                      metadata: {}
                    }).catch((err) => {
                      console.error("Failed to save final response to persistent storage:", err);
                    });
                  }
                  appTracer.recordCompletionAttempt(sessionId2, true, finalResult);
                  if (this.debug) {
                    console.log(`[DEBUG] Completion attempted successfully. Final Result captured.`);
                    try {
                      const systemPrompt2 = await this.getSystemMessage();
                      let debugContent = `system: ${systemPrompt2}

`;
                      for (const msg of currentMessages) {
                        if (msg.role === "user" || msg.role === "assistant") {
                          debugContent += `${msg.role}: ${msg.content}

`;
                        }
                      }
                      debugContent += `assistant (final result): ${finalResult}

`;
                      writeFileSync(debugFilePath, debugContent, { flag: "w" });
                      if (this.debug) console.log(`[DEBUG] Wrote complete chat history to ${debugFilePath}`);
                    } catch (error) {
                      console.error(`Error writing chat history to debug file: ${error.message}`);
                    }
                  }
                }
                break;
              } else if (this.toolImplementations[toolName]) {
                const toolInstance = this.toolImplementations[toolName];
                let toolResultContent = "";
                appTracer.withIterationContext(sessionId2, currentIteration, () => {
                  appTracer.startToolExecution(sessionId2, currentIteration, toolName, params);
                });
                try {
                  const enhancedParams2 = { ...params, sessionId: this.sessionId };
                  if (this.debug) console.log(`[DEBUG] Executing tool '${toolName}' with params:`, enhancedParams2);
                  const executionResult = await toolInstance.execute(enhancedParams2);
                  toolResultContent = typeof executionResult === "string" ? executionResult : JSON.stringify(executionResult, null, 2);
                  if (this.debug) {
                    const preview = toolResultContent.substring(0, 200).replace(/\n/g, " ") + (toolResultContent.length > 200 ? "..." : "");
                    console.log(`[DEBUG] Tool '${toolName}' executed successfully. Result preview: ${preview}`);
                  }
                  appTracer.endToolExecution(sessionId2, currentIteration, true, toolResultContent.length, null, toolResultContent);
                } catch (error) {
                  console.error(`Error executing tool ${toolName}:`, error);
                  toolResultContent = `Error executing tool ${toolName}: ${error.message}`;
                  if (this.debug) console.log(`[DEBUG] Tool '${toolName}' execution FAILED.`);
                  let errorCategory = "execution";
                  if (error.message?.includes("validation")) {
                    errorCategory = "validation";
                  } else if (error.message?.includes("permission") || error.message?.includes("access")) {
                    errorCategory = "filesystem";
                  } else if (error.message?.includes("network") || error.message?.includes("fetch")) {
                    errorCategory = "network";
                  } else if (error.message?.includes("timeout")) {
                    errorCategory = "timeout";
                  }
                  appTracer.recordToolError(sessionId2, currentIteration, toolName, {
                    category: errorCategory,
                    message: error.message,
                    exitCode: error.code || 0,
                    signal: error.signal || "",
                    params: enhancedParams
                  });
                  appTracer.endToolExecution(sessionId2, currentIteration, false, 0, error.message, toolResultContent);
                }
                const toolResultMessage = `<tool_result>
${toolResultContent}
</tool_result>`;
                currentMessages.push({ role: "user", content: toolResultMessage });
                this.tokenCounter.calculateContextSize(currentMessages);
                if (this.debug) console.log(`[DEBUG] Context size after adding tool result for '${toolName}': ${this.tokenCounter.contextSize}`);
              } else {
                if (this.debug) console.log(`[DEBUG] Assistant used invalid tool name: ${toolName}`);
                const errorContent = `<tool_result>
Error: Invalid tool name specified: '${toolName}'. Please use one of: search, query, extract, attempt_completion.
</tool_result>`;
                currentMessages.push({ role: "user", content: errorContent });
                this.tokenCounter.calculateContextSize(currentMessages);
              }
            } else {
              if (this.debug) console.log(`[DEBUG] Assistant response did not contain a valid XML tool call.`);
              const forceToolContent = `Your response did not contain a valid tool call in the required XML format. You MUST respond with exactly one tool call (e.g., <search>...</search> or <attempt_completion>...</attempt_completion>) based on the previous steps and the user's goal. Analyze the situation and choose the appropriate next tool.`;
              currentMessages.push({ role: "user", content: forceToolContent });
              this.tokenCounter.calculateContextSize(currentMessages);
            }
            if (currentMessages.length > MAX_HISTORY_MESSAGES + 3) {
              const messagesBefore = currentMessages.length;
              const removeCount = currentMessages.length - MAX_HISTORY_MESSAGES;
              currentMessages = currentMessages.slice(removeCount);
              appTracer.recordHistoryOperation(sessionId2, "trim", {
                messagesBefore,
                messagesAfter: currentMessages.length,
                messagesRemoved: removeCount,
                reason: "loop_memory_limit"
              });
              if (this.debug) console.log(`[DEBUG] Trimmed 'currentMessages' within loop to ${currentMessages.length} (removed ${removeCount}).`);
              this.tokenCounter.calculateContextSize(currentMessages);
            }
            appTracer.endIteration(sessionId2, currentIteration, true, completionAttempted ? "completion_attempted" : "tool_executed");
          }
          if (currentIteration >= MAX_TOOL_ITERATIONS && !completionAttempted) {
            console.warn(`[WARN] Max tool iterations (${MAX_TOOL_ITERATIONS}) reached for session ${this.sessionId}. Returning current error state.`);
          }
          appTracer.endAgentLoop(sessionId2, currentIteration, completionAttempted, completionAttempted ? "completion" : "max_iterations");
          this.history = currentMessages.map((msg) => ({ ...msg }));
          if (this.history.length > MAX_HISTORY_MESSAGES) {
            const messagesBefore = this.history.length;
            const finalRemoveCount = this.history.length - MAX_HISTORY_MESSAGES;
            this.history = this.history.slice(finalRemoveCount);
            appTracer.recordHistoryOperation(sessionId2, "trim", {
              messagesBefore,
              messagesAfter: this.history.length,
              messagesRemoved: finalRemoveCount,
              reason: "max_length"
            });
            if (this.debug) console.log(`[DEBUG] Final history trim applied. Length: ${this.history.length} (removed ${finalRemoveCount})`);
          }
          this.tokenCounter.updateHistory(this.history);
          const tokenUsage = this.tokenCounter.getTokenUsage();
          appTracer.recordTokenMetrics(sessionId2, {
            contextWindow: tokenUsage.contextWindow || 0,
            currentTotal: tokenUsage.current?.total || 0,
            requestTokens: tokenUsage.current?.request || 0,
            responseTokens: tokenUsage.current?.response || 0,
            cacheRead: tokenUsage.current?.cacheRead || 0,
            cacheWrite: tokenUsage.current?.cacheWrite || 0
          });
          appTracer.endUserMessageProcessing(sessionId2, completionAttempted);
          if (this.debug) {
            console.log(`[DEBUG] Updated tokenCounter history with ${this.history.length} messages`);
            console.log(`[DEBUG] Context size after history update: ${this.tokenCounter.contextSize}`);
            console.log(`[DEBUG] ===== Ending XML Tool Chat Loop =====`);
            console.log(`[DEBUG] Loop finished after ${currentIteration} iterations.`);
            console.log(`[DEBUG] Completion attempted: ${completionAttempted}`);
            console.log(`[DEBUG] Final history length: ${this.history.length}`);
            const resultPreview = (typeof finalResult === "string" ? finalResult : JSON.stringify(finalResult)).substring(0, 200).replace(/\n/g, " ");
            console.log(`[DEBUG] Returning final result: "${resultPreview}..."`);
          }
          this.tokenCounter.calculateContextSize(this.history);
          const updatedTokenUsage = this.tokenCounter.getTokenUsage();
          if (this.debug) {
            console.log(`[DEBUG] Final context window size: ${updatedTokenUsage.contextWindow}`);
            console.log(`[DEBUG] Cache metrics - Read: ${updatedTokenUsage.current.cacheRead}, Write: ${updatedTokenUsage.current.cacheWrite}`);
          }
          return {
            response: finalResult,
            tokenUsage: updatedTokenUsage
          };
        } catch (error) {
          const isCriticalApiError = this._isCriticalApiError(error);
          if (this.cancelled || error.message && error.message.includes("cancelled")) {
            appTracer.recordSessionCancellation(sessionId2, "processing_cancelled", {
              currentIteration,
              errorMessage: error.message
            });
          } else {
            appTracer.recordAiModelError(sessionId2, currentIteration || 0, {
              category: isCriticalApiError ? "critical_api_error" : "processing_error",
              message: error.message,
              model: this.model,
              provider: this.apiType,
              statusCode: error.statusCode || 0,
              retryAttempt: 0
            });
          }
          appTracer.endChatSession(sessionId2, false, 0);
          appTracer.cleanup(sessionId2);
          console.error("Error in chat processing loop:", error);
          if (this.debug) console.error("Error in chat processing loop:", error);
          this.tokenCounter.updateHistory(this.history);
          if (this.debug) console.log(`[DEBUG] Error case - Updated tokenCounter history with ${this.history.length} messages`);
          this.tokenCounter.calculateContextSize(this.history);
          const updatedTokenUsage = this.tokenCounter.getTokenUsage();
          if (this.debug) {
            console.log(`[DEBUG] Error case - Final context window size: ${updatedTokenUsage.contextWindow}`);
            console.log(`[DEBUG] Error case - Cache metrics - Read: ${updatedTokenUsage.current.cacheRead}, Write: ${updatedTokenUsage.current.cacheWrite}`);
          }
          if (this.cancelled || error.message && error.message.includes("cancelled")) {
            return { response: "Request cancelled.", tokenUsage: updatedTokenUsage };
          }
          if (isCriticalApiError) {
            throw error;
          }
          return {
            response: `Error during chat processing: ${error.message || "An unexpected error occurred."}`,
            tokenUsage: updatedTokenUsage
          };
        } finally {
          this.abortController = null;
        }
      }
      /**
       * Check if an error is a critical API error that should cause process exit
       * @param {Error} error - The error to check
       * @returns {boolean} - True if this is a critical API error
       * @private
       */
      _isCriticalApiError(error) {
        if (error[Symbol.for("vercel.ai.error.AI_APICallError")]) {
          const statusCode = error.statusCode;
          const errorMessage2 = error.message?.toLowerCase() || "";
          if (statusCode === 401 || statusCode === 403) {
            return true;
          }
          if (statusCode === 404) {
            return true;
          }
          if (statusCode >= 500 && statusCode < 600) {
            return false;
          }
        }
        const errorMessage = error.message?.toLowerCase() || "";
        if (errorMessage.includes("not found")) {
          return true;
        }
        if (errorMessage.includes("unauthorized") || errorMessage.includes("invalid api key")) {
          return true;
        }
        if (errorMessage.includes("forbidden") || errorMessage.includes("access denied")) {
          return true;
        }
        if (errorMessage.includes("empty response from ai model")) {
          return true;
        }
        return false;
      }
      /**
       * Get the current token usage summary
       * @returns {Object} - Raw token usage data for UI display
       */
      getTokenUsage() {
        const usage = this.tokenCounter.getTokenUsage();
        return usage;
      }
      /**
       * Clear the entire history and reset session/token usage
       * @returns {string} - The new session ID
       */
      clearHistory() {
        const oldHistoryLength = this.history.length;
        const oldSessionId = this.sessionId;
        this.history = [];
        this.sessionId = randomUUID4();
        this.tokenCounter.clear();
        if (this.tokenCounter.history && this.tokenCounter.history.length > 0) {
          this.tokenCounter.history = [];
          if (this.debug) {
            console.log(`[DEBUG] Explicitly cleared tokenCounter history after clear() call`);
          }
        }
        this.cancelled = false;
        if (this.abortController) {
          try {
            this.abortController.abort("History cleared");
          } catch (e) {
          }
          this.abortController = null;
        }
        if (this.debug) {
          console.log(`[DEBUG] ===== CLEARING CHAT HISTORY & STATE =====`);
          console.log(`[DEBUG] Cleared ${oldHistoryLength} messages from history`);
          console.log(`[DEBUG] Old session ID: ${oldSessionId}`);
          console.log(`[DEBUG] New session ID: ${this.sessionId}`);
          console.log(`[DEBUG] Token counter reset.`);
          console.log(`[DEBUG] Cancellation flag reset.`);
        }
        return this.sessionId;
      }
      /**
       * Get the session ID for this chat instance
       * @returns {string} - The session ID
       */
      getSessionId() {
        return this.sessionId;
      }
    };
  }
});

// auth.js
import "dotenv/config";
function authMiddleware(req, res, next) {
  const AUTH_ENABLED = process.env.AUTH_ENABLED === "1";
  if (!AUTH_ENABLED) {
    return next(req, res);
  }
  const AUTH_USERNAME = process.env.AUTH_USERNAME || "admin";
  const AUTH_PASSWORD = process.env.AUTH_PASSWORD || "password";
  const authHeader = req.headers.authorization;
  if (!authHeader) {
    res.writeHead(401, {
      "Content-Type": "text/plain",
      "WWW-Authenticate": 'Basic realm="Probe Code Search"'
    });
    res.end("Authentication required");
    return;
  }
  try {
    const authParts = authHeader.split(" ");
    if (authParts.length !== 2 || authParts[0] !== "Basic") {
      throw new Error("Invalid Authorization header format");
    }
    const credentials = Buffer.from(authParts[1], "base64").toString("utf-8");
    const [username, password] = credentials.split(":");
    if (username === AUTH_USERNAME && password === AUTH_PASSWORD) {
      return next(req, res);
    } else {
      res.writeHead(401, {
        "Content-Type": "text/plain",
        "WWW-Authenticate": 'Basic realm="Probe Code Search"'
      });
      res.end("Invalid credentials");
      return;
    }
  } catch (error) {
    res.writeHead(400, { "Content-Type": "text/plain" });
    res.end("Invalid Authorization header");
    return;
  }
}
var init_auth = __esm({
  "auth.js"() {
    __name(authMiddleware, "authMiddleware");
  }
});

// cancelRequest.js
function registerRequest(sessionId2, requestData) {
  if (!sessionId2) {
    console.warn("Attempted to register request without session ID");
    return;
  }
  console.log(`Registering request for session: ${sessionId2}`);
  activeRequests.set(sessionId2, requestData);
}
function cancelRequest(sessionId2) {
  if (!sessionId2) {
    console.warn("Attempted to cancel request without session ID");
    return false;
  }
  const requestData = activeRequests.get(sessionId2);
  if (!requestData) {
    console.warn(`No active request found for session: ${sessionId2}`);
    return false;
  }
  console.log(`Cancelling request for session: ${sessionId2}`);
  if (typeof requestData.abort === "function") {
    try {
      requestData.abort();
      console.log(`Successfully aborted request for session: ${sessionId2}`);
    } catch (error) {
      console.error(`Error aborting request for session ${sessionId2}:`, error);
    }
  }
  activeRequests.delete(sessionId2);
  return true;
}
function clearRequest(sessionId2) {
  if (!sessionId2) {
    console.warn("Attempted to clear request without session ID");
    return;
  }
  if (activeRequests.has(sessionId2)) {
    console.log(`Clearing request for session: ${sessionId2}`);
    activeRequests.delete(sessionId2);
  }
}
var activeRequests;
var init_cancelRequest = __esm({
  "cancelRequest.js"() {
    activeRequests = /* @__PURE__ */ new Map();
    __name(registerRequest, "registerRequest");
    __name(cancelRequest, "cancelRequest");
    __name(clearRequest, "clearRequest");
  }
});

// storage/JsonChatStorage.js
import { homedir } from "os";
import { join as join2 } from "path";
import { existsSync as existsSync3, mkdirSync as mkdirSync2, writeFileSync as writeFileSync2, readFileSync, readdirSync, statSync, unlinkSync } from "fs";
var JsonChatStorage;
var init_JsonChatStorage = __esm({
  "storage/JsonChatStorage.js"() {
    JsonChatStorage = class {
      static {
        __name(this, "JsonChatStorage");
      }
      constructor(options = {}) {
        this.webMode = options.webMode || false;
        this.verbose = options.verbose || false;
        this.baseDir = this.getChatHistoryDir();
        this.sessionsDir = join2(this.baseDir, "sessions");
        this.fallbackToMemory = false;
        this.memorySessions = /* @__PURE__ */ new Map();
        this.memoryMessages = /* @__PURE__ */ new Map();
      }
      /**
       * Get the appropriate directory for storing chat history
       */
      getChatHistoryDir() {
        if (process.platform === "win32") {
          const localAppData = process.env.LOCALAPPDATA || join2(homedir(), "AppData", "Local");
          return join2(localAppData, "probe");
        } else {
          return join2(homedir(), ".probe");
        }
      }
      /**
       * Ensure the chat history directory exists
       */
      ensureChatHistoryDir() {
        try {
          if (!existsSync3(this.baseDir)) {
            mkdirSync2(this.baseDir, { recursive: true });
          }
          if (!existsSync3(this.sessionsDir)) {
            mkdirSync2(this.sessionsDir, { recursive: true });
          }
          return true;
        } catch (error) {
          console.warn(`Failed to create chat history directory ${this.baseDir}:`, error.message);
          return false;
        }
      }
      /**
       * Get the file path for a session
       */
      getSessionFilePath(sessionId2) {
        return join2(this.sessionsDir, `${sessionId2}.json`);
      }
      /**
       * Initialize storage - JSON files if in web mode and directory is accessible
       */
      async initialize() {
        if (!this.webMode) {
          this.fallbackToMemory = true;
          if (this.verbose) {
            console.log("Using in-memory storage (CLI mode)");
          }
          return true;
        }
        try {
          if (!this.ensureChatHistoryDir()) {
            this.fallbackToMemory = true;
            if (this.verbose) {
              console.log("Cannot create history directory, using in-memory storage");
            }
            return true;
          }
          if (this.verbose) {
            console.log(`JSON file storage initialized at: ${this.sessionsDir}`);
          }
          return true;
        } catch (error) {
          console.warn("Failed to initialize JSON storage, falling back to memory:", error.message);
          this.fallbackToMemory = true;
          return true;
        }
      }
      /**
       * Save or update session data
       */
      async saveSession(sessionData) {
        const { id, createdAt, lastActivity, firstMessagePreview, metadata = {} } = sessionData;
        if (this.fallbackToMemory) {
          this.memorySessions.set(id, {
            id,
            created_at: createdAt,
            last_activity: lastActivity,
            first_message_preview: firstMessagePreview,
            metadata
          });
          return true;
        }
        try {
          const filePath = this.getSessionFilePath(id);
          let existingData = {
            id,
            created_at: createdAt,
            last_activity: lastActivity,
            first_message_preview: firstMessagePreview,
            metadata,
            messages: []
          };
          if (existsSync3(filePath)) {
            try {
              const fileContent = readFileSync(filePath, "utf8");
              const existing = JSON.parse(fileContent);
              existingData = {
                ...existing,
                last_activity: lastActivity,
                first_message_preview: firstMessagePreview || existing.first_message_preview,
                metadata: { ...existing.metadata, ...metadata }
              };
            } catch (error) {
              console.warn(`Failed to read existing session file ${filePath}:`, error.message);
            }
          }
          writeFileSync2(filePath, JSON.stringify(existingData, null, 2));
          return true;
        } catch (error) {
          console.error("Failed to save session:", error);
          return false;
        }
      }
      /**
       * Update session activity timestamp
       */
      async updateSessionActivity(sessionId2, timestamp = Date.now()) {
        if (this.fallbackToMemory) {
          const session = this.memorySessions.get(sessionId2);
          if (session) {
            session.last_activity = timestamp;
          }
          return true;
        }
        try {
          const filePath = this.getSessionFilePath(sessionId2);
          if (existsSync3(filePath)) {
            const fileContent = readFileSync(filePath, "utf8");
            const sessionData = JSON.parse(fileContent);
            sessionData.last_activity = timestamp;
            writeFileSync2(filePath, JSON.stringify(sessionData, null, 2));
          }
          return true;
        } catch (error) {
          console.error("Failed to update session activity:", error);
          return false;
        }
      }
      /**
       * Save a message to the session
       */
      async saveMessage(sessionId2, messageData) {
        const {
          role,
          content,
          timestamp = Date.now(),
          displayType,
          visible = 1,
          images = [],
          metadata = {}
        } = messageData;
        const message = {
          role,
          content,
          timestamp,
          display_type: displayType,
          visible,
          images,
          metadata
        };
        if (this.fallbackToMemory) {
          if (!this.memoryMessages.has(sessionId2)) {
            this.memoryMessages.set(sessionId2, []);
          }
          this.memoryMessages.get(sessionId2).push(message);
          return true;
        }
        try {
          const filePath = this.getSessionFilePath(sessionId2);
          let sessionData = {
            id: sessionId2,
            created_at: timestamp,
            last_activity: timestamp,
            first_message_preview: null,
            metadata: {},
            messages: []
          };
          if (existsSync3(filePath)) {
            try {
              const fileContent = readFileSync(filePath, "utf8");
              sessionData = JSON.parse(fileContent);
            } catch (error) {
              console.warn(`Failed to read session file ${filePath}:`, error.message);
            }
          }
          sessionData.messages.push(message);
          sessionData.last_activity = timestamp;
          if (role === "user" && !sessionData.first_message_preview) {
            const preview = content.length > 100 ? content.substring(0, 100) + "..." : content;
            sessionData.first_message_preview = preview;
          }
          writeFileSync2(filePath, JSON.stringify(sessionData, null, 2));
          return true;
        } catch (error) {
          console.error("Failed to save message:", error);
          return false;
        }
      }
      /**
       * Get session history (display messages only)
       */
      async getSessionHistory(sessionId2, limit = 100) {
        if (this.fallbackToMemory) {
          const messages = this.memoryMessages.get(sessionId2) || [];
          return messages.filter((msg) => msg.visible).slice(0, limit);
        }
        try {
          const filePath = this.getSessionFilePath(sessionId2);
          if (!existsSync3(filePath)) {
            return [];
          }
          const fileContent = readFileSync(filePath, "utf8");
          const sessionData = JSON.parse(fileContent);
          return (sessionData.messages || []).filter((msg) => msg.visible).slice(0, limit);
        } catch (error) {
          console.error("Failed to get session history:", error);
          return [];
        }
      }
      /**
       * List recent sessions using file modification dates
       */
      async listSessions(limit = 50, offset = 0) {
        if (this.fallbackToMemory) {
          const sessions = Array.from(this.memorySessions.values()).sort((a, b) => b.last_activity - a.last_activity).slice(offset, offset + limit);
          return sessions;
        }
        try {
          if (!existsSync3(this.sessionsDir)) {
            return [];
          }
          const files = readdirSync(this.sessionsDir).filter((file) => file.endsWith(".json")).map((file) => {
            const filePath = join2(this.sessionsDir, file);
            const stat = statSync(filePath);
            return {
              file,
              filePath,
              mtime: stat.mtime.getTime(),
              sessionId: file.replace(".json", "")
            };
          }).sort((a, b) => b.mtime - a.mtime).slice(offset, offset + limit);
          const sessions = [];
          for (const fileInfo of files) {
            try {
              const fileContent = readFileSync(fileInfo.filePath, "utf8");
              const sessionData = JSON.parse(fileContent);
              sessions.push({
                id: sessionData.id,
                created_at: sessionData.created_at,
                last_activity: sessionData.last_activity || fileInfo.mtime,
                first_message_preview: sessionData.first_message_preview,
                metadata: sessionData.metadata || {}
              });
            } catch (error) {
              console.warn(`Failed to read session file ${fileInfo.filePath}:`, error.message);
            }
          }
          return sessions;
        } catch (error) {
          console.error("Failed to list sessions:", error);
          return [];
        }
      }
      /**
       * Delete a session and its file
       */
      async deleteSession(sessionId2) {
        if (this.fallbackToMemory) {
          this.memorySessions.delete(sessionId2);
          this.memoryMessages.delete(sessionId2);
          return true;
        }
        try {
          const filePath = this.getSessionFilePath(sessionId2);
          if (existsSync3(filePath)) {
            unlinkSync(filePath);
          }
          return true;
        } catch (error) {
          console.error("Failed to delete session:", error);
          return false;
        }
      }
      /**
       * Prune old sessions (older than specified days)
       */
      async pruneOldSessions(olderThanDays = 30) {
        const cutoffTime = Date.now() - olderThanDays * 24 * 60 * 60 * 1e3;
        if (this.fallbackToMemory) {
          let pruned = 0;
          for (const [sessionId2, session] of this.memorySessions.entries()) {
            if (session.last_activity < cutoffTime) {
              this.memorySessions.delete(sessionId2);
              this.memoryMessages.delete(sessionId2);
              pruned++;
            }
          }
          return pruned;
        }
        try {
          if (!existsSync3(this.sessionsDir)) {
            return 0;
          }
          const files = readdirSync(this.sessionsDir).filter((file) => file.endsWith(".json"));
          let pruned = 0;
          for (const file of files) {
            const filePath = join2(this.sessionsDir, file);
            const stat = statSync(filePath);
            if (stat.mtime.getTime() < cutoffTime) {
              unlinkSync(filePath);
              pruned++;
            }
          }
          return pruned;
        } catch (error) {
          console.error("Failed to prune old sessions:", error);
          return 0;
        }
      }
      /**
       * Get storage statistics
       */
      async getStats() {
        if (this.fallbackToMemory) {
          let messageCount = 0;
          let visibleMessageCount = 0;
          for (const messages of this.memoryMessages.values()) {
            messageCount += messages.length;
            visibleMessageCount += messages.filter((msg) => msg.visible).length;
          }
          return {
            session_count: this.memorySessions.size,
            message_count: messageCount,
            visible_message_count: visibleMessageCount,
            storage_type: "memory"
          };
        }
        try {
          if (!existsSync3(this.sessionsDir)) {
            return {
              session_count: 0,
              message_count: 0,
              visible_message_count: 0,
              storage_type: "json_files"
            };
          }
          const files = readdirSync(this.sessionsDir).filter((file) => file.endsWith(".json"));
          let messageCount = 0;
          let visibleMessageCount = 0;
          for (const file of files) {
            try {
              const filePath = join2(this.sessionsDir, file);
              const fileContent = readFileSync(filePath, "utf8");
              const sessionData = JSON.parse(fileContent);
              if (sessionData.messages) {
                messageCount += sessionData.messages.length;
                visibleMessageCount += sessionData.messages.filter((msg) => msg.visible).length;
              }
            } catch (error) {
            }
          }
          return {
            session_count: files.length,
            message_count: messageCount,
            visible_message_count: visibleMessageCount,
            storage_type: "json_files"
          };
        } catch (error) {
          console.error("Failed to get storage stats:", error);
          return {
            session_count: 0,
            message_count: 0,
            visible_message_count: 0,
            storage_type: "error"
          };
        }
      }
      /**
       * Check if using persistent storage
       */
      isPersistent() {
        return !this.fallbackToMemory;
      }
      /**
       * Close storage (no-op for JSON files)
       */
      async close() {
      }
    };
  }
});

// webServer.js
var webServer_exports = {};
__export(webServer_exports, {
  startWebServer: () => startWebServer
});
import "dotenv/config";
import { createServer } from "http";
import { readFileSync as readFileSync2, existsSync as existsSync4 } from "fs";
import { resolve, dirname as dirname2, join as join3 } from "path";
import { fileURLToPath as fileURLToPath2 } from "url";
import { randomUUID as randomUUID5 } from "crypto";
function getOrCreateChat(sessionId2, apiCredentials = null) {
  if (!sessionId2) {
    sessionId2 = randomUUID5();
    console.warn(`[WARN] Missing sessionId, generated fallback: ${sessionId2}`);
  }
  if (chatSessions.has(sessionId2)) {
    const existingChat = chatSessions.get(sessionId2);
    if (globalStorage) {
      globalStorage.updateSessionActivity(sessionId2).catch((err) => {
        console.error("Failed to update session activity:", err);
      });
    }
    return existingChat;
  }
  const options = { sessionId: sessionId2 };
  if (apiCredentials) {
    options.apiProvider = apiCredentials.apiProvider;
    options.apiKey = apiCredentials.apiKey;
    options.apiUrl = apiCredentials.apiUrl;
  }
  if (globalStorage) {
    options.storage = globalStorage;
  }
  const newChat = new ProbeChat(options);
  const now = Date.now();
  newChat.createdAt = now;
  newChat.lastActivity = now;
  chatSessions.set(sessionId2, newChat);
  if (globalStorage) {
    globalStorage.saveSession({
      id: sessionId2,
      createdAt: now,
      lastActivity: now,
      firstMessagePreview: null,
      // Will be updated when first message is sent
      metadata: {
        apiProvider: apiCredentials?.apiProvider || null
      }
    }).catch((err) => {
      console.error("Failed to save session to persistent storage:", err);
    });
  }
  if (process.env.DEBUG_CHAT === "1") {
    console.log(`[DEBUG] Created and stored new chat instance for session: ${sessionId2}. Total sessions: ${chatSessions.size}`);
    if (apiCredentials && apiCredentials.apiKey) {
      console.log(`[DEBUG] Chat instance created with client-provided API credentials (provider: ${apiCredentials.apiProvider})`);
    }
  }
  return newChat;
}
function startWebServer(version, hasApiKeys = true, options = {}) {
  const allowEdit = options?.allowEdit || false;
  if (allowEdit) {
    console.log("Edit mode enabled: implement tool is available");
  }
  const AUTH_ENABLED = process.env.AUTH_ENABLED === "1";
  const AUTH_USERNAME = process.env.AUTH_USERNAME || "admin";
  const AUTH_PASSWORD = process.env.AUTH_PASSWORD || "password";
  if (AUTH_ENABLED) {
    console.log(`Authentication enabled (username: ${AUTH_USERNAME})`);
  } else {
    console.log("Authentication disabled");
  }
  globalStorage = new JsonChatStorage({
    webMode: true,
    verbose: process.env.DEBUG_CHAT === "1"
  });
  (async () => {
    try {
      await globalStorage.initialize();
      const stats = await globalStorage.getStats();
      console.log(`Chat history storage: ${stats.storage_type} (${stats.session_count} sessions, ${stats.visible_message_count} messages)`);
    } catch (error) {
      console.warn("Failed to initialize chat history storage:", error.message);
    }
  })();
  const sseClients = /* @__PURE__ */ new Map();
  const staticAllowedFolders = process.env.ALLOWED_FOLDERS ? process.env.ALLOWED_FOLDERS.split(",").map((folder) => folder.trim()).filter(Boolean) : [];
  let noApiKeysMode = !hasApiKeys;
  if (noApiKeysMode) {
    console.log("Running in No API Keys mode - will show setup instructions to users");
  } else {
    console.log("API keys detected. Chat functionality enabled.");
  }
  const directApiTools = {
    search: searchToolInstance2,
    query: queryToolInstance2,
    extract: extractToolInstance2
  };
  if (allowEdit) {
    directApiTools.implement = implementToolInstance;
  }
  function sendSSEData(res, data, eventType = "message") {
    const DEBUG = process.env.DEBUG_CHAT === "1";
    try {
      if (!res.writable || res.writableEnded) {
        if (DEBUG) console.log(`[DEBUG] SSE stream closed for event type ${eventType}, cannot send.`);
        return;
      }
      if (DEBUG) {
      }
      res.write(`event: ${eventType}
`);
      res.write(`data: ${JSON.stringify(data)}

`);
      if (DEBUG) {
      }
    } catch (error) {
      console.error(`[ERROR] Error sending SSE data:`, error);
      try {
        if (res.writable && !res.writableEnded) res.end();
      } catch (closeError) {
        console.error(`[ERROR] Error closing SSE stream after send error:`, closeError);
      }
    }
  }
  __name(sendSSEData, "sendSSEData");
  const activeChatInstances = /* @__PURE__ */ new Map();
  const server = createServer(async (req, res) => {
    const processRequest = /* @__PURE__ */ __name((routeHandler) => {
      authMiddleware(req, res, () => {
        routeHandler(req, res);
      });
    }, "processRequest");
    const routes = {
      // Handle OPTIONS requests for CORS preflight (Common)
      "OPTIONS /api/token-usage": /* @__PURE__ */ __name((req2, res2) => handleOptions(res2), "OPTIONS /api/token-usage"),
      "OPTIONS /api/sessions": /* @__PURE__ */ __name((req2, res2) => handleOptions(res2), "OPTIONS /api/sessions"),
      "OPTIONS /chat": /* @__PURE__ */ __name((req2, res2) => handleOptions(res2), "OPTIONS /chat"),
      "OPTIONS /api/search": /* @__PURE__ */ __name((req2, res2) => handleOptions(res2), "OPTIONS /api/search"),
      "OPTIONS /api/query": /* @__PURE__ */ __name((req2, res2) => handleOptions(res2), "OPTIONS /api/query"),
      "OPTIONS /api/extract": /* @__PURE__ */ __name((req2, res2) => handleOptions(res2), "OPTIONS /api/extract"),
      "OPTIONS /api/implement": /* @__PURE__ */ __name((req2, res2) => handleOptions(res2), "OPTIONS /api/implement"),
      "OPTIONS /cancel-request": /* @__PURE__ */ __name((req2, res2) => handleOptions(res2), "OPTIONS /cancel-request"),
      "OPTIONS /folders": /* @__PURE__ */ __name((req2, res2) => handleOptions(res2), "OPTIONS /folders"),
      // Added for /folders
      // Token usage API endpoint
      "GET /api/token-usage": /* @__PURE__ */ __name((req2, res2) => {
        const sessionId2 = getSessionIdFromUrl(req2);
        if (!sessionId2) return sendError(res2, 400, "Missing sessionId parameter");
        const chatInstance = chatSessions.get(sessionId2);
        if (!chatInstance) return sendError(res2, 404, "Session not found");
        const DEBUG = process.env.DEBUG_CHAT === "1";
        if (chatInstance.tokenCounter && typeof chatInstance.tokenCounter.updateHistory === "function" && chatInstance.history) {
          chatInstance.tokenCounter.updateHistory(chatInstance.history);
          if (DEBUG) {
            console.log(`[DEBUG] Updated tokenCounter history with ${chatInstance.history.length} messages for token usage request`);
          }
        }
        const tokenUsage = chatInstance.getTokenUsage();
        if (DEBUG) {
          console.log(`[DEBUG] Token usage request - Context window size: ${tokenUsage.contextWindow}`);
          console.log(`[DEBUG] Token usage request - Cache metrics - Read: ${tokenUsage.current.cacheRead}, Write: ${tokenUsage.current.cacheWrite}`);
        }
        sendJson(res2, 200, tokenUsage);
      }, "GET /api/token-usage"),
      // Session history API endpoint for URL-based session restoration
      "GET /api/session/:sessionId/history": /* @__PURE__ */ __name(async (req2, res2) => {
        const sessionId2 = extractSessionIdFromHistoryPath(req2.url);
        if (!sessionId2) return sendError(res2, 400, "Missing sessionId in URL path");
        const DEBUG = process.env.DEBUG_CHAT === "1";
        if (DEBUG) {
          console.log(`[DEBUG] Fetching history for session: ${sessionId2}`);
        }
        try {
          const chatInstance = chatSessions.get(sessionId2);
          let history = [];
          let tokenUsage = null;
          let exists2 = false;
          if (chatInstance) {
            history = chatInstance.displayHistory || [];
            tokenUsage = chatInstance.getTokenUsage();
            exists2 = true;
          } else if (globalStorage) {
            const persistentHistory = await globalStorage.getSessionHistory(sessionId2);
            if (persistentHistory && persistentHistory.length > 0) {
              history = persistentHistory.map((msg) => ({
                role: msg.role,
                content: msg.content,
                timestamp: new Date(msg.timestamp).toISOString(),
                displayType: msg.display_type,
                visible: msg.visible,
                images: msg.images || []
              }));
              exists2 = true;
            }
          }
          sendJson(res2, 200, {
            history,
            tokenUsage,
            sessionId: sessionId2,
            exists: exists2,
            timestamp: (/* @__PURE__ */ new Date()).toISOString()
          });
        } catch (error) {
          console.error("Error fetching session history:", error);
          sendJson(res2, 200, {
            history: [],
            tokenUsage: null,
            sessionId: sessionId2,
            exists: false,
            timestamp: (/* @__PURE__ */ new Date()).toISOString()
          });
        }
      }, "GET /api/session/:sessionId/history"),
      // Sessions list endpoint for history dropdown
      "GET /api/sessions": /* @__PURE__ */ __name(async (req2, res2) => {
        const DEBUG = process.env.DEBUG_CHAT === "1";
        if (DEBUG) {
          console.log(`[DEBUG] Fetching sessions list`);
        }
        try {
          const sessions = [];
          const now = Date.now();
          const maxAge = 2 * 60 * 60 * 1e3;
          if (globalStorage) {
            const storedSessions = await globalStorage.listSessions(50);
            for (const session of storedSessions) {
              if (now - session.last_activity > maxAge) {
                continue;
              }
              let preview = session.first_message_preview;
              if (!preview) {
                const history = await globalStorage.getSessionHistory(session.id, 1);
                if (history.length > 0 && history[0].role === "user") {
                  const cleanContent = extractContentFromMessage(history[0].content);
                  preview = cleanContent.length > 100 ? cleanContent.substring(0, 100) + "..." : cleanContent;
                }
              }
              if (preview) {
                sessions.push({
                  sessionId: session.id,
                  preview,
                  messageCount: 0,
                  // We could calculate this but it's not critical
                  createdAt: new Date(session.created_at).toISOString(),
                  lastActivity: new Date(session.last_activity).toISOString(),
                  relativeTime: getRelativeTime(session.last_activity)
                });
              }
            }
          } else {
            for (const [sessionId2, chatInstance] of chatSessions.entries()) {
              if (!chatInstance.history || chatInstance.history.length === 0) {
                continue;
              }
              const createdAt = chatInstance.createdAt || now;
              const lastActivity = chatInstance.lastActivity || createdAt;
              if (now - lastActivity > maxAge) {
                continue;
              }
              const firstUserMessage = chatInstance.history.find((msg) => msg.role === "user");
              if (!firstUserMessage) {
                continue;
              }
              const cleanContent = extractContentFromMessage(firstUserMessage.content);
              const preview = cleanContent.length > 100 ? cleanContent.substring(0, 100) + "..." : cleanContent;
              sessions.push({
                sessionId: sessionId2,
                preview,
                messageCount: chatInstance.history.length,
                createdAt: new Date(createdAt).toISOString(),
                lastActivity: new Date(lastActivity).toISOString(),
                relativeTime: getRelativeTime(lastActivity)
              });
            }
          }
          if (DEBUG) {
            console.log(`[DEBUG] Returning ${sessions.length} sessions`);
          }
          sendJson(res2, 200, {
            sessions,
            total: sessions.length,
            timestamp: (/* @__PURE__ */ new Date()).toISOString()
          });
        } catch (error) {
          console.error("Error fetching sessions list:", error);
          sendJson(res2, 500, { error: "Failed to fetch sessions" });
        }
      }, "GET /api/sessions"),
      // Static file routes
      "GET /logo.png": /* @__PURE__ */ __name((req2, res2) => serveStatic(res2, join3(__dirname2, "logo.png"), "image/png"), "GET /logo.png"),
      // UI Routes
      "GET /": /* @__PURE__ */ __name((req2, res2) => {
        const htmlPath = join3(__dirname2, "index.html");
        serveHtml(res2, htmlPath, { "data-no-api-keys": noApiKeysMode ? "true" : "false" });
      }, "GET /"),
      // Chat session route - serves HTML with injected session ID
      "GET /chat/:sessionId": /* @__PURE__ */ __name((req2, res2) => {
        const sessionId2 = extractSessionIdFromPath(req2.url);
        if (!sessionId2) {
          return sendError(res2, 400, "Invalid session ID in URL");
        }
        if (!isValidUUID(sessionId2)) {
          return sendError(res2, 400, "Invalid session ID format");
        }
        const htmlPath = join3(__dirname2, "index.html");
        serveHtml(res2, htmlPath, {
          "data-no-api-keys": noApiKeysMode ? "true" : "false",
          "data-session-id": sessionId2
        });
      }, "GET /chat/:sessionId"),
      "GET /folders": /* @__PURE__ */ __name((req2, res2) => {
        const currentWorkingDir = process.cwd();
        const folders = staticAllowedFolders.length > 0 ? staticAllowedFolders : [currentWorkingDir];
        const currentDir = staticAllowedFolders.length > 0 ? staticAllowedFolders[0] : currentWorkingDir;
        sendJson(res2, 200, {
          folders,
          currentDir,
          noApiKeysMode
        });
      }, "GET /folders"),
      "GET /openapi.yaml": /* @__PURE__ */ __name((req2, res2) => serveStatic(res2, join3(__dirname2, "openapi.yaml"), "text/yaml"), "GET /openapi.yaml"),
      // SSE endpoint for tool calls - NO AUTH for easier client implementation
      "GET /api/tool-events": /* @__PURE__ */ __name((req2, res2) => {
        const DEBUG = process.env.DEBUG_CHAT === "1";
        const sessionId2 = getSessionIdFromUrl(req2);
        if (!sessionId2) {
          if (DEBUG) console.error(`[DEBUG] SSE: No sessionId found in URL: ${req2.url}`);
          return sendError(res2, 400, "Missing sessionId parameter");
        }
        if (DEBUG) console.log(`[DEBUG] SSE: Setting up connection for session: ${sessionId2}`);
        res2.writeHead(200, {
          "Content-Type": "text/event-stream",
          "Cache-Control": "no-cache",
          "Connection": "keep-alive",
          "Access-Control-Allow-Origin": "*"
          // Allow all origins for SSE
        });
        if (DEBUG) console.log(`[DEBUG] SSE: Headers set for session: ${sessionId2}`);
        const connectionData = { type: "connection", message: "SSE Connection Established", sessionId: sessionId2, timestamp: (/* @__PURE__ */ new Date()).toISOString() };
        sendSSEData(res2, connectionData, "connection");
        if (DEBUG) console.log(`[DEBUG] SSE: Sent connection event for session: ${sessionId2}`);
        const handleToolCall = /* @__PURE__ */ __name((toolCall) => {
          if (DEBUG) {
          }
          const chatInstance = chatSessions.get(sessionId2);
          if (chatInstance && toolCall.status === "completed") {
            const displayToolCall = {
              role: "toolCall",
              name: toolCall.name,
              args: toolCall.args || {},
              timestamp: toolCall.timestamp || (/* @__PURE__ */ new Date()).toISOString(),
              visible: true,
              displayType: "toolCall"
            };
            if (!chatInstance.displayHistory) {
              chatInstance.displayHistory = [];
            }
            chatInstance.displayHistory.push(displayToolCall);
            if (globalStorage) {
              globalStorage.saveMessage(sessionId2, {
                role: "toolCall",
                content: `Tool: ${toolCall.name}
Args: ${JSON.stringify(toolCall.args || {}, null, 2)}`,
                timestamp: toolCall.timestamp ? new Date(toolCall.timestamp).getTime() : Date.now(),
                displayType: "toolCall",
                visible: 1,
                metadata: {
                  name: toolCall.name,
                  args: toolCall.args || {}
                }
              }).catch((err) => {
                console.error("Failed to save tool call to persistent storage:", err);
              });
            }
            if (DEBUG) {
              console.log(`[DEBUG] Stored tool call in display history: ${toolCall.name}`);
            }
          }
          const serializableCall = {
            ...toolCall,
            timestamp: toolCall.timestamp || (/* @__PURE__ */ new Date()).toISOString(),
            _sse_sent_at: (/* @__PURE__ */ new Date()).toISOString()
          };
          sendSSEData(res2, serializableCall, "toolCall");
        }, "handleToolCall");
        const eventName = `toolCall:${sessionId2}`;
        const existingHandler = sseClients.get(sessionId2)?.handler;
        if (existingHandler) {
          toolCallEmitter.removeListener(eventName, existingHandler);
        }
        toolCallEmitter.on(eventName, handleToolCall);
        if (DEBUG) console.log(`[DEBUG] SSE: Registered listener for ${eventName}`);
        sseClients.set(sessionId2, { res: res2, handler: handleToolCall });
        if (DEBUG) console.log(`[DEBUG] SSE: Client added for session ${sessionId2}. Total clients: ${sseClients.size}`);
        req2.on("close", () => {
          if (DEBUG) console.log(`[DEBUG] SSE: Client disconnecting: ${sessionId2}`);
          toolCallEmitter.removeListener(eventName, handleToolCall);
          sseClients.delete(sessionId2);
          if (DEBUG) console.log(`[DEBUG] SSE: Client removed for session ${sessionId2}. Remaining clients: ${sseClients.size}`);
        });
      }, "GET /api/tool-events"),
      // Cancellation endpoint
      "POST /cancel-request": /* @__PURE__ */ __name(async (req2, res2) => {
        handlePostRequest(req2, res2, async (body) => {
          const { sessionId: sessionId2 } = body;
          if (!sessionId2) return sendError(res2, 400, "Missing required parameter: sessionId");
          const DEBUG = process.env.DEBUG_CHAT === "1";
          if (DEBUG) console.log(`
[DEBUG] ===== Cancel Request for Session: ${sessionId2} =====`);
          const toolExecutionsCancelled = cancelToolExecutions(sessionId2);
          const chatInstance = activeChatInstances.get(sessionId2);
          let chatInstanceAborted = false;
          if (chatInstance && typeof chatInstance.abort === "function") {
            try {
              chatInstance.abort();
              chatInstanceAborted = true;
              if (DEBUG) console.log(`[DEBUG] Aborted chat instance processing for session: ${sessionId2}`);
            } catch (error) {
              console.error(`Error aborting chat instance for session ${sessionId2}:`, error);
            }
          } else {
            if (DEBUG) console.log(`[DEBUG] No active chat instance found in map for session ${sessionId2} to abort.`);
          }
          const requestCancelled = cancelRequest(sessionId2);
          activeChatInstances.delete(sessionId2);
          console.log(`Cancellation processed for session ${sessionId2}: Tools=${toolExecutionsCancelled}, Chat=${chatInstanceAborted}, RequestTracking=${requestCancelled}`);
          sendJson(res2, 200, {
            success: true,
            message: "Cancellation request processed",
            details: { toolExecutionsCancelled, chatInstanceAborted, requestCancelled },
            timestamp: (/* @__PURE__ */ new Date()).toISOString()
          });
        });
      }, "POST /cancel-request"),
      // --- Direct API Tool Endpoints (Bypass LLM Loop) ---
      "POST /api/search": /* @__PURE__ */ __name(async (req2, res2) => {
        handlePostRequest(req2, res2, async (body) => {
          const { query, path: path5, allow_tests, maxResults, maxTokens, sessionId: reqSessionId } = body;
          if (!query) return sendError(res2, 400, "Missing required parameter: query");
          const sessionId2 = reqSessionId || randomUUID5();
          const toolParams = { query, path: path5, allow_tests, maxResults, maxTokens, sessionId: sessionId2 };
          await executeDirectTool(res2, directApiTools.search, "search", toolParams, sessionId2);
        });
      }, "POST /api/search"),
      "POST /api/query": /* @__PURE__ */ __name(async (req2, res2) => {
        handlePostRequest(req2, res2, async (body) => {
          const { pattern, path: path5, language, allow_tests, sessionId: reqSessionId } = body;
          if (!pattern) return sendError(res2, 400, "Missing required parameter: pattern");
          const sessionId2 = reqSessionId || randomUUID5();
          const toolParams = { pattern, path: path5, language, allow_tests, sessionId: sessionId2 };
          await executeDirectTool(res2, directApiTools.query, "query", toolParams, sessionId2);
        });
      }, "POST /api/query"),
      "POST /api/extract": /* @__PURE__ */ __name(async (req2, res2) => {
        handlePostRequest(req2, res2, async (body) => {
          const { file_path, line, end_line, allow_tests, context_lines, format, input_content, sessionId: reqSessionId } = body;
          if (!file_path && !input_content) return sendError(res2, 400, "Missing required parameter: file_path or input_content");
          const sessionId2 = reqSessionId || randomUUID5();
          const toolParams = { file_path, line, end_line, allow_tests, context_lines, format, input_content, sessionId: sessionId2 };
          await executeDirectTool(res2, directApiTools.extract, "extract", toolParams, sessionId2);
        });
      }, "POST /api/extract"),
      // Implement tool endpoint (only available if allowEdit is true)
      "POST /api/implement": /* @__PURE__ */ __name(async (req2, res2) => {
        if (!directApiTools.implement) {
          return sendError(res2, 403, "Implement tool is not enabled. Start server with --allow-edit to enable.");
        }
        handlePostRequest(req2, res2, async (body) => {
          const { task, sessionId: reqSessionId } = body;
          if (!task) return sendError(res2, 400, "Missing required parameter: task");
          const sessionId2 = reqSessionId || randomUUID5();
          const toolParams = { task, sessionId: sessionId2 };
          await executeDirectTool(res2, directApiTools.implement, "implement", toolParams, sessionId2);
        });
      }, "POST /api/implement"),
      // --- Main Chat Endpoint (Handles the Loop) ---
      "POST /chat": /* @__PURE__ */ __name((req2, res2) => {
        handlePostRequest(req2, res2, async (requestData) => {
          const {
            message,
            images = [],
            // Array of base64 image URLs
            sessionId: reqSessionId,
            clearHistory,
            apiProvider,
            apiKey,
            apiUrl
          } = requestData;
          const DEBUG = process.env.DEBUG_CHAT === "1";
          if (DEBUG) {
            console.log(`
[DEBUG] ===== UI Chat Request =====`);
            console.log(`[DEBUG] Request Data:`, { ...requestData, apiKey: requestData.apiKey ? "******" : void 0 });
          }
          const chatSessionId = reqSessionId || randomUUID5();
          if (!reqSessionId && DEBUG) console.log(`[DEBUG] No session ID from UI, generated: ${chatSessionId}`);
          else if (DEBUG) console.log(`[DEBUG] Using session ID from UI: ${chatSessionId}`);
          const apiCredentials = apiKey ? { apiProvider, apiKey, apiUrl } : null;
          const chatInstance = getOrCreateChat(chatSessionId, apiCredentials);
          chatInstance.lastActivity = Date.now();
          if (chatInstance.noApiKeysMode) {
            console.warn(`[WARN] Chat request for session ${chatSessionId} cannot proceed: No API keys configured.`);
            return sendError(res2, 503, "Chat service unavailable: API key not configured on server.");
          }
          registerRequest(chatSessionId, { abort: /* @__PURE__ */ __name(() => chatInstance.abort(), "abort") });
          if (DEBUG) console.log(`[DEBUG] Registered cancellable request for session: ${chatSessionId}`);
          activeChatInstances.set(chatSessionId, chatInstance);
          if (message === "__clear_history__" || clearHistory) {
            console.log(`Clearing chat history for session: ${chatSessionId}`);
            const newSessionId = chatInstance.clearHistory();
            clearRequest(chatSessionId);
            activeChatInstances.delete(chatSessionId);
            clearToolExecutionData(chatSessionId);
            chatSessions.delete(chatSessionId);
            const emptyTokenUsage = {
              contextWindow: 0,
              current: {
                request: 0,
                response: 0,
                total: 0,
                cacheRead: 0,
                cacheWrite: 0,
                cacheTotal: 0
              },
              total: {
                request: 0,
                response: 0,
                total: 0,
                cacheRead: 0,
                cacheWrite: 0,
                cacheTotal: 0
              }
            };
            sendJson(res2, 200, {
              response: "Chat history cleared",
              tokenUsage: emptyTokenUsage,
              // Include empty token usage data
              newSessionId,
              // Inform UI about the new ID
              timestamp: (/* @__PURE__ */ new Date()).toISOString()
            });
            return;
          }
          try {
            const apiCredentials2 = apiKey ? { apiProvider, apiKey, apiUrl } : null;
            const result = await chatInstance.chat(message, chatSessionId, apiCredentials2, images);
            let responseText;
            let tokenUsage;
            if (result && typeof result === "object" && "response" in result) {
              responseText = result.response;
              tokenUsage = result.tokenUsage;
              if (process.env.DEBUG_CHAT === "1") {
                console.log(`[DEBUG] Received structured response with token usage data`);
                console.log(`[DEBUG] Context window size: ${tokenUsage.contextWindow}`);
                console.log(`[DEBUG] Cache metrics - Read: ${tokenUsage.current.cacheRead}, Write: ${tokenUsage.current.cacheWrite}`);
              }
            } else {
              responseText = result;
              tokenUsage = chatInstance.getTokenUsage();
              if (process.env.DEBUG_CHAT === "1") {
                console.log(`[DEBUG] Received legacy response format, fetched token usage separately`);
              }
            }
            const responseObject = {
              response: responseText,
              tokenUsage,
              sessionId: chatSessionId,
              timestamp: (/* @__PURE__ */ new Date()).toISOString()
            };
            sendJson(res2, 200, responseObject, { "X-Token-Usage": JSON.stringify(tokenUsage) });
            console.log(`Finished chat request for session: ${chatSessionId}`);
          } catch (error) {
            let errorResponse = error;
            let tokenUsage;
            if (error && typeof error === "object" && error.response && error.tokenUsage) {
              errorResponse = error.response;
              tokenUsage = error.tokenUsage;
              if (process.env.DEBUG_CHAT === "1") {
                console.log(`[DEBUG] Received structured error response with token usage data`);
                console.log(`[DEBUG] Context window size: ${tokenUsage.contextWindow}`);
                console.log(`[DEBUG] Cache metrics - Read: ${tokenUsage.current.cacheRead}, Write: ${tokenUsage.current.cacheWrite}`);
              }
            } else {
              if (chatInstance.tokenCounter && typeof chatInstance.tokenCounter.updateHistory === "function" && chatInstance.history) {
                chatInstance.tokenCounter.updateHistory(chatInstance.history);
                if (DEBUG) {
                  console.log(`[DEBUG] Updated tokenCounter history with ${chatInstance.history.length} messages for error case`);
                }
              }
              if (chatInstance.tokenCounter && typeof chatInstance.tokenCounter.calculateContextSize === "function") {
                chatInstance.tokenCounter.calculateContextSize(chatInstance.history);
                if (DEBUG) {
                  console.log(`[DEBUG] Forced recalculation of context window size for error case`);
                }
              }
              tokenUsage = chatInstance.getTokenUsage();
              if (DEBUG) {
                console.log(`[DEBUG] Error case - Final context window size: ${tokenUsage.contextWindow}`);
                console.log(`[DEBUG] Error case - Cache metrics - Read: ${tokenUsage.current.cacheRead}, Write: ${tokenUsage.current.cacheWrite}`);
              }
            }
            if (errorResponse.message && errorResponse.message.includes("cancelled") || typeof errorResponse === "string" && errorResponse.includes("cancelled")) {
              console.log(`Chat request processing was cancelled for session: ${chatSessionId}`);
              sendJson(res2, 499, {
                error: "Request cancelled by user",
                tokenUsage,
                sessionId: chatSessionId,
                timestamp: (/* @__PURE__ */ new Date()).toISOString()
              });
            } else {
              console.error(`Error processing chat for session ${chatSessionId}:`, error);
              sendJson(res2, 500, {
                error: `Chat processing error: ${typeof errorResponse === "string" ? errorResponse : errorResponse.message || "Unknown error"}`,
                tokenUsage,
                sessionId: chatSessionId,
                timestamp: (/* @__PURE__ */ new Date()).toISOString()
              });
            }
          } finally {
            clearRequest(chatSessionId);
            activeChatInstances.delete(chatSessionId);
            if (DEBUG) console.log(`[DEBUG] Cleaned up active request tracking for session: ${chatSessionId}`);
          }
        });
      }, "POST /chat")
      // End /chat route
    };
    const parsedUrl = new URL(req.url, `http://${req.headers.host}`);
    const routeKey = `${req.method} ${parsedUrl.pathname}`;
    let handler = routes[routeKey];
    if (!handler) {
      if (req.method === "GET" && parsedUrl.pathname.match(/^\/chat\/[^/?]+$/)) {
        handler = routes["GET /chat/:sessionId"];
      } else if (req.method === "GET" && parsedUrl.pathname.match(/^\/api\/session\/[^/?]+\/history$/)) {
        handler = routes["GET /api/session/:sessionId/history"];
      }
    }
    if (handler) {
      const publicRoutes = ["GET /openapi.yaml", "GET /api/tool-events", "GET /logo.png", "GET /", "GET /folders", "OPTIONS"];
      const isPublicRoute = publicRoutes.includes(routeKey) || req.method === "OPTIONS" || parsedUrl.pathname.match(/^\/chat\/[^/?]+$/) || // Chat sessions are public
      parsedUrl.pathname.match(/^\/api\/session\/[^/?]+\/history$/);
      if (isPublicRoute) {
        handler(req, res);
      } else {
        processRequest(handler);
      }
    } else {
      sendError(res, 404, "Not Found");
    }
  });
  const PORT = process.env.PORT || 8080;
  server.listen(PORT, () => {
    console.log(`Probe Web Interface v${version}`);
    console.log(`Server running on http://localhost:${PORT}`);
    console.log(`Environment: ${"development"}`);
    if (noApiKeysMode) {
      console.log("*** Running in NO API KEYS mode. Chat functionality disabled. ***");
    }
  });
}
function handleOptions(res) {
  res.writeHead(200, {
    "Access-Control-Allow-Origin": "*",
    // Or specific origin
    "Access-Control-Allow-Methods": "GET, POST, OPTIONS",
    "Access-Control-Allow-Headers": "Content-Type, Authorization, X-Session-ID",
    // Add any custom headers needed
    "Access-Control-Max-Age": "86400"
    // 24 hours
  });
  res.end();
}
function sendJson(res, statusCode, data, headers = {}) {
  if (res.headersSent) return;
  res.writeHead(statusCode, {
    "Content-Type": "application/json",
    "Access-Control-Allow-Origin": "*",
    // Adjust as needed
    "Access-Control-Expose-Headers": "X-Token-Usage",
    // Expose custom headers
    ...headers
  });
  res.end(JSON.stringify(data));
}
function sendError(res, statusCode, message) {
  if (res.headersSent) return;
  console.error(`Sending error (${statusCode}): ${message}`);
  res.writeHead(statusCode, {
    "Content-Type": "application/json",
    "Access-Control-Allow-Origin": "*"
  });
  res.end(JSON.stringify({ error: message, status: statusCode }));
}
function serveStatic(res, filePath, contentType) {
  if (res.headersSent) return;
  if (existsSync4(filePath)) {
    res.writeHead(200, { "Content-Type": contentType });
    const fileData = readFileSync2(filePath);
    res.end(fileData);
  } else {
    sendError(res, 404, `${contentType} not found`);
  }
}
function serveHtml(res, filePath, bodyAttributes = {}) {
  if (res.headersSent) return;
  if (existsSync4(filePath)) {
    res.writeHead(200, { "Content-Type": "text/html" });
    let html = readFileSync2(filePath, "utf8");
    const attributesString = Object.entries(bodyAttributes).map(([key, value]) => `${key}="${String(value).replace(/"/g, '"')}"`).join(" ");
    if (attributesString) {
      html = html.replace("<body", `<body ${attributesString}`);
    }
    res.end(html);
  } else {
    sendError(res, 404, "HTML file not found");
  }
}
function getSessionIdFromUrl(req) {
  try {
    const url = new URL(req.url, `http://${req.headers.host}`);
    return url.searchParams.get("sessionId");
  } catch (error) {
    console.error(`Error parsing URL for sessionId: ${error.message}`);
    const match = req.url.match(/[?&]sessionId=([^&]+)/);
    return match ? match[1] : null;
  }
}
async function handlePostRequest(req, res, callback) {
  let body = "";
  req.on("data", (chunk) => body += chunk);
  req.on("end", async () => {
    try {
      const parsedBody = JSON.parse(body);
      await callback(parsedBody);
    } catch (error) {
      if (error instanceof SyntaxError) {
        sendError(res, 400, "Invalid JSON in request body");
      } else {
        console.error("Error handling POST request:", error);
        sendError(res, 500, `Internal Server Error: ${error.message}`);
      }
    }
  });
  req.on("error", (err) => {
    console.error("Request error:", err);
    sendError(res, 500, "Request error");
  });
}
async function executeDirectTool(res, toolInstance, toolName, toolParams, sessionId2) {
  const DEBUG = process.env.DEBUG_CHAT === "1";
  if (DEBUG) {
    console.log(`
[DEBUG] ===== Direct API Tool Call: ${toolName} =====`);
    console.log(`[DEBUG] Session ID: ${sessionId2}`);
    console.log(`[DEBUG] Params:`, toolParams);
  }
  try {
    const result = await toolInstance.execute(toolParams);
    sendJson(res, 200, { results: result, timestamp: (/* @__PURE__ */ new Date()).toISOString() });
  } catch (error) {
    console.error(`Error executing direct tool ${toolName}:`, error);
    let statusCode = 500;
    let errorMessage = `Error executing ${toolName}`;
    if (error.message.includes("cancelled")) {
      statusCode = 499;
      errorMessage = "Operation cancelled";
    } else if (error.code === "ENOENT") {
      statusCode = 404;
      errorMessage = "File or path not found";
    } else if (error.code === "EACCES") {
      statusCode = 403;
      errorMessage = "Permission denied";
    }
    sendError(res, statusCode, `${errorMessage}: ${error.message}`);
  }
}
function extractSessionIdFromPath(url) {
  const match = url.match(/^\/chat\/([^/?]+)/);
  return match ? match[1] : null;
}
function isValidUUID(str) {
  const uuidRegex = /^[0-9a-f]{8}-?[0-9a-f]{4}-?[0-9a-f]{4}-?[0-9a-f]{4}-?[0-9a-f]{12}$/i;
  return uuidRegex.test(str);
}
function extractSessionIdFromHistoryPath(url) {
  const match = url.match(/^\/api\/session\/([^/?]+)\/history/);
  return match ? match[1] : null;
}
function extractContentFromMessage(content) {
  const patterns = [
    /<task>([\s\S]*?)<\/task>/,
    /<attempt_completion>\s*<result>([\s\S]*?)<\/result>\s*<\/attempt_completion>/,
    /<result>([\s\S]*?)<\/result>/
  ];
  for (const pattern of patterns) {
    const match = content.match(pattern);
    if (match) {
      return match[1].trim();
    }
  }
  return content.trim();
}
function getRelativeTime(timestamp) {
  const now = Date.now();
  const diff = now - timestamp;
  const seconds = Math.floor(diff / 1e3);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);
  if (days > 0) return `${days}d ago`;
  if (hours > 0) return `${hours}h ago`;
  if (minutes > 0) return `${minutes}m ago`;
  return "just now";
}
var __dirname2, globalStorage, chatSessions;
var init_webServer = __esm({
  "webServer.js"() {
    init_probeChat();
    init_tokenUsageDisplay();
    init_auth();
    init_probeTool();
    init_cancelRequest();
    init_JsonChatStorage();
    __dirname2 = dirname2(fileURLToPath2(import.meta.url));
    globalStorage = null;
    chatSessions = /* @__PURE__ */ new Map();
    __name(getOrCreateChat, "getOrCreateChat");
    __name(startWebServer, "startWebServer");
    __name(handleOptions, "handleOptions");
    __name(sendJson, "sendJson");
    __name(sendError, "sendError");
    __name(serveStatic, "serveStatic");
    __name(serveHtml, "serveHtml");
    __name(getSessionIdFromUrl, "getSessionIdFromUrl");
    __name(handlePostRequest, "handlePostRequest");
    __name(executeDirectTool, "executeDirectTool");
    __name(extractSessionIdFromPath, "extractSessionIdFromPath");
    __name(isValidUUID, "isValidUUID");
    __name(extractSessionIdFromHistoryPath, "extractSessionIdFromHistoryPath");
    __name(extractContentFromMessage, "extractContentFromMessage");
    __name(getRelativeTime, "getRelativeTime");
  }
});

// index.js
import "dotenv/config";
import inquirer from "inquirer";
import chalk2 from "chalk";
import ora from "ora";
import { Command } from "commander";
import { existsSync as existsSync5, realpathSync, readFileSync as readFileSync3 } from "fs";
import { resolve as resolve2, dirname as dirname3, join as join4 } from "path";
import { fileURLToPath as fileURLToPath3 } from "url";
init_probeChat();
init_tokenUsageDisplay();
import { DEFAULT_SYSTEM_MESSAGE as DEFAULT_SYSTEM_MESSAGE3 } from "@buger/probe";
if (process.argv.includes("-m") || process.argv.includes("--message")) {
  process.env.PROBE_NON_INTERACTIVE = "1";
}
if (!process.stdin.isTTY) {
  process.env.PROBE_NON_INTERACTIVE = "1";
  process.env.PROBE_STDIN_PIPED = "1";
}
function main() {
  const __dirname3 = dirname3(fileURLToPath3(import.meta.url));
  const packageJsonPath = join4(__dirname3, "package.json");
  let version = "1.0.0";
  try {
    const packageJson = JSON.parse(readFileSync3(packageJsonPath, "utf8"));
    version = packageJson.version || version;
  } catch (error) {
  }
  const program = new Command();
  program.name("probe-chat").description("CLI chat interface for Probe code search").version(version).option("-d, --debug", "Enable debug mode").option("--model-name <model>", "Specify the model to use").option("-f, --force-provider <provider>", "Force a specific provider (options: anthropic, openai, google)").option("-w, --web", "Run in web interface mode").option("-p, --port <port>", "Port to run web server on (default: 8080)").option("-m, --message <message>", "Send a single message and exit (non-interactive mode)").option("-s, --session-id <sessionId>", "Specify a session ID for the chat (optional)").option("--json", "Output the response as JSON in non-interactive mode").option("--max-iterations <number>", "Maximum number of tool iterations allowed (default: 30)").option("--prompt <value>", "Use a custom prompt (values: architect, code-review, support, path to a file, or arbitrary string)").option("--allow-edit", "Enable the implement tool for editing files").option("--implement-tool-backend <backend>", "Choose implementation tool backend (aider, claude-code)").option("--implement-tool-timeout <ms>", "Implementation tool timeout in milliseconds").option("--implement-tool-config <path>", "Path to implementation tool configuration file").option("--implement-tool-list-backends", "List available implementation tool backends").option("--implement-tool-backend-info <backend>", "Show information about a specific implementation tool backend").option("--trace-file [path]", "Enable tracing to file (default: ./traces.jsonl)").option("--trace-remote [endpoint]", "Enable tracing to remote endpoint (default: http://localhost:4318/v1/traces)").option("--trace-console", "Enable tracing to console (for debugging)").argument("[path]", "Path to the codebase to search (overrides ALLOWED_FOLDERS)").parse(process.argv);
  const options = program.opts();
  const pathArg = program.args[0];
  const isPipedInput = process.env.PROBE_STDIN_PIPED === "1";
  const isNonInteractive = !!options.message || isPipedInput;
  if (isNonInteractive && process.env.PROBE_NON_INTERACTIVE !== "1") {
    process.env.PROBE_NON_INTERACTIVE = "1";
  }
  const rawLog = /* @__PURE__ */ __name((...args) => console.log(...args), "rawLog");
  const rawError = /* @__PURE__ */ __name((...args) => console.error(...args), "rawError");
  if (isNonInteractive && !options.json && !options.debug) {
    chalk2.level = 0;
  }
  const logInfo = /* @__PURE__ */ __name((...args) => {
    if (!isNonInteractive || options.debug) {
      console.log(...args);
    }
  }, "logInfo");
  const logWarn = /* @__PURE__ */ __name((...args) => {
    if (!isNonInteractive || options.debug) {
      console.warn(...args);
    } else if (isNonInteractive) {
    }
  }, "logWarn");
  const logError = /* @__PURE__ */ __name((...args) => {
    if (isNonInteractive) {
      rawError("Error:", ...args);
    } else {
      console.error(...args);
    }
  }, "logError");
  if (options.debug) {
    process.env.DEBUG_CHAT = "1";
    logInfo(chalk2.yellow("Debug mode enabled"));
  }
  if (options.modelName) {
    process.env.MODEL_NAME = options.modelName;
    logInfo(chalk2.blue(`Using model: ${options.modelName}`));
  }
  if (options.forceProvider) {
    const provider = options.forceProvider.toLowerCase();
    if (!["anthropic", "openai", "google"].includes(provider)) {
      logError(chalk2.red(`Invalid provider "${provider}". Must be one of: anthropic, openai, google`));
      process.exit(1);
    }
    process.env.FORCE_PROVIDER = provider;
    logInfo(chalk2.blue(`Forcing provider: ${provider}`));
  }
  if (options.maxIterations) {
    const maxIterations = parseInt(options.maxIterations, 10);
    if (isNaN(maxIterations) || maxIterations <= 0) {
      logError(chalk2.red(`Invalid max iterations value: ${options.maxIterations}. Must be a positive number.`));
      process.exit(1);
    }
    process.env.MAX_TOOL_ITERATIONS = maxIterations.toString();
    logInfo(chalk2.blue(`Setting maximum tool iterations to: ${maxIterations}`));
  }
  if (options.implementToolListBackends) {
    (async () => {
      const { listBackendNames: listBackendNames2, getBackendMetadata: getBackendMetadata2 } = await Promise.resolve().then(() => (init_registry(), registry_exports));
      const backends = listBackendNames2();
      console.log("\nAvailable implementation tool backends:");
      for (const backend of backends) {
        const metadata = getBackendMetadata2(backend);
        console.log(`
  ${chalk2.bold(backend)} - ${metadata.description}`);
        console.log(`    Version: ${metadata.version}`);
        console.log(`    Languages: ${metadata.capabilities.supportsLanguages.join(", ")}`);
      }
      process.exit(0);
    })();
  }
  if (options.implementToolBackendInfo) {
    (async () => {
      const { getBackendMetadata: getBackendMetadata2 } = await Promise.resolve().then(() => (init_registry(), registry_exports));
      const metadata = getBackendMetadata2(options.implementToolBackendInfo);
      if (!metadata) {
        console.error(`Backend '${options.implementToolBackendInfo}' not found`);
        process.exit(1);
      }
      console.log(`
${chalk2.bold("Backend Information: " + options.implementToolBackendInfo)}`);
      console.log(`
Description: ${metadata.description}`);
      console.log(`Version: ${metadata.version}`);
      console.log(`
Capabilities:`);
      console.log(`  Languages: ${metadata.capabilities.supportsLanguages.join(", ")}`);
      console.log(`  Streaming: ${metadata.capabilities.supportsStreaming ? "\u2713" : "\u2717"}`);
      console.log(`  Direct File Edit: ${metadata.capabilities.supportsDirectFileEdit ? "\u2713" : "\u2717"}`);
      console.log(`  Test Generation: ${metadata.capabilities.supportsTestGeneration ? "\u2713" : "\u2717"}`);
      console.log(`  Plan Generation: ${metadata.capabilities.supportsPlanGeneration ? "\u2713" : "\u2717"}`);
      console.log(`  Max Sessions: ${metadata.capabilities.maxConcurrentSessions}`);
      console.log(`
Required Dependencies:`);
      for (const dep of metadata.dependencies) {
        console.log(`  - ${dep.name} (${dep.type}): ${dep.description}`);
        if (dep.installCommand) {
          console.log(`    Install: ${dep.installCommand}`);
        }
      }
      process.exit(0);
    })();
  }
  if (options.allowEdit) {
    process.env.ALLOW_EDIT = "1";
    logInfo(chalk2.blue(`Enabling implement tool with --allow-edit flag`));
  }
  if (options.implementToolBackend) {
    process.env.IMPLEMENT_TOOL_BACKEND = options.implementToolBackend;
    logInfo(chalk2.blue(`Using implementation tool backend: ${options.implementToolBackend}`));
  }
  if (options.implementToolTimeout) {
    process.env.IMPLEMENT_TOOL_TIMEOUT = options.implementToolTimeout;
    logInfo(chalk2.blue(`Implementation tool timeout: ${options.implementToolTimeout}ms`));
  }
  if (options.implementToolConfig) {
    process.env.IMPLEMENT_TOOL_CONFIG_PATH = options.implementToolConfig;
    logInfo(chalk2.blue(`Using implementation tool config: ${options.implementToolConfig}`));
  }
  if (options.traceFile !== void 0) {
    process.env.OTEL_ENABLE_FILE = "true";
    process.env.OTEL_FILE_PATH = options.traceFile || "./traces.jsonl";
    logInfo(chalk2.blue(`Enabling file tracing to: ${process.env.OTEL_FILE_PATH}`));
  }
  if (options.traceRemote !== void 0) {
    process.env.OTEL_ENABLE_REMOTE = "true";
    process.env.OTEL_EXPORTER_OTLP_TRACES_ENDPOINT = options.traceRemote || "http://localhost:4318/v1/traces";
    logInfo(chalk2.blue(`Enabling remote tracing to: ${process.env.OTEL_EXPORTER_OTLP_TRACES_ENDPOINT}`));
  }
  if (options.traceConsole) {
    process.env.OTEL_ENABLE_CONSOLE = "true";
    logInfo(chalk2.blue(`Enabling console tracing`));
  }
  let customPrompt = null;
  if (options.prompt) {
    const predefinedPrompts = ["architect", "code-review", "support", "engineer"];
    if (predefinedPrompts.includes(options.prompt)) {
      process.env.PROMPT_TYPE = options.prompt;
      logInfo(chalk2.blue(`Using predefined prompt: ${options.prompt}`));
    } else {
      try {
        const promptPath = resolve2(options.prompt);
        if (existsSync5(promptPath)) {
          customPrompt = readFileSync3(promptPath, "utf8");
          process.env.CUSTOM_PROMPT = customPrompt;
          logInfo(chalk2.blue(`Loaded custom prompt from file: ${promptPath}`));
        } else {
          customPrompt = options.prompt;
          process.env.CUSTOM_PROMPT = customPrompt;
          logInfo(chalk2.blue(`Using custom prompt string`));
        }
      } catch (error) {
        customPrompt = options.prompt;
        process.env.CUSTOM_PROMPT = customPrompt;
        logInfo(chalk2.blue(`Using custom prompt string`));
      }
    }
  }
  const allowedFolders2 = process.env.ALLOWED_FOLDERS ? process.env.ALLOWED_FOLDERS.split(",").map((folder) => folder.trim()).filter(Boolean) : [];
  if (pathArg) {
    const resolvedPath = resolve2(pathArg);
    if (existsSync5(resolvedPath)) {
      const realPath = realpathSync(resolvedPath);
      process.env.ALLOWED_FOLDERS = realPath;
      logInfo(chalk2.blue(`Using codebase path: ${realPath}`));
      allowedFolders2.length = 0;
      allowedFolders2.push(realPath);
    } else {
      logError(chalk2.red(`Path does not exist: ${resolvedPath}`));
      process.exit(1);
    }
  } else {
    logInfo("Configured search folders:");
    for (const folder of allowedFolders2) {
      const exists2 = existsSync5(folder);
      logInfo(`- ${folder} ${exists2 ? "\u2713" : "\u2717 (not found)"}`);
      if (!exists2) {
        logWarn(chalk2.yellow(`Warning: Folder "${folder}" does not exist or is not accessible`));
      }
    }
    if (allowedFolders2.length === 0 && !isNonInteractive) {
      logWarn(chalk2.yellow("No folders configured. Set ALLOWED_FOLDERS in .env file or provide a path argument."));
    }
  }
  if (options.port) {
    process.env.PORT = options.port;
  }
  const anthropicApiKey = process.env.ANTHROPIC_API_KEY;
  const openaiApiKey = process.env.OPENAI_API_KEY;
  const googleApiKey = process.env.GOOGLE_API_KEY;
  const hasApiKeys = !!(anthropicApiKey || openaiApiKey || googleApiKey);
  if (isNonInteractive) {
    if (!hasApiKeys) {
      logError(chalk2.red("No API key provided. Please set ANTHROPIC_API_KEY, OPENAI_API_KEY, or GOOGLE_API_KEY environment variable."));
      process.exit(1);
    }
    let chat2;
    try {
      chat2 = new ProbeChat({
        sessionId: options.sessionId,
        isNonInteractive: true,
        customPrompt,
        promptType: options.prompt && ["architect", "code-review", "support", "engineer"].includes(options.prompt) ? options.prompt : null,
        allowEdit: options.allowEdit
      });
      logInfo(chalk2.blue(`Using Session ID: ${chat2.getSessionId()}`));
    } catch (error) {
      logError(chalk2.red(`Initializing chat failed: ${error.message}`));
      process.exit(1);
    }
    const readFromStdin = /* @__PURE__ */ __name(() => {
      return new Promise((resolve3) => {
        let data = "";
        process.stdin.on("data", (chunk) => {
          data += chunk;
        });
        process.stdin.on("end", () => {
          resolve3(data.trim());
        });
      });
    }, "readFromStdin");
    const runNonInteractiveChat = /* @__PURE__ */ __name(async () => {
      try {
        let message = options.message;
        if (!message && isPipedInput) {
          logInfo("Reading message from stdin...");
          message = await readFromStdin();
        }
        if (!message) {
          logError("No message provided. Use --message option or pipe input to stdin.");
          process.exit(1);
        }
        logInfo("Sending message...");
        const result = await chat2.chat(message, chat2.getSessionId());
        if (result && typeof result === "object" && result.response !== void 0) {
          if (options.json) {
            const outputData = {
              response: result.response,
              sessionId: chat2.getSessionId(),
              tokenUsage: result.tokenUsage || null
              // Include usage if available
            };
            rawLog(JSON.stringify(outputData, null, 2));
          } else {
            rawLog(result.response);
          }
          process.exit(0);
        } else if (typeof result === "string") {
          if (options.json) {
            rawLog(JSON.stringify({ response: result, sessionId: chat2.getSessionId(), tokenUsage: null }, null, 2));
          } else {
            rawLog(result);
          }
          process.exit(0);
        } else {
          logError("Received an unexpected or empty response structure from chat.");
          if (options.json) {
            rawError(JSON.stringify({ error: "Unexpected response structure", response: result, sessionId: chat2.getSessionId() }, null, 2));
          }
          process.exit(1);
        }
      } catch (error) {
        logError(`Chat request failed: ${error.message}`);
        if (options.json) {
          rawError(JSON.stringify({ error: error.message, sessionId: chat2.getSessionId() }, null, 2));
        }
        process.exit(1);
      }
    }, "runNonInteractiveChat");
    runNonInteractiveChat();
    return;
  }
  if (options.web) {
    if (!hasApiKeys) {
      logWarn(chalk2.yellow("Warning: No API key provided. The web interface will show instructions on how to set up API keys."));
    }
    Promise.resolve().then(() => (init_webServer(), webServer_exports)).then((module) => {
      const { startWebServer: startWebServer2 } = module;
      logInfo(`Starting web server on port ${process.env.PORT || 8080}...`);
      startWebServer2(version, hasApiKeys, { allowEdit: options.allowEdit });
    }).catch((error) => {
      logError(chalk2.red(`Error starting web server: ${error.message}`));
      process.exit(1);
    });
    return;
  }
  if (!hasApiKeys) {
    logError(chalk2.red("No API key provided. Please set ANTHROPIC_API_KEY, OPENAI_API_KEY, or GOOGLE_API_KEY environment variable."));
    console.log(chalk2.cyan("You can find these instructions in the .env.example file:"));
    console.log(chalk2.cyan("1. Create a .env file by copying .env.example"));
    console.log(chalk2.cyan("2. Add your API key to the .env file"));
    console.log(chalk2.cyan("3. Restart the application"));
    process.exit(1);
  }
  let chat;
  try {
    chat = new ProbeChat({
      sessionId: options.sessionId,
      isNonInteractive: false,
      customPrompt,
      promptType: options.prompt && ["architect", "code-review", "support", "engineer"].includes(options.prompt) ? options.prompt : null,
      allowEdit: options.allowEdit
    });
    if (chat.apiType === "anthropic") {
      logInfo(chalk2.green(`Using Anthropic API with model: ${chat.model}`));
    } else if (chat.apiType === "openai") {
      logInfo(chalk2.green(`Using OpenAI API with model: ${chat.model}`));
    } else if (chat.apiType === "google") {
      logInfo(chalk2.green(`Using Google API with model: ${chat.model}`));
    }
    logInfo(chalk2.blue(`Session ID: ${chat.getSessionId()}`));
    logInfo(chalk2.cyan('Type "exit" or "quit" to end the chat'));
    logInfo(chalk2.cyan('Type "usage" to see token usage statistics'));
    logInfo(chalk2.cyan('Type "clear" to clear the chat history'));
    logInfo(chalk2.cyan("-------------------------------------------"));
  } catch (error) {
    logError(chalk2.red(`Error initializing chat: ${error.message}`));
    process.exit(1);
  }
  function formatResponseInteractive(response) {
    let textResponse = "";
    if (response && typeof response === "object" && "response" in response) {
      textResponse = response.response;
    } else if (typeof response === "string") {
      textResponse = response;
    } else {
      return chalk2.red("[Error: Invalid response format]");
    }
    return textResponse.replace(
      /<tool_call>(.*?)<\/tool_call>/gs,
      (match, toolCall) => chalk2.magenta(`[Tool Call] ${toolCall}`)
    );
  }
  __name(formatResponseInteractive, "formatResponseInteractive");
  async function startChat() {
    while (true) {
      const { message } = await inquirer.prompt([
        {
          type: "input",
          name: "message",
          message: chalk2.blue("You:"),
          prefix: ""
        }
      ]);
      if (message.toLowerCase() === "exit" || message.toLowerCase() === "quit") {
        logInfo(chalk2.yellow("Goodbye!"));
        break;
      } else if (message.toLowerCase() === "usage") {
        const usage = chat.getTokenUsage();
        const display = new TokenUsageDisplay();
        const formatted = display.format(usage);
        logInfo(chalk2.blue("Current:", formatted.current.total));
        logInfo(chalk2.blue("Context:", formatted.contextWindow));
        logInfo(chalk2.blue(
          "Cache:",
          `Read: ${formatted.current.cache.read},`,
          `Write: ${formatted.current.cache.write},`,
          `Total: ${formatted.current.cache.total}`
        ));
        logInfo(chalk2.blue("Total:", formatted.total.total));
        process.stdout.write("\x1B]0;Context: " + formatted.contextWindow + "\x07");
        continue;
      } else if (message.toLowerCase() === "clear") {
        const newSessionId = chat.clearHistory();
        logInfo(chalk2.yellow("Chat history cleared"));
        logInfo(chalk2.blue(`New session ID: ${newSessionId}`));
        continue;
      }
      const spinner = ora("Thinking...").start();
      try {
        const result = await chat.chat(message);
        spinner.stop();
        logInfo(chalk2.green("Assistant:"));
        console.log(formatResponseInteractive(result));
        console.log();
        if (result && typeof result === "object" && result.tokenUsage && result.tokenUsage.contextWindow) {
          process.stdout.write("\x1B]0;Context: " + result.tokenUsage.contextWindow + "\x07");
        }
      } catch (error) {
        spinner.stop();
        logError(chalk2.red(`Error: ${error.message}`));
      }
    }
  }
  __name(startChat, "startChat");
  startChat().catch((error) => {
    logError(chalk2.red(`Fatal error in interactive chat: ${error.message}`));
    process.exit(1);
  });
}
__name(main, "main");
if (import.meta.url.startsWith("file:") && process.argv[1] === fileURLToPath3(import.meta.url)) {
  main();
}
export {
  main
};
//# sourceMappingURL=index.js.map
