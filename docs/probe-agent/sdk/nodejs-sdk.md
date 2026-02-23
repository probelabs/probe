# Node.js SDK

Probe provides a powerful Node.js SDK that allows you to integrate its code search capabilities directly into your JavaScript and TypeScript applications. This document covers the installation, usage, and advanced features of the Node.js SDK.

## Installation

### Local Installation

```bash
npm install @probelabs/probe@latest
```

### Global Installation

```bash
npm install -g @probelabs/probe@latest
```

During installation, the package will automatically download the appropriate Probe binary for your platform (Windows, macOS, or Linux).

## Features

- **Search Code**: Search for patterns in your codebase using Elasticsearch-like query syntax
- **Query Code**: Find specific code structures using tree-sitter patterns
- **Extract Code**: Extract code blocks from files based on file paths and line numbers
- **AI Tools Integration**: Ready-to-use tools for Vercel AI SDK, LangChain, and other AI frameworks
- **System Message**: Default system message for AI assistants with instructions on using Probe tools
- **Cross-Platform**: Works on Windows, macOS, and Linux
- **Automatic Binary Management**: Automatically downloads and manages the Probe binary
- **Direct CLI Access**: Use the Probe binary directly from the command line when installed globally

## Basic Usage

### Using as a Node.js Library

```javascript
import { search, query, extract } from '@probelabs/probe@latest';

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
```

The package installs the actual Probe binary, not a JavaScript wrapper, so you get the full native performance and all features of the original Probe CLI.

## Core Functions

### Search

The `search` function allows you to search for patterns in your codebase using Elasticsearch-like query syntax.

```javascript
import { search } from '@probelabs/probe@latest';

const results = await search({
  path: '/path/to/your/project',
  query: 'function',
  // Optional parameters
  filesOnly: false,
  ignore: ['node_modules', 'dist'],
  excludeFilenames: false,
  reranker: 'hybrid',
  frequencySearch: true,
  exact: false,
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

| Parameter | Type | Description | Default |
|-----------|------|-------------|---------|
| `path` | string | Path to search in | (required) |
| `query` | string \| string[] | Search query or queries | (required) |
| `filesOnly` | boolean | Only output file paths | `false` |
| `ignore` | string[] | Patterns to ignore | `[]` |
| `excludeFilenames` | boolean | Exclude filenames from search | `false` |
| `reranker` | string | Reranking method ('hybrid', 'hybrid2', 'bm25', 'tfidf') | `'hybrid'` |
| `frequencySearch` | boolean | Use frequency-based search | `true` |
| `exact` | boolean | Use exact matching | `false` |
| `maxResults` | number | Maximum number of results | `10` |
| `maxBytes` | number | Maximum bytes to return | `1000000` |
| `maxTokens` | number | Maximum tokens to return | `40000` |
| `allowTests` | boolean | Include test files | `false` |
| `noMerge` | boolean | Don't merge adjacent blocks | `false` |
| `mergeThreshold` | number | Merge threshold | `5` |
| `session` | string | Session ID for caching results | `''` |
| `json` | boolean | Return results as parsed JSON instead of string | `false` |
| `binaryOptions` | object | Options for getting the binary | `{}` |

### Query

The `query` function allows you to find specific code structures using tree-sitter patterns.

```javascript
import { query } from '@probelabs/probe@latest';

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

| Parameter | Type | Description | Default |
|-----------|------|-------------|---------|
| `path` | string | Path to search in | (required) |
| `pattern` | string | The ast-grep pattern to search for | (required) |
| `language` | string | Programming language to search in | (inferred from files) |
| `ignore` | string[] | Patterns to ignore | `[]` |
| `allowTests` | boolean | Include test files | `false` |
| `maxResults` | number | Maximum number of results | `10` |
| `format` | string | Output format ('markdown', 'plain', 'json', 'color') | `'markdown'` |
| `json` | boolean | Return results as parsed JSON instead of string | `false` |
| `binaryOptions` | object | Options for getting the binary | `{}` |

### Extract

The `extract` function allows you to extract code blocks from files based on file paths and line numbers.

```javascript
import { extract } from '@probelabs/probe@latest';

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

| Parameter | Type | Description | Default |
|-----------|------|-------------|---------|
| `files` | string[] | Files to extract from (can include line numbers with colon) | (required) |
| `allowTests` | boolean | Include test files | `false` |
| `contextLines` | number | Number of context lines to include | `0` |
| `format` | string | Output format ('markdown', 'plain', 'json') | `'markdown'` |
| `json` | boolean | Return results as parsed JSON instead of string | `false` |
| `binaryOptions` | object | Options for getting the binary | `{}` |

### Binary Management

The SDK provides functions for managing the Probe binary:

```javascript
import { getBinaryPath, setBinaryPath } from '@probelabs/probe@latest';

// Get the path to the probe binary
const binaryPath = await getBinaryPath({
  forceDownload: false,
  version: '1.0.0'
});

// Manually set the path to the probe binary
setBinaryPath('/path/to/probe/binary');
```

## AI Tools Integration

The SDK provides built-in tools for integrating with AI frameworks. The latest version introduces tool generators that allow for better configuration and session isolation:

### Vercel AI SDK Integration

```javascript
import { generateText } from 'ai';
import { searchTool, queryTool, extractTool } from '@probelabs/probe@latest';
import { randomUUID } from 'crypto';

// Generate a session ID
const sessionId = randomUUID();

// Create configured tools with the session ID
const configOptions = {
  sessionId,
  debug: process.env.DEBUG === 'true'
};

// Create configured tool instances
const configuredTools = {
  search: searchTool(configOptions),
  query: queryTool(configOptions),
  extract: extractTool(configOptions)
};

// Use the configured tools with Vercel AI SDK
async function chatWithAI(userMessage) {
  const result = await generateText({
    model: provider(modelName),
    messages: [{ role: 'user', content: userMessage }],
    system: "You are a code intelligence assistant. Use the provided tools to search and analyze code.",
    tools: configuredTools,
    maxSteps: 15,
    temperature: 0.7
  });
  
  return result.text;
}
```

### Tool Generators and Configuration

The SDK provides tool generators that allow you to create configured instances of tools with specific options:

```javascript
import { searchTool, queryTool, extractTool } from '@probelabs/probe@latest';
import { randomUUID } from 'crypto';

// Generate a session ID
const sessionId = randomUUID();

// Configure tools with options
const configOptions = {
  sessionId,                        // Session ID for caching search results
  debug: process.env.DEBUG === '1', // Enable debug logging
  maxTokens: 30000                  // Override default max tokens (search tool only)
};

// Create configured tool instances
const search = searchTool(configOptions);
const query = queryTool({ debug: true });  // Each tool can have different options
const extract = extractTool({ debug: true });
```

#### Configuration Options

| Option | Type | Description | Applicable Tools |
|--------|------|-------------|-----------------|
| `sessionId` | string | Session ID for caching search results | searchTool |
| `debug` | boolean | Enable debug logging | All tools |
| `maxTokens` | number | Override default max tokens | searchTool |

#### Backward Compatibility

For backward compatibility, the package still exports pre-configured tools:

```javascript
import { tools } from '@probelabs/probe@latest';

// Use pre-configured tools
const { searchTool, queryTool, extractTool } = tools;
```

### LangChain Integration

```javascript
import { ChatOpenAI } from '@langchain/openai';
import { tools, createSearchTool, createQueryTool, createExtractTool } from '@probelabs/probe@latest';
import { randomUUID } from 'crypto';

// Generate a session ID
const sessionId = randomUUID();

// Create the LangChain tools with configuration
const searchTool = tools.createSearchTool({
  sessionId,
  debug: process.env.DEBUG === 'true'
});
const queryTool = tools.createQueryTool({ debug: true });
const extractTool = tools.createExtractTool({ debug: true });

// Create a ChatOpenAI instance with tools
const model = new ChatOpenAI({
  modelName: "gpt-5.2",
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

### Default System Message

The package provides a default system message that you can use with your AI assistants:

```javascript
import { DEFAULT_SYSTEM_MESSAGE, searchTool, queryTool, extractTool } from '@probelabs/probe@latest';

// Use the default system message in your AI application
const systemMessage = DEFAULT_SYSTEM_MESSAGE;

// Generate a session ID
const sessionId = randomUUID();

// Create configured tools with the session ID
const configOptions = { sessionId };

// Example with Vercel AI SDK
const result = await generateText({
  model: provider(modelName),
  messages: [{ role: 'user', content: userMessage }],
  system: DEFAULT_SYSTEM_MESSAGE,
  tools: {
    search: searchTool(configOptions),
    query: queryTool(configOptions),
    extract: extractTool(configOptions)
  }
});
```

## Advanced Examples

### Building a Code Search API

```javascript
import express from 'express';
import { search, query, extract } from '@probelabs/probe@latest';

const app = express();
app.use(express.json());

// Search endpoint
app.post('/api/search', async (req, res) => {
  try {
    const { path, query, options } = req.body;
    const results = await search({
      path,
      query,
      ...options
    });
    res.json({ results });
  } catch (error) {
    res.status(500).json({ error: error.message });
  }
});

// Query endpoint
app.post('/api/query', async (req, res) => {
  try {
    const { path, pattern, language, options } = req.body;
    const results = await query({
      path,
      pattern,
      language,
      ...options
    });
    res.json({ results });
  } catch (error) {
    res.status(500).json({ error: error.message });
  }
});

// Extract endpoint
app.post('/api/extract', async (req, res) => {
  try {
    const { files, options } = req.body;
    const results = await extract({
      files,
      ...options
    });
    res.json({ results });
  } catch (error) {
    res.status(500).json({ error: error.message });
  }
});

app.listen(3000, () => {
  console.log('Code search API running on port 3000');
});
```

### Creating a Custom AI Assistant

```javascript
import { search, query, extract } from '@probelabs/probe@latest';
import { ChatOpenAI } from '@langchain/openai';
import { PromptTemplate } from '@langchain/core/prompts';
import { StringOutputParser } from '@langchain/core/output_parsers';

// Create a custom AI assistant that can search code
async function createCodeAssistant() {
  // Create a chat model
  const model = new ChatOpenAI({
    modelName: "gpt-5.2",
    temperature: 0.7
  });
  
  // Create a prompt template
  const promptTemplate = PromptTemplate.fromTemplate(`
    You are a code assistant. I'll provide you with a question and some code search results.
    Please analyze the code and answer the question.
    
    Question: {question}
    
    Code search results:
    {searchResults}
    
    Your analysis:
  `);
  
  // Create a chain
  const chain = promptTemplate
    .pipe(model)
    .pipe(new StringOutputParser());
  
  // Function to answer questions about code
  async function answerCodeQuestion(question, codebasePath) {
    // Search for relevant code
    const searchResults = await search({
      path: codebasePath,
      query: question,
      maxResults: 5,
      maxTokens: 10000
    });
    
    // Get the answer from the AI
    const answer = await chain.invoke({
      question,
      searchResults
    });
    
    return answer;
  }
  
  return { answerCodeQuestion };
}

// Usage
const assistant = await createCodeAssistant();
const answer = await assistant.answerCodeQuestion(
  "How is authentication implemented?",
  "/path/to/your/project"
);
console.log(answer);
```

### Batch Processing Multiple Repositories

```javascript
import { search } from '@probelabs/probe@latest';
import fs from 'fs/promises';
import path from 'path';

async function batchSearch(repositories, searchQuery) {
  const results = {};
  
  for (const repo of repositories) {
    console.log(`Searching in ${repo}...`);
    try {
      const searchResults = await search({
        path: repo,
        query: searchQuery,
        maxResults: 20,
        json: true // Get structured results
      });
      
      results[repo] = searchResults;
    } catch (error) {
      console.error(`Error searching in ${repo}:`, error);
      results[repo] = { error: error.message };
    }
  }
  
  return results;
}

// Example usage
const repositories = [
  '/path/to/repo1',
  '/path/to/repo2',
  '/path/to/repo3'
];

const results = await batchSearch(repositories, 'security AND (vulnerability OR exploit)');

// Save results to a file
await fs.writeFile(
  path.join(process.cwd(), 'search-results.json'),
  JSON.stringify(results, null, 2)
);

console.log('Search completed and results saved to search-results.json');
```

### Code Analysis Pipeline

```javascript
import { search, query } from '@probelabs/probe@latest';
import fs from 'fs/promises';

async function analyzeCodebase(codebasePath) {
  const analysis = {
    timestamp: new Date().toISOString(),
    codebasePath,
    metrics: {},
    patterns: {},
    potentialIssues: []
  };
  
  // Count functions by language
  const languages = ['javascript', 'typescript', 'python', 'rust', 'go'];
  const functionCounts = {};
  
  for (const lang of languages) {
    try {
      const pattern = lang === 'javascript' || lang === 'typescript'
        ? 'function $NAME($$$PARAMS) $$$BODY'
        : lang === 'python'
          ? 'def $NAME($$$PARAMS): $$$BODY'
          : lang === 'rust'
            ? 'fn $NAME($$$PARAMS) $$$BODY'
            : 'func $NAME($$$PARAMS) $$$BODY';
      
      const results = await query({
        path: codebasePath,
        pattern,
        language: lang,
        maxResults: 1000,
        json: true
      });
      
      functionCounts[lang] = results.matches ? results.matches.length : 0;
    } catch (error) {
      console.error(`Error counting functions in ${lang}:`, error);
      functionCounts[lang] = -1; // Error indicator
    }
  }
  
  analysis.metrics.functionCounts = functionCounts;
  
  // Find potential security issues
  const securityPatterns = [
    'password',
    'token',
    'api_key',
    'apikey',
    'secret',
    'credential',
    'eval(',
    'exec(',
    'shell_exec'
  ];
  
  for (const pattern of securityPatterns) {
    try {
      const results = await search({
        path: codebasePath,
        query: pattern,
        maxResults: 50,
        json: true
      });
      
      if (results.matches && results.matches.length > 0) {
        analysis.potentialIssues.push({
          pattern,
          matches: results.matches.map(match => ({
            file: match.file,
            line: match.line,
            content: match.content.substring(0, 100) + '...' // Truncate long content
          }))
        });
      }
    } catch (error) {
      console.error(`Error searching for pattern ${pattern}:`, error);
    }
  }
  
  // Save analysis to file
  await fs.writeFile(
    'codebase-analysis.json',
    JSON.stringify(analysis, null, 2)
  );
  
  return analysis;
}

// Usage
const analysis = await analyzeCodebase('/path/to/your/project');
console.log('Analysis complete. Results saved to codebase-analysis.json');
console.log(`Found ${Object.values(analysis.metrics.functionCounts).reduce((a, b) => a + (b > 0 ? b : 0), 0)} functions across all languages`);
console.log(`Found ${analysis.potentialIssues.length} potential security issues`);
```

## How It Works

When you install the `@probelabs/probe` package:

1. A placeholder binary is included in the package
2. During installation, the postinstall script downloads the actual Probe binary for your platform
3. The placeholder is replaced with the actual binary
4. When installed globally, npm creates a symlink to this binary in your system path

This approach ensures that you get the actual native binary, not a JavaScript wrapper, providing full performance and all features of the original Probe CLI.

## Troubleshooting

### Common Issues

#### Binary Not Found

If you encounter a "Binary not found" error:

```javascript
import { setBinaryPath } from '@probelabs/probe@latest';

// Manually set the path to the probe binary
setBinaryPath('/path/to/probe/binary');
```

#### Permission Denied

If you encounter a "Permission denied" error:

```bash
# Make the binary executable
chmod +x /path/to/probe/binary
```

#### Network Error During Binary Download

If you encounter a network error during binary download:

```javascript
import { getBinaryPath } from '@probelabs/probe@latest';

// Force download with a specific version
const binaryPath = await getBinaryPath({
  forceDownload: true,
  version: '1.0.0'
});
```

#### Timeout Error

If you encounter a timeout error:

```javascript
import { search } from '@probelabs/probe@latest';

// Increase the timeout by using the execAsync function directly
import { promisify } from 'util';
import { exec } from 'child_process';
const execAsync = promisify(exec);

// Get the binary path
import { getBinaryPath } from '@probelabs/probe@latest';
const binaryPath = await getBinaryPath();

// Execute the command with a longer timeout
const { stdout } = await execAsync(`${binaryPath} search "query" /path/to/project`, {
  timeout: 60000 // 60 seconds
});
```

## Best Practices

1. **Use Specific Queries**: More specific queries yield better results
2. **Limit Result Size**: Use `maxResults` and `maxTokens` to limit the size of results
3. **Handle Errors**: Always wrap API calls in try/catch blocks
4. **Cache Results**: Consider caching results for frequently used queries
5. **Use JSON Format**: Use `json: true` for programmatic processing of results
6. **Combine with Other Tools**: Use Probe alongside other tools for a more comprehensive understanding of your codebase
7. **Optimize for Performance**: Use `filesOnly` for initial broad searches, then refine with more specific queries
8. **Use Session IDs**: For related searches, use the same session ID to avoid seeing duplicate code blocks
9. **Use Tool Generators**: Create configured tool instances with session IDs for better isolation in concurrent environments
10. **Enable Debug Logging**: Set the debug option to true when troubleshooting tool execution

### Session-Based Caching Example

There are two ways to use session-based caching:

#### 1. Using the search function directly

```javascript
import { search } from '@probelabs/probe@latest';

// First search with empty session string (generates a session ID)
const results1 = await search({
  path: '/path/to/your/project',
  query: 'authentication',
  session: ''
});

// Get the session ID from the results
const sessionId = results1.session;
console.log(`Session ID: ${sessionId}`);

// Use the same session ID for related searches
const results2 = await search({
  path: '/path/to/your/project',
  query: 'login',
  session: sessionId
});

// This will skip code blocks already shown in the previous search
console.log(`Found ${results2.matches.length} new matches`);
```

#### 2. Using the tool generators with Vercel AI SDK

```javascript
import { searchTool } from '@probelabs/probe@latest';
import { randomUUID } from 'crypto';

// Generate a session ID
const sessionId = randomUUID();
console.log(`Session ID: ${sessionId}`);

// Create a configured search tool with the session ID
const configuredSearchTool = searchTool({
  sessionId,
  debug: true // Enable debug logging
});

// Use the configured tool with Vercel AI SDK
// All searches performed with this tool will use the same session ID
const result = await generateText({
  model: provider(modelName),
  messages: [{ role: 'user', content: userMessage }],
  system: "You are a code intelligence assistant. Use the provided tools to search and analyze code.",
  tools: {
    search: configuredSearchTool
  },
  maxSteps: 15
});
```

Session-based caching is particularly useful when:
- Building interactive search interfaces
- Conducting progressive searches that refine or expand on previous queries
- Creating AI assistants that need to avoid repeating the same code blocks
- Implementing search workflows that build on previous results
- Handling concurrent searches in multi-user environments

## Related Resources

- [Probe GitHub Repository](https://github.com/probelabs/probe)
- [Probe MCP Server](https://github.com/probelabs/probe/tree/main/mcp)
- [Probe AI Chat](https://github.com/probelabs/probe/tree/main/examples/chat)