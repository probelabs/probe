# @probelabs/probe

A Node.js wrapper for the [probe](https://github.com/probelabs/probe) code search tool.

## Installation

### Local Installation

```bash
npm install @probelabs/probe
```

### Global Installation

```bash
npm install -g @probelabs/probe
```

During installation, the package will automatically download the appropriate probe binary for your platform.

## Features

- **Search Code**: Search for patterns in your codebase using Elasticsearch-like query syntax
- **Query Code**: Find specific code structures using tree-sitter patterns
- **Extract Code**: Extract code blocks from files based on file paths and line numbers
- **Edit Code**: AI-powered code editing with text replacement (fuzzy matching), AST-aware symbol replace/insert across 16 languages, and line-targeted editing with optional hash-based integrity verification
- **AI Tools Integration**: Ready-to-use tools for Vercel AI SDK, LangChain, and other AI frameworks
- **System Message**: Default system message for AI assistants with instructions on using probe tools
- **Cross-Platform**: Works on Windows, macOS, and Linux
- **Automatic Binary Management**: Automatically downloads and manages the probe binary
- **Direct CLI Access**: Use the probe binary directly from the command line when installed globally
- **MCP Server**: Built-in Model Context Protocol server for AI assistant integration
- **Context Window Compaction**: Automatic conversation history compression when approaching token limits

## Usage

### Using as a Node.js Library

```javascript
import { search, query, extract } from '@probelabs/probe';

// Search for code
const searchResults = await search({
  path: '/path/to/your/project',
  query: 'function',
  maxResults: 10
});

// Query for specific code structures
const queryResults = await query({
  path: '/path/to/your/project',
  pattern: 'function $NAME($$$PARAMS) $$$BODY',
  language: 'javascript'
});

// Extract code blocks
const extractResults = await extract({
  files: ['/path/to/your/project/src/main.js:42']
});
```

### Using as a Command-Line Tool

When installed globally, the `probe` command will be available directly from the command line:

```bash
# Search for code
probe search "function" /path/to/your/project

# Query for specific code structures
probe query "function $NAME($$$PARAMS) $$$BODY" /path/to/your/project

# Extract code blocks
probe extract /path/to/your/project/src/main.js:42

# Run MCP server for AI assistant integration
probe mcp
```

The package installs the actual probe binary, not a JavaScript wrapper, so you get the full native performance and all features of the original probe CLI.

### Using ProbeAgent (AI-Powered Code Assistant)

ProbeAgent provides a high-level AI-powered interface for interacting with your codebase:

```javascript
import { ProbeAgent } from '@probelabs/probe';

// Create an AI agent for your project
const agent = new ProbeAgent({
  sessionId: 'my-session',  // Optional: for conversation continuity
  path: '/path/to/your/project',
  provider: 'anthropic',   // or 'openai', 'google'
  model: 'claude-3-5-sonnet-20241022',  // Optional: override model
  allowEdit: true,         // Optional: enable edit + create tools for code modification
  debug: true,            // Optional: enable debug logging
  allowedTools: ['*'],    // Optional: filter available tools (see Tool Filtering below)
  enableMcp: true,        // Optional: enable MCP tool integration
  mcpConfig: {           // Optional: MCP configuration (see MCP section below)
    mcpServers: {...}
  }
});

// Ask questions about your codebase
const answer = await agent.answer("How does authentication work in this codebase?");
console.log(answer);

// The agent maintains conversation history automatically
const followUp = await agent.answer("Can you show me the login implementation?");
console.log(followUp);

// Get token usage statistics
const usage = agent.getTokenUsage();
console.log(`Used ${usage.total} tokens total`);

// Clear conversation history if needed
agent.history = [];
```

**Environment Variables:**
```bash
# Set your API key for the chosen provider
export ANTHROPIC_API_KEY=your_anthropic_key
export OPENAI_API_KEY=your_openai_key  
export GOOGLE_API_KEY=your_google_key

# Optional: Force a specific provider
export FORCE_PROVIDER=anthropic

# Optional: Override model name
export MODEL_NAME=claude-3-5-sonnet-20241022
```

**ProbeAgent Features:**
- **Multi-turn conversations** with automatic history management
- **Code search integration** - Uses probe's search capabilities transparently
- **Multiple AI providers** - Supports Anthropic Claude, OpenAI GPT, Google Gemini, AWS Bedrock
- **Automatic retry with exponential backoff** - Handles transient API failures gracefully
- **Provider fallback** - Seamlessly switch between providers if one fails (e.g., Azure Claude → Bedrock Claude → OpenAI)
- **Session management** - Maintain conversation context across calls
- **Token tracking** - Monitor usage and costs
- **Configurable personas** - Engineer, architect, code-review, and more

### Agent Skills (repo-local)

ProbeAgent can discover and activate Agent Skills stored inside your repository. Place skills under:
- `.claude/skills/<skill-name>/SKILL.md`
- `.codex/skills/<skill-name>/SKILL.md`
- `skills/<skill-name>/SKILL.md`
- `.skills/<skill-name>/SKILL.md`

`SKILL.md` should contain YAML frontmatter followed by Markdown instructions. Example:

```markdown
---
name: onboarding
description: Help new engineers understand the repo structure and conventions.
---

Use this skill to explain key modules, build steps, and common workflows.
```

Then in the agent loop you can call:
```xml
<listSkills></listSkills>
```
or:
```xml
<useSkill>
<name>onboarding</name>
</useSkill>
```

### Retry and Fallback Support

ProbeAgent includes comprehensive retry and fallback capabilities for maximum reliability:

```javascript
import { ProbeAgent } from '@probelabs/probe';

const agent = new ProbeAgent({
  path: '/path/to/your/project',

  // Configure retry behavior
  retry: {
    maxRetries: 5,           // Retry up to 5 times per provider
    initialDelay: 1000,      // Start with 1 second delay
    maxDelay: 30000,         // Cap delays at 30 seconds
    backoffFactor: 2         // Double the delay each time
  },

  // Configure provider fallback
  fallback: {
    strategy: 'custom',
    providers: [
      {
        provider: 'anthropic',
        apiKey: process.env.ANTHROPIC_API_KEY,
        model: 'claude-3-7-sonnet-20250219',
        maxRetries: 5  // Can override retry config per provider
      },
      {
        provider: 'bedrock',
        region: 'us-west-2',
        accessKeyId: process.env.AWS_ACCESS_KEY_ID,
        secretAccessKey: process.env.AWS_SECRET_ACCESS_KEY,
        model: 'anthropic.claude-sonnet-4-20250514-v1:0'
      },
      {
        provider: 'openai',
        apiKey: process.env.OPENAI_API_KEY,
        model: 'gpt-4o'
      }
    ],
    maxTotalAttempts: 15  // Maximum attempts across all providers
  }
});

// API calls automatically retry on failures and fallback to other providers
const answer = await agent.answer("Explain this codebase");
```

**Retry & Fallback Features:**
- **Exponential backoff** - Intelligently delays retries to avoid overwhelming APIs
- **Automatic error detection** - Retries on transient errors (Overloaded, 429, 503, timeouts, network errors)
- **Multi-provider support** - Fallback across Anthropic, OpenAI, Google, and AWS Bedrock
- **Cross-cloud failover** - Use Azure Claude → Bedrock Claude → OpenAI as fallback chain
- **Statistics tracking** - Monitor retry attempts and provider usage
- **Environment variable support** - Configure via env vars for easy deployment

**Quick Setup with Auto-Fallback:**
```bash
# Set all your API keys
export ANTHROPIC_API_KEY=sk-ant-xxx
export OPENAI_API_KEY=sk-xxx
export GOOGLE_API_KEY=xxx
export AUTO_FALLBACK=1  # Enable automatic fallback
export MAX_RETRIES=5    # Configure retry limit
```

```javascript
// No configuration needed - uses all available providers automatically!
const agent = new ProbeAgent({
  path: '/path/to/your/project',
  fallback: { auto: true }
});
```

See [docs/RETRY_AND_FALLBACK.md](./docs/RETRY_AND_FALLBACK.md) for complete documentation and examples.

### Tool Filtering

ProbeAgent supports filtering available tools to control what operations the AI can perform. This is useful for security, cost control, or limiting functionality to specific use cases.

```javascript
import { ProbeAgent } from '@probelabs/probe';

// Allow all tools (default behavior)
const agent1 = new ProbeAgent({
  path: '/path/to/project',
  allowedTools: ['*']  // or undefined
});

// Allow only specific tools (whitelist mode)
const agent2 = new ProbeAgent({
  path: '/path/to/project',
  allowedTools: ['search', 'query', 'extract']
});

// Allow all except specific tools (exclusion mode)
const agent3 = new ProbeAgent({
  path: '/path/to/project',
  allowedTools: ['*', '!bash', '!implement']
});

// Raw AI mode - no tools at all
const agent4 = new ProbeAgent({
  path: '/path/to/project',
  allowedTools: []  // or use disableTools: true
});

// Convenience flag for raw AI mode (better DX)
const agent5 = new ProbeAgent({
  path: '/path/to/project',
  disableTools: true  // Clearer than allowedTools: []
});
```

**Available Tools:**
- `search` - Semantic code search
- `query` - Tree-sitter pattern matching
- `extract` - Extract code blocks
- `listFiles` - List files and directories
- `searchFiles` - Find files by glob pattern
- `bash` - Execute bash commands (requires `enableBash: true`)
- `edit` - Edit files with text replacement, AST-aware symbol editing, or line-targeted editing (requires `allowEdit: true`)
- `create` - Create new files (requires `allowEdit: true`)
- `delegate` - Delegate tasks to subagents (requires `enableDelegate: true`)
- `attempt_completion` - Signal task completion
- `mcp__*` - MCP tools use the `mcp__` prefix (e.g., `mcp__filesystem__read_file`)

**MCP Tool Filtering:**
MCP tools follow the `mcp__toolname` naming convention. You can:
- Allow all MCP tools: `allowedTools: ['*']`
- Allow specific MCP tool: `allowedTools: ['mcp__filesystem__read_file']`
- Allow all from a server: `allowedTools: ['mcp__filesystem__*']` (using pattern matching)
- Block MCP tools: `allowedTools: ['*', '!mcp__*']`

**CLI Usage:**
```bash
# Allow only search and extract tools
probe agent "Explain this code" --allowed-tools search,extract

# Raw AI mode (no tools) - option 1
probe agent "What is this project about?" --allowed-tools none

# Raw AI mode (no tools) - option 2 (better DX)
probe agent "Tell me about this project" --disable-tools

# All tools (default)
probe agent "Analyze the architecture" --allowed-tools all
```

**Notes:**
- Tool filtering works in conjunction with feature flags (`allowEdit`, `enableBash`, `enableDelegate`)
- Both the feature flag AND `allowedTools` must permit a tool for it to be available
- Blocked tools will not appear in the system message and cannot be executed
- Use `allowedTools: []` for pure conversational AI without code analysis tools

### Code Editing (Edit & Create Tools)

Probe provides built-in code modification tools that work across all interfaces. The `edit` tool supports four modes: text-based find/replace with fuzzy matching, AST-aware symbol replacement, symbol insertion, and line-targeted editing with optional hash-based integrity verification.

#### Enabling Edit Tools

**CLI Agent:**
```bash
# Enable with --allow-edit flag
probe agent "Fix the bug in auth.js" --allow-edit --path ./my-project

# Enable with environment variable
ALLOW_EDIT=1 probe agent "Refactor the login flow" --path ./my-project
```

**SDK (ProbeAgent):**
```javascript
import { ProbeAgent } from '@probelabs/probe';

const agent = new ProbeAgent({
  path: '/path/to/project',
  allowEdit: true,       // enables edit + create tools
  // hashLines defaults to true when allowEdit is true (line integrity verification)
  provider: 'anthropic'
});

// The agent can now modify files when answering questions
const answer = await agent.answer("Fix the off-by-one error in calculateTotal");
```

**SDK (Standalone Tools):**
```javascript
import { editTool, createTool } from '@probelabs/probe';

const edit = editTool({
  allowedFolders: ['/path/to/project'],
  cwd: '/path/to/project'
});

const create = createTool({
  allowedFolders: ['/path/to/project'],
  cwd: '/path/to/project'
});
```

#### Edit Tool Modes

| Mode | Parameters | Use Case |
|------|-----------|----------|
| **Text edit** | `old_string` + `new_string` | Small, precise changes: fix a condition, rename a variable, update a value |
| **Symbol replace** | `symbol` + `new_string` | Replace an entire function/class/method by name (AST-aware, 16 languages) |
| **Symbol insert** | `symbol` + `new_string` + `position` | Insert new code before or after an existing symbol |
| **Line-targeted edit** | `start_line` + `new_string` | Edit specific lines from extract/search output; ideal for changes inside large functions |

#### Edit Tool Parameters

| Parameter | Required | Description |
|-----------|----------|-------------|
| `file_path` | Yes | Path to the file to edit (absolute or relative to cwd) |
| `new_string` | Yes | Replacement text or new code content |
| `old_string` | No | Text to find and replace. Copy verbatim from the file. |
| `replace_all` | No | Replace all occurrences (default: `false`, text mode only) |
| `symbol` | No | Code symbol name for AST-aware editing (e.g. `"myFunction"`, `"MyClass.myMethod"`) |
| `position` | No | `"before"` or `"after"` — insert near the symbol or line instead of replacing it |
| `start_line` | No | Line reference for line-targeted editing (e.g. `"42"` or `"42:ab"` with hash) |
| `end_line` | No | End of line range, inclusive (e.g. `"55"` or `"55:cd"`). Defaults to `start_line`. |

#### Examples

**Text edit — find and replace:**
```javascript
await edit.execute({
  file_path: 'src/main.js',
  old_string: 'return false;',
  new_string: 'return true;'
});
// => "Successfully edited src/main.js (1 replacement)"
```

**Text edit — fuzzy matching handles whitespace differences:**
```javascript
// File has: "  const x = 1;"  (2-space indent)
// Your search string omits the indent — fuzzy matching finds it anyway
await edit.execute({
  file_path: 'src/main.js',
  old_string: 'const x = 1;',
  new_string: '  const x = 2;'
});
// => "Successfully edited src/main.js (1 replacement, matched via line-trimmed)"
```

**Symbol replace — rewrite a function by name:**
```javascript
await edit.execute({
  file_path: 'src/utils.js',
  symbol: 'calculateTotal',
  new_string: `function calculateTotal(items) {
  return items.reduce((sum, item) => sum + item.price * item.quantity, 0);
}`
});
// => "Successfully replaced symbol "calculateTotal" in src/utils.js (was lines 10-15, now 3 lines)"
```

**Symbol insert — add a new function after an existing one:**
```javascript
await edit.execute({
  file_path: 'src/utils.js',
  symbol: 'calculateTotal',
  position: 'after',
  new_string: `function calculateTax(total, rate) {
  return total * rate;
}`
});
// => "Successfully inserted 3 lines after symbol "calculateTotal" in src/utils.js (at line 16)"
```

**Line-targeted edit — edit specific lines from extract output:**
```javascript
// After extracting "src/order.js#processOrder" and seeing lines 143-145
await edit.execute({
  file_path: 'src/order.js',
  start_line: '143',
  end_line: '145',
  new_string: '  const items = order.items.filter(i => i.active);'
});
// => "Successfully edited src/order.js (lines 143-145 replaced with 1 line)"
```

**Line-targeted edit — with hash integrity verification:**
```javascript
// Use the hash from extract output (e.g. "143:cd") for verification
await edit.execute({
  file_path: 'src/order.js',
  start_line: '143:cd',
  new_string: '  const items = order.items.filter(i => i.active);'
});
// If hash matches: edit succeeds
// If hash doesn't match: error with updated reference for retry
```

**Create a new file:**
```javascript
await create.execute({
  file_path: 'src/newModule.js',
  content: `export function greet(name) {
  return \`Hello, \${name}!\`;
}`
});
// => "Successfully created src/newModule.js (52 bytes)"
```

#### Key Features

- **Fuzzy matching**: Text mode automatically handles minor whitespace and indentation differences between your search string and the actual file content. Four strategies are tried in order: exact match, line-trimmed, whitespace-normalized, indent-flexible.
- **AST-aware symbol editing**: Symbol mode uses tree-sitter to locate functions, classes, and methods by name across 16 languages (JavaScript, TypeScript, Python, Rust, Go, Java, C, C++, Ruby, PHP, Swift, Kotlin, Scala, C#, Lua, Zig).
- **Auto-indentation**: Symbol mode automatically adjusts the indentation of `new_string` to match the original symbol's indentation level.
- **Line-targeted editing**: Use line numbers from `extract`/`search` output to make precise edits inside large functions. Optional hash-based integrity verification detects stale references.
- **Extract→Edit workflow**: Extract a function by symbol name (e.g. `"file.js#myFunction"`), then use the line numbers from the output to surgically edit specific lines with `start_line`/`end_line`.
- **File state tracking**: Automatic per-session tracking ensures edits are never based on stale content. Files read via `search`/`extract` are tracked; edits to unread or modified files are blocked with recovery instructions.
- **Self-healing errors**: All error messages include specific recovery instructions telling you (or the LLM) exactly how to fix the call and retry.
- **Security**: Path traversal attacks are blocked — all file operations are restricted to `allowedFolders`.

### Using as an MCP Server

Probe includes a built-in MCP (Model Context Protocol) server for integration with AI assistants:

```bash
# Start the MCP server
probe mcp

# With custom timeout
probe mcp --timeout 60
```

Add to your AI assistant's MCP configuration:

```json
{
  "mcpServers": {
    "probe": {
      "command": "npx",
      "args": ["-y", "@probelabs/probe", "mcp"]
    }
  }
}
```

### Using MCP with ProbeAgent SDK

When using ProbeAgent programmatically, you can integrate MCP servers to extend the agent's capabilities:

```javascript
const agent = new ProbeAgent({
  enableMcp: true,  // Enable MCP support

  // Option 1: Provide MCP configuration directly
  mcpConfig: {
    mcpServers: {
      'my-server': {
        command: 'node',
        args: ['path/to/server.js'],
        transport: 'stdio',
        enabled: true
      }
    }
  },

  // Option 2: Load from config file
  mcpConfigPath: '/path/to/mcp-config.json',

  // Option 3: Auto-discovery from standard locations
  // (~/.mcp/config.json, or via MCP_CONFIG_PATH env var)
});
```

**Note:** MCP tools are automatically initialized when needed (lazy initialization), so you don't need to call `agent.initialize()` when using the SDK.

## Claude Code Integration

ProbeAgent now supports Claude Code's `claude` command for zero-configuration usage in Claude Code environments. See the [Claude Code Integration Guide](./docs/CLAUDE_CODE_INTEGRATION.md) for full details.

### Quick Start

```javascript
import { ProbeAgent } from '@probelabs/probe';

// Works automatically if claude command is installed!
const agent = new ProbeAgent({
  allowedFolders: ['/path/to/your/code']
});

await agent.initialize();
const response = await agent.answer('Explain how this codebase works');
```

### Auto-Fallback

ProbeAgent automatically detects and uses Claude Code when:
- No API keys are configured (no ANTHROPIC_API_KEY, OPENAI_API_KEY, etc.)
- The `claude` command is available on your system

Priority order:
1. Explicit `provider: 'claude-code'`
2. API keys (Anthropic, OpenAI, Google, AWS)
3. Claude command (auto-detected)

### Features

- **Zero Configuration**: No API keys needed in Claude Code environments
- **Black-box Operation**: Claude Code handles its own agentic loop
- **Tool Event Extraction**: Visibility into internal tool usage
- **Built-in MCP Server**: Provides Probe's semantic search tools
- **Auto-fallback**: Seamlessly switches based on environment

For complete documentation, examples, and troubleshooting, see [docs/CLAUDE_CODE_INTEGRATION.md](./docs/CLAUDE_CODE_INTEGRATION.md).

## API Reference

### Search

```javascript
import { search } from '@probelabs/probe';

const results = await search({
  path: '/path/to/your/project',
  query: 'function',
  // Optional parameters
  filesOnly: false,
  ignore: ['node_modules', 'dist'],
  excludeFilenames: false,
  reranker: 'hybrid',
  frequencySearch: true,
  maxResults: 10,
  maxBytes: 1000000,
  maxTokens: 40000,
  allowTests: false,
  noMerge: false,
  mergeThreshold: 5,
  json: false,
  binaryOptions: {
    forceDownload: false,
    version: '1.0.0'
  }
});
```

#### Parameters

- `path` (required): Path to search in
- `query` (required): Search query or queries (string or array of strings)
- `filesOnly`: Only output file paths
- `ignore`: Patterns to ignore (array of strings)
- `excludeFilenames`: Exclude filenames from search
- `reranker`: Reranking method ('hybrid', 'hybrid2', 'bm25', 'tfidf')
- `frequencySearch`: Use frequency-based search
- `maxResults`: Maximum number of results
- `maxBytes`: Maximum bytes to return
- `maxTokens`: Maximum tokens to return
- `allowTests`: Include test files
- `noMerge`: Don't merge adjacent blocks
- `mergeThreshold`: Merge threshold
- `json`: Return results as parsed JSON instead of string
- `binaryOptions`: Options for getting the binary
  - `forceDownload`: Force download even if binary exists
  - `version`: Specific version to download

### Query

```javascript
import { query } from '@probelabs/probe';

const results = await query({
  path: '/path/to/your/project',
  pattern: 'function $NAME($$$PARAMS) $$$BODY',
  // Optional parameters
  language: 'javascript',
  ignore: ['node_modules', 'dist'],
  allowTests: false,
  maxResults: 10,
  format: 'markdown',
  json: false,
  binaryOptions: {
    forceDownload: false,
    version: '1.0.0'
  }
});
```

#### Parameters

- `path` (required): Path to search in
- `pattern` (required): The ast-grep pattern to search for
- `language`: Programming language to search in
- `ignore`: Patterns to ignore (array of strings)
- `allowTests`: Include test files
- `maxResults`: Maximum number of results
- `format`: Output format ('markdown', 'plain', 'json', 'color')
- `json`: Return results as parsed JSON instead of string
- `binaryOptions`: Options for getting the binary
  - `forceDownload`: Force download even if binary exists
  - `version`: Specific version to download

### Extract

```javascript
import { extract } from '@probelabs/probe';

const results = await extract({
  files: [
    '/path/to/your/project/src/main.js',
    '/path/to/your/project/src/utils.js:42'  // Extract from line 42
  ],
  // Optional parameters
  allowTests: false,
  contextLines: 2,
  format: 'markdown',
  json: false,
  binaryOptions: {
    forceDownload: false,
    version: '1.0.0'
  }
});
```

#### Parameters

- `files` (required): Files to extract from (can include line numbers with colon, e.g., "/path/to/file.rs:10")
- `allowTests`: Include test files
- `contextLines`: Number of context lines to include
- `format`: Output format ('markdown', 'plain', 'json')
- `json`: Return results as parsed JSON instead of string
- `binaryOptions`: Options for getting the binary
  - `forceDownload`: Force download even if binary exists
  - `version`: Specific version to download

### Binary Management

```javascript
import { getBinaryPath, setBinaryPath } from '@probelabs/probe';

// Get the path to the probe binary
const binaryPath = await getBinaryPath({
  forceDownload: false,
  version: '1.0.0'
});

// Manually set the path to the probe binary
setBinaryPath('/path/to/probe/binary');
```

### AI Tools

```javascript
import { tools } from '@probelabs/probe';

// Vercel AI SDK tools
const { searchTool, queryTool, extractTool } = tools;

// LangChain tools
const searchLangChainTool = tools.createSearchTool();
const queryLangChainTool = tools.createQueryTool();
const extractLangChainTool = tools.createExtractTool();
// Access schemas
const { searchSchema, querySchema, extractSchema } = tools;

// Access default system message
const systemMessage = tools.DEFAULT_SYSTEM_MESSAGE;
```

#### Vercel AI SDK Tools

- `searchTool`: Tool for searching code using Elasticsearch-like query syntax
- `queryTool`: Tool for searching code using tree-sitter patterns
- `extractTool`: Tool for extracting code blocks from files

#### LangChain Tools

- `createSearchTool()`: Creates a tool for searching code using Elasticsearch-like query syntax
- `createQueryTool()`: Creates a tool for searching code using tree-sitter patterns
- `createExtractTool()`: Creates a tool for extracting code blocks from files

#### Schemas

- `searchSchema`: Zod schema for search tool parameters
- `querySchema`: Zod schema for query tool parameters
- `extractSchema`: Zod schema for extract tool parameters

#### System Message

- `DEFAULT_SYSTEM_MESSAGE`: Default system message for AI assistants with instructions on how to use the probe tools
- `extractSchema`: Zod schema for extract tool parameters

## Examples

### Basic Search Example

```javascript
import { search } from '@probelabs/probe';

async function basicSearchExample() {
  try {
    const results = await search({
      path: '/path/to/your/project',
      query: 'function',
      maxResults: 5
    });
    
    console.log('Search results:');
    console.log(results);
  } catch (error) {
    console.error('Search error:', error);
  }
}
```

### Advanced Search with Multiple Options

```javascript
import { search } from '@probelabs/probe';

async function advancedSearchExample() {
  try {
    const results = await search({
      path: '/path/to/your/project',
      query: 'config AND (parse OR tokenize)',
      ignore: ['node_modules', 'dist'],
      reranker: 'hybrid',
      frequencySearch: true,
      maxResults: 10,
      maxTokens: 20000,
      allowTests: false
    });
    
    console.log('Advanced search results:');
    console.log(results);
  } catch (error) {
    console.error('Advanced search error:', error);
  }
}
```

### Query for Specific Code Structures

```javascript
import { query } from '@probelabs/probe';

async function queryExample() {
  try {
    // Find all JavaScript functions
    const jsResults = await query({
      path: '/path/to/your/project',
      pattern: 'function $NAME($$$PARAMS) $$$BODY',
      language: 'javascript',
      maxResults: 5
    });
    
    console.log('JavaScript functions:');
    console.log(jsResults);
    
    // Find all Rust structs
    const rustResults = await query({
      path: '/path/to/your/project',
      pattern: 'struct $NAME $$$BODY',
      language: 'rust',
      maxResults: 5
    });
    
    console.log('Rust structs:');
    console.log(rustResults);
  } catch (error) {
    console.error('Query error:', error);
  }
}
```

### Extract Code Blocks

```javascript
import { extract } from '@probelabs/probe';

async function extractExample() {
  try {
    const results = await extract({
      files: [
        '/path/to/your/project/src/main.js',
        '/path/to/your/project/src/utils.js:42'  // Extract from line 42
      ],
      contextLines: 2,
      format: 'markdown'
    });
    
    console.log('Extracted code:');
    console.log(results);
  } catch (error) {
    console.error('Extract error:', error);
  }
}
```

## How It Works

When you install this package:

1. A placeholder binary is included in the package
2. During installation, the postinstall script downloads the actual probe binary for your platform
3. The placeholder is replaced with the actual binary
4. When installed globally, npm creates a symlink to this binary in your system path

This approach ensures that you get the actual native binary, not a JavaScript wrapper, providing full performance and all features of the original probe CLI.

## AI Tools Integration

The package provides built-in tools for integrating with AI SDKs like Vercel AI SDK and LangChain, allowing you to use probe's powerful code search capabilities in AI applications.

### Using with Vercel AI SDK

```javascript
import { generateText } from 'ai';
import { tools } from '@probelabs/probe';

// Use the pre-built tools with Vercel AI SDK
async function chatWithAI(userMessage) {
  const result = await generateText({
    model: provider(modelName),
    messages: [{ role: 'user', content: userMessage }],
    system: "You are a code intelligence assistant. Use the provided tools to search and analyze code.",
    tools: {
      search: tools.searchTool,
      query: tools.queryTool,
      extract: tools.extractTool
    },
    maxSteps: 15,
    temperature: 0.7
  });
  
  return result.text;
}
```

### Using with LangChain

```javascript
import { ChatOpenAI } from '@langchain/openai';
import { tools } from '@probelabs/probe';

// Create the LangChain tools
const searchTool = tools.createSearchTool();
const queryTool = tools.createQueryTool();
const extractTool = tools.createExtractTool();

// Create a ChatOpenAI instance with tools
const model = new ChatOpenAI({
  modelName: "gpt-4o",
  temperature: 0.7
}).withTools([searchTool, queryTool, extractTool]);

// Use the model with tools
async function chatWithAI(userMessage) {
  const result = await model.invoke([
    { role: "system", content: "You are a code intelligence assistant. Use the provided tools to search and analyze code." },
    { role: "user", content: userMessage }
  ]);
  
  return result.content;
}
```

### Using the Default System Message

The package provides a default system message that you can use with your AI assistants:

```javascript
import { tools } from '@probelabs/probe';

// Use the default system message in your AI application
const systemMessage = tools.DEFAULT_SYSTEM_MESSAGE;

// Example with Vercel AI SDK
const result = await generateText({
  model: provider(modelName),
  messages: [{ role: 'user', content: userMessage }],
  system: tools.DEFAULT_SYSTEM_MESSAGE,
  tools: {
    search: tools.searchTool,
    query: tools.queryTool,
    extract: tools.extractTool
  }
});
```

The default system message provides instructions for AI assistants on how to use the probe tools effectively, including search query formatting, tool execution sequence, and best practices.

## License

ISC

## Migration from @probelabs/probe-mcp

If you're migrating from the standalone `@probelabs/probe-mcp` package, `probe mcp` is a drop-in replacement:

**Old usage:**
```bash
npx @probelabs/probe-mcp
# or
probe-mcp --timeout 60
```

**New usage (drop-in replacement):**
```bash
probe mcp
# or  
probe mcp --timeout 60
```

**MCP Configuration:**
```json
// Old configuration
{
  "mcpServers": {
    "probe": {
      "command": "npx",
      "args": ["-y", "@probelabs/probe-mcp"]
    }
  }
}

// New configuration (drop-in replacement)
{
  "mcpServers": {
    "probe": {
      "command": "npx", 
      "args": ["-y", "@probelabs/probe", "mcp"]
    }
  }
}
```

## Additional Documentation

- [Edit & Create Tools](./docs/EDIT_CREATE_TOOLS.md) - Code editing with fuzzy matching, AST-aware symbol editing, and file creation
- [Context Window Compaction](./CONTEXT_COMPACTION.md) - Automatic conversation history compression
- [MCP Integration](./MCP_INTEGRATION_SUMMARY.md) - Model Context Protocol support details
- [Delegate Tool](./DELEGATE_TOOL_README.md) - Task distribution to subagents
- [Maid Integration](./MAID_INTEGRATION.md) - Integration with Maid LLM framework

## Related Projects

- [probe](https://github.com/probelabs/probe) - The core probe code search tool
