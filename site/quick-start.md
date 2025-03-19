# Quick Start

This guide will help you get up and running with Probe quickly. For more detailed information, check out the other sections of the documentation.

## Installation

The easiest way to install Probe is via npm:

```bash
npm install -g @buger/probe
```

Or using curl (for macOS and Linux):

```bash
curl -fsSL https://raw.githubusercontent.com/buger/probe/main/install.sh | bash
```

Or using PowerShell (for Windows):

```powershell
iwr -useb https://raw.githubusercontent.com/buger/probe/main/install.ps1 | iex
```

For more detailed installation instructions, including manual installation and building from source, see the [Installation Guide](/installation).

## Basic Search Example

Search for code containing the phrase "llm pricing" in the current directory:

```bash
probe search "llm pricing" ./
```

This will search for the terms "llm" and "pricing" in your codebase and return the most relevant code blocks.

For more advanced search techniques and options, see the [Search Functionality](/search-functionality) documentation.

## Advanced Search (with Token Limiting)

Search for "prompt injection" in the current directory but limit the total tokens to 10000 (useful for AI tools with context window constraints):

```bash
probe search "prompt injection" ./ --max-tokens 10000
```

This is particularly useful when you need to feed the results into an AI model with a limited context window. Learn more about AI integration in the [AI Integration Overview](/ai-integration).

## Session-Based Caching

Use session IDs to avoid seeing the same code blocks multiple times in related searches:

```bash
# First search - generates a session ID
probe search "authentication" --session ""
# Session: a1b2 (example output)

# Subsequent searches - reuse the session ID
probe search "login" --session "a1b2"
# Will skip code blocks already shown in the previous search
```

In Node.js applications:

```javascript
import { search } from '@buger/probe';

// First search with empty session string (generates a session ID)
const results1 = await search({
  path: '/path/to/your/project',
  query: 'authentication',
  session: ''
});

// Get the session ID from the results
const sessionId = results1.session;

// Use the same session ID for related searches
const results2 = await search({
  path: '/path/to/your/project',
  query: 'login',
  session: sessionId
});
```

This is especially useful when performing multiple related searches, as it helps avoid duplicate results and provides a more cohesive search experience.

## Elastic Search Queries

Use advanced query syntax for more powerful searches:

```bash
# Use AND operator for terms that must appear together
probe search "error AND handling" ./

# Use OR operator for alternative terms
probe search "login OR authentication OR auth" ./src

# Group terms with parentheses for complex queries
probe search "(error OR exception) AND (handle OR process)" ./

# Use wildcards for partial matching
probe search "auth* connect*" ./

# Exclude terms with NOT operator
probe search "database NOT sqlite" ./
```

For a complete reference of search syntax and capabilities, see the [Search Functionality](/search-functionality#elastic-search-syntax) documentation.

## Extract Code Blocks

Extract a specific function or code block containing line 42 in main.rs:

```bash
probe extract src/main.rs:42
```

This will use tree-sitter to find the closest suitable parent node (function, struct, class, etc.) for that line.

You can even pipe failing test output and it will extract needed files and AST out of it:

```bash
go test | probe extract
```

Extract code with LLM prompt and instructions for AI integration:

```bash
# Extract with engineer prompt template
probe extract src/auth.rs#authenticate --prompt engineer --instructions "Explain this authentication function"

# Extract with architect prompt template
probe extract src/api.js --prompt architect --instructions "Analyze this API module"
```

To learn more about code extraction features, see the [Code Extraction](/code-extraction) documentation.

## Query Code Structures

Find specific code structures using tree-sitter patterns:

```bash
# Find JavaScript functions
probe query "function $NAME($$$PARAMS) $$$BODY" ./src --language javascript

# Find Python functions
probe query "def $NAME($$$PARAMS): $$$BODY" ./src --language python

# Find Go structs
probe query "type $NAME struct { $$$FIELDS }" ./src --language go
```

The query command uses tree-sitter patterns to find specific code structures in your codebase. This is more precise than text-based search and understands the syntax of the programming language.

For more details on the query command, see the [CLI Mode](/cli-mode#query-command) documentation.

## Interactive AI Chat

Use the built-in AI assistant to ask questions about your codebase:

```bash
# Run directly with npx (no installation needed)
npx -y @buger/probe-chat

# Set your API key first
export ANTHROPIC_API_KEY=your_api_key
# Or for OpenAI
# export OPENAI_API_KEY=your_api_key

# Specify a directory to search (optional)
npx -y @buger/probe-chat /path/to/your/project
```

Example questions you might ask:
- "How does the ranking algorithm work?"
- "Explain the file structure of this project"
- "What are the main components of the search functionality?"

For more details on the AI chat capabilities, see the [AI Chat Mode](/ai-chat) documentation.

## MCP Server Integration

Integrate with any AI editor by adding this to your MCP configuration:

```json
{
  "mcpServers": {
    "memory": {
      "command": "npx",
      "args": [
        "-y",
        "@buger/probe-mcp"
      ]
    }
  }
}
```

Example queries you can use with your AI assistant:
- "Do the probe and search my codebase for implementations of the ranking algorithm"
- "Using probe find all functions related to error handling in the src directory"

For more information on MCP integration, see the [MCP Integration](/mcp-integration) and [MCP Server](/mcp-server) documentation.

## Node.js SDK Usage

Use Probe programmatically in your Node.js applications:

```javascript
import { search, query, extract } from '@buger/probe';

// Search for code
const searchResults = await search({
  path: '/path/to/your/project',
  query: 'authentication',
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

// Extract with LLM prompt and instructions
const extractWithPrompt = await extract({
  files: ['/path/to/your/project/src/auth.js#authenticate'],
  prompt: 'engineer',
  instructions: 'Explain this authentication function'
});

console.log(searchResults);
```

For comprehensive documentation on the Node.js SDK, see the [Node.js SDK](/nodejs-sdk) documentation.

## Choose Your Path

Depending on how you plan to use Probe, we have specific guides tailored to different user needs:

### I am an AI Code Editor User

If you want to integrate Probe with your AI-powered code editor (like Cline, GitHub Copilot, or other MCP-compatible assistants):

- [Integrating Probe into AI Code Editors](/use-cases/integrating-probe-into-ai-code-editors) - Set up the MCP server, configure your editor, and start asking questions about your code

### I am a CLI Power User

If you need advanced text-based queries or want to feed code blocks into LLMs:

- [Advanced CLI: Speed, Token Limits, and Large Repos](/use-cases/advanced-cli) - Learn techniques for handling large codebases, optimizing token usage, and integrating with other CLI tools

### I am a Team Admin

If you're looking for a centralized server or chat interface for your team:

- [Deploying the Probe Web Interface](/use-cases/deploying-probe-web-interface) - Set up a Docker-based deployment, manage security, and configure for team usage

### I am an AI Tooling Developer

If you want to embed Probe programmatically into your Node.js or Python tools:

- [Building AI Tools on Probe](/use-cases/building-ai-tools) - Use the Node.js SDK, integrate with LangChain or Vercel AI SDK, and build custom AI-powered code tools

## Reference Documentation

For comprehensive reference information:

- [CLI Reference (Commands & Flags)](/cli-mode) - Complete command-line reference
- [AI Integration Overview](/ai-integration) - Overview of all AI capabilities
- [MCP Protocol & Tools Reference](/mcp-server) - Detailed MCP server documentation
- [Node.js SDK API Reference](/nodejs-sdk) - Complete SDK API reference
- [Output Formats Reference](/output-formats) - JSON, XML, and other output formats
- [How It Works](/how-it-works) - Understand the technology behind Probe
- [Language Support](/language-support-overview) - See how Probe understands different programming languages