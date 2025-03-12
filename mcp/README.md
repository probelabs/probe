# @buger/probe-mcp

MCP server for the Probe code search tool.

## Installation

```bash
# Install globally
npm install -g @buger/probe-mcp

# Or use directly with npx
npx @buger/probe-mcp
```

During installation, the package will automatically download the appropriate Probe binary for your system. This binary is required for the MCP server to function.

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
        "@buger/probe-mcp"
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

1. **Binary Download**: The Probe binary should be automatically downloaded during package installation. If this fails, you can manually download it from [GitHub Releases](https://github.com/buger/probe/releases) and place it in the `node_modules/@buger/probe-mcp/bin` directory (or in the global npm package location if installed globally).

2. **Environment Variable**: You can set the `PROBE_PATH` environment variable to specify the location of an existing Probe binary.

3. **Package Name**: Make sure you're using `@buger/probe-mcp` (not `@buger/probe`) in your MCP configuration.

## License

ISC