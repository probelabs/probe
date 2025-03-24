# Probe MCP Agent

An MCP server for Probe that uses an agentic approach to answer questions about codebases.

## Overview

This MCP server exposes a single tool called `search_code` that returns AI-generated responses to questions about a codebase. Behind the scenes, it uses the Vercel AI SDK to run AI calls with access to Probe's code search tools.

## Features

- Uses AI to answer questions about codebases
- Hides the complexity of tool calling from the user
- Provides relevant code blocks and explanations
- Supports Anthropic, OpenAI, and Google models
- Configurable via environment variables
- Pure JavaScript implementation for simplicity

## Installation

### From npm

```bash
# Install globally
npm install -g @buger/probe-mcp-agent

# Or install locally
npm install @buger/probe-mcp-agent
```

### From Source

```bash
# Clone the repository
git clone https://github.com/buger/probe.git

# Navigate to the directory
cd probe/mcp-agent

# Install dependencies
npm install

# Build the package
npm run build
```

## Configuration

The server can be configured using environment variables:

```bash
# API Keys (required - at least one)
ANTHROPIC_API_KEY=your_anthropic_api_key
OPENAI_API_KEY=your_openai_api_key
GOOGLE_API_KEY=your_google_api_key

# API URLs (optional)
ANTHROPIC_API_URL=https://api.anthropic.com/v1
OPENAI_API_URL=https://api.openai.com/v1
GOOGLE_API_URL=https://generativelanguage.googleapis.com

# Force specific provider (optional)
FORCE_PROVIDER=anthropic|openai|google

# Model Configuration (optional)
MODEL_NAME=claude-3-7-sonnet-latest

# Token Limits (optional)
MAX_TOKENS=4000
MAX_HISTORY_MESSAGES=20

# Allowed Folders (optional, but recommended for security)
ALLOWED_FOLDERS=/path/to/repo1,/path/to/repo2

# Setting ALLOWED_FOLDERS restricts code search to only these directories
# and prevents access to other parts of the filesystem

# Debug Mode (optional)
DEBUG=true
```

You can create a `.env` file in the root directory with these variables.

## Usage

### Starting the Server

```bash
# If installed globally
probe-mcp-agent

# If installed locally
npx probe-mcp-agent

# Or start with npm
npm start

# Force a specific provider
probe-mcp-agent --provider anthropic
probe-mcp-agent --provider openai
probe-mcp-agent --provider google
```

### Using with MCP Clients

The server exposes a single tool called `search_code` with the following parameters:

- `query` (required): The question or request about the codebase
- `path` (optional): Path to the directory to search in. If ALLOWED_FOLDERS is set, this path must be within one of the allowed folders for security reasons
- `context` (optional): Additional context to help the AI understand the request
- `max_tokens` (optional): Maximum number of tokens to return

Example usage with an MCP client:

```javascript
const result = await useMcpTool({
  serverName: 'probe-mcp-agent',
  toolName: 'search_code',
  arguments: {
    query: "How does the search functionality work in this codebase?",
    path: "/path/to/codebase"
  }
});

console.log(result);
```

## Model Selection

The agent will use models in the following priority:

1. If `--provider` flag or `FORCE_PROVIDER` environment variable is set, it will use the specified provider
2. Otherwise, it will use the first available API key in this order: Anthropic, OpenAI, Google

You can also specify a custom model name using the `MODEL_NAME` environment variable, which will override the default model for the selected provider.

Default models:
- Anthropic: `claude-3-7-sonnet-latest`
- OpenAI: `gpt-4o-2024-05-13`
- Google: `gemini-1.5-pro-latest`

## Security Considerations

### Folder Protection

The MCP agent implements folder protection to prevent unauthorized access to files outside of allowed directories:

1. When the `ALLOWED_FOLDERS` environment variable is set, the agent will only allow searches within those directories
2. Any attempt to search outside of allowed folders will result in an error
3. The path parameter in search requests is validated to ensure it's within an allowed folder
4. This protection is communicated to the AI model in the system message

It's strongly recommended to set `ALLOWED_FOLDERS` in production environments to limit the scope of code search to specific repositories or directories.

Example:
```bash
# Restrict searches to only these two repositories
ALLOWED_FOLDERS=/home/user/projects/repo1,/home/user/projects/repo2
```

Without this setting, the agent will default to using the current working directory, which may expose more files than intended.

## Development

```bash
# Run in development mode
npm run dev
```

## Project Structure

```
mcp-agent/
├── src/                    # Source code
│   ├── agent.js            # AI agent implementation
│   ├── config.js           # Configuration handling
│   └── index.js            # MCP server entry point
├── build/                  # Built JavaScript files
├── .env.example            # Example environment variables
└── package.json            # Project metadata and dependencies
```

## License

MIT