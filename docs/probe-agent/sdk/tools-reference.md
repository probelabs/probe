# Tools Reference

ProbeAgent provides a comprehensive set of tools that the AI can use to interact with your codebase. This document covers all available tools, their parameters, and usage patterns.

---

## Tool Categories

| Category | Tools | Description |
|----------|-------|-------------|
| **Search & Query** | search, query, extract | Find and retrieve code |
| **File Operations** | edit, create, listFiles, searchFiles | Modify and explore files |
| **Execution** | bash | Run shell commands |
| **Analysis** | analyze_all, readImage | Comprehensive analysis |
| **Agent Control** | delegate, attempt_completion | Orchestration and completion |
| **Skills** | listSkills, useSkill | Dynamic capabilities |
| **Tasks** | task | Multi-step tracking |

---

## Core Search Tools

### search

Semantic code search using Elasticsearch-style queries.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | Yes | Search query (supports AND, OR, NOT, wildcards) |
| `path` | string | No | Directory to search (default: workspace root) |
| `limit` | number | No | Maximum results (default: 20) |
| `exact` | boolean | No | Exact match mode (default: false) |
| `allowTests` | boolean | No | Include test files (default: false) |
| `maxTokens` | number/null | No | Max tokens to return. Default 20000. Set to `null` for unlimited. |

**Example AI Usage:**
```xml
<search>
  <query>authentication AND middleware</query>
  <path>./src</path>
  <limit>10</limit>
</search>
```

---

### searchAll

Exhaustive search that auto-paginates to retrieve ALL matching results. Use when you need complete coverage for bulk analysis.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `query` | string | Yes | Search query (supports AND, OR, NOT, wildcards) |
| `path` | string | No | Directory to search (default: workspace root) |
| `exact` | boolean | No | Exact match mode (default: false) |
| `maxTokensPerPage` | number | No | Tokens per page (default: 20000) |
| `maxPages` | number | No | Maximum pages to retrieve (default: 50, safety limit) |

**Example DSL Usage:**
```javascript
// Get ALL matching results across the entire codebase
const allResults = searchAll("authentication")

// With options
const results = searchAll({
  query: "API endpoint",
  path: "./src",
  maxPages: 100
})
```

**When to use:**
- Use `search()` for targeted queries where first results are sufficient
- Use `searchAll()` when you need complete coverage (e.g., "find ALL usages of X")

---

### Session-Based Pagination (DSL)

Within `execute_plan` DSL scripts, search uses session-based pagination:

**How it works:**
- Each `execute_plan` invocation gets a unique session ID
- Multiple `search()` calls with the **same query** return successive pages
- The session is isolated — different `execute_plan` calls don't interfere

**Manual pagination example:**
```javascript
// In execute_plan DSL script
let allResults = ""
let page = search("authentication")

while (page && !page.includes("All results retrieved")) {
  allResults = allResults + "\n" + page
  page = search("authentication")  // Same query = next page
}

return allResults
```

**Benefits of manual pagination:**
- Process each page before fetching the next (memory efficient)
- Stop early when you find what you need
- Apply different logic per page (e.g., LLM classification)

---

**Searching Dependencies:**

The agent can search inside project dependencies using special path prefixes:

| Prefix | Language | Example |
|--------|----------|---------|
| `go:` | Go modules | `go:github.com/gin-gonic/gin` |
| `js:` | npm packages | `js:express` or `js:@ai-sdk/anthropic` |
| `rust:` | Rust crates | `rust:serde` |

```xml
<!-- Search in an npm package -->
<search>
  <query>createAnthropic</query>
  <path>js:@ai-sdk/anthropic</path>
</search>

<!-- Search in a Go module -->
<search>
  <query>Context AND middleware</query>
  <path>go:github.com/gin-gonic/gin</path>
</search>
```

**Prompting the Agent:**

To ask the agent to search dependencies, use natural language like:
- "Search for how createAnthropic works in the @ai-sdk/anthropic package"
- "Look inside the gin library to see how middleware is implemented"
- "Find the Serialize trait definition in the serde crate"

**Configuration:**
```javascript
const agent = new ProbeAgent({
  path: './src',
  searchDelegate: true,  // Use code-search subagent (default)
  outline: false         // Use outline-xml format
});
```

---

### query

AST-based structural queries using tree-sitter patterns.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `pattern` | string | Yes | AST-grep pattern |
| `path` | string | No | Directory to search |
| `language` | string | No | Programming language |
| `limit` | number | No | Maximum results |

**Example AI Usage:**
```xml
<query>
  <pattern>fn $NAME($PARAMS) { $$$BODY }</pattern>
  <language>rust</language>
  <path>./src</path>
</query>
```

**Common Patterns:**
- Functions: `fn $NAME() { $$$BODY }`
- Classes: `class $NAME { $$$BODY }`
- Interfaces: `interface $NAME { $$$BODY }`
- Variables: `let $NAME = $VALUE`

---

### extract

Extract code blocks from files by location or symbol.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `file` | string | Yes | File path with line or symbol (e.g., `src/auth.ts:42` or `src/auth.ts#login`) |
| `context` | number | No | Context lines before/after |

**Example AI Usage:**
```xml
<extract>
  <file>src/auth/login.ts:42</file>
  <context>5</context>
</extract>
```

---

## File Operation Tools

### edit

Edit files using text replacement, AST-aware symbol operations, or line-targeted editing. Supports four modes: text-based find/replace with fuzzy matching, AST-aware symbol replacement, symbol insertion, and line-targeted editing with optional hash-based integrity verification.

**Enabled By:** `allowEdit: true`

**Four Editing Modes:**

| Mode | Parameters | When to Use |
|------|-----------|-------------|
| **Text edit** | `old_string` + `new_string` | Small, precise changes: fix a condition, rename a variable, update a value |
| **Symbol replace** | `symbol` + `new_string` | Replace an entire function, class, or method by name (no exact text matching needed) |
| **Symbol insert** | `symbol` + `new_string` + `position` | Insert new code before or after an existing symbol |
| **Line-targeted edit** | `start_line` + `new_string` | Edit specific lines from extract/search output; ideal for changes inside large functions |

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `file_path` | string | Yes | Path to the file to edit (absolute or relative to cwd) |
| `new_string` | string | Yes | Replacement text or new code content |
| `old_string` | string | No | Text to find and replace. Copy verbatim from the file. |
| `replace_all` | boolean | No | Replace all occurrences (default: false, text mode only) |
| `symbol` | string | No | Code symbol name for AST-aware editing (e.g. `"myFunction"`, `"MyClass.myMethod"`) |
| `position` | string | No | `"before"` or `"after"` — insert near the symbol or line instead of replacing it |
| `start_line` | string | No | Line reference for line-targeted editing (e.g. `"42"` or `"42:ab"` with hash) |
| `end_line` | string | No | End of line range, inclusive (e.g. `"55"` or `"55:cd"`). Defaults to `start_line`. |

**Mode Selection Rules (Priority Order):**
- If `symbol` is provided → AST-aware mode (symbol replace or symbol insert depending on `position`)
- If `start_line` is provided (without `symbol`) → line-targeted mode
- If `old_string` is provided (without `symbol` or `start_line`) → text-based mode
- If none are provided → error with guidance

#### Text Mode — Find and Replace

Provide `old_string` with text copied verbatim from the file and `new_string` with the replacement.

```xml
<edit>
<file_path>src/main.js</file_path>
<old_string>return false;</old_string>
<new_string>return true;</new_string>
</edit>
```

**Fuzzy Matching:** If exact matching fails, the tool automatically tries progressively relaxed matching:
1. **Exact match** — verbatim string comparison
2. **Line-trimmed** — strips leading/trailing whitespace from each line
3. **Whitespace-normalized** — collapses all runs of whitespace to single spaces
4. **Indent-flexible** — matches code structure regardless of base indentation level

Replace all occurrences:
```xml
<edit>
<file_path>config.json</file_path>
<old_string>"debug": false</old_string>
<new_string>"debug": true</new_string>
<replace_all>true</replace_all>
</edit>
```

#### Symbol Replace Mode — Rewrite by Name

Provide `symbol` with the name of a function, class, or method and `new_string` with the complete new implementation. No need to quote the old code.

```xml
<edit>
<file_path>src/utils.js</file_path>
<symbol>calculateTotal</symbol>
<new_string>function calculateTotal(items) {
  return items.reduce((sum, item) => sum + item.price * item.quantity, 0);
}</new_string>
</edit>
```

The tool uses tree-sitter AST parsing to find the symbol by name, then replaces the entire definition with your `new_string`. Supported across 16 languages: JavaScript, TypeScript, Python, Rust, Go, Java, C, C++, Ruby, PHP, Swift, Kotlin, Scala, C#, Lua, Zig.

**Auto-indentation:** The tool detects the original symbol's indentation level and reindents your `new_string` to match.

**Symbol naming:** Use the name as it appears in source — functions: `"calculateTotal"`, classes: `"UserService"`, methods: `"MyClass.myMethod"` (dot notation).

#### Symbol Insert Mode — Add Code Near a Symbol

Provide `symbol`, `new_string`, and `position` (`"before"` or `"after"`) to insert code near an existing symbol.

```xml
<edit>
<file_path>src/utils.js</file_path>
<symbol>calculateTotal</symbol>
<position>after</position>
<new_string>function calculateTax(total, rate) {
  return total * rate;
}</new_string>
</edit>
```

#### Line-Targeted Mode — Edit by Line Number

Use line numbers from `extract` or `search` output to make precise edits. Ideal for editing inside large functions without rewriting the entire symbol.

```xml
<edit>
<file_path>src/main.js</file_path>
<start_line>42</start_line>
<end_line>55</end_line>
<new_string>  // simplified implementation
  return processItems(order.items);</new_string>
</edit>
```

When `allowEdit` is enabled, `hashLines` is on by default — line references include content hashes for integrity verification (e.g. `"42:ab"`). If the hash doesn't match, the error provides updated references. Disable with `hashLines: false` or `--no-hash-lines`.

**Heuristic auto-corrections** handle common LLM mistakes: stripping accidental line-number prefixes, removing echoed boundary lines, and restoring indentation.

**Error Handling — Self-Healing Messages:**

All error messages include specific recovery instructions. When an edit fails, the error tells the AI exactly how to fix the call and retry. For example:
- **String not found:** Suggests reading current file content, trying symbol mode, verifying path
- **Symbol not found:** Suggests using `search`/`extract` to find the correct name, offers text mode fallback
- **Multiple occurrences:** Suggests `replace_all=true` or adding more context
- **Hash mismatch:** Provides the updated line:hash reference for retrying

**Configuration:**
```javascript
const agent = new ProbeAgent({
  path: './src',
  allowEdit: true,
  allowedFolders: ['./src', './tests']  // restrict to specific directories
});
```

**Extract→Edit Workflow (Large Functions):**

For editing inside large functions, combine `extract` (to get line numbers) with `edit` (to make precise changes):

```xml
<!-- Step 1: Extract the function to see its line numbers -->
<extract>
<targets>src/order.js#processOrder</targets>
</extract>

<!-- Output shows:
  142:ab | function processOrder(order) {
  143:cd |   const items = order.items;
  ...
  189:ef |   return total;
  190:12 | }
-->

<!-- Step 2: Edit specific lines within the function -->
<edit>
<file_path>src/order.js</file_path>
<start_line>143:cd</start_line>
<end_line>145</end_line>
<new_string>  const items = order.items.filter(i => i.active);
  const validated = validateItems(items);</new_string>
</edit>
```

With `allowEdit`, `hashLines` is on by default — the extract output includes content hashes (e.g. `143:cd`) for integrity verification in the edit call. If the file changed since extraction, the error provides updated references.

**File State Tracking (Multi-Edit Safety):**

When `allowEdit: true`, the agent automatically tracks which files have been read via `search`/`extract`. Before any edit, the tracker verifies that:
1. The file was previously read (blocks blind edits)
2. The file hasn't been modified since it was last read (detects stale content)

After each successful write, the tracker is updated so chained edits to the same file work correctly. If a check fails, the error message guides the LLM to re-read the file with `extract` before retrying.

**Standalone SDK Usage:**
```javascript
import { editTool } from '@probelabs/probe';

const edit = editTool({
  allowedFolders: ['/path/to/project'],
  cwd: '/path/to/project'
});

// Text edit
await edit.execute({
  file_path: 'src/main.js',
  old_string: 'return false;',
  new_string: 'return true;'
});

// Symbol replace
await edit.execute({
  file_path: 'src/utils.js',
  symbol: 'calculateTotal',
  new_string: `function calculateTotal(items) {
  return items.reduce((sum, item) => sum + item.price * item.quantity, 0);
}`
});

// Symbol insert
await edit.execute({
  file_path: 'src/utils.js',
  symbol: 'calculateTotal',
  position: 'after',
  new_string: `function calculateTax(total, rate) {
  return total * rate;
}`
});

// Line-targeted edit (replace a range)
await edit.execute({
  file_path: 'src/order.js',
  start_line: '143',
  end_line: '145',
  new_string: '  const items = order.items.filter(i => i.active);'
});

// Line-targeted edit with hash verification
await edit.execute({
  file_path: 'src/order.js',
  start_line: '143:cd',
  new_string: '  const items = order.items.filter(i => i.active);'
});

// Insert after a line
await edit.execute({
  file_path: 'src/order.js',
  start_line: '142',
  position: 'after',
  new_string: '  console.log("Processing order:", order.id);'
});
```

---

### create

Create new files with specified content. Parent directories are created automatically.

**Enabled By:** `allowEdit: true`

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `file_path` | string | Yes | Path where the file should be created |
| `content` | string | Yes | Content to write to the file |
| `overwrite` | boolean | No | Whether to overwrite if file exists (default: false) |

**Example AI Usage:**
```xml
<create>
<file_path>src/utils/helpers.ts</file_path>
<content>export function formatDate(date: Date): string {
  return date.toISOString();
}</content>
</create>
```

Overwrite an existing file:
```xml
<create>
<file_path>src/config.json</file_path>
<content>{"debug": true, "verbose": false}</content>
<overwrite>true</overwrite>
</create>
```

---

### listFiles

List files and directories.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `directory` | string | No | Directory to list (default: workspace root) |
| `maxDepth` | number | No | Maximum directory depth |

**Example AI Usage:**
```xml
<listFiles>
  <directory>./src/components</directory>
  <maxDepth>2</maxDepth>
</listFiles>
```

---

### searchFiles

Search for files by name pattern.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `pattern` | string | Yes | File name pattern (glob) |
| `directory` | string | No | Search directory |
| `limit` | number | No | Maximum results |

**Example AI Usage:**
```xml
<searchFiles>
  <pattern>*.test.ts</pattern>
  <directory>./src</directory>
  <limit>20</limit>
</searchFiles>
```

---

## Execution Tool

### bash

Execute shell commands.

**Enabled By:** `enableBash: true`

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `command` | string | Yes | Command to execute |
| `cwd` | string | No | Working directory |
| `timeout` | number | No | Timeout in ms (default: 30000) |

**Example AI Usage:**
```xml
<bash>
  <command>npm test -- --coverage</command>
  <cwd>./</cwd>
  <timeout>60000</timeout>
</bash>
```

**Configuration:**
```javascript
const agent = new ProbeAgent({
  path: './src',
  enableBash: true,
  bashConfig: {
    allow: ['npm test', 'npm run *', 'git status'],
    deny: ['rm -rf', 'sudo *'],
    disableDefaultAllow: false,
    disableDefaultDeny: false,
    debug: false
  }
});
```

**Default Allowed Commands:**
- `npm`, `yarn`, `pnpm` (package managers)
- `node`, `npx` (Node.js)
- `git` (version control)
- `cat`, `ls`, `pwd`, `echo` (basic utilities)

**Default Denied Commands:**
- `rm -rf /`, `rm -rf ~` (destructive)
- `sudo` (privilege escalation)
- `chmod 777` (dangerous permissions)

---

## Analysis Tools

### analyze_all

Comprehensive codebase analysis.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | No | Directory to analyze |
| `depth` | string | No | Analysis depth: 'shallow', 'normal', 'deep' |

**Example AI Usage:**
```xml
<analyze_all>
  <path>./src</path>
  <depth>normal</depth>
</analyze_all>
```

---

### readImage

Load and analyze images.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | Yes | Path to image file |

**Supported Formats:** `.png`, `.jpg`, `.jpeg`, `.gif`, `.webp`, `.svg`

**Example AI Usage:**
```xml
<readImage>
  <path>./docs/architecture-diagram.png</path>
</readImage>
```

---

## Agent Control Tools

### delegate

Delegate tasks to specialized subagents.

**Enabled By:** `enableDelegate: true`

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `task` | string | Yes | Self-contained task description |
| `subagent` | string | No | Subagent type |
| `context` | string | No | Additional context |

**Example AI Usage:**
```xml
<delegate>
  <task>Research how authentication is implemented and summarize the flow</task>
</delegate>
```

**Configuration:**
```javascript
const agent = new ProbeAgent({
  path: './src',
  enableDelegate: true
});
```

---

### attempt_completion

Signal task completion.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `result` | string | Yes | Summary of completed work |
| `command` | string | No | Optional command to demonstrate |

**Example AI Usage:**
```xml
<attempt_completion>
  <result>Added input validation to the login form:
- Email format validation
- Password strength requirements
- Real-time error feedback
All tests passing.</result>
</attempt_completion>
```

---

## Skills Tools

### listSkills

List available agent skills.

**Enabled By:** `allowSkills: true`

**Example AI Usage:**
```xml
<listSkills />
```

---

### useSkill

Execute a discovered skill.

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `skillName` | string | Yes | Name of skill |
| `input` | any | No | Skill parameters |

**Example AI Usage:**
```xml
<useSkill>
  <skillName>generate-tests</skillName>
  <input>{"file": "src/auth.ts"}</input>
</useSkill>
```

---

## Task Management

### task

Manage multi-step tasks.

**Enabled By:** `enableTasks: true`

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `action` | string | Yes | 'create', 'update', 'complete', 'list' |
| `taskId` | string | Conditional | Required for update/complete |
| `title` | string | Conditional | Required for create |
| `description` | string | No | Task description |
| `status` | string | No | Task status |
| `blockedBy` | string[] | No | Blocking task IDs |

**Example AI Usage:**
```xml
<task>
  <action>create</action>
  <title>Implement user authentication</title>
  <description>Add login/logout functionality with JWT tokens</description>
</task>
```

---

## MCP Tools

When MCP is enabled, external tools are available with `mcp__` prefix:

```javascript
const agent = new ProbeAgent({
  enableMcp: true,
  mcpConfig: {
    mcpServers: {
      'github': {
        command: 'npx',
        args: ['-y', '@modelcontextprotocol/server-github'],
        transport: 'stdio',
        enabled: true
      }
    }
  }
});
```

**MCP tools appear as:**
- `mcp__github__create_issue`
- `mcp__github__list_pulls`
- `mcp__filesystem__read_file`

---

## Tool Filtering

Control which tools are available to the AI:

```javascript
// All tools (default)
const agent = new ProbeAgent({
  allowedTools: ['*']
});

// Specific tools only
const agent = new ProbeAgent({
  allowedTools: ['search', 'extract', 'query']
});

// Exclude specific tools
const agent = new ProbeAgent({
  allowedTools: ['*', '!bash', '!edit']
});

// No tools (conversation only)
const agent = new ProbeAgent({
  disableTools: true
});
```

---

## Tool Execution Events

Monitor tool execution:

```javascript
agent.events.on('toolCall', (event) => {
  console.log(`Tool: ${event.name}`);
  console.log(`Status: ${event.status}`);
  console.log(`Parameters: ${JSON.stringify(event.params)}`);

  if (event.status === 'completed') {
    console.log(`Duration: ${event.duration}ms`);
    console.log(`Result: ${event.result}`);
  }

  if (event.status === 'failed') {
    console.log(`Error: ${event.error}`);
  }
});
```

---

## Related Documentation

- [API Reference](./api-reference.md) - Complete ProbeAgent documentation
- [Hooks System](./hooks.md) - Event hooks for tool execution
- [MCP Protocol](../protocols/mcp.md) - Adding external MCP tools
- [Delegation](../advanced/delegation.md) - Task delegation patterns
