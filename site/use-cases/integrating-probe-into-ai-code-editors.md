# Integrating Probe into AI Code Editors

This guide explains how to integrate Probe with AI-powered code editors to enhance your coding experience with intelligent code search and understanding.

## Overview

AI code editors and assistants are powerful tools, but they often struggle with large codebases because they can't efficiently search and understand your entire project. Probe solves this problem by providing a specialized code search capability that AI assistants can use to find and understand relevant code.

## Quick Setup

### Installing @buger/probe-mcp with NPX

The easiest way to integrate Probe with your AI code editor is through the MCP (Model Context Protocol) server:

```bash
# You don't need to install anything permanently
# NPX will run the package directly
npx -y @buger/probe-mcp
```

This command will:
1. Download the Probe MCP server package
2. Automatically download the appropriate Probe binary for your platform
3. Start the MCP server, which will listen for requests from your AI editor

### Configuring Your AI Editor

#### For Cline

1. Create a `.cline` directory in your project root:
   ```bash
   mkdir -p .cline
   ```

2. Create an MCP configuration file:
   ```bash
   # Create the configuration file
   touch .cline/mcp_config.json
   ```

3. Add the following configuration to `.cline/mcp_config.json`:
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

4. Start Cline in your project directory and begin asking questions about your code.

#### For Roo Code

[Roo Code](https://github.com/RooVetGit/Roo-Code) can be configured the same way as other editors via MCP:

1. Follow the installation instructions from the Roo Code GitHub repository
2. Configure Roo to use Probe via MCP using the same configuration pattern
3. Start using Roo with Probe integration

#### For Cursor and Windsurf

1. Install Cursor or Windsurf
2. Configure the editor to use MCP
3. Add the Probe MCP server configuration
4. Restart the editor to apply changes

#### For Other AI Assistants

Check your AI editor's documentation for MCP configuration. The general pattern is similar:

1. Find where your editor stores MCP configuration
2. Add the Probe MCP server configuration
3. Restart your editor or reload the configuration

## Using Probe with Your AI Editor

Once configured, you can ask your AI assistant questions about your codebase:

- "Find all authentication-related functions in the project"
- "Show me how error handling is implemented"
- "Search for code that processes user input"

The AI will use Probe to search your codebase and provide relevant code snippets and explanations.

## Checking Logs and Debugging

If you encounter issues with the integration:

### Enable Debug Mode

```bash
# Run the MCP server with debug logging
DEBUG=1 npx -y @buger/probe-mcp
```

This will show detailed logs of:
- Tool calls from the AI
- Search queries being executed
- Results being returned

### Common Issues and Solutions

| Issue | Solution |
|-------|----------|
| AI doesn't use Probe | Explicitly ask the AI to use Probe: "Using Probe, search for..." |
| Empty search results | Try a simpler query or check if you're searching in the right directory |
| MCP server not starting | Check your MCP configuration and ensure npx is installed |
| Permission errors | Check file permissions on your codebase |

### Verifying the Integration

To verify that the integration is working:

1. Start your AI editor
2. Ask a question like: "Using Probe, find all functions related to authentication in this project"
3. Check if the AI responds with relevant code snippets
4. If not, check the MCP server logs for errors

## Advanced Configuration

For more control over the MCP server:

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
        "PROBE_DEFAULT_PATHS": "/path/to/project1,/path/to/project2",
        "PROBE_MAX_TOKENS": "20000",
        "DEBUG": "1"
      }
    }
  }
}
```

This configuration:
- Sets default search paths
- Limits the maximum tokens returned
- Enables debug logging

## Next Steps

- For advanced MCP usage, see the [MCP Protocol & Tools Reference](/mcp-server)
- For larger queries or team collaboration, check out the [Web Interface for Teams](/use-cases/team-chat)
- For CLI-based workflows, explore [Advanced CLI Usage](/use-cases/advanced-cli)