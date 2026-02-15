# LLM Script

LLM Script is Probe's programmable orchestration engine for running complex, multi-step code analysis tasks. Instead of relying on unpredictable multi-turn AI conversations, LLM Script lets you (or the AI) write short, deterministic programs that orchestrate search, extraction, and LLM calls in a sandboxed environment.

Think of it as **stored procedures for code intelligence** — predictable, reproducible, and capable of processing entire codebases in a single execution.

## Why LLM Script?

Traditional AI agent workflows have a fundamental problem: each step is a separate LLM call that can drift, hallucinate, or lose context. When you ask "find all API endpoints and classify them by auth method," a typical agent might:

1. Search once, get partial results
2. Lose track of what it already found
3. Produce inconsistent classifications across calls
4. Take dozens of expensive LLM round-trips

LLM Script solves this by letting the AI write a **complete program** upfront that:

- Searches systematically across the entire codebase
- Processes results in parallel with controlled concurrency
- Uses LLM calls only where needed (classification, summarization)
- Accumulates structured data in a persistent store
- Computes statistics with pure JavaScript — no LLM needed
- Returns formatted, predictable results

## How It Works

LLM Script programs look like simple JavaScript but run in a secure sandbox with special capabilities:

```javascript
// Find all error handling patterns across the codebase
const results = search("error handling try catch")
const chunks = chunk(results)

var patterns = []
for (const c of chunks) {
  const found = LLM(
    "Extract error handling patterns as JSON: [{type, file, description}]. ONLY JSON.",
    c
  )
  try {
    const parsed = JSON.parse(String(found))
    for (const item of parsed) { patterns.push(item) }
  } catch (e) { log("Parse error, skipping chunk") }
}

const byType = groupBy(patterns, "type")
var table = "| Pattern | Count |\n|---------|-------|\n"
for (const type of Object.keys(byType)) {
  table = table + "| " + type + " | " + byType[type].length + " |\n"
}

return table
```

**The execution pipeline:**

1. **Validate** — AST-level whitelist ensures only safe constructs are used (no `eval`, `require`, `import`, `class`, `new`, etc.)
2. **Transform** — Automatically injects `await` before async tool calls and adds loop guards to prevent infinite loops
3. **Execute** — Runs in a SandboxJS environment with a configurable timeout (default 2 minutes)
4. **Self-heal** — If execution fails, the AI automatically gets the error and fixes the script (up to 2 retries)

## Two Ways to Use LLM Script

### 1. Through Prompting (AI-Generated Scripts)

The most common way — you describe what you want in natural language, and the AI writes the script for you:

```
You: "Find all API endpoints in this codebase, classify each by HTTP method,
      and produce a markdown table with counts per method."
```

The AI generates and executes a script like:

```javascript
// Discover repo structure first
const files = listFiles("**/*.{js,ts,py,go,rs}")
const sample = search("API endpoint route handler")

// Let LLM determine the best search strategy
const strategy = LLM(
  "Based on this codebase structure, what search queries would find ALL API endpoints? Return as JSON array of strings.",
  files.join("\n") + "\n\nSample results:\n" + sample
)

const queries = JSON.parse(String(strategy))
var allResults = ""
for (const q of queries) {
  allResults = allResults + "\n" + search(q)
}

// Process in chunks with LLM classification
const chunks = chunk(allResults)
const classified = map(chunks, (c) => LLM(
  "Extract API endpoints as JSON: [{method, path, handler, file}]. ONLY JSON.", c
))

var endpoints = []
for (const batch of classified) {
  try {
    const parsed = JSON.parse(String(batch))
    for (const ep of parsed) { endpoints.push(ep) }
  } catch (e) { log("Parse error") }
}

// Pure JS statistics — no LLM needed
endpoints = unique(endpoints)
const byMethod = groupBy(endpoints, "method")
var table = "| Method | Count | Example |\n|--------|-------|---------|\n"
for (const method of Object.keys(byMethod)) {
  const examples = byMethod[method]
  table = table + "| " + method + " | " + examples.length + " | " + examples[0].path + " |\n"
}

return table + "\nTotal: " + endpoints.length + " endpoints"
```

### 2. User-Provided Scripts

You can also write scripts directly — useful for repeatable analysis tasks, CI pipelines, or when you want precise control over the execution:

```javascript
// Audit: find all TODO/FIXME comments with their context
const todos = search("TODO OR FIXME")
const chunks = chunk(todos)
const items = map(chunks, (c) => LLM(
  "Extract TODO/FIXME items as JSON: [{text, file, priority, category}]. " +
  "Priority: high/medium/low. Category: bug/feature/refactor/debt. ONLY JSON.", c
))

var all = []
for (const batch of items) {
  try {
    const parsed = JSON.parse(String(batch))
    for (const item of parsed) { all.push(item) }
  } catch (e) { log("Parse error") }
}

const byPriority = groupBy(all, "priority")
var report = "# TODO Audit Report\n\n"
for (const priority of ["high", "medium", "low"]) {
  const group = byPriority[priority] || []
  report = report + "## " + priority.toUpperCase() + " (" + group.length + ")\n\n"
  for (const item of group) {
    report = report + "- **" + item.file + "**: " + item.text + " [" + item.category + "]\n"
  }
  report = report + "\n"
}

return report
```

## Available Functions

### Search & Extraction (async, auto-awaited)

| Function | Description | Returns |
|----------|-------------|---------|
| `search(query)` | Semantic code search with Elasticsearch-like syntax | `string` — code snippets with file paths |
| `query(pattern)` | AST-based structural code search (tree-sitter) | `string` — matching code elements |
| `extract(targets)` | Extract code by file path + line number | `string` — extracted code content |
| `listFiles(pattern)` | List files matching a glob pattern | `array` — array of file path strings |
| `bash(command)` | Execute a shell command | `string` — command output |

### AI (async, auto-awaited)

| Function | Description | Returns |
|----------|-------------|---------|
| `LLM(instruction, data)` | Make a focused LLM call to process/classify/summarize data | `string` — AI response |
| `map(array, fn)` | Process items in parallel with concurrency control (default 3) | `array` — results |

### Data Utilities (sync)

| Function | Description | Returns |
|----------|-------------|---------|
| `chunk(data, tokens?)` | Split a large string into token-sized chunks (default 20,000 tokens) | `array` of strings |
| `batch(array, size)` | Split array into sub-arrays of given size | `array` of arrays |
| `groupBy(array, key)` | Group array items by a key or function | `object` |
| `unique(array)` | Deduplicate array items | `array` |
| `flatten(array)` | Flatten one level of nesting | `array` |
| `range(start, end)` | Generate array of integers [start, end) | `array` |
| `parseJSON(text)` | Parse JSON from LLM output (strips markdown fences). Returns `null` on parse failure. | `any\|null` |
| `log(message)` | Log a message for debugging | `void` |

### Direct Output (sync)

| Function | Description | Returns |
|----------|-------------|---------|
| `output(content)` | Write content directly to the user's response, bypassing LLM rewriting. Use for large tables, JSON, or CSV that should be delivered verbatim. | `void` |

When you use `output()`, the content is appended directly to the final response after the AI's summary — the AI never sees or rewrites it. This preserves data fidelity for large structured outputs like tables with 50+ rows.

### Session Store (sync, persists across executions)

The session store allows data to persist across multiple script executions within the same conversation. This enables multi-phase workflows where one script collects data and a later script processes it.

| Function | Description | Returns |
|----------|-------------|---------|
| `storeSet(key, value)` | Store a value | `void` |
| `storeGet(key)` | Retrieve a value (returns `undefined` if missing) | `any` |
| `storeAppend(key, item)` | Append to an array (auto-creates if key doesn't exist) | `void` |
| `storeKeys()` | List all stored keys | `array` of strings |
| `storeGetAll()` | Return entire store as a plain object | `object` |

## Patterns

### Pattern 1: Discovery-First (Recommended)

Start by exploring the repository structure, then let the LLM determine the optimal search strategy:

```javascript
// Phase 1: Discover
const files = listFiles("**/*.{js,ts,py}")
const sample = search("authentication")

// Phase 2: Let LLM plan the strategy
const strategy = LLM(
  "Based on this repo structure, what are the best search queries to find ALL authentication code? Return as JSON array.",
  files.join("\n") + "\n\nSample:\n" + sample
)

// Phase 3: Execute with discovered strategy
const queries = JSON.parse(String(strategy))
var allCode = ""
for (const q of queries) {
  allCode = allCode + "\n" + search(q)
}

// Phase 4: Analyze
const analysis = LLM("Provide a comprehensive analysis of the authentication system.", allCode)
return analysis
```

### Pattern 2: Data Pipeline with Session Store

Process large datasets in phases — extract, accumulate, compute, format:

```javascript
// Phase 1: Collect and classify
const results = search("API endpoints")
const chunks = chunk(results)
const extracted = map(chunks, (c) => LLM(
  "Extract endpoints as JSON array: [{method, path, handler}]. ONLY JSON.", c
))
for (const batch of extracted) {
  try {
    const parsed = JSON.parse(String(batch))
    for (const item of parsed) { storeAppend("endpoints", item) }
  } catch (e) { log("Parse error, skipping") }
}

// Phase 2: Pure JS statistics (no LLM needed!)
const all = storeGet("endpoints")
const byMethod = groupBy(all, "method")
var table = "| Method | Count |\n|--------|-------|\n"
for (const method of Object.keys(byMethod)) {
  table = table + "| " + method + " | " + byMethod[method].length + " |\n"
}

// Phase 3: Small LLM summary
const summary = LLM("Write a brief summary of this API surface.", table)
return table + "\n" + summary
```

### Pattern 3: Batch Processing with Parallel Execution

Process many items efficiently using `map()` for controlled concurrency:

```javascript
const files = listFiles("src/**/*.ts")
const batches = batch(files, 5)

var allIssues = []
for (const b of batches) {
  const results = map(b, (file) => {
    const code = extract(file)
    return LLM("Find potential bugs. Return JSON: [{file, line, issue, severity}]. ONLY JSON.", code)
  })
  for (const r of results) {
    try {
      const parsed = JSON.parse(String(r))
      for (const issue of parsed) { allIssues.push(issue) }
    } catch (e) { log("Parse error") }
  }
}

const bySeverity = groupBy(allIssues, "severity")
var report = "# Bug Report\n\n"
for (const sev of ["high", "medium", "low"]) {
  const items = bySeverity[sev] || []
  report = report + "## " + sev.toUpperCase() + " (" + items.length + ")\n"
  for (const item of items) {
    report = report + "- " + item.file + ":" + item.line + " — " + item.issue + "\n"
  }
  report = report + "\n"
}

return report
```

### Pattern 4: Multi-Search Synthesis

Combine results from multiple targeted searches:

```javascript
const topics = ["authentication", "authorization", "session management", "CSRF", "XSS"]
var allFindings = ""

for (const topic of topics) {
  try {
    const results = search(topic + " security")
    allFindings = allFindings + "\n## " + topic + "\n" + results
  } catch (e) {
    log("Search failed for: " + topic)
  }
}

const chunks = chunk(allFindings)
const analyses = map(chunks, (c) => LLM(
  "Analyze this code for security issues. Be specific about file and line.", c
))

var report = "# Security Audit\n\n"
for (const a of analyses) {
  report = report + a + "\n\n"
}

return report
```

### Pattern 5: Iterative Deepening

Start broad, then drill into the most interesting results:

```javascript
// Broad search
const overview = search("database connection pool")
const summary = LLM(
  "Which files are most important for understanding connection pooling? Return as JSON array of file paths.",
  overview
)

// Deep dive
const importantFiles = JSON.parse(String(summary))
var details = ""
for (const file of importantFiles) {
  try {
    details = details + "\n\n" + extract(file)
  } catch (e) { log("Could not extract: " + file) }
}

// Final analysis
const analysis = LLM(
  "Provide a detailed analysis of the connection pooling implementation. Include architecture decisions and potential improvements.",
  details
)
return analysis
```

### Pattern 6: Direct Output for Large Data

When your script produces large structured data (tables, JSON, CSV), use `output()` to deliver it directly to the user without the AI rewriting or summarizing it:

```javascript
const results = search("customer onboarding")
const chunks = chunk(results)

const classified = map(chunks, (c) => LLM(
  "Extract customers as JSON: [{name, industry, status}]. ONLY JSON.", c
))

var customers = []
for (const batch of classified) {
  try {
    const parsed = parseJSON(String(batch))
    if (Array.isArray(parsed)) {
      for (const item of parsed) { customers.push(item) }
    }
  } catch (e) { log("Parse error, skipping") }
}

// Build a markdown table
var table = "| Customer | Industry | Status |\n|----------|----------|--------|\n"
for (const c of customers) {
  table = table + "| " + (c.name || "Unknown") + " | " + (c.industry || "Unknown") + " | " + (c.status || "-") + " |\n"
}

// output() sends the full table directly to the user — no summarization
output(table)

// return value is what the AI sees — keep it short
return "Generated table with " + customers.length + " customers"
```

The AI will respond with something like "Here's the customer analysis..." and the full table will be appended verbatim below its response.

## Writing Rules

LLM Script uses a safe subset of JavaScript. Keep these rules in mind:

**Do:**
- Use `var` for variables (or `const`/`let`)
- Use `for...of` loops for iteration
- Use plain objects and arrays
- Check for errors with `if (result.indexOf("ERROR:") === 0)` — tool functions never throw, they return `"ERROR: ..."` strings
- Use string concatenation with `+` (not template literals with `${}`)
- Use `parseJSON()` instead of `JSON.parse()` when parsing LLM output (handles markdown fences)
- Use `output()` for large structured data that should reach the user verbatim

**Don't:**
- Use `async`/`await` (auto-injected by the transformer)
- Use `class`, `new`, `this`
- Use `eval`, `require`, `import`
- Use `process`, `globalThis`, `__proto__`
- Define helper functions that call tools (the transformer can't inject `await` inside user-defined functions)
- Use regex literals (`/pattern/`) — use `indexOf()`, `includes()`, `startsWith()` instead
- Use `.matchAll()` or `.join()` (SandboxJS limitations — use `for...of` loops instead)

## Safety Model

LLM Script runs in a multi-layer security sandbox:

1. **AST Validation** — Before execution, the script's Abstract Syntax Tree is checked against a whitelist. Only safe constructs are allowed. No `eval`, `require`, `import`, `class`, `new`, `this`, `__proto__`, `constructor`, or `prototype` access.

2. **SandboxJS Isolation** — Scripts execute in SandboxJS, a JavaScript sandbox that prevents access to Node.js globals, the filesystem, and the network. Only the explicitly provided tool functions are available.

3. **Loop Guards** — Automatic loop iteration limits (default 5,000) prevent infinite loops. The transformer injects a `__checkLoop()` call into every loop body.

4. **Execution Timeout** — A configurable timeout (default 2 minutes) kills scripts that take too long.

5. **Self-Healing** — If a script fails, the error is sent to the LLM which generates a fixed version. Up to 2 retries are attempted before returning an error.

## Enabling LLM Script

### ProbeAgent SDK

```javascript
import { ProbeAgent } from '@probelabs/probe';

const agent = new ProbeAgent({
  path: '/path/to/your/codebase',
  provider: 'anthropic',
  enableExecutePlan: true  // Enable LLM Script
});

// The agent will now use LLM Script for complex analysis tasks
const report = await agent.answer(
  'Find all API endpoints and classify them by HTTP method'
);
```

### CLI

```bash
probe agent "Find all API endpoints" \
  --path /path/to/project \
  --provider google \
  --enable-execute-plan
```

### When Does LLM Script Trigger?

The AI automatically chooses LLM Script (over simple search) for questions that require:

- **Comprehensive coverage**: "Find **all** error handling patterns"
- **Complete inventories**: "Give me a **complete inventory** of API routes"
- **Multi-topic analysis**: "Compare authentication, authorization, and session handling"
- **Batch processing**: "Classify **every** TODO comment by priority"
- **Quantitative answers**: "How many functions in each module?"

For simple, focused questions like "How does the login function work?", the AI uses direct search instead.

## Real-World Examples

### Codebase Health Report

```
You: "Generate a comprehensive health report for this codebase —
      code complexity, test coverage gaps, dependency analysis,
      and security concerns."
```

### API Documentation Generator

```
You: "Find every API endpoint, extract its parameters, authentication
      requirements, and response types, then generate OpenAPI-style
      documentation as markdown."
```

### Migration Planning

```
You: "We're migrating from Express to Fastify. Find all Express-specific
      patterns (middleware, route handlers, error handlers) and produce
      a migration checklist with effort estimates."
```

### Dependency Impact Analysis

```
You: "We need to upgrade the 'auth' library. Find every file that imports
      from it, classify each usage pattern, and identify which ones will
      break with the new API."
```

## Related Resources

- [AI Integration Overview](/ai-integration) — Overview of all Probe AI features
- [Node.js SDK API Reference](/nodejs-sdk) — Programmatic access to Probe
- [ProbeAgent SDK](/ai-integration#probeagent-sdk) — Building AI-powered code analysis apps
- [AI Chat Mode](/ai-chat) — Interactive chat interface
