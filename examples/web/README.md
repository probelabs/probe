# Code Search Chat Interface

A web interface for the Probe code search tool, powered by AI (Anthropic Claude or OpenAI GPT).

## Features

- Interactive chat interface for code search
- Support for both Anthropic Claude and OpenAI GPT models
- Multiple search tools: search, query, and extract
- Markdown rendering with syntax highlighting
- Mermaid diagram support
- Streaming responses for real-time feedback
- API endpoints for programmatic access
- Optional basic authentication
- Uses the @buger/probe npm package

## Setup

### Environment Variables

Create a `.env` file in the web directory with the following variables:

```
# Required: At least one of these API keys must be provided
ANTHROPIC_API_KEY=your_anthropic_api_key
OPENAI_API_KEY=your_openai_api_key

# Optional: Override the default model
MODEL_NAME=claude-3-7-sonnet-latest  # or gpt-4o, etc.

# Optional: Override the default API URLs
ANTHROPIC_API_URL=https://api.anthropic.com
OPENAI_API_URL=https://api.openai.com/v1

# Optional: Configure the port (default: 3000)
PORT=8080

# Optional: Enable debug mode
DEBUG=true

# Optional: Enable raw request debugging to see the exact prompts sent to LLMs
DEBUG_RAW_REQUEST=true

# You can combine both for maximum debugging information
DEBUG=true DEBUG_RAW_REQUEST=true

# Required: Configure folders to search
ALLOWED_FOLDERS=/path/to/repo1,/path/to/repo2

# Optional: Authentication settings
AUTH_ENABLED=true  # Set to true to enable authentication
AUTH_USERNAME=admin  # Custom username (default: admin)
AUTH_PASSWORD=secure_password  # Custom password (default: password)
```

### Running Locally

1. Install dependencies:
   ```
   npm install
   ```

2. Start the server:
   ```
   npm start
   ```

3. Open your browser and navigate to `http://localhost:8080` (or whatever port you configured)

## Docker

### Building the Docker Image

```bash
docker build -t code-search-chat .
```

### Running with Docker

```bash
docker run -p 8080:8080 \
  -e ANTHROPIC_API_KEY=your_anthropic_api_key \
  -e ALLOWED_FOLDERS=/app/code1,/app/code2 \
  -v /path/to/local/code1:/app/code1 \
  -v /path/to/local/code2:/app/code2 \
  code-search-chat
```

Or with OpenAI and authentication:

```bash
docker run -p 8080:8080 \
  -e OPENAI_API_KEY=your_openai_api_key \
  -e MODEL_NAME=gpt-4o \
  -e ALLOWED_FOLDERS=/app/code1,/app/code2 \
  -e AUTH_ENABLED=true \
  -e AUTH_USERNAME=admin \
  -e AUTH_PASSWORD=secure_password \
  -v /path/to/local/code1:/app/code1 \
  -v /path/to/local/code2:/app/code2 \
  code-search-chat
```

### Environment Variables in Docker

All the environment variables mentioned in the Setup section can be passed to the Docker container using the `-e` flag.

## API Documentation

The application provides a full OpenAPI specification at `/openapi.yaml`. You can use this specification with tools like Swagger UI or Postman to explore and test the API.

### API Endpoints

The application provides the following API endpoints:

### 1. Search Code (`POST /api/search`)

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

### 2. Query Code (`POST /api/query`)

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

### 3. Extract Code (`POST /api/extract`)

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

### 4. Chat with AI (`POST /api/chat`)

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
  "toolCalls": [
    {
      "name": "searchCode",
      "arguments": {
        "keywords": "search pattern",
        "folder": "/path/to/repo"
      },
      "result": "search results"
    }
  ],
  "timestamp": "2025-08-03T07:10:00.000Z"
}
```

**Response (stream=true):**
Text stream of the AI response.

## Authentication

When authentication is enabled (`AUTH_ENABLED=true`), all endpoints (both UI and API) require basic authentication. The default username is `admin` and the default password is `password`, but these can be customized using the `AUTH_USERNAME` and `AUTH_PASSWORD` environment variables.

To authenticate API requests, include the `Authorization` header with the value `Basic <base64-encoded-credentials>`, where `<base64-encoded-credentials>` is the Base64 encoding of `username:password`.

Example:
```
Authorization: Basic YWRtaW46cGFzc3dvcmQ=
```

## API Model Support

The application will use the first available API in this order:
1. Anthropic Claude (if `ANTHROPIC_API_KEY` is provided)
2. OpenAI GPT (if `OPENAI_API_KEY` is provided)

You can override the default model by setting the `MODEL_NAME` environment variable.

Default models:
- Anthropic: `claude-3-7-sonnet-latest`
- OpenAI: `gpt-4o`

## Custom API URLs

If you're using a proxy or a custom endpoint for the APIs, you can override the default URLs:

- `ANTHROPIC_API_URL`: Default is `https://api.anthropic.com`
- `OPENAI_API_URL`: Default is `https://api.openai.com/v1`

## Dependencies

This application uses the following key dependencies:

- [@buger/probe](https://www.npmjs.com/package/@buger/probe): Node.js wrapper for the probe tool
- [Vercel AI SDK](https://sdk.vercel.ai/): The AI SDK used for model integration
- [Marked.js](https://marked.js.org/): For Markdown rendering
- [Highlight.js](https://highlightjs.org/): For syntax highlighting
- [Mermaid.js](https://mermaid.js.org/): For diagram rendering