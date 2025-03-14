# Probe AI Integration

Probe offers powerful AI integration capabilities that allow you to leverage large language models (LLMs) to understand and navigate your codebase more effectively. This document provides comprehensive information about Probe's AI features, including the AI chat mode, MCP server integration, and Node.js SDK for programmatic access.

## Table of Contents

- [Privacy Considerations](#privacy-considerations)
- [AI Chat Mode](#ai-chat-mode)
  - [Overview](#overview)
  - [Installation and Setup](#installation-and-setup)
  - [Features](#features)
  - [Configuration Options](#configuration-options)
  - [Advanced Usage](#advanced-usage)
  - [Best Practices](#best-practices)
- [MCP Server Integration](#mcp-server-integration)
  - [Overview](#mcp-overview)
  - [Setting Up the MCP Server](#setting-up-the-mcp-server)
  - [Available Tools](#available-tools)
  - [Integration with AI Assistants](#integration-with-ai-assistants)
  - [Advanced Configuration](#advanced-configuration)
  - [Troubleshooting](#troubleshooting)
- [Node.js SDK](#nodejs-sdk)
  - [Overview](#sdk-overview)
  - [Installation](#sdk-installation)
  - [Core Functions](#core-functions)
  - [AI Tools Integration](#ai-tools-integration)
  - [Examples](#examples)
  - [API Reference](#api-reference)

## Privacy Considerations {#privacy-considerations}

When using Probe's AI integration features, it's important to understand the privacy implications:

- **Local Search Engine**: Probe itself is a fully local semantic code search tool that doesn't require embedding generation or cloud indexing
- **Works Like Elastic Search**: Probe functions like a full elastic search on top of your codebase or documentation, without requiring indexing
- **AI Service Integration**: When using Probe with external AI services (Anthropic, OpenAI, etc.), code snippets found by Probe are sent to those services
- **Data Transmission**: The following data may be transmitted to external AI providers:
  - Your natural language queries
  - Code snippets and context found by Probe's search
  - Conversation history for contextual awareness
- **Local Model Options**: For maximum privacy, Probe can be used with locally-running AI models, keeping all data on your machine

Consider these privacy aspects when choosing how to integrate Probe with AI services, especially when working with sensitive or proprietary code.

## AI Chat Mode {#ai-chat-mode}

### Overview {#overview}

Probe's AI Chat mode provides an interactive CLI interface where you can ask questions about your codebase and get AI-powered responses. This mode combines Probe's powerful code search capabilities with large language models to help you understand and navigate your codebase more effectively.

Key benefits:

- **Natural Language Understanding**: Ask questions about your code in plain English
- **Contextual Awareness**: The AI maintains conversation history for follow-up questions
- **Code-Aware Responses**: Get explanations that reference specific files and line numbers
- **Intelligent Search**: The AI automatically formulates optimal search queries based on your questions

### Installation and Setup {#installation-and-setup}

The AI chat functionality is available as a standalone npm package that can be run directly with npx.

#### Using npx (Recommended)

```bash
# Run directly with npx (no installation needed)
npx -y @buger/probe-chat

# Set your API key first
export ANTHROPIC_API_KEY=your_api_key
# Or for OpenAI
# export OPENAI_API_KEY=your_api_key

# Or specify a directory to search
npx -y @buger/probe-chat /path/to/your/project
```

#### Using the npm package

```bash
# Install globally
npm install -g @buger/probe-chat

# Start the chat interface
probe-chat
```

#### Using the example code

```bash
# Navigate to the examples directory
cd examples/chat

# Install dependencies
npm install

# Set your API key
export ANTHROPIC_API_KEY=your_api_key
# Or for OpenAI
# export OPENAI_API_KEY=your_api_key

# Start the chat interface
node index.js
```

### Features {#features}

#### AI-Powered Search

The AI Chat mode uses large language models to understand your questions and search your codebase intelligently. It can:

- Find relevant code based on natural language descriptions
- Explain how different parts of your codebase work together
- Identify patterns and architectural decisions
- Help you understand complex code

#### Multi-Model Support

Probe's AI Chat mode supports both Anthropic's Claude and OpenAI's GPT models:

- **Claude Models**: Provide excellent code understanding and explanation capabilities
- **GPT Models**: Offer strong general-purpose capabilities

The default model is selected based on which API key you provide.

#### Token Tracking

The AI Chat mode monitors token usage for both requests and responses, helping you keep track of your API usage:

```
Token Usage: Request: 1245 Response: 1532 (Current message only: ~1532)
Total: 2777 tokens (Cumulative for entire session)
```

#### Conversation History

The chat maintains context across multiple interactions, allowing for follow-up questions and deeper exploration of topics. The history is managed efficiently to prevent context overflow:

- Maintains up to 20 previous messages by default
- Automatically trims older messages when the limit is reached
- Preserves context for follow-up questions

#### Session-Based Caching

The AI Chat mode uses a session-based caching system to avoid showing the same code blocks multiple times in a conversation:

- Each chat instance generates a unique session ID
- The session ID is used to track which code blocks have already been shown
- This prevents redundant information in responses
- The cache is maintained for the duration of the chat session

#### Colored Output

The terminal interface provides user-friendly colored output with syntax highlighting for code blocks, making it easier to read and understand the AI's responses.

### Configuration Options {#configuration-options}

You can configure the AI Chat mode using environment variables:

#### Model Selection

```bash
# Override the default model
export MODEL_NAME=claude-3-opus-20240229
probe-chat
```

#### API URLs

```bash
# Override API URLs (useful for proxies or enterprise deployments)
export ANTHROPIC_API_URL=https://your-anthropic-proxy.com
export OPENAI_API_URL=https://your-openai-proxy.com/v1
probe-chat
```

#### Debug Mode

```bash
# Enable debug mode for detailed logging
export DEBUG=1 probe-chat
```

#### Allowed Folders

```bash
# Specify which folders the AI can search
export ALLOWED_FOLDERS=/path/to/project1,/path/to/project2
probe-chat
```

### Advanced Usage {#advanced-usage}

#### Programmatic Usage in Node.js

You can also use the AI Chat functionality programmatically in your Node.js applications:

```javascript
import { ProbeChat } from '@buger/probe-chat';
import { StreamingTextResponse } from 'ai';

// Create a chat instance
const chat = new ProbeChat({
  model: 'claude-3-sonnet-20240229',
  anthropicApiKey: process.env.ANTHROPIC_API_KEY,
  allowedFolders: ['/path/to/your/project']
});

// In an API route or Express handler
export async function POST(req) {
  const { messages } = await req.json();
  const userMessage = messages[messages.length - 1].content;
  
  // Get a streaming response from the AI
  const stream = await chat.chat(userMessage, { stream: true });
  
  // Return a streaming response
  return new StreamingTextResponse(stream);
}

// Or use it in a non-streaming way
const response = await chat.chat('How is authentication implemented?');
console.log(response);
```

#### Custom System Messages

You can customize the system message to provide specific instructions to the AI:

```javascript
const chat = new ProbeChat({
  model: 'claude-3-sonnet-20240229',
  anthropicApiKey: process.env.ANTHROPIC_API_KEY,
  allowedFolders: ['/path/to/your/project'],
  systemMessage: 'You are a code expert focusing on security issues. When analyzing code, prioritize identifying security vulnerabilities.'
});
```

#### Experimental Thinking

For Claude 3.7 models, you can enable the experimental thinking feature for more detailed reasoning:

```javascript
const chat = new ProbeChat({
  model: 'claude-3-7-sonnet-latest',
  anthropicApiKey: process.env.ANTHROPIC_API_KEY,
  allowedFolders: ['/path/to/your/project'],
  experimentalThinking: {
    enabled: true,
    budget: 8000
  }
});
```

### Best Practices {#best-practices}

1. **Be Specific**: Ask specific questions about your codebase for more targeted answers
2. **Start with Overview Questions**: Begin with high-level questions to understand the structure before diving into details
3. **Follow Up**: Use follow-up questions to explore topics in more depth
4. **Reference Files**: Mention specific files or directories if you want to focus on a particular area
5. **Ask for Explanations**: The AI is particularly good at explaining complex code or concepts
6. **Request Examples**: Ask for examples if you're trying to understand how to use a particular feature or API
7. **Use Multiple Queries**: If you don't find what you're looking for, try reformulating your question
8. **Combine with CLI**: Use the AI chat for exploration and understanding, then switch to the CLI for specific searches

## MCP Server Integration {#mcp-server-integration}

### Overview {#mcp-overview}

The Model Context Protocol (MCP) server mode allows Probe to integrate seamlessly with AI editors and assistants. This mode exposes Probe's powerful search capabilities through a standardized interface that AI tools can use to search and understand your codebase.

Key benefits:

- **Seamless AI Integration**: Allows AI assistants to search and analyze your code
- **Standardized Protocol**: Uses the Model Context Protocol for compatibility with various AI tools
- **Enhanced AI Capabilities**: Gives AI assistants code-aware capabilities
- **Secure Access**: Provides controlled access to your codebase

### Setting Up the MCP Server {#setting-up-the-mcp-server}

#### Using NPX (Recommended)

The easiest way to use Probe's MCP server is through NPX:

```json
{
  "mcpServers": {
    "probe": {
      "command": "npx",
      "args": [
        "-y",
        "@buger/probe-mcp"
      ]
    }
  }
}
```

Add this configuration to your AI editor's MCP configuration file. The exact location depends on your editor, but common locations include:

- For Cline: `.cline/mcp_config.json` in your project directory
- For other editors: Check your editor's documentation for MCP configuration

#### Manual Installation

If you prefer to install the MCP server manually:

1. Install the NPM package globally:
   ```bash
   npm install -g @buger/probe-mcp
   ```

2. Configure your AI editor to use the installed package:
   ```json
   {
     "mcpServers": {
       "probe": {
         "command": "probe-mcp"
       }
     }
   }
   ```

#### Technical Implementation

The Probe MCP server:

- Implements the Model Context Protocol specification
- Uses stdio for communication with AI editors
- Automatically downloads and manages the Probe binary
- Provides three main tools: search_code, query_code, and extract_code
- Handles tool execution and error reporting

### Available Tools {#available-tools}

The Probe MCP server provides the following tools:

#### search_code

Search code in a specified directory using Elasticsearch-like query syntax with session-based caching.

```json
{
  "path": "/path/to/your/project",
  "query": "authentication flow",
  "maxTokens": 20000
}
```

The search tool supports Elasticsearch-like query syntax with the following features:
- Basic term searching: "config" or "search"
- Field-specific searching: "field:value" (e.g., "function:parse")
- Required terms with + prefix: "+required"
- Excluded terms with - prefix: "-excluded"
- Logical operators: "term1 AND term2", "term1 OR term2"
- Grouping with parentheses: "(term1 OR term2) AND term3"

#### query_code

Find specific code structures (functions, classes, etc.) using tree-sitter patterns.

```json
{
  "path": "/path/to/your/project",
  "pattern": "fn $NAME($$$PARAMS) $$$BODY",
  "language": "rust"
}
```

Pattern syntax:
- `$NAME`: Matches an identifier (e.g., function name)
- `$$$PARAMS`: Matches parameter lists
- `$$$BODY`: Matches function bodies
- `$$$FIELDS`: Matches struct/class fields
- `$$$METHODS`: Matches class methods

#### extract_code

Extract code blocks from files based on file paths and optional line numbers.

```json
{
  "path": "/path/to/your/project",
  "files": ["/path/to/your/project/src/main.rs:42"]
}
```

### Integration with AI Assistants {#integration-with-ai-assistants}

Once configured, you can ask your AI assistant to search your codebase with natural language queries. The AI will translate your request into appropriate Probe commands and display the results.

#### Example Queries

Here are some examples of natural language queries you can use:

- "Do the probe and search my codebase for implementations of the ranking algorithm"
- "Using probe find all functions related to error handling in the src directory"
- "Search for code that handles user authentication"
- "Find all instances where we're using the BM25 algorithm"
- "Look for functions that process query parameters"

#### How It Works

1. You ask a question about your codebase to your AI assistant
2. The AI assistant recognizes that Probe can help answer this question
3. The assistant formulates an appropriate search query and parameters
4. The MCP server executes the Probe search command
5. The results are returned to the AI assistant
6. The assistant analyzes the code and provides you with an answer

#### Technical Details

The MCP server:

- Receives tool call requests from the AI assistant
- Parses the request parameters
- Executes the appropriate Probe command
- Returns the results to the AI assistant
- Handles errors and provides appropriate error messages
- Maintains session-based caching to avoid duplicate results

### Advanced Configuration {#advanced-configuration}

#### Custom Search Paths

You can configure the MCP server to search specific directories by default:

```json
{
  "mcpServers": {
    "probe": {
      "command": "npx",
      "args": [
        "-y",
        "@buger/probe-mcp"
      ],
      "env": {
        "PROBE_DEFAULT_PATHS": "/path/to/project1,/path/to/project2"
      }
    }
  }
}
```

#### Limiting Results

You can set default limits for search results:

```json
{
  "mcpServers": {
    "probe": {
      "command": "npx",
      "args": [
        "-y",
        "@buger/probe-mcp"
      ],
      "env": {
        "PROBE_MAX_TOKENS": "20000"
      }
    }
  }
}
```

#### Custom Binary Path

If you have a custom build of the Probe binary, you can specify its path:

```json
{
  "mcpServers": {
    "probe": {
      "command": "npx",
      "args": [
        "-y",
        "@buger/probe-mcp"
      ],
      "env": {
        "PROBE_PATH": "/path/to/custom/probe"
      }
    }
  }
}
```

#### Debug Mode

Enable debug mode for detailed logging:

```json
{
  "mcpServers": {
    "probe": {
      "command": "npx",
      "args": [
        "-y",
        "@buger/probe-mcp"
      ],
      "env": {
        "DEBUG": "1"
      }
    }
  }
}
```

### Troubleshooting {#troubleshooting}

If you encounter issues with the MCP server:

1. **Check Installation**: Ensure the Probe binary was downloaded correctly during package installation
2. **Verify Configuration**: Double-check your MCP configuration file for errors
3. **Check Permissions**: Make sure the AI editor has permission to execute the MCP server
4. **Check Logs**: Look for error messages in your AI editor's logs
5. **Update Packages**: Ensure you're using the latest version of the `@buger/probe-mcp` package
6. **Manual Binary Download**: If the automatic download failed, you can manually download the binary from [GitHub Releases](https://github.com/buger/probe/releases) and place it in the `node_modules/@buger/probe-mcp/bin` directory

#### Common Issues and Solutions

| Issue | Solution |
|-------|----------|
| "Binary not found" error | Set the `PROBE_PATH` environment variable to the location of your Probe binary |
| "Permission denied" error | Make sure the binary is executable (`chmod +x /path/to/probe`) |
| Tool calls timeout | Increase the timeout value in your AI editor's configuration |
| Empty search results | Check your query syntax and try a simpler query |
| "Network error" during binary download | Check your internet connection and firewall settings |

## Node.js SDK {#nodejs-sdk}

### Overview {#sdk-overview}

The Node.js SDK provides programmatic access to Probe's powerful code search capabilities. It allows you to integrate Probe into your Node.js applications, build custom tools, and create AI-powered code assistants.

Key benefits:

- **Programmatic Access**: Use Probe's capabilities directly from your Node.js code
- **AI Integration**: Ready-to-use tools for Vercel AI SDK, LangChain, and other AI frameworks
- **Cross-Platform**: Works on Windows, macOS, and Linux
- **Automatic Binary Management**: Handles downloading and managing the Probe binary
- **Type Safety**: Includes TypeScript type definitions

### Installation {#sdk-installation}

#### Local Installation

```bash
npm install @buger/probe
```

#### Global Installation

```bash
npm install -g @buger/probe
```

During installation, the package will automatically download the appropriate probe binary for your platform.

### Core Functions {#core-functions}

The SDK provides three main functions:

#### search

Search for patterns in your codebase using Elasticsearch-like query syntax.

```javascript
import { search } from '@buger/probe';

const searchResults = await search({
  path: '/path/to/your/project',
  query: 'function',
  maxResults: 10
});
```

#### query

Find specific code structures using tree-sitter patterns.

```javascript
import { query } from '@buger/probe';

const queryResults = await query({
  path: '/path/to/your/project',
  pattern: 'function $NAME($$$PARAMS) $$$BODY',
  language: 'javascript'
});
```

#### extract

Extract code blocks from files based on file paths and line numbers.

```javascript
import { extract } from '@buger/probe';

const extractResults = await extract({
  files: ['/path/to/your/project/src/main.js:42']
});
```

### AI Tools Integration {#ai-tools-integration}

The SDK provides built-in tools for integrating with AI frameworks:

#### Vercel AI SDK Integration

```javascript
import { generateText } from 'ai';
import { tools } from '@buger/probe';

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

#### LangChain Integration

```javascript
import { ChatOpenAI } from '@langchain/openai';
import { tools } from '@buger/probe';

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

#### Default System Message

The package provides a default system message that you can use with your AI assistants:

```javascript
import { tools } from '@buger/probe';

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

### Examples {#examples}

#### Basic Search Example

```javascript
import { search } from '@buger/probe';

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

#### Advanced Search with Multiple Options

```javascript
import { search } from '@buger/probe';

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
      allowTests: false,
      anyTerm: false
    });
    
    console.log('Advanced search results:');
    console.log(results);
  } catch (error) {
    console.error('Advanced search error:', error);
  }
}
```

#### Query for Specific Code Structures

```javascript
import { query } from '@buger/probe';

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

#### Extract Code Blocks

```javascript
import { extract } from '@buger/probe';

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

#### Building a Custom AI Assistant

```javascript
import { search, query, extract } from '@buger/probe';
import { ChatOpenAI } from '@langchain/openai';
import { PromptTemplate } from '@langchain/core/prompts';
import { StringOutputParser } from '@langchain/core/output_parsers';

// Create a custom AI assistant that can search code
async function createCodeAssistant() {
  // Create a chat model
  const model = new ChatOpenAI({
    modelName: "gpt-4o",
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

### API Reference {#api-reference}

#### Search

```javascript
import { search } from '@buger/probe';

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
  anyTerm: false,
  noMerge: false,
  mergeThreshold: 5,
  json: false,
  binaryOptions: {
    forceDownload: false,
    version: '1.0.0'
  }
});
```

##### Parameters

- `path` (required): Path to search in
- `query` (required): Search query or queries (string or array of strings)
- `filesOnly`: Only output file paths
- `ignore`: Patterns to ignore (array of strings)
- `excludeFilenames`: Exclude filenames from search
- `reranker`: Reranking method ('hybrid', 'hybrid2', 'bm25', 'tfidf')
- `frequencySearch`: Use frequency-based search
- `exact`: Use exact matching
- `maxResults`: Maximum number of results
- `maxBytes`: Maximum bytes to return
- `maxTokens`: Maximum tokens to return
- `allowTests`: Include test files
- `anyTerm`: Match any term
- `noMerge`: Don't merge adjacent blocks
- `mergeThreshold`: Merge threshold
- `json`: Return results as parsed JSON instead of string
- `binaryOptions`: Options for getting the binary
  - `forceDownload`: Force download even if binary exists
  - `version`: Specific version to download

#### Query

```javascript
import { query } from '@buger/probe';

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

##### Parameters

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

#### Extract

```javascript
import { extract } from '@buger/probe';

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

##### Parameters

- `files` (required): Files to extract from (can include line numbers with colon, e.g., "/path/to/file.rs:10")
- `allowTests`: Include test files
- `contextLines`: Number of context lines to include
- `format`: Output format ('markdown', 'plain', 'json')
- `json`: Return results as parsed JSON instead of string
- `binaryOptions`: Options for getting the binary
  - `forceDownload`: Force download even if binary exists
  - `version`: Specific version to download

#### Binary Management

```javascript
import { getBinaryPath, setBinaryPath } from '@buger/probe';

// Get the path to the probe binary
const binaryPath = await getBinaryPath({
  forceDownload: false,
  version: '1.0.0'
});

// Manually set the path to the probe binary
setBinaryPath('/path/to/probe/binary');
```

#### AI Tools

```javascript
import { tools } from '@buger/probe';

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