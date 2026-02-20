/**
 * Tool Environment Generator
 *
 * Reads Zod schemas (native tools) and MCP tool schemas to generate:
 * 1. Sandbox globals object (function bindings that bridge to real tools)
 * 2. Set of async function names (for the AST transformer)
 */

import {
  searchSchema,
  searchAllSchema,
  querySchema,
  extractSchema,
  bashSchema,
} from '../../tools/common.js';

// Map of native tool names to their Zod schemas
const NATIVE_TOOL_SCHEMAS = {
  search: searchSchema,
  searchAll: searchAllSchema,
  query: querySchema,
  extract: extractSchema,
  bash: bashSchema,
};

// Tools that are inherently async (make network/LLM calls)
const ALWAYS_ASYNC = new Set([
  'search', 'searchAll', 'query', 'extract', 'listFiles', 'searchFiles', 'bash',
  'LLM', 'map',
]);

/**
 * Generate the set of async function names from native tools and MCP tools.
 *
 * @param {Object} [mcpTools={}] - MCP tools keyed by name
 * @returns {Set<string>} Names of all async functions available in the DSL
 */
export function getAsyncFunctionNames(mcpTools = {}) {
  const names = new Set(ALWAYS_ASYNC);
  // All MCP tools are async
  for (const name of Object.keys(mcpTools)) {
    names.add(name);
  }
  return names;
}

/**
 * Wrap a tool function with OTEL tracing and error-safe return.
 * On error, returns "ERROR: <message>" instead of throwing — SandboxJS
 * has unreliable try/catch for async errors, so tools never throw.
 *
 * @param {string} toolName - Name of the tool for the span
 * @param {Function} fn - The async tool function to wrap
 * @param {Object|null} tracer - SimpleAppTracer instance (or null)
 * @param {Function} logFn - Function to write to execution logs
 * @returns {Function} Wrapped function
 */
function traceToolCall(toolName, fn, tracer, logFn) {
  if (!tracer) {
    return async (...args) => {
      try {
        return await fn(...args);
      } catch (e) {
        const msg = 'ERROR: ' + (e.message || String(e));
        logFn?.('[' + toolName + '] ' + msg);
        return msg;
      }
    };
  }

  return async (...args) => {
    const span = tracer.createToolSpan?.(`dsl.${toolName}`, {
      'dsl.tool': toolName,
      'dsl.params': JSON.stringify(args).substring(0, 500),
    });

    const startTime = Date.now();
    try {
      const result = await fn(...args);
      const elapsed = Date.now() - startTime;

      const resultStr = typeof result === 'string' ? result : JSON.stringify(result);
      span?.setAttributes?.({
        'dsl.tool.duration_ms': elapsed,
        'dsl.tool.result_length': resultStr?.length || 0,
        'dsl.tool.success': true,
      });
      span?.setStatus?.('OK');
      span?.end?.();

      tracer.recordToolResult?.(
        `dsl.${toolName}`, result, true, elapsed,
        { 'dsl.context': 'execute_plan' }
      );

      return result;
    } catch (e) {
      const elapsed = Date.now() - startTime;
      span?.setAttributes?.({
        'dsl.tool.duration_ms': elapsed,
        'dsl.tool.success': false,
        'dsl.tool.error': e.message?.substring(0, 500),
      });
      span?.setStatus?.('ERROR');
      span?.addEvent?.('exception', {
        'exception.message': e.message,
      });
      span?.end?.();

      tracer.recordToolResult?.(
        `dsl.${toolName}`, e.message, false, elapsed,
        { 'dsl.context': 'execute_plan' }
      );

      const msg = 'ERROR: ' + (e.message || String(e));
      logFn?.('[' + toolName + '] ' + msg);
      return msg;
    }
  };
}

/**
 * Try to parse a string as JSON if it looks like a JSON object or array.
 * Checks if the first non-whitespace character is '{' or '[' before attempting parse.
 * Returns the original string if it's not JSON.
 *
 * @param {string} text - The text to try parsing
 * @returns {any} Parsed JSON value, or the original string
 */
function tryParseJSONValue(text) {
  if (typeof text !== 'string') return text;
  const firstChar = text.trimStart()[0];
  if (firstChar === '{' || firstChar === '[') {
    try { return JSON.parse(text); } catch (_) { /* not valid JSON */ }
  }
  return text;
}

/**
 * Generate sandbox globals that bridge DSL function calls to real tool implementations.
 *
 * @param {Object} options
 * @param {Object} options.toolImplementations - Native tool execute functions keyed by name
 * @param {Object} [options.mcpBridge] - MCP bridge with callTool method
 * @param {Object} [options.mcpTools={}] - MCP tools metadata keyed by name
 * @param {Function} options.llmCall - Function to make focused LLM calls: (instruction, data, options?) => Promise<any>
 * @param {number} [options.mapConcurrency=3] - Max concurrent operations in map()
 * @param {Object} [options.tracer=null] - SimpleAppTracer for OTEL tracing
 * @returns {Object} Globals object to pass to SandboxJS
 */
export function generateSandboxGlobals(options) {
  const {
    toolImplementations = {},
    mcpBridge = null,
    mcpTools = {},
    llmCall,
    mapConcurrency = 3,
    tracer = null,
    sessionStore = {},
    outputBuffer = null,
  } = options;

  const globals = {};

  // Log function — writes to the execution logs array (set by runtime before each execute())
  const logFn = (msg) => { if (globals._logs) globals._logs.push(String(msg)); };

  // Bridge native tools
  for (const [name, schema] of Object.entries(NATIVE_TOOL_SCHEMAS)) {
    if (!toolImplementations[name]) continue;

    const rawFn = async (...args) => {
      // Support both (params) and (arg1, arg2) calling conventions
      let params;
      if (args.length === 1 && typeof args[0] === 'object' && args[0] !== null && !Array.isArray(args[0])) {
        params = args[0];
      } else {
        // Map positional args to schema keys
        const keys = Object.keys(schema.shape);
        params = {};
        args.forEach((arg, i) => {
          if (i < keys.length) params[keys[i]] = arg;
        });
      }

      // Issue #444: Auto-coerce object paths to strings for search()
      // AI-generated DSL sometimes passes field objects instead of field.file_path strings
      if (params.path && typeof params.path === 'object') {
        const coercedPath = params.path.file_path || params.path.path || params.path.directory || params.path.filename;
        if (coercedPath && typeof coercedPath === 'string') {
          logFn?.(`[${name}] Warning: Coerced object path to string "${coercedPath}" (issue #444)`);
          params.path = coercedPath;
        }
      }

      const validated = schema.safeParse(params);
      if (!validated.success) {
        throw new Error(`Invalid parameters for ${name}: ${validated.error.message}`);
      }
      return toolImplementations[name].execute(validated.data);
    };

    globals[name] = traceToolCall(name, rawFn, tracer, logFn);
  }

  // Bridge listFiles and searchFiles (no Zod schema, simpler interface)
  if (toolImplementations.listFiles) {
    const rawListFiles = async (pattern) => {
      return toolImplementations.listFiles.execute({ pattern });
    };
    globals.listFiles = traceToolCall('listFiles', rawListFiles, tracer, logFn);
  }
  if (toolImplementations.searchFiles) {
    const rawSearchFiles = async (query) => {
      return toolImplementations.searchFiles.execute({ query });
    };
    globals.searchFiles = traceToolCall('searchFiles', rawSearchFiles, tracer, logFn);
  }

  // Bridge MCP tools
  // Note: mcpBridge is used as a gate (non-null check), but actual execution
  // goes through tool.execute() since MCPXmlBridge stores tools in mcpTools
  // and doesn't have a callTool() method (that's on MCPManager)
  if (mcpBridge) {
    for (const [name, tool] of Object.entries(mcpTools)) {
      const rawMcpFn = async (params = {}) => {
        const result = await tool.execute(params);
        // Extract text from MCP response envelope: { content: [{ type: 'text', text: '...' }] }
        const text = result?.content?.[0]?.text;
        if (text === undefined) {
          // No envelope — if raw result is a JSON-like string, try parsing it
          if (typeof result === 'string') return tryParseJSONValue(result);
          return result;
        }
        return tryParseJSONValue(text);
      };
      globals[name] = traceToolCall(name, rawMcpFn, tracer, logFn);
    }
  }

  // LLM() built-in — delegate already has its own OTEL, but we add a DSL-level span
  // When schema is provided, auto-parse the JSON result for easier downstream processing
  if (llmCall) {
    const rawLLM = async (instruction, data, opts = {}) => {
      // Issue #444: Guard against error strings being passed as data
      // When previous tool calls fail, they return "ERROR: ..." strings
      // Passing these to LLM() spawns useless delegates that can't help
      const dataStr = typeof data === 'string' ? data : JSON.stringify(data);
      if (dataStr && dataStr.startsWith('ERROR:')) {
        logFn?.('[LLM] Blocked: data contains error from previous tool call');
        return 'ERROR: Previous tool call failed - ' + dataStr.substring(0, 200);
      }

      const result = await llmCall(instruction, data, opts);
      // Auto-parse JSON when schema is provided and result is a string
      if (opts.schema && typeof result === 'string') {
        try {
          return JSON.parse(result);
        } catch (e) {
          // If parsing fails, return the raw string (may have formatting issues)
          logFn?.('[LLM] Warning: schema provided but result is not valid JSON');
          return result;
        }
      }
      return result;
    };
    globals.LLM = traceToolCall('LLM', rawLLM, tracer, logFn);
  }

  // map() with concurrency control
  const rawMap = async (items, fn) => {
    if (!Array.isArray(items)) {
      throw new Error('map() first argument must be an array');
    }
    const results = [];
    const executing = new Set();

    for (const item of items) {
      const p = Promise.resolve(fn(item)).then(result => {
        executing.delete(p);
        return result;
      });
      executing.add(p);
      results.push(p);

      if (executing.size >= mapConcurrency) {
        await Promise.race(executing);
      }
    }

    return Promise.all(results);
  };
  globals.map = traceToolCall('map', rawMap, tracer, logFn);

  // chunk() - split data into token-sized chunks
  globals.chunk = (data, tokens = 20000) => {
    const CHARS_PER_TOKEN = 4;
    const chunkSizeChars = tokens * CHARS_PER_TOKEN;
    const text = typeof data === 'string' ? data : JSON.stringify(data);

    // Split by file blocks (``` markers) to avoid breaking mid-block
    const fileBlocks = text.split(/(?=^```)/m);
    const chunks = [];
    let current = '';

    for (const block of fileBlocks) {
      const blockSize = block.length;

      // If a single block exceeds chunk size and we have accumulated content, flush first
      if (blockSize > chunkSizeChars && current.length > 0) {
        chunks.push(current.trim());
        current = '';
      }

      // If a single block exceeds chunk size, split it by character boundary
      if (blockSize > chunkSizeChars) {
        for (let i = 0; i < blockSize; i += chunkSizeChars) {
          const slice = block.slice(i, i + chunkSizeChars);
          if (slice.trim().length > 0) {
            chunks.push(slice.trim());
          }
        }
        continue;
      }

      // If adding this block exceeds chunk size, flush
      if (current.length + blockSize > chunkSizeChars && current.length > 0) {
        chunks.push(current.trim());
        current = '';
      }

      current += block;
    }

    if (current.trim().length > 0) {
      chunks.push(current.trim());
    }

    return chunks;
  };

  // chunkByKey() - chunk data ensuring same-key items stay together
  // - Chunks CAN have multiple keys (customers)
  // - But same key NEVER splits across chunks
  globals.chunkByKey = (data, keyFn, maxTokens = 20000) => {
    const CHARS_PER_TOKEN = 4;
    const maxChars = maxTokens * CHARS_PER_TOKEN;
    const text = typeof data === 'string' ? data : JSON.stringify(data);

    // Find all File: markers
    const blockRegex = /^File: ([^\n]+)/gm;
    const markers = [];
    let match;
    while ((match = blockRegex.exec(text)) !== null) {
      markers.push({ index: match.index, file: match[1].trim() });
    }

    // No File: headers - fallback to regular chunk
    if (markers.length === 0) {
      return globals.chunk(data, maxTokens);
    }

    const chunks = [];
    let currentChunk = '';
    let currentSize = 0;
    let keysInChunk = new Set();  // Track which keys are in current chunk

    // Process each block
    for (let i = 0; i < markers.length; i++) {
      const start = markers[i].index;
      const end = i + 1 < markers.length ? markers[i + 1].index : text.length;
      const block = text.slice(start, end).trim();
      const file = markers[i].file;
      const key = typeof keyFn === 'function' ? keyFn(file) : file;

      const blockSize = block.length + 2; // +2 for \n\n separator
      const wouldOverflow = currentSize + blockSize > maxChars;
      const keyAlreadyInChunk = keysInChunk.has(key);

      // Decision logic:
      // - If key already in chunk: MUST add (never split a key)
      // - If new key and would overflow: flush first, then add
      // - If new key and fits: add to current chunk

      if (!keyAlreadyInChunk && wouldOverflow && currentChunk) {
        // New key would overflow - flush current chunk first
        chunks.push(currentChunk.trim());
        currentChunk = '';
        currentSize = 0;
        keysInChunk = new Set();
      }

      // Add block to current chunk
      if (currentChunk) currentChunk += '\n\n';
      currentChunk += block;
      currentSize += blockSize;
      keysInChunk.add(key);
    }

    // Flush final chunk
    if (currentChunk.trim()) {
      chunks.push(currentChunk.trim());
    }

    return chunks.length > 0 ? chunks : [''];
  };

  // extractPaths() - extract unique file paths from search results
  // Parses File: headers and returns deduplicated array of paths
  globals.extractPaths = (searchResults) => {
    const text = typeof searchResults === 'string' ? searchResults : JSON.stringify(searchResults);
    const matches = text.match(/^File: ([^\n]+)/gm) || [];
    const paths = matches.map(m => m.replace('File: ', '').trim());
    return [...new Set(paths)];
  };

  // Utility functions (pure, no async)
  globals.log = (message) => {
    // Collected by the runtime for the execution log
    if (globals._logs) globals._logs.push(String(message));
  };

  globals.range = (start, end) => {
    const result = [];
    for (let i = start; i < end; i++) result.push(i);
    return result;
  };

  globals.flatten = (arr) => {
    if (!Array.isArray(arr)) return arr;
    return arr.flat(1);
  };

  globals.unique = (arr) => {
    if (!Array.isArray(arr)) return arr;
    const seen = new Set();
    return arr.filter(item => {
      const key = JSON.stringify(item);
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
  };

  globals.batch = (arr, size) => {
    if (!Array.isArray(arr)) return [arr];
    if (!size || size < 1) size = 10;
    const batches = [];
    for (let i = 0; i < arr.length; i += size) {
      batches.push(arr.slice(i, i + size));
    }
    return batches;
  };

  // parseJSON — safely parse JSON from LLM responses that may be wrapped in markdown fences
  // Returns null on parse failure instead of throwing (SandboxJS try/catch is unreliable)
  globals.parseJSON = (text) => {
    try {
      let s = String(text || '').trim();
      // Strip markdown code fences (```json ... ``` or ``` ... ```)
      s = s.replace(/^```(?:json|javascript|js)?\s*\n?/i, '').replace(/\n?```\s*$/i, '').trim();
      // Try to find JSON array or object within the text
      const arrayStart = s.indexOf('[');
      const objectStart = s.indexOf('{');
      if (arrayStart >= 0 && (objectStart < 0 || arrayStart < objectStart)) {
        const end = s.lastIndexOf(']');
        if (end > arrayStart) s = s.substring(arrayStart, end + 1);
      } else if (objectStart >= 0) {
        const end = s.lastIndexOf('}');
        if (end > objectStart) s = s.substring(objectStart, end + 1);
      }
      return JSON.parse(s);
    } catch (e) {
      logFn('[parseJSON] ERROR: ' + e.message);
      return null;
    }
  };

  globals.groupBy = (arr, key) => {
    if (!Array.isArray(arr)) return {};
    const groups = {};
    for (const item of arr) {
      const k = typeof key === 'function' ? key(item) : item[key];
      const groupKey = String(k);
      if (!groups[groupKey]) groups[groupKey] = [];
      groups[groupKey].push(item);
    }
    return groups;
  };

  // Session-scoped store — persists across execute_plan calls within the same agent session
  globals.storeSet = (key, value) => {
    if (typeof key !== 'string') throw new Error('storeSet: key must be a string');
    sessionStore[key] = value;
  };

  globals.storeGet = (key) => {
    if (typeof key !== 'string') throw new Error('storeGet: key must be a string');
    return sessionStore[key];
  };

  globals.storeAppend = (key, item) => {
    if (typeof key !== 'string') throw new Error('storeAppend: key must be a string');
    if (!Array.isArray(sessionStore[key])) sessionStore[key] = [];
    sessionStore[key].push(item);
  };

  globals.storeKeys = () => Object.keys(sessionStore);

  globals.storeGetAll = () => ({ ...sessionStore });

  // output() — write content directly to user's response, bypassing LLM rewriting
  if (outputBuffer) {
    globals.output = (content) => {
      if (content === undefined || content === null) return;
      const str = typeof content === 'string' ? content : JSON.stringify(content, null, 2);
      outputBuffer.items.push(str);
      if (globals._logs) globals._logs.push('[output] ' + str.length + ' chars written to output buffer');
    };
  }

  return globals;
}
