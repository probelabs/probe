# MCP Server Integration

The Model Context Protocol (MCP) server integration allows Probe to seamlessly connect with AI assistants and editors. This document provides comprehensive information about setting up, configuring, and using Probe's MCP server capabilities.

## What is MCP?

MCP (Model Context Protocol) is a standardized protocol that enables AI assistants to access external tools and resources. By running Probe as an MCP server, AI assistants can use Probe's powerful search capabilities to find and understand code in your projects.

Key benefits:

- **Seamless AI Integration**: Allows AI assistants to search and analyze your code
- **Standardized Protocol**: Uses the Model Context Protocol for compatibility with various AI tools
- **Enhanced AI Capabilities**: Gives AI assistants code-aware capabilities
- **Secure Access**: Provides controlled access to your codebase

## Setting Up the MCP Server

### Using NPX (Recommended)

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

### Manual Installation

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

### Technical Implementation

The Probe MCP server:

- Implements the Model Context Protocol specification
- Uses stdio for communication with AI editors
- Automatically downloads and manages the Probe binary
- Provides three main tools: search_code, query_code, and extract_code
- Handles tool execution and error reporting

## Available Tools

The Probe MCP server provides the following tools:

### search_code

Search code in a specified directory using Elasticsearch-like query syntax with session-based caching.

#### Parameters

| Parameter | Type | Description | Required |
|-----------|------|-------------|----------|
| `path` | string | Absolute path to the directory to search in | Yes |
| `query` | string \| string[] | Query patterns to search for with Elasticsearch-like syntax support | Yes |
| `filesOnly` | boolean | Skip AST parsing and just output unique files | No |
| `ignore` | string[] | Custom patterns to ignore | No |
| `excludeFilenames` | boolean | Exclude filenames from being used for matching | No |
| `reranker` | string | Reranking method ('hybrid', 'hybrid2', 'bm25', 'tfidf') | No |
| `frequencySearch` | boolean | Use frequency-based search with stemming and stopword removal | No |
| `exact` | boolean | Use exact matching without stemming or stopword removal | No |
| `maxResults` | number | Maximum number of results to return | No |
| `maxBytes` | number | Maximum total bytes of code content to return | No |
| `maxTokens` | number | Maximum total tokens in code content to return (for AI usage) | No |
| `allowTests` | boolean | Allow test files and test code blocks in search results | No |
| `noMerge` | boolean | Disable merging of adjacent code blocks after ranking | No |
| `mergeThreshold` | number | Maximum number of lines between code blocks to consider them adjacent for merging | No |
| `session` | string | Session identifier for caching | No |

#### Example

```json
{
  "path": "/path/to/your/project",
  "query": "authentication flow",
  "maxTokens": 20000
}
```

#### Query Syntax

The search tool supports Elasticsearch-like query syntax with the following features:

- **Basic term searching**: `config` or `search`
- **Field-specific searching**: `field:value` (e.g., `function:parse`)
- **Required terms with + prefix**: `+required`
- **Excluded terms with - prefix**: `-excluded`
- **Logical operators**: `term1 AND term2`, `term1 OR term2`
- **Grouping with parentheses**: `(term1 OR term2) AND term3`

### query_code

Find specific code structures (functions, classes, etc.) using tree-sitter patterns.

#### Parameters

| Parameter | Type | Description | Required |
|-----------|------|-------------|----------|
| `path` | string | Absolute path to the directory to search in | Yes |
| `pattern` | string | The ast-grep pattern to search for | Yes |
| `language` | string | The programming language to search in | No |
| `ignore` | string[] | Custom patterns to ignore | No |
| `allowTests` | boolean | Allow test files and test code blocks in results | No |
| `maxResults` | number | Maximum number of results to return | No |
| `format` | string | Output format ('markdown', 'plain', 'json', 'color') | No |

#### Example

```json
{
  "path": "/path/to/your/project",
  "pattern": "fn $NAME($$$PARAMS) $$$BODY",
  "language": "rust"
}
```

#### Pattern Syntax

- `$NAME`: Matches an identifier (e.g., function name)
- `$$$PARAMS`: Matches parameter lists
- `$$$BODY`: Matches function bodies
- `$$$FIELDS`: Matches struct/class fields
- `$$$METHODS`: Matches class methods

### extract_code

Extract code blocks from files based on file paths and optional line numbers.

#### Parameters

| Parameter | Type | Description | Required |
|-----------|------|-------------|----------|
| `path` | string | Absolute path to the directory to search in | Yes |
| `files` | string[] | Files to extract from (can include line numbers with colon) | Yes |
| `allowTests` | boolean | Allow test files and test code blocks in results | No |
| `contextLines` | number | Number of context lines to include | No |
| `format` | string | Output format ('markdown', 'plain', 'json') | No |

#### Example

```json
{
  "path": "/path/to/your/project",
  "files": ["/path/to/your/project/src/main.rs:42"]
}
```

## Integration with AI Assistants

Once configured, you can ask your AI assistant to search your codebase with natural language queries. The AI will translate your request into appropriate Probe commands and display the results.

### Example Queries

Here are some examples of natural language queries you can use:

- "Do the probe and search my codebase for implementations of the ranking algorithm"
- "Using probe find all functions related to error handling in the src directory"
- "Search for code that handles user authentication"
- "Find all instances where we're using the BM25 algorithm"
- "Look for functions that process query parameters"

### How It Works

1. You ask a question about your codebase to your AI assistant
2. The AI assistant recognizes that Probe can help answer this question
3. The assistant formulates an appropriate search query and parameters
4. The MCP server executes the Probe search command
5. The results are returned to the AI assistant
6. The assistant analyzes the code and provides you with an answer

### Technical Details

The MCP server:

- Receives tool call requests from the AI assistant
- Parses the request parameters
- Executes the appropriate Probe command
- Returns the results to the AI assistant
- Handles errors and provides appropriate error messages
- Maintains session-based caching to avoid duplicate results

## Advanced Configuration

### Custom Search Paths

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

### Limiting Results

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

### Custom Binary Path

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

### Debug Mode

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

## Implementation Details

### Server Architecture

The Probe MCP server is implemented as a Node.js application that:

1. Initializes an MCP server instance
2. Sets up tool handlers for search_code, query_code, and extract_code
3. Connects to the AI editor using stdio transport
4. Handles tool call requests and returns results

### Binary Management

The server automatically manages the Probe binary:

1. Checks for an existing binary in the bin directory
2. Checks the PROBE_PATH environment variable
3. Downloads the binary if not found
4. Makes the binary executable
5. Tests the binary to ensure it works correctly

### Error Handling

The server includes robust error handling:

1. Catches and reports errors during tool execution
2. Provides detailed error messages for common issues
3. Includes suggestions for resolving errors
4. Gracefully handles timeouts and network errors

### Session-Based Caching

The server implements session-based caching to avoid showing the same code blocks multiple times:

1. Generates a unique session ID if not provided
2. Tracks which code blocks have been shown in the session
3. Filters out duplicate results in subsequent searches
4. Maintains the cache for the duration of the session

## Integration Examples

### Cline Integration

To integrate Probe with Cline:

1. Create a `.cline` directory in your project root
2. Create a `mcp_config.json` file with the following content:

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

3. Start Cline and ask questions about your codebase

### VSCode Integration

To integrate Probe with VSCode AI assistants:

1. Install the appropriate AI assistant extension
2. Configure the extension to use the Probe MCP server
3. Add the MCP configuration to your settings.json file

### Custom AI Application Integration

To integrate Probe with a custom AI application:

1. Implement the MCP client protocol in your application
2. Connect to the Probe MCP server using stdio or another supported transport
3. Send tool call requests to the server
4. Process the results and display them to the user

## Troubleshooting

### Common Issues

#### Binary Not Found

If you encounter a "Binary not found" error:

1. Check if the Probe binary exists in the expected location
2. Set the PROBE_PATH environment variable to the location of your Probe binary
3. Try reinstalling the @buger/probe-mcp package

#### Permission Denied

If you encounter a "Permission denied" error:

1. Make sure the binary is executable (`chmod +x /path/to/probe`)
2. Check if your user has permission to execute the binary
3. Try running the MCP server with elevated privileges

#### Network Error During Binary Download

If you encounter a network error during binary download:

1. Check your internet connection
2. Verify that GitHub API is accessible from your network
3. Try downloading the binary manually and placing it in the bin directory

#### Tool Calls Timeout

If tool calls timeout:

1. Increase the timeout value in your AI editor's configuration
2. Try a simpler query that will execute faster
3. Limit the search scope to a smaller directory

#### Empty Search Results

If you get empty search results:

1. Check your query syntax
2. Try a simpler query
3. Verify that the path exists and contains the expected files
4. Check if the files are being ignored by default patterns

### Detailed Error Messages

The MCP server provides detailed error messages for common issues:

- **Version not found**: The specified version of the Probe binary was not found in the repository
- **Network error**: A network error occurred while downloading the binary
- **Permission error**: A permission error occurred while downloading or extracting the binary
- **Binary extraction failed**: The binary could not be extracted from the downloaded archive
- **Binary execution failed**: The binary could not be executed

## Best Practices

1. **Be Specific**: More specific queries yield better results
2. **Mention File Types**: If you're looking for code in specific file types, mention them
3. **Mention Directories**: If you know which directory contains the code, include it in your query
4. **Use Multiple Queries**: If you don't find what you're looking for, try reformulating your query
5. **Combine with Other Tools**: Use Probe alongside other tools for a more comprehensive understanding of your codebase
6. **Use Session IDs**: For related searches, use the same session ID to avoid seeing duplicate code blocks
7. **Limit Result Size**: Use maxResults and maxTokens to limit the size of results
8. **Use Debug Mode**: Enable debug mode for detailed logging when troubleshooting issues

## Advanced Usage

### Custom Tool Implementations

You can create custom implementations of the Probe tools for specific use cases:

```javascript
// Custom search tool implementation
const customSearchTool = {
  name: 'custom_search',
  description: 'Custom search tool that filters results',
  parameters: {
    type: 'object',
    properties: {
      path: {
        type: 'string',
        description: 'Path to search in'
      },
      query: {
        type: 'string',
        description: 'Search query'
      }
    },
    required: ['path', 'query']
  },
  execute: async ({ path, query }) => {
    // Call the Probe search function
    const results = await search({
      path,
      query,
      maxResults: 10
    });
    
    // Apply custom filtering or processing
    const filteredResults = customFilterFunction(results);
    
    return filteredResults;
  }
};
```

### Integrating with Multiple AI Assistants

You can configure multiple AI assistants to use the same Probe MCP server:

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
        "PROBE_DEFAULT_PATHS": "/path/to/project"
      }
    }
  }
}
```

Then, configure each AI assistant to use the "probe" MCP server.

### Extending the MCP Server

You can extend the MCP server with additional tools:

1. Fork the @buger/probe-mcp repository
2. Add new tool implementations
3. Build and publish your custom version
4. Configure your AI editor to use your custom MCP server

## Security and Privacy Considerations

When using the Probe MCP server, consider the following security and privacy implications:

### Security

1. **Code Access**: The MCP server has access to your codebase, so be careful about which directories you allow it to search
2. **API Keys**: If your codebase contains API keys or other secrets, be aware that the AI assistant may have access to them
3. **Execution Environment**: The MCP server runs with the same permissions as the user who started it
4. **Network Access**: The MCP server needs network access to download the Probe binary
5. **Binary Verification**: Consider verifying the Probe binary before using it

### Privacy

1. **Local Search Engine**: Probe itself is a fully local semantic code search tool that doesn't require embedding generation or cloud indexing
2. **External AI Services**: When using Probe with external AI services (Anthropic, OpenAI, etc.), code snippets found by Probe are sent to those services
3. **Data Transmission**: The following data may be transmitted to external AI providers:
   - Your natural language queries
   - Code snippets and context found by Probe's search
   - Conversation history for contextual awareness
4. **Sensitive Code**: Be cautious when searching for sensitive or proprietary code that should not be shared with external services
5. **Local Model Options**: For maximum privacy, consider using Probe with locally-running AI models to keep all data on your machine

## Related Resources

- [Probe GitHub Repository](https://github.com/buger/probe)
- [Model Context Protocol Specification](https://github.com/modelcontextprotocol/mcp)
- [Probe Node.js SDK](https://github.com/buger/probe/tree/main/npm)
- [Probe AI Chat](https://github.com/buger/probe/tree/main/examples/chat)