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
    "probe": {
      "command": "npx",
      "args": ["@buger/probe-mcp"],
      "env": {}
    }
  }
}
```

2. Ask Claude to search your codebase using the probe tool.

## License

ISC