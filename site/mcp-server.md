# MCP Server Mode

The Model Context Protocol (MCP) server mode allows Probe to integrate seamlessly with AI editors and assistants. This mode exposes Probe's powerful search capabilities through a standardized interface that AI tools can use to search and understand your codebase.

> **Note**: For comprehensive documentation on the MCP server integration, including advanced configuration, implementation details, and security considerations, see the [MCP Integration](./mcp-integration.md) page. For information on all AI integration features, see the [AI Integration](./ai-integration.md) page.

## What is MCP?

MCP (Model Context Protocol) is a protocol that enables AI assistants to access external tools and resources. By running Probe as an MCP server, AI assistants can use Probe's search capabilities to find and understand code in your projects.

## Migrating from @probelabs/probe-mcp

If you're currently using the standalone `@probelabs/probe-mcp` package, `probe mcp` is a **drop-in replacement**:

### Quick Migration

**Old configuration:**
```json
{
  "mcpServers": {
    "probe": {
      "command": "npx",
      "args": ["-y", "@probelabs/probe-mcp"]
    }
  }
}
```

**New configuration (drop-in replacement):**
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

All the same functionality, same tools, same parameters - just with the integrated `probe mcp` command!

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
        "@probelabs/probe@latest",
        "mcp"
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
   npm install -g @probelabs/probe@latest
   ```

2. Configure your AI editor to use the installed package:
   ```json
   {
     "mcpServers": {
       "memory": {
         "command": "probe",
      "args": ["mcp"]
       }
     }
   }
   ```

### Configuring Timeout

For large codebases, you may need to increase the timeout for search operations. You can configure the timeout using the `--timeout` or `-t` flag:

```json
{
  "mcpServers": {
    "memory": {
      "command": "npx",
      "args": [
        "-y",
        "@probelabs/probe@latest",
        "mcp",
        "--timeout",
        "60"
      ]
    }
  }
}
```

Or with manual installation:

```json
{
  "mcpServers": {
    "memory": {
      "command": "probe",
      "args": [
        "mcp",
        "--timeout",
        "60"
      ]
    }
  }
}
```

The timeout is specified in seconds (default: 30 seconds). You can also see all available options by running:

```bash
probe mcp --help
```

## Available Tools

The Probe MCP server provides the following tools:

### search_code

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

### query_code

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

### extract_code

Extract code blocks from files based on file paths and optional line numbers.

```json
{
  "path": "/path/to/your/project",
  "files": ["/path/to/your/project/src/main.rs:42"],
  "prompt": "engineer",
  "instructions": "Explain this function"
}
```

The extract_code tool supports the following parameters:
- `path`: The base directory to search in
- `files`: Array of file paths to extract from (can include line numbers with colon, e.g., "file.rs:42")
- `prompt`: Optional system prompt template for LLM models ("engineer", "architect", or path to file)
- `instructions`: Optional user instructions for LLM models
- `contextLines`: Number of context lines to include (default: 0)
- `format`: Output format (default: "json")

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
        "@probelabs/probe@latest"
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
        "@probelabs/probe@latest"
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
    "memory": {
      "command": "npx",
      "args": [
        "-y",
        "@probelabs/probe@latest"
      ],
      "env": {
        "PROBE_PATH": "/path/to/custom/probe"
      }
    }
  }
}
```

## Troubleshooting

If you encounter issues with the MCP server:

1. **Check Installation**: Ensure the Probe binary was downloaded correctly during package installation
2. **Verify Configuration**: Double-check your MCP configuration file for errors
3. **Check Permissions**: Make sure the AI editor has permission to execute the MCP server
4. **Check Logs**: Look for error messages in your AI editor's logs
5. **Update Packages**: Ensure you're using the latest version of the `@probelabs/probe@latest` package
6. **Manual Binary Download**: If the automatic download failed, you can manually download the binary from [GitHub Releases](https://github.com/probelabs/probe/releases) and place it in the `node_modules/@probelabs/probe/bin` directory

## Best Practices

1. **Be Specific**: More specific queries yield better results
2. **Mention File Types**: If you're looking for code in specific file types, mention them
3. **Mention Directories**: If you know which directory contains the code, include it in your query
4. **Use Multiple Queries**: If you don't find what you're looking for, try reformulating your query
5. **Combine with Other Tools**: Use Probe alongside other tools for a more comprehensive understanding of your codebase
6. **Use Session IDs**: For related searches, use the same session ID to avoid seeing duplicate code blocks