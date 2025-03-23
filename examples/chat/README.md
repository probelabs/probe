# Probe Chat

A command-line and web interface for interacting with Probe code search using AI models through the Vercel AI SDK.

## Features

- Interactive CLI chat interface
- Web-based chat interface with Markdown and syntax highlighting
- Support for both Anthropic Claude and OpenAI models
- Semantic code search using Probe's search capabilities
- AST-based code querying for finding specific code structures
- Code extraction for viewing complete context
- Session-based search caching for improved performance
- Token usage tracking
- Colorized output for better readability (CLI mode)
- Diagram generation with Mermaid.js (Web mode)

## Prerequisites

- Node.js 18 or higher
- Probe CLI installed and available in your PATH
- An API key for either Anthropic Claude or OpenAI

## Installation

1. Clone the repository
2. Navigate to the `examples/chat` directory
3. Install dependencies:

```bash
npm install
```

4. Create a `.env` file with your API keys:

```
# API Keys (uncomment and add your key)
ANTHROPIC_API_KEY=your_anthropic_api_key
# OPENAI_API_KEY=your_openai_api_key

# Debug mode (set to true for verbose logging)
DEBUG=false

# Default model (optional)
# For Anthropic: MODEL_NAME=claude-3-7-sonnet-latest
# For OpenAI: MODEL_NAME=gpt-4o-2024-05-13

# Folders to search (comma-separated list of paths)
# If not specified, the current directory will be used by default
# ALLOWED_FOLDERS=/path/to/folder1,/path/to/folder2

# Web interface settings (optional)
# PORT=8080
# AUTH_ENABLED=false
# AUTH_USERNAME=admin
# AUTH_PASSWORD=password
```

## Usage

### CLI Mode

Start the chat interface in CLI mode:

```bash
node index.js
```

Or with npm:

```bash
npm start
```

### Web Mode

Start the chat interface in web mode:

```bash
node index.js --web
```

Or with npm:

```bash
npm run web
```

You can specify a custom port:

```bash
node index.js --web --port 3000
```

You can also specify a path to the codebase you want to search:

```bash
node index.js /path/to/codebase
```

For example, to search in a repository located at ../../tyk:

```bash
node index.js ../../tyk
```

This will override any ALLOWED_FOLDERS setting in your .env file.

### Command-line Options

- `-d, --debug`: Enable debug mode for verbose logging
- `-m, --model <model>`: Specify the model to use (e.g., `claude-3-7-sonnet-latest`, `gpt-4o-2024-05-13`)
- `-w, --web`: Run in web interface mode
- `-p, --port <port>`: Port to run web server on (default: 8080)
- `[path]`: Path to the codebase to search (overrides ALLOWED_FOLDERS)

### Special Commands

During the chat, you can use these special commands:

- `exit` or `quit`: End the chat session
- `usage`: Display token usage statistics
- `clear`: Clear the chat history and start a new session

## How It Works

This CLI tool uses the Vercel AI SDK to interact with AI models and provides them with tools to search and analyze your codebase:

1. **search**: Searches code using Elasticsearch-like query syntax
2. **query**: Searches code using AST-based pattern matching
3. **extract**: Extracts code blocks from files with context

The AI is instructed to use these tools to answer your questions about the codebase, providing relevant code snippets and explanations.

### Search Caching

The tool automatically generates a unique session ID for each chat session and passes it to the Probe CLI commands using the `--session` parameter. This enables caching of search results within a session, which can significantly improve performance when similar searches are performed multiple times.

The session ID is managed internally and doesn't require any user intervention. When you start a new chat session (or use the "clear" command), a new session ID is generated, and a new cache is created.

## Example Queries

- "How does the config loading work?"
- "Show me all RPC handlers"
- "What does the process_file function do?"
- "Find all implementations of the extract tool"
- "Show me the main entry point of the application"

## Architecture

- `index.js`: Main entry point for both CLI and web interfaces
- `probeChat.js`: Core chat functionality
- `webServer.js`: Web server implementation
- `auth.js`: Authentication middleware for web interface
- `probeTool.js`: Tool definitions for code search, query, and extraction
- `tokenCounter.js`: Utility for tracking token usage
- `index.html`: Web interface HTML template

## License

Apache-2.0 