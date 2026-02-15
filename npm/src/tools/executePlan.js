/**
 * execute_plan tool - DSL-based programmatic orchestration.
 *
 * Allows the LLM to write small JavaScript programs that orchestrate
 * tool calls, keeping intermediate data out of the agent's context window.
 */

import { tool } from 'ai';
import { executePlanSchema, parseAndResolvePaths } from './common.js';
import { createDSLRuntime } from '../agent/dsl/runtime.js';
import { search } from '../search.js';
import { query } from '../query.js';
import { extract } from '../extract.js';
import { delegate } from '../delegate.js';
import { glob } from 'glob';

export { executePlanSchema };

/**
 * Strip markdown fences and XML tags that LLMs sometimes wrap code in.
 */
function stripCodeWrapping(code) {
  let s = String(code || '');
  // Strip markdown code fences
  s = s.replace(/^```(?:javascript|js)?\n?/gm, '').replace(/```$/gm, '');
  // Strip XML-style tags: <execute_plan>, </execute_plan>, <code>, </code>
  s = s.replace(/<\/?(?:execute_plan|code)>/g, '');
  return s.trim();
}

/**
 * Build DSL-compatible tool implementations from the agent's configOptions.
 *
 * @param {Object} configOptions - Agent config (sessionId, cwd, provider, model, etc.)
 * @returns {Object} toolImplementations for createDSLRuntime
 */
function buildToolImplementations(configOptions) {
  const { sessionId, cwd } = configOptions;
  const tools = {};

  tools.search = {
    execute: async (params) => {
      try {
        let searchPaths;
        if (params.path) {
          searchPaths = parseAndResolvePaths(params.path, cwd);
        }
        if (!searchPaths || searchPaths.length === 0) {
          searchPaths = [cwd || '.'];
        }
        return await search({
          query: params.query,
          path: searchPaths.join(' '),
          cwd,
          allowTests: true,
          exact: params.exact || false,
          json: false,
          maxTokens: 20000,
          session: sessionId,
          timeout: 60,
        });
      } catch (e) {
        return `Search error: ${e.message}`;
      }
    },
  };

  tools.query = {
    execute: async (params) => {
      try {
        let queryPath = cwd || '.';
        if (params.path) {
          const resolved = parseAndResolvePaths(params.path, cwd);
          if (resolved.length > 0) queryPath = resolved[0];
        }
        return await query({
          pattern: params.pattern,
          path: queryPath,
          cwd,
          language: params.language || 'rust',
          allowTests: params.allow_tests ?? true,
        });
      } catch (e) {
        return `Query error: ${e.message}`;
      }
    },
  };

  tools.extract = {
    execute: async (params) => {
      try {
        if (!params.targets && !params.input_content) {
          return 'Extract error: no file path provided. Usage: extract("path/to/file.md")';
        }
        return await extract({
          files: params.targets ? [params.targets] : undefined,
          content: params.input_content || undefined,
          cwd,
          allowTests: params.allow_tests ?? true,
        });
      } catch (e) {
        return `Extract error: ${e.message}`;
      }
    },
  };

  tools.listFiles = {
    execute: async (params) => {
      try {
        const files = await glob(params.pattern || '**/*', {
          cwd: cwd || '.',
          ignore: ['node_modules/**', '.git/**'],
          nodir: true,
        });
        files.sort();
        return files;
      } catch (e) {
        return `listFiles error: ${e.message}`;
      }
    },
  };

  return tools;
}

/**
 * Build an llmCall function using delegate with disableTools.
 *
 * Uses the full delegate infrastructure (OTEL, retries, fallbacks, schema support)
 * but with tools disabled and maxIterations: 1 since LLM() is pure text processing.
 *
 * @param {Object} configOptions - Agent config
 * @returns {Function} llmCall(instruction, data, options?) => Promise<string>
 */
function buildLLMCall(configOptions) {
  const { provider, model, debug, tracer, sessionId } = configOptions;

  return async (instruction, data, options = {}) => {
    const dataStr = data == null ? '' : (typeof data === 'string' ? data : JSON.stringify(data, null, 2));
    const task = `${instruction}\n\n---\n\n${dataStr || '(empty)'}`;

    return delegate({
      task,
      disableTools: true,
      maxIterations: 1,
      provider,
      model,
      debug,
      tracer,
      parentSessionId: sessionId,
      schema: options.schema || null,
      timeout: options.timeout || 120,
    });
  };
}

/**
 * Create the execute_plan tool for the Vercel AI SDK.
 *
 * Accepts EITHER:
 * - Agent configOptions (sessionId, cwd, provider, model, etc.) — auto-builds tools + LLM via delegate
 * - Direct DSL options (toolImplementations, llmCall, etc.) — used as-is (tests, manual scripts)
 *
 * @param {Object} options
 * @returns {Object} Vercel AI SDK tool
 */
export function createExecutePlanTool(options) {
  let runtimeOptions;
  let llmCallFn;
  const tracer = options.tracer || null;

  // Session-scoped store persists across execute_plan calls within the same agent session
  const sessionStore = options.sessionStore || {};

  // Output buffer for direct-to-user content (bypasses LLM context window)
  const outputBuffer = options.outputBuffer || null;

  if (options.toolImplementations) {
    // Direct DSL options — used by tests and manual scripts
    runtimeOptions = { ...options, tracer, sessionStore, outputBuffer };
    llmCallFn = options.llmCall;
  } else {
    // Agent configOptions — build everything from the agent's config
    llmCallFn = buildLLMCall(options);
    runtimeOptions = {
      toolImplementations: buildToolImplementations(options),
      llmCall: llmCallFn,
      mcpBridge: options.mcpBridge || null,
      mcpTools: options.mcpTools || {},
      mapConcurrency: options.mapConcurrency || 5,
      timeoutMs: options.timeoutMs || 300000,
      maxLoopIterations: options.maxLoopIterations || 5000,
      tracer,
      sessionStore,
      outputBuffer,
    };
  }

  const runtime = createDSLRuntime(runtimeOptions);
  const maxRetries = options.maxRetries ?? 2;

  return tool({
    description: 'Execute a JavaScript DSL program to orchestrate tool calls. ' +
      'Use for batch processing, paginated APIs, multi-step workflows where intermediate data is large. ' +
      'Write simple synchronous-looking code — do NOT use async/await.',
    parameters: executePlanSchema,
    execute: async ({ code, description }) => {
      // Create top-level OTEL span for the entire execute_plan invocation
      const planSpan = tracer?.createToolSpan?.('execute_plan', {
        'dsl.description': description || '',
        'dsl.code_length': code.length,
        'dsl.code': code,
        'dsl.max_retries': maxRetries,
      }) || null;

      // Strip XML tags and markdown fences LLMs sometimes wrap code in
      let currentCode = stripCodeWrapping(code);
      let lastError = null;
      let finalOutput;

      try {
        for (let attempt = 0; attempt <= maxRetries; attempt++) {
          // On retry, ask the LLM to fix the code
          if (attempt > 0 && llmCallFn && lastError) {
            planSpan?.addEvent?.('dsl.self_heal_start', {
              'dsl.attempt': attempt,
              'dsl.error': lastError.substring(0, 1000),
            });

            try {
              const fixPrompt = `The following DSL script failed with an error. Fix the script and return ONLY the corrected JavaScript code — no markdown, no explanation, no backtick fences.

ORIGINAL SCRIPT:
${currentCode}

ERROR:
${lastError}

RULES REMINDER:
- search(query) is KEYWORD SEARCH — pass a search query, NOT a filename. Use extract(filepath) to read file contents.
- search(), query(), extract(), listFiles(), bash() all return STRINGS, not arrays.
- Use chunk(stringData) to split a string into an array of chunks.
- Use map(array, fn) only with arrays. Do NOT pass strings to map().
- Do NOT use .map(), .forEach(), .filter(), .join() — use for..of loops instead.
- Do NOT define helper functions that call tools — write logic inline.
- Do NOT use async/await, template literals, or shorthand properties.
- Do NOT use regex literals (/pattern/) — use String methods like indexOf, includes, startsWith instead.
- String concatenation with +, not template literals.`;

              const fixedCode = await llmCallFn(fixPrompt, '', { maxTokens: 4000, temperature: 0.2 });
              // Strip markdown fences and XML tags the LLM might add
              currentCode = stripCodeWrapping(fixedCode);

              planSpan?.addEvent?.('dsl.self_heal_complete', {
                'dsl.attempt': attempt,
                'dsl.fixed_code_length': currentCode.length,
              });

              if (!currentCode) {
                finalOutput = `Plan execution failed after ${attempt} retries: LLM returned empty fix.\n\nLast error: ${lastError}`;
                planSpan?.setAttributes?.({ 'dsl.result': 'empty_fix', 'dsl.attempts': attempt });
                planSpan?.setStatus?.('ERROR');
                planSpan?.end?.();
                return finalOutput;
              }
            } catch (fixError) {
              finalOutput = `Plan execution failed and self-heal failed: ${fixError.message}\n\nOriginal error: ${lastError}`;
              planSpan?.setAttributes?.({ 'dsl.result': 'self_heal_error', 'dsl.attempts': attempt });
              planSpan?.setStatus?.('ERROR');
              planSpan?.end?.();
              return finalOutput;
            }
          }

          const result = await runtime.execute(currentCode, description);

          if (result.status === 'success') {
            finalOutput = formatSuccess(result, description, attempt, outputBuffer);
            planSpan?.setAttributes?.({
              'dsl.result': 'success',
              'dsl.attempts': attempt,
              'dsl.self_healed': attempt > 0,
              'dsl.result_length': finalOutput.length,
              'dsl.log_count': result.logs.length,
            });
            planSpan?.setStatus?.('OK');
            planSpan?.end?.();
            return finalOutput;
          }

          // Execution failed — prepare for retry
          const logOutput = result.logs.length > 0 ? `\nLogs: ${result.logs.join(' | ')}` : '';
          lastError = `${result.error}${logOutput}`;

          planSpan?.addEvent?.('dsl.execution_failed', {
            'dsl.attempt': attempt,
            'dsl.error': lastError.substring(0, 1000),
          });
        }

        // All retries exhausted
        finalOutput = `Plan execution failed after ${maxRetries} retries.\n\nLast error: ${lastError}`;
        planSpan?.setAttributes?.({
          'dsl.result': 'all_retries_exhausted',
          'dsl.attempts': maxRetries,
          'dsl.last_error': lastError?.substring(0, 1000),
        });
        planSpan?.setStatus?.('ERROR');
        planSpan?.end?.();
        return finalOutput;
      } catch (e) {
        planSpan?.setStatus?.('ERROR');
        planSpan?.addEvent?.('exception', {
          'exception.message': e.message,
          'exception.stack': e.stack,
        });
        planSpan?.end?.();
        throw e;
      }
    },
  });
}

function formatSuccess(result, description, attempt, outputBuffer) {
  let output = '';

  if (description) {
    output += `Plan: ${description}\n\n`;
  }

  if (attempt > 0) {
    output += `(Self-healed after ${attempt} ${attempt === 1 ? 'retry' : 'retries'})\n\n`;
  }

  if (result.logs.length > 0) {
    const userLogs = result.logs.filter(l => !l.startsWith('[runtime]') && !l.startsWith('[output]'));
    if (userLogs.length > 0) {
      output += `Logs:\n${userLogs.join('\n')}\n\n`;
    }
  }

  // Format the result value
  const resultValue = result.result;
  if (resultValue === undefined || resultValue === null) {
    output += 'Plan completed (no return value).';
  } else if (typeof resultValue === 'string') {
    output += `Result:\n${resultValue}`;
  } else {
    try {
      output += `Result:\n${JSON.stringify(resultValue, null, 2)}`;
    } catch {
      output += `Result: ${String(resultValue)}`;
    }
  }

  // If output buffer has content, tell the LLM the data was written to direct output
  if (outputBuffer && outputBuffer.items && outputBuffer.items.length > 0) {
    const totalChars = outputBuffer.items.reduce((sum, item) => sum + item.length, 0);
    output += `\n\n[Output buffer: ${totalChars} chars written via output(). This content will be appended directly to your response. Do NOT repeat or summarize it.]`;
  }

  return output;
}

/**
 * XML tool definition for the system prompt.
 *
 * @param {string[]} availableFunctions - List of available DSL function names
 * @returns {string} Tool definition text
 */
export function getExecutePlanToolDefinition(availableFunctions = []) {
  const funcList = availableFunctions.length > 0
    ? availableFunctions.join(', ')
    : 'search, query, extract, LLM, map, chunk, batch, listFiles, bash, log, range, flatten, unique, groupBy, parseJSON, storeSet, storeGet, storeAppend, storeKeys, storeGetAll, output';

  return `## execute_plan
Description: Execute a JavaScript DSL program to orchestrate tool calls. Use for batch processing, large data analysis, and multi-step workflows where intermediate data is large.

ALWAYS use this tool when:
- The question asks about "all", "every", "comprehensive", "complete", or "inventory" of something
- The question covers multiple topics or requires scanning across the full codebase
- Open-ended discovery questions where you don't know the right search keywords (use the discovery-first pattern)
- Processing large search results that exceed context limits
- Iterating over paginated APIs or many files
- Batch operations with the same logic applied to many items
- Chaining multiple tool calls where intermediate data is large

Do NOT use this tool for:
- Simple single searches or extractions (1-2 tool calls)
- Questions about a specific function, class, or file
- Tasks where you need to see and reason about every detail of results

Parameters:
- code: (required) JavaScript DSL code to execute. Write synchronous-looking code — do NOT use async/await.
- description: (optional) Human-readable description of what this plan does.

<examples>

Discovery-first analysis (RECOMMENDED for open-ended questions — explore before searching):
<execute_plan>
<code>
const files = listFiles("**/*");
const sample = search("initial keyword");
const plan = LLM(
  "Based on this repo structure and sample results, suggest the best search strategy. " +
  "Return JSON: {keywords: [2-4 queries], extractionFocus: string, aggregation: string}. ONLY valid JSON.",
  "Files:\\n" + String(files).substring(0, 3000) + "\\nSample:\\n" + String(sample).substring(0, 3000)
);
const strategy = parseJSON(plan);
log("Strategy: " + strategy.keywords.length + " keywords");
const allFindings = [];
for (const kw of strategy.keywords) {
  const results = search(kw);
  if (String(results).length > 500) {
    const chunks = chunk(results);
    const findings = map(chunks, (c) => LLM(strategy.extractionFocus, c));
    for (const f of findings) { allFindings.push(String(f)); }
  }
}
var combined = "";
for (const f of allFindings) { combined = combined + f + "\\n---\\n"; }
return LLM("Synthesize all findings into a comprehensive answer.", combined);
</code>
<description>Discover optimal search strategy, then analyze</description>
</execute_plan>

Analyze large search results:
<execute_plan>
<code>
const results = search("error handling");
const chunks = chunk(results);
log("Processing " + chunks.length + " chunks");
const extracted = map(chunks, (c) => LLM("List error handling patterns found. Be brief.", c));
var combined = "";
for (const e of extracted) { combined = combined + String(e) + "\\n---\\n"; }
return LLM("Combine into a summary.", combined);
</code>
<description>Analyze error handling patterns across the codebase</description>
</execute_plan>

Multi-topic analysis:
<execute_plan>
<code>
const topics = ["authentication", "authorization"];
const allFindings = [];
for (const topic of topics) {
  const results = search(topic);
  const chunks = chunk(results);
  const findings = map(chunks, (c) => LLM("Extract key findings about " + topic + ". Be brief.", c));
  for (const f of findings) { allFindings.push(String(f)); }
}
var combined = "";
for (const f of allFindings) { combined = combined + f + "\\n---\\n"; }
return LLM("Synthesize all findings into a report.", combined);
</code>
<description>Cross-topic analysis of auth patterns</description>
</execute_plan>

Process each file individually (use extract to read files, NOT search):
<execute_plan>
<code>
const files = listFiles("**/*.md");
log("Found " + files.length + " files");
const batches = batch(files, 5);
const results = [];
for (const b of batches) {
  const batchResults = map(b, (filepath) => {
    try {
      const content = extract(filepath);
      if (String(content).length > 100) {
        const info = LLM("Extract: customer name, industry, key use case. Return JSON: {customer, industry, useCase}. ONLY JSON.", content);
        try { return parseJSON(info); } catch (e) { return null; }
      }
    } catch (e) { return null; }
    return null;
  });
  for (const r of batchResults) { if (r) { results.push(r); } }
  log("Batch done, total: " + results.length);
}
var table = "| Customer | Industry | Use Case |";
for (const r of results) {
  table = table + "\n| " + r.customer + " | " + r.industry + " | " + r.useCase + " |";
}
return table;
</code>
<description>Read each file with extract() and classify with LLM</description>
</execute_plan>

</examples>

### Rules
- Write simple, synchronous-looking JavaScript. Do NOT use async/await — the runtime injects it automatically.
- Do NOT use: class, new, import, require, eval, this, Promise, async, await, setTimeout.
- Do NOT use these as variable names: eval, Function, require, process, globalThis, constructor, prototype, exports, Proxy, Reflect, Symbol.
- Use \`map(items, fn)\` for **parallel** batch processing. Use \`for..of\` only for **sequential** logic where order matters.
- **CRITICAL: When processing multiple files**, use \`batch(files, 5)\` + \`map(batch, fn)\` for parallel processing. NEVER use a sequential for..of loop with LLM() or extract() calls on many files — it will timeout.
- Do NOT use Array.prototype.map (.map()) — use the global \`map()\` function instead.
- Use \`LLM(instruction, data)\` for AI processing — returns a string.
- Use \`log(message)\` for debugging — messages appear in the output.
- Use \`parseJSON(text)\` instead of \`JSON.parse()\` when parsing LLM output — LLM responses often have markdown fences.
- Use \`try/catch\` for error handling, \`while/break\` for loops and pagination. Do NOT nest try/catch blocks — use flat try/catch instead.
- Always use explicit property assignment: \`{ key: value }\` not shorthand \`{ key }\`.
- String concatenation with \`+\`, no template literals with backticks.
- Use \`String(value)\` before calling \`.trim()\`, \`.split()\`, or \`.length\` on tool results.
- Use \`for (const item of array)\` loops instead of \`.forEach()\`, \`.map()\`, \`.filter()\`, or \`.join()\` array methods.
- Do NOT define helper functions that call tools. Write all logic inline or use for..of loops.
- Do NOT use regex literals (/pattern/) — use String methods like indexOf, includes, startsWith instead.
- ONLY use functions listed below. Do NOT call functions that are not listed.

### Available functions

**Tools (async, auto-awaited):**
${funcList}

**Return types — IMPORTANT:**
- \`search(query)\` → **keyword search** — pass a search query (e.g. "error handling"), NOT a filename. Returns a **string** (matching code snippets). To process parts, use \`chunk()\` to split it.
- \`query(pattern)\` → **AST search** — pass a tree-sitter pattern. Returns a **string** (matching code elements).
- \`extract(targets)\` → **read file contents** — pass a file path like "src/main.js" or "src/main.js:42". Use this to read specific files found by listFiles(). Returns a **string**.
- \`listFiles(pattern)\` → **list files** — pass a glob pattern like "**/*.md". Returns an **array** of file path strings. Use directly with \`for (const f of listFiles("**/*.md"))\`.
- \`LLM(instruction, data)\` → returns a **string** (AI response)
- \`map(array, fn)\` → returns an **array** of results. First argument MUST be an array.
- \`bash(command)\` → returns a **string** (command output)

**COMMON MISTAKE:** Do NOT use \`search(filename)\` to read a file's contents — search() is for keyword queries. Use \`extract(filepath)\` to read file contents.

**Parallel processing:**
- \`map(array, fn)\` — process array items **in parallel** (concurrency=3). Use this for batch operations, NOT for..of loops.

**Utilities (sync):**
- \`chunk(data, tokens)\` — split a string into token-sized array of chunks (default 20000 tokens). Returns an **array of strings**.
- \`batch(array, size)\` — split an array into sub-arrays of \`size\` (default 10). Returns an **array of arrays**.
- \`log(message)\` — log a message (collected in output)
- \`range(start, end)\` — generate array of integers [start, end)
- \`flatten(arr)\` — flatten one level of nesting
- \`unique(arr)\` — deduplicate array
- \`groupBy(arr, key)\` — group array of objects by key or function
- \`parseJSON(text)\` — **safely parse JSON from LLM responses**. Strips markdown fences and extracts JSON. ALWAYS use \`parseJSON()\` instead of \`JSON.parse()\` when parsing LLM output.

**Direct output (sync):**
- \`output(content)\` — **write content directly to the user's response**, bypassing LLM rewriting. Use for large tables, JSON, or CSV that should be delivered verbatim. Can be called multiple times; all content is appended to the final response. The \`return\` value still goes to the tool result for you to see.

**Session store (sync, persists across execute_plan calls):**
- \`storeSet(key, value)\` — store a value that persists across execute_plan calls in this session
- \`storeGet(key)\` — retrieve a stored value (returns undefined if not found)
- \`storeAppend(key, item)\` — append item to an array in the store (auto-creates array if key doesn't exist)
- \`storeKeys()\` — list all keys in the store
- \`storeGetAll()\` — return entire store as a plain object

### Patterns

**Pattern 1: Discovery-first (RECOMMENDED for open-ended questions)**
When you don't know the right keywords, explore the repo first, then use LLM to determine the best search strategy:
\`\`\`
// Phase 1: Discover repo structure and test queries
const files = listFiles("**/*");
const sample = search("initial keyword guess");
log("Files overview length: " + String(files).length + ", sample length: " + String(sample).length);

// Phase 2: Ask LLM to determine optimal strategy based on what exists
const plan = LLM(
  "Based on this repository structure and sample search results, determine the best search strategy. " +
  "Return a JSON object with: keywords (array of 2-4 search queries that will find relevant data), " +
  "extractionFocus (what to extract from each result), " +
  "and aggregation (summarize, list_unique, count, or group_by). " +
  "IMPORTANT: Only suggest keywords likely to match actual content you see. Return ONLY valid JSON.",
  "Repository files:\\n" + String(files).substring(0, 3000) + "\\nSample results:\\n" + String(sample).substring(0, 3000)
);
const strategy = parseJSON(plan);
log("Strategy: " + strategy.keywords.length + " keywords, focus: " + strategy.extractionFocus);

// Phase 3: Execute with discovered strategy
const allFindings = [];
for (const kw of strategy.keywords) {
  const results = search(kw);
  if (String(results).length > 500) {
    const chunks = chunk(results);
    const findings = map(chunks, (c) => LLM(strategy.extractionFocus, c));
    for (const f of findings) { allFindings.push(String(f)); }
    log("Keyword '" + kw + "': " + chunks.length + " chunks processed");
  } else {
    log("Keyword '" + kw + "': skipped (too few results)");
  }
}
var combined = "";
for (const f of allFindings) { combined = combined + f + "\\n---\\n"; }
return LLM("Synthesize all findings into a comprehensive answer.", combined);
\`\`\`

**Pattern 2: Large result analysis**
search() returns a big string. Split into 20K-token chunks, process in parallel, synthesize:
\`\`\`
const results = search("error handling");
const chunks = chunk(results);
log("Processing " + chunks.length + " chunks");
const extracted = map(chunks, (c) => LLM("List error handling patterns found. Be brief.", c));
var combined = "";
for (const e of extracted) { combined = combined + String(e) + "\\n---\\n"; }
return LLM("Combine into a summary. Max 5 bullet points.", combined);
\`\`\`

**Pattern 3: Paginated API with while loop**
For APIs that return pages of results:
\`\`\`
const allItems = [];
let page = 1;
while (true) {
  const result = mcp_api_list_items({ page: page, per_page: 50 });
  for (const item of result.items) {
    allItems.push(item);
  }
  log("Page " + page + ": " + result.items.length + " items");
  if (!result.has_more) break;
  page = page + 1;
}
return allItems;
\`\`\`

**Pattern 4: Batch classify/process with map**
For processing many items in parallel:
\`\`\`
const items = mcp_api_get_tickets({ status: "open" });
const classified = map(items, (item) => {
  const sentiment = LLM("Classify as positive, negative, or neutral. Return ONLY the word.", item.description);
  return { id: item.id, title: item.title, sentiment: String(sentiment).trim() };
});
return groupBy(classified, "sentiment");
\`\`\`

**Pattern 5: Multi-search with error handling**
For searching multiple topics and combining results:
\`\`\`
const queries = ["authentication", "authorization", "session management"];
const results = [];
for (const q of queries) {
  try {
    const r = search(q);
    if (r.length > 500) {
      const summary = LLM("Summarize the key patterns found. Be concise.", r);
      results.push({ query: q, summary: summary });
    } else {
      results.push({ query: q, summary: "No significant results" });
    }
  } catch (e) {
    results.push({ query: q, summary: "Search failed" });
  }
}
return LLM("Combine these findings into a security overview.", results);
\`\`\`

**Pattern 6: Iterative deepening**
Search broadly, then drill into specific findings:
\`\`\`
const broad = search("database");
const keyFunctions = LLM("List the 3 most important function names. Return comma-separated, nothing else.", broad);
const names = [];
const parts = keyFunctions.split(",");
for (const p of parts) {
  const trimmed = p.trim();
  if (trimmed.length > 0) names.push(trimmed);
}
const details = map(names, (fn) => {
  const code = search(fn);
  return { name: fn, analysis: LLM("Explain what " + fn + " does in 2 sentences.", code) };
});
return details;
\`\`\`

**Pattern 7: Multi-topic analysis with chunking**
Search multiple topics, chunk each result, process in parallel:
\`\`\`
const topics = ["authentication", "authorization", "session"];
const allFindings = [];
for (const topic of topics) {
  const results = search(topic);
  const chunks = chunk(results);
  const findings = map(chunks, (c) => LLM("Extract key patterns for " + topic + ". Be brief.", c));
  for (const f of findings) { allFindings.push(String(f)); }
  log("Processed " + topic + ": " + chunks.length + " chunks");
}
var combined = "";
for (const f of allFindings) { combined = combined + f + "\\n---\\n"; }
return LLM("Synthesize all findings into a security report.", combined);
\`\`\`

**Pattern 8: Batched file processing**
Process many files in parallel batches:
\`\`\`
const files = listFiles("*.js");
log("Found " + files.length + " files");
const batches = batch(files, 5);
const allResults = [];
for (const b of batches) {
  const batchResults = map(b, (file) => {
    const content = extract(file);
    return LLM("Summarize this file in one sentence.", content);
  });
  for (const r of batchResults) { allResults.push(r); }
  log("Processed batch, total: " + allResults.length);
}
return allResults;
\`\`\`

**Pattern 9: Data pipeline with session store**
Extract structured data, accumulate, compute statistics with pure JS, format as table:
\`\`\`
// Phase 1: Extract structured data from search results
const results = search("API endpoints");
const chunks = chunk(results);
const extracted = map(chunks, (c) => LLM(
  "Extract API endpoints as JSON array: [{method, path, description}]. Return ONLY valid JSON.",
  c
));
for (const e of extracted) {
  try {
    const parsed = JSON.parse(String(e));
    for (const item of parsed) { storeAppend("endpoints", item); }
  } catch (err) { log("Parse error, skipping chunk"); }
}

// Phase 2: Compute statistics with pure JS (no LLM needed)
const all = storeGet("endpoints");
log("Total endpoints: " + all.length);
const byMethod = groupBy(all, "method");
var table = "| Method | Count | % |\\n|--------|-------|---|\\n";
const methods = Object.keys(byMethod);
for (const m of methods) {
  const count = byMethod[m].length;
  const pct = Math.round(count / all.length * 100);
  table = table + "| " + m + " | " + count + " | " + pct + "% |\\n";
}

// Phase 3: Small LLM summary of the statistics
const summary = LLM("Write a 2-sentence executive summary of this API surface analysis.", table);
return table + "\\n" + summary;
\`\`\`

**Pattern 10: Direct output for large structured data**
Use \`output()\` to deliver tables/JSON directly to the user without LLM rewriting. The \`return\` value is what you (the AI) see as the tool result:
\`\`\`
const files = listFiles("**/*.md");
const batches = batch(files, 5);
const results = [];
for (const b of batches) {
  const batchResults = map(b, (f) => {
    try {
      const content = extract(f);
      return LLM("Extract: name, category. Return JSON: {name, category}. ONLY JSON.", content);
    } catch (e) { return null; }
  });
  for (const r of batchResults) {
    try { if (r) results.push(parseJSON(r)); } catch (e) { /* skip */ }
  }
}
var table = "| Name | Category |\\n|------|----------|\\n";
for (const r of results) {
  table = table + "| " + (r.name || "?") + " | " + (r.category || "?") + " |\\n";
}
output(table);
return "Generated table with " + results.length + " items.";
\`\`\``;
}
