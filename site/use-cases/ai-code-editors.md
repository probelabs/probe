# Using Probe with AI Code Editors

This guide explains how to integrate Probe with AI-powered code editors to enhance your coding experience with intelligent code search and understanding.

## Overview

AI code editors and assistants are powerful tools, but they often struggle with large codebases because they can't efficiently search and understand your entire project. Probe solves this problem by providing a specialized code search capability that AI assistants can use to find and understand relevant code.

By integrating Probe with your AI code editor, you enable your AI assistant to:

- Search your codebase with natural language queries
- Find specific code structures and patterns
- Extract complete code blocks with proper context
- Understand the relationships between different parts of your code

## Integration Options

### 1. MCP Server Integration (Recommended)

The Model Context Protocol (MCP) server integration is the most powerful way to connect Probe with AI editors. It allows the AI to directly use Probe's search capabilities through a standardized protocol.

#### Setting Up MCP Integration

1. **Create an MCP configuration file** in your project:

   For Cline, create `.cline/mcp_config.json`:

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

   For other editors, check your editor's documentation for MCP configuration.

2. **Start your AI editor** and begin asking questions about your codebase.

3. **Use natural language queries** to search your code:
   - "Find all authentication-related functions in the project"
   - "Show me how error handling is implemented"
   - "Search for code that processes user input"

### 2. Manual Search and Copy

If your AI editor doesn't support MCP, you can still use Probe manually:

1. **Run Probe searches** in your terminal:
   ```bash
   probe search "authentication" ./src
   ```

2. **Copy the results** into your conversation with the AI assistant.

3. **Ask follow-up questions** based on the code you've shared.

## Example Workflows

### Exploring a New Codebase

When joining a new project or exploring an unfamiliar codebase:

1. **Ask for an overview**: "Can you help me understand the structure of this project?"
2. **Explore key components**: "Show me the main entry points of the application"
3. **Dive deeper**: "How is authentication implemented in this project?"

### Implementing New Features

When adding new functionality to an existing project:

1. **Find similar patterns**: "Show me examples of API endpoints in this codebase"
2. **Understand dependencies**: "What modules would I need to modify to add a new user role?"
3. **Get implementation guidance**: "Based on the existing code, how should I implement this new feature?"

### Debugging Issues

When troubleshooting problems:

1. **Find error handling**: "Show me how errors are handled in the authentication flow"
2. **Trace execution paths**: "Find all places where user input is validated"
3. **Identify patterns**: "Are there similar issues elsewhere in the codebase?"

## Editor-Specific Setup

### Cline

1. Create `.cline/mcp_config.json` with the configuration shown above
2. Start Cline in your project directory
3. Ask questions about your codebase

### VSCode with GitHub Copilot

1. Install the GitHub Copilot extension
2. Configure Copilot to use external tools (if supported)
3. Add the Probe MCP configuration to your settings

### JetBrains IDEs with AI Assistant

1. Install the AI Assistant plugin
2. Configure the plugin to use external tools
3. Add the Probe MCP configuration to your settings

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

For large codebases, you can limit the amount of code returned:

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

## Best Practices

1. **Be specific in your queries**: Instead of "show me the code," ask "show me the authentication implementation in the user service"

2. **Provide context**: Tell the AI what you're trying to accomplish so it can formulate better searches

3. **Use follow-up questions**: Build on previous results to explore the code more deeply

4. **Combine with documentation**: Ask the AI to explain the code it finds by referencing project documentation

5. **Iterate on searches**: If you don't find what you're looking for, try reformulating your query

6. **Consider privacy implications**: Remember that when using Probe with external AI services, code snippets are sent to those services

## Troubleshooting

### Common Issues

| Issue | Solution |
|-------|----------|
| AI doesn't use Probe | Explicitly ask the AI to use Probe: "Using Probe, search for..." |
| Empty search results | Try a simpler query or check if you're searching in the right directory |
| Too much code returned | Use more specific queries or configure result limits |
| MCP server not starting | Check your MCP configuration and ensure npx is installed |
| Privacy concerns with sensitive code | Use local AI models or limit searches to non-sensitive code areas |

### Getting Help

If you encounter issues with the MCP integration:

1. Check the [MCP Server Reference](/mcp-server) for detailed information
2. Visit the [Probe GitHub repository](https://github.com/buger/probe) for the latest updates
3. Open an issue if you encounter a bug or have a feature request

## Next Steps

- Learn about [Probe's CLI capabilities](/cli-mode) for advanced usage
- Explore [hosting a team chat](/use-cases/team-chat) for collaborative code exploration
- Check out [building custom tools](/use-cases/nodejs-sdk) with the Node.js SDK