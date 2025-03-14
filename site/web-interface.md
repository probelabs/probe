# Web Interface

Probe includes a web-based chat interface that provides a user-friendly way to interact with your codebase using AI. The web interface offers a modern UI for code search and AI-powered code exploration.

## Quick Start with npx

The easiest way to use Probe's web interface is through npx:

```bash
# Run directly with npx (no installation needed)
npx -y @buger/probe-web

# Set your API key first (either Anthropic or OpenAI)
export ANTHROPIC_API_KEY=your_api_key
# OR
export OPENAI_API_KEY=your_api_key

# Configure allowed folders (required)
export ALLOWED_FOLDERS=/path/to/folder1,/path/to/folder2
```

This will start a local web server and open the interface in your default browser.

## Features

### Interactive Chat UI

The web interface provides a clean, modern chat interface with:

- Markdown rendering for rich text formatting
- Syntax highlighting for code blocks
- Support for multiple programming languages
- Persistent conversation history
- Streaming responses for real-time feedback

### AI-Powered Code Search

The web interface uses AI (Anthropic Claude or OpenAI GPT) to:

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

### API Endpoints

The web interface provides RESTful API endpoints for programmatic access:

- `/api/search`: Search code repositories
- `/api/query`: Perform AST-based structural pattern matching
- `/api/extract`: Extract code blocks from files
- `/api/chat`: Chat with the AI about your code

### Optional Authentication

The web interface includes optional basic authentication to:

- Secure access to your codebase
- Prevent unauthorized usage
- Customize username and password
- Protect sensitive code information

## Setup and Configuration

### Prerequisites

- Node.js (v14 or later)
- NPM (v6 or later)
- An Anthropic API key for Claude OR an OpenAI API key for GPT models

### Manual Installation

1. **Navigate to the web directory**:
   ```bash
   cd examples/web
   ```

2. **Install dependencies**:
   ```bash
   npm install
   ```

3. **Configure environment variables**:
   Create or edit the `.env` file in the web directory:
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
   ```

4. **Start the server**:
   ```bash
   npm start
   ```

5. **Access the web interface**:
   Open your browser and navigate to `http://localhost:8080` (or whatever port you configured)

### Environment Variables

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

## Using the Web Interface

### Starting a Conversation

1. Open your browser and navigate to `http://localhost:8080`
2. Type your question or request in the input field
3. Press Enter or click the Send button

### Example Queries

- "Explain the architecture of this project"
- "How does the search functionality work?"
- "Create a diagram showing the main components"
- "Find all code related to authentication"
- "Explain the ranking algorithm implementation"

### Viewing Code

Code blocks are displayed with syntax highlighting for better readability. You can:

- Copy code blocks with the copy button
- Expand/collapse long code blocks
- See the file path for each code block

### Viewing Diagrams

When you ask for a diagram, the AI will generate a Mermaid diagram that is rendered directly in the chat. You can:

- Zoom in/out of diagrams
- Copy the diagram as an image
- Copy the Mermaid source code

## API Documentation

The web interface provides a full OpenAPI specification at `/openapi.yaml`. You can use this specification with tools like Swagger UI or Postman to explore and test the API.

### API Endpoints

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

## Technical Details

### Architecture

The web interface consists of:

- A Node.js backend server
- A vanilla JavaScript frontend
- The Vercel AI SDK for model integration
- Probe command execution via the probeTool.js module

### Technologies Used

- **@buger/probe**: Node.js wrapper for the probe tool
- **Vercel AI SDK**: For model integration and streaming responses
- **Marked.js**: For rendering markdown
- **Highlight.js**: For syntax highlighting
- **Mermaid.js**: For diagram generation and visualization
- **Express.js**: For the backend server

### API Model Support

The application will use the first available API in this order:
1. Anthropic Claude (if `ANTHROPIC_API_KEY` is provided)
2. OpenAI GPT (if `OPENAI_API_KEY` is provided)

You can override the default model by setting the `MODEL_NAME` environment variable.

Default models:
- Anthropic: `claude-3-7-sonnet-latest`
- OpenAI: `gpt-4o`

### Security Considerations

- The web interface only allows searching in directories specified in the `ALLOWED_FOLDERS` environment variable
- API keys are kept server-side and never exposed to the client
- User inputs are sanitized to prevent command injection
- The server runs locally by default and is not exposed to the internet
- Optional authentication can be enabled to restrict access

## Deployment

### Docker Deployment

The web interface includes a Dockerfile for containerized deployment.

#### Building the Docker Image

```bash
docker build -t code-search-chat .
```

#### Running with Docker

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

### Cloud Deployment

While the web interface is designed to run locally, you can deploy it to cloud platforms:

1. **Deploy to a VPS or dedicated server**:
   - Set up a Node.js environment
   - Clone the repository
   - Configure environment variables
   - Use a process manager like PM2 to keep the server running
   - Set up a reverse proxy with Nginx or Apache

2. **Deploy to a container platform**:
   - Use the provided Dockerfile
   - Deploy to platforms like AWS ECS, Google Cloud Run, or Azure Container Instances
   - Configure environment variables in the platform's settings
   - Mount volumes for code repositories or use cloud storage

3. **Security considerations for cloud deployment**:
   - Always enable authentication
   - Use HTTPS with a valid SSL certificate
   - Implement IP whitelisting if possible
   - Consider using a VPN for additional security
   - Regularly update dependencies and the base image

## Customization

### Styling

You can customize the appearance of the web interface by editing the CSS in the `examples/web/index.html` file.

### Adding Features

The web interface is built with vanilla JavaScript, making it easy to modify and extend. Common customizations include:

- Adding authentication
- Implementing user preferences
- Adding support for additional AI models
- Creating custom visualizations

### Integration with Other Tools

The web interface can be integrated with other development tools:

#### IDE Extensions

You can create extensions for popular IDEs that launch the web interface with the current project:

```javascript
// Example VS Code extension command
vscode.commands.registerCommand('probe.openWebInterface', () => {
  const currentWorkspace = vscode.workspace.workspaceFolders[0].uri.fsPath;
  const env = Object.assign({}, process.env, {
    ALLOWED_FOLDERS: currentWorkspace,
    ANTHROPIC_API_KEY: config.get('anthropicApiKey')
  });
  
  const server = spawn('npx', ['-y', '@buger/probe-web'], { env });
  // Handle server output...
});
```

#### CI/CD Pipelines

You can integrate the web interface into CI/CD pipelines for code review and documentation:

```yaml
# Example GitHub Actions workflow
jobs:
  code-review:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Run Probe Web Interface
        run: |
          export ANTHROPIC_API_KEY=${{ secrets.ANTHROPIC_API_KEY }}
          export ALLOWED_FOLDERS=$GITHUB_WORKSPACE
          npx -y @buger/probe-web &
          # Use Playwright or similar to automate queries and save results
```

## Debugging

The web interface includes debugging options to help troubleshoot issues:

### Debug Mode

Enable debug mode to see detailed logging:

```bash
DEBUG=true npm start
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