# Web Interface

Probe includes a web-based chat interface that provides a user-friendly way to interact with your codebase using AI. The web interface offers a modern UI for code search and AI-powered code exploration.

## Unified Interface

Probe now features a unified interface that combines both CLI and web functionality in a single package. This change merges the previously separate `@buger/probe-web` and `@buger/probe-chat` packages into a single `@buger/probe-chat` package that supports both modes of operation.

### Benefits of the Unified Interface

- **Code Reuse**: Shared codebase for both CLI and web interfaces reduces duplication
- **Easier Maintenance**: Single package to update and maintain
- **Consistent Experience**: Same core functionality and tools available in both interfaces
- **Simplified Installation**: One package to install for all Probe chat functionality
- **Unified Configuration**: Same environment variables and settings for both modes

> **Note**: The `@buger/probe-web` package is now deprecated. Its functionality is included in the `@buger/probe-chat` package.

## Quick Start with npx

The easiest way to use Probe's interface is through npx:

```bash
# Run in CLI mode (default)
npx -y @buger/probe-chat

# Run in web interface mode
npx -y @buger/probe-chat --web

# Set your API key first (either Anthropic or OpenAI)
export ANTHROPIC_API_KEY=your_api_key
# OR
export OPENAI_API_KEY=your_api_key

# Configure allowed folders (required)
export ALLOWED_FOLDERS=/path/to/folder1,/path/to/folder2
```

## Installation and Setup

### Using npm

```bash
# Install globally
npm install -g @buger/probe-chat

# Run in CLI mode
probe-chat

# Run in web mode
probe-chat --web
```

### Manual Installation

1. **Clone the repository**:
   ```bash
   git clone https://github.com/buger/probe.git
   cd probe/examples/chat
   ```

2. **Install dependencies**:
   ```bash
   npm install
   ```

3. **Configure environment variables**:
   Create or edit the `.env` file in the chat directory:
   ```
   # Required: At least one of these API keys must be provided
   ANTHROPIC_API_KEY=your_anthropic_api_key
   OPENAI_API_KEY=your_openai_api_key
   
   # Required: Configure folders to search
   ALLOWED_FOLDERS=/path/to/repo1,/path/to/repo2
   
   # Optional configuration
   PORT=8080
   MODEL_NAME=claude-3-7-sonnet-latest
   AUTH_ENABLED=false
   AUTH_USERNAME=admin
   AUTH_PASSWORD=password
   ```

4. **Start the interface**:
   ```bash
   # CLI mode
   npm start
   
   # Web mode
   npm run web
   ```

## Command-Line Options

The unified interface supports the following command-line options:

| Option | Description | Default |
|--------|-------------|---------|
| `-d, --debug` | Enable debug mode for verbose logging | false |
| `-m, --model <model>` | Specify the model to use | claude-3-7-sonnet-latest (Anthropic) or gpt-4o (OpenAI) |
| `-w, --web` | Run in web interface mode | false (CLI mode) |
| `-p, --port <port>` | Port to run web server on (web mode only) | 8080 |
| `[path]` | Path to the codebase to search (overrides ALLOWED_FOLDERS) | Current directory |

Examples:

```bash
# Run in web mode on port 3000
node index.js --web --port 3000

# Run in CLI mode with a specific model
node index.js --model claude-3-7-sonnet-latest

# Search a specific codebase
node index.js /path/to/codebase
```

## Features

### Interactive Chat UI

The web interface provides a clean, modern chat interface with:

- Markdown rendering for rich text formatting
- Syntax highlighting for code blocks
- Support for multiple programming languages
- Persistent conversation history
- Streaming responses for real-time feedback

### AI-Powered Code Search

The interface uses AI (Anthropic Claude or OpenAI GPT) to:

- Search your codebase based on natural language queries
- Explain code functionality and architecture
- Generate diagrams and visualizations
- Provide context-aware responses

### Multiple Search Tools

The interface provides access to three powerful search tools:

1. **Search Tool**: Standard regex-based code search with stemming and stopword removal
2. **Query Tool**: AST-based structural pattern matching for precise code queries
3. **Extract Tool**: File content extraction with optional line ranges and context

### Mermaid Diagram Support

The web interface can render visual diagrams for:

- Code architecture
- Data flows
- Class hierarchies
- Sequence diagrams
- State machines

### Configurable Search Paths

You can define which directories can be searched via environment variables, allowing you to:

- Limit search to specific projects
- Include multiple codebases
- Exclude sensitive directories
- Control access to different parts of your filesystem

## Authentication System

The web interface includes optional basic authentication to secure access to your codebase. This feature is disabled by default but can be enabled via environment variables.

### Enabling Authentication

To enable authentication, set the following environment variables:

```bash
# Enable authentication
export AUTH_ENABLED=true

# Set custom username and password (optional)
export AUTH_USERNAME=admin
export AUTH_PASSWORD=secure_password
```

Or in your `.env` file:

```
AUTH_ENABLED=true
AUTH_USERNAME=admin
AUTH_PASSWORD=secure_password
```

### How Authentication Works

When authentication is enabled:

1. All web interface endpoints (both UI and API) require basic authentication
2. The browser will prompt for username and password when accessing the interface
3. API requests must include the `Authorization` header with the value `Basic <base64-encoded-credentials>`
4. Invalid or missing credentials will result in a 401 Unauthorized response

### Authentication Configuration

| Variable | Description | Default |
|----------|-------------|---------|
| `AUTH_ENABLED` | Enable basic authentication | false |
| `AUTH_USERNAME` | Username for basic authentication | admin |
| `AUTH_PASSWORD` | Password for basic authentication | password |

## API Endpoints

The web interface provides RESTful API endpoints for programmatic access without using the AI chat interface. These endpoints allow direct access to Probe's core functionality.

### Available Endpoints

#### 1. Search Code (`POST /api/search`)

Search code repositories using the Probe tool.

**Request:**
```json
{
  "keywords": "search pattern",
  "folder": "/path/to/repo",
  "exact": false,
  "allow_tests": false
}
```

**Parameters:**
- `keywords` (required): Search pattern
- `folder` (optional): Path to search in (must be one of the allowed folders)
- `exact` (optional): Use exact match (default: false)
- `allow_tests` (optional): Include test files in results (default: false)

**Response:**
```json
{
  "results": "search results text",
  "command": "probe command that was executed",
  "timestamp": "2025-08-03T07:10:00.000Z"
}
```

#### 2. Query Code (`POST /api/query`)

Search code using ast-grep structural pattern matching.

**Request:**
```json
{
  "pattern": "function $NAME($$$PARAMS) { $$$BODY }",
  "path": "/path/to/repo",
  "language": "javascript",
  "allow_tests": false
}
```

**Parameters:**
- `pattern` (required): AST pattern to search for
- `path` (optional): Path to search in (must be one of the allowed folders)
- `language` (optional): Programming language to use for parsing
- `allow_tests` (optional): Include test files in results (default: false)

**Response:**
```json
{
  "results": "query results text",
  "timestamp": "2025-08-03T07:10:00.000Z"
}
```

#### 3. Extract Code (`POST /api/extract`)

Extract code blocks from files based on file paths and optional line numbers.

**Request:**
```json
{
  "file_path": "src/main.js:42",
  "line": 42,
  "end_line": 60,
  "allow_tests": false,
  "context_lines": 10,
  "format": "plain"
}
```

**Parameters:**
- `file_path` (required): Path to the file to extract from
- `line` (optional): Start line number
- `end_line` (optional): End line number
- `allow_tests` (optional): Allow test files (default: false)
- `context_lines` (optional): Number of context lines (default: 10)
- `format` (optional): Output format (default: "plain")

**Response:**
```json
{
  "results": "extracted code text",
  "timestamp": "2025-08-03T07:10:00.000Z"
}
```

#### 4. Chat with AI (`POST /api/chat`)

Send a message to the AI and get a response.

**Request:**
```json
{
  "message": "your question about the code",
  "stream": true
}
```

**Parameters:**
- `message` (required): The message to send to the AI
- `stream` (optional): Whether to stream the response (default: true)

**Response (stream=false):**
```json
{
  "response": "AI response text",
  "tokenUsage": {
    "request": 123,
    "response": 456,
    "total": 579
  },
  "timestamp": "2025-08-03T07:10:00.000Z"
}
```

**Response (stream=true):**
Text stream of the AI response.

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `ANTHROPIC_API_KEY` | Your Anthropic API key for Claude | (Optional if `OPENAI_API_KEY` is provided) |
| `OPENAI_API_KEY` | Your OpenAI API key for GPT models | (Optional if `ANTHROPIC_API_KEY` is provided) |
| `ALLOWED_FOLDERS` | Comma-separated list of folders that can be searched | (Required) |
| `PORT` | The port to run the server on | 8080 |
| `MODEL_NAME` | Override the default model | claude-3-7-sonnet-latest (Anthropic) or gpt-4o (OpenAI) |
| `ANTHROPIC_API_URL` | Override the default Anthropic API URL | https://api.anthropic.com/v1 |
| `OPENAI_API_URL` | Override the default OpenAI API URL | https://api.openai.com/v1 |
| `DEBUG` | Enable debug mode | false |
| `DEBUG_RAW_REQUEST` | Enable raw request debugging | false |
| `AUTH_ENABLED` | Enable basic authentication | false |
| `AUTH_USERNAME` | Username for basic authentication | admin |
| `AUTH_PASSWORD` | Password for basic authentication | password |

## Architecture

The unified interface consists of:

- `index.js`: Main entry point for both CLI and web interfaces
- `probeChat.js`: Core chat functionality
- `webServer.js`: Web server implementation
- `auth.js`: Authentication middleware for web interface
- `probeTool.js`: Tool definitions for code search, query, and extraction
- `tokenCounter.js`: Utility for tracking token usage
- `index.html`: Web interface HTML template

## Debugging

The unified interface includes debugging options to help troubleshoot issues:

### Debug Mode

Enable debug mode to see detailed logging:

```bash
# Via environment variable
DEBUG=true npm start

# Via command-line option
node index.js --debug
```

This will output:
- API requests and responses
- Tool usage information
- System message details
- Token usage estimates

### Raw Request Debugging

Enable raw request debugging to see the exact prompts sent to LLMs:

```bash
DEBUG_RAW_REQUEST=true npm start
```

### Combined Debugging

For maximum debugging information, enable both options:

```bash
DEBUG=true DEBUG_RAW_REQUEST=true npm start
```

## Troubleshooting

### Common Issues

1. **API Key Issues**:
   - Ensure your API key is correctly set in the environment variables
   - Check for any spaces or special characters in the key
   - Verify the API key is active and has sufficient permissions

2. **Folder Access Issues**:
   - Ensure the folders in `ALLOWED_FOLDERS` exist and are accessible
   - Check file permissions on the folders
   - Use absolute paths to avoid path resolution issues

3. **Model Errors**:
   - Check if the specified model is available in your API plan
   - Verify the model name is correct
   - Try using a different model if you encounter rate limits

4. **Connection Issues**:
   - Check your internet connection
   - Verify the API URLs are correct if using custom endpoints
   - Check if there are any firewalls blocking the connection

### Getting Help

If you encounter issues not covered in this documentation:

1. Check the console output for error messages
2. Enable debug mode for more detailed logging
3. Check the GitHub repository for known issues
4. Open a new issue on GitHub with detailed information about your problem