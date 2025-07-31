# @buger/probe-mcp

MCP server for the Probe code search tool.

## Installation

```bash
# Install globally
npm install -g @buger/probe-mcp

# Or use directly with npx
npx @buger/probe-mcp
```

This package now uses the `@buger/probe` package as a dependency, which will automatically handle downloading the appropriate Probe binary for your system when needed.

## Usage

This package provides an MCP server that allows AI assistants to use the Probe code search tool.

### Command Line Options

```bash
probe-mcp [options]

Options:
  --timeout, -t <seconds>  Set timeout for search operations (default: 30)
  --help, -h              Show help message
```

### In Claude Desktop or VSCode

1. Add the MCP server to your configuration:

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

To use a custom timeout (e.g., 60 seconds):

```json
{
  "mcpServers": {
    "memory": {
      "command": "npx",
      "args": [
        "-y",
        "@buger/probe-mcp",
        "--timeout",
        "60"
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

## Troubleshooting

If you encounter issues with the MCP server:

1. **Package Dependencies**: Make sure both `@buger/probe-mcp` and `@buger/probe` are properly installed. The `@buger/probe` package handles the binary download and management.

2. **Binary Issues**: If you encounter issues with the Probe binary, you can check the binary path using:
   ```javascript
   import { getBinaryPath } from '@buger/probe';
   console.log(getBinaryPath());
   ```

3. **Package Name**: Make sure you're using `@buger/probe-mcp` (not `@buger/probe`) in your MCP configuration.

## License

ISC