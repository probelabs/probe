# @buger/probe-mcp

MCP server for the Probe code search tool.

## Installation

```bash
# Install globally
npm install -g @buger/probe-mcp

# Or use directly with npx
npx @buger/probe-mcp
```

## Usage

This package provides an MCP server that allows AI assistants to use the Probe code search tool.

### In Claude Desktop or VSCode

1. Add the MCP server to your configuration:

```json
{
  "mcpServers": {
    "memory": {
      "command": "npx",
      "args": [
        "-y",
        "@buger/probe"
      ]
    }
  }
}
```

2. Ask your AI assistant to search your codebase using natural language queries like:

   - "Search my codebase for implementations of the ranking algorithm"
   - "Find all functions related to error handling in the src directory"
   - "Look for code that handles user authentication"

The AI will use the Probe tool to search your codebase and provide relevant code snippets and explanations.

## License

ISC