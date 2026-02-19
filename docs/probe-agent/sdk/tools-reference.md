# Tools Reference

ProbeAgent provides a comprehensive set of tools that the AI can use to interact with your codebase. This document covers all available tools, their parameters, and usage patterns.

---

## Tool Categories

| Category | Tools | Description |
|----------|-------|-------------|
| **Search & Query** | search, query, extract | Find and retrieve code |
| **File Operations** | edit, create, listFiles, searchFiles | Modify and explore files |
| **Execution** | bash, implement | Run commands and implement features |
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

Edit existing files with precise replacements.

**Enabled By:** `allowEdit: true`

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `file` | string | Yes | File path to edit |
| `edits` | array | Yes | Array of edit operations |

**Edit Object:**

| Field | Type | Description |
|-------|------|-------------|
| `start` | number | Start line (1-indexed) |
| `end` | number | End line (1-indexed) |
| `content` | string | Replacement content |

**Example AI Usage:**
```xml
<edit>
  <file>src/utils.ts</file>
  <edits>
    <edit>
      <start>10</start>
      <end>15</end>
      <content>// New implementation
function processData(data: Data): Result {
  return transform(data);
}</content>
    </edit>
  </edits>
</edit>
```

**Configuration:**
```javascript
const agent = new ProbeAgent({
  path: './src',
  allowEdit: true
});
```

---

### create

Create new files.

**Enabled By:** `allowEdit: true`

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `file` | string | Yes | Path for new file |
| `content` | string | Yes | File content |
| `overwrite` | boolean | No | Allow overwriting (default: false) |

**Example AI Usage:**
```xml
<create>
  <file>src/utils/helpers.ts</file>
  <content>export function formatDate(date: Date): string {
  return date.toISOString();
}</content>
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

## Execution Tools

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

### implement

Implement features using pluggable backends.

**Enabled By:** `allowEdit: true`

**Parameters:**

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `task` | string | Yes | Task description |
| `autoCommits` | boolean | No | Enable auto-commits |

**Example AI Usage:**
```xml
<implement>
  <task>Add input validation to the login form</task>
  <autoCommits>false</autoCommits>
</implement>
```

**Backends:**
- Claude Code (default)
- Aider
- Custom backends

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
