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
- **AI Tools Integration**: Ready-to-use tools for Vercel AI SDK, LangChain, and other AI frameworks
- **System Message**: Default system message for AI assistants with instructions on using probe tools
- **Cross-Platform**: Works on Windows, macOS, and Linux
- **Automatic Binary Management**: Automatically downloads and manages the probe binary
- **Direct CLI Access**: Use the probe binary directly from the command line when installed globally
- **MCP Server**: Built-in Model Context Protocol server for AI assistant integration

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
import { ProbeAgent } from '@buger/probe';

// Create an AI agent for your project
const agent = new ProbeAgent({
  sessionId: 'my-session',  // Optional: for conversation continuity  
  path: '/path/to/your/project',
  provider: 'anthropic',   // or 'openai', 'google'
  model: 'claude-3-5-sonnet-20241022',  // Optional: override model
  allowEdit: false,        // Optional: enable code modification
  debug: true             // Optional: enable debug logging
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
- **Multiple AI providers** - Supports Anthropic Claude, OpenAI GPT, Google Gemini
- **Session management** - Maintain conversation context across calls
- **Token tracking** - Monitor usage and costs
- **Configurable personas** - Engineer, architect, code-review, and more

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

## Related Projects

- [probe](https://github.com/probelabs/probe) - The core probe code search tool