# Probe Chat

A command-line and web interface for interacting with Probe code search using AI models through the Vercel AI SDK.

## Features

- Interactive CLI chat interface
- Web-based chat interface with Markdown and syntax highlighting
- Support for Anthropic Claude, OpenAI, and Google Gemini models
- Force provider option to specify which AI provider to use
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
- An API key for Anthropic Claude, OpenAI, or Google Gemini

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
# GOOGLE_API_KEY=your_google_api_key

# Force a specific provider (optional)
# FORCE_PROVIDER=anthropic  # Options: anthropic, openai, google

# Debug mode (set to true for verbose logging)
DEBUG=false

# Default model (optional)
# For Anthropic: MODEL_NAME=claude-3-7-sonnet-latest
# For OpenAI: MODEL_NAME=gpt-4o-2024-05-13
# For Google: MODEL_NAME=gemini-2.0-flash

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
- `-m, --model <model>`: Specify the model to use (e.g., `claude-3-7-sonnet-latest`, `gpt-4o-2024-05-13`, `gemini-2.0-flash`)
- `-f, --force-provider <provider>`: Force a specific provider (options: `anthropic`, `openai`, `google`)
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

## Provider Options

Probe Chat supports multiple AI providers, giving you flexibility in choosing which model to use for your code search and analysis:

### Supported Providers

1. **Anthropic Claude**
   - Default model: `claude-3-7-sonnet-latest`
   - Environment variable: `ANTHROPIC_API_KEY`
   - Best for: Complex code analysis, detailed explanations, and understanding nuanced patterns

2. **OpenAI GPT**
   - Default model: `gpt-4o-2024-05-13`
   - Environment variable: `OPENAI_API_KEY`
   - Best for: General code search, pattern recognition, and concise explanations

3. **Google Gemini**
   - Default model: `gemini-2.0-flash`
   - Environment variable: `GOOGLE_API_KEY`
   - Best for: Fast responses, code generation, and efficient search

### Forcing a Specific Provider

You can force Probe Chat to use a specific provider in two ways:

1. **Using the command line option**:
   ```bash
   node index.js --force-provider anthropic
   node index.js --force-provider openai
   node index.js --force-provider google
   ```

2. **Using the environment variable**:
   Add this to your `.env` file:
   ```
   FORCE_PROVIDER=anthropic  # or openai, google
   ```

When forcing a provider, Probe Chat will verify that you have the corresponding API key set. If the API key is missing, it will display an error message.

### Customizing Models

You can specify which model to use for each provider:

1. **Using the command line option**:
   ```bash
   node index.js --model claude-3-7-sonnet-latest
   node index.js --model gpt-4o-2024-05-13
   node index.js --model gemini-2.0-flash
   ```

2. **Using the environment variable**:
   Add this to your `.env` file:
   ```
   MODEL_NAME=claude-3-7-sonnet-latest
   ```

Note that the model must be compatible with the selected provider. If you force a specific provider and specify a model, the model must be available for that provider.

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