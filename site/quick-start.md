# Quick Start

This guide will help you get up and running with Probe quickly. For more detailed information, check out the other sections of the documentation.

## Basic Search Example

Search for code containing the phrase "llm pricing" in the current directory:

```bash
probe search "llm pricing" ./
```

This will search for the terms "llm" and "pricing" in your codebase and return the most relevant code blocks.

## Advanced Search (with Token Limiting)

Search for "prompt injection" in the current directory but limit the total tokens to 10000 (useful for AI tools with context window constraints):

```bash
probe search "prompt injection" ./ --max-tokens 10000
```

This is particularly useful when you need to feed the results into an AI model with a limited context window.

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

## Interactive AI Chat

Use the built-in AI assistant to ask questions about your codebase:

```bash
# Set your API key first
export ANTHROPIC_API_KEY=your_api_key
# Then start the chat interface
probe chat
```

Example questions you might ask:
- "How does the ranking algorithm work?"
- "Explain the file structure of this project"
- "What are the main components of the search functionality?"

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

## Next Steps

- Learn more about the [CLI Mode](/cli-mode) for detailed command options
- Explore the [AI Chat Mode](/ai-chat) for interactive code exploration
- Check out the [Web Interface](/web-interface) for a browser-based experience
- Understand [How It Works](/how-it-works) to get the most out of Probe