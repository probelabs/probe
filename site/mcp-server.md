# MCP Server Mode

The Model Context Protocol (MCP) server mode allows Probe to integrate seamlessly with AI editors and assistants. This mode exposes Probe's powerful search capabilities through a standardized interface that AI tools can use to search and understand your codebase.

## What is MCP?

MCP (Model Context Protocol) is a protocol that enables AI assistants to access external tools and resources. By running Probe as an MCP server, AI assistants can use Probe's search capabilities to find and understand code in your projects.

## Setting Up the MCP Server

### Using NPX (Recommended)

The easiest way to use Probe's MCP server is through NPX:

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
       "memory": {
         "command": "probe-mcp"
       }
     }
   }
   ```

## Using Probe with AI Assistants

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

## Advanced Configuration

### Custom Search Paths

You can configure the MCP server to search specific directories by default:

```json
{
  "mcpServers": {
    "memory": {
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
    "memory": {
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

## Troubleshooting

If you encounter issues with the MCP server:

1. **Check Installation**: Ensure Probe is correctly installed and accessible in your PATH
2. **Verify Configuration**: Double-check your MCP configuration file for errors
3. **Check Permissions**: Make sure the AI editor has permission to execute the MCP server
4. **Check Logs**: Look for error messages in your AI editor's logs
5. **Update Packages**: Ensure you're using the latest version of the `@buger/probe-mcp` package

## Best Practices

1. **Be Specific**: More specific queries yield better results
2. **Mention File Types**: If you're looking for code in specific file types, mention them
3. **Mention Directories**: If you know which directory contains the code, include it in your query
4. **Use Multiple Queries**: If you don't find what you're looking for, try reformulating your query
5. **Combine with Other Tools**: Use Probe alongside other tools for a more comprehensive understanding of your codebase