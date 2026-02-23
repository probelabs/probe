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
# For OpenAI: MODEL_NAME=gpt-5.2
# For Google: MODEL_NAME=gemini-2.5-flash

# API URL configuration (optional)
# Generic base URL for all providers (if provider-specific URL not set)
# LLM_BASE_URL=https://your-custom-endpoint.com
# Provider-specific URLs (override LLM_BASE_URL)
# ANTHROPIC_API_URL=https://your-anthropic-endpoint.com
# OPENAI_API_URL=https://your-openai-endpoint.com
# GOOGLE_API_URL=https://your-google-endpoint.com

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
- `-m, --model <model>`: Specify the model to use (e.g., `claude-3-7-sonnet-latest`, `gpt-5.2`, `gemini-2.5-flash`)
- `-f, --force-provider <provider>`: Force a specific provider (options: `anthropic`, `openai`, `google`)
- `-w, --web`: Run in web interface mode
- `-p, --port <port>`: Port to run web server on (default: 8080)
- `--prompt <value>`: Use a custom prompt (values: `architect`, `code-review`, `support`, or path to a file)
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
   - Default model: `gpt-5.2`
   - Environment variable: `OPENAI_API_KEY`
   - Best for: General code search, pattern recognition, and concise explanations

3. **Google Gemini**
   - Default model: `gemini-2.5-flash`
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
   node index.js --model gpt-5.2
   node index.js --model gemini-2.5-flash
   ```

2. **Using the environment variable**:
   Add this to your `.env` file:
   ```
   MODEL_NAME=claude-3-7-sonnet-latest
   ```

Note that the model must be compatible with the selected provider. If you force a specific provider and specify a model, the model must be available for that provider.

### Custom API Endpoints

You can configure custom API endpoints for each provider:

1. **Generic endpoint for all providers**:
   ```
   LLM_BASE_URL=https://your-custom-endpoint.com
   ```
   This will be used for all providers unless a provider-specific URL is set.

2. **Provider-specific endpoints**:
   ```
   ANTHROPIC_API_URL=https://your-anthropic-endpoint.com
   OPENAI_API_URL=https://your-openai-endpoint.com
   GOOGLE_API_URL=https://your-google-endpoint.com
   ```
   These override the generic LLM_BASE_URL for their respective providers.
Provider-specific URLs always take precedence over the generic LLM_BASE_URL.

## Custom Prompts

Probe Chat allows you to customize the system prompt used by the AI assistant. This can be useful for tailoring the assistant's behavior to specific use cases or domains.

### Predefined Prompts

By default, Probe Chat uses a "Code Explorer" prompt that's optimized for answering questions about code, explaining how systems work, and providing insights into code functionality.

The `--prompt` option accepts several predefined prompt types to specialize the assistant for different tasks:

1. **code-explorer** (default): Focuses on explaining and navigating code. The assistant will provide clear explanations of how code works, find relevant snippets, and trace function calls and data flow.

   ```bash
   node index.js --prompt code-explorer
   ```
   
   Note: This is the default behavior, so you don't need to specify this prompt explicitly.

2. **architect**: Focuses on software architecture and design. The assistant will analyze code from an architectural perspective, identify patterns, suggest improvements, and create high-level design documentation.

   ```bash
   node index.js --prompt architect
   ```

2. **code-review**: Focuses on code quality and best practices. The assistant will identify issues, suggest improvements, and ensure code follows best practices.

   ```bash
   node index.js --prompt code-review
   ```

3. **support**: Focuses on troubleshooting and problem-solving. The assistant will help diagnose errors, understand unexpected behaviors, and find solutions.

   ```bash
   node index.js --prompt support
   ```

Each predefined prompt type maintains the core functionality of Probe Chat while specializing in a particular area of focus. The standard instructions for using tools and following the XML format are automatically included with all predefined prompts.

### Custom Prompt Files

You can also provide a path to a file containing your own custom prompt:

```bash
node index.js --prompt /path/to/your/prompt.txt
```

The file should contain the complete system prompt that you want to use. This completely replaces the default system prompt. If you're creating a custom prompt, make sure to include instructions for using the available tools and following the XML format.

Example custom prompt file:

```
You are ProbeChat Custom, a specialized AI assistant for [your specific use case].
You focus on [specific area of expertise] and excel at [key strengths].

Follow these instructions carefully:
1.  Analyze the user's request with a focus on [your specific focus area].
2.  Use <thinking></thinking> tags to analyze the situation and determine the appropriate tool for each step.
3.  Use the available tools step-by-step to fulfill the request.
4.  Ensure to get really deep and understand the full picture before answering.
5.  You MUST respond with exactly ONE tool call per message, using the specified XML format, until the task is complete.
6.  Wait for the tool execution result (provided in the next user message in a <tool_result> block) before proceeding to the next step.
7.  Once the task is fully completed, and you have confirmed the success of all steps, use the '<attempt_completion>' tool to provide the final result.
8.  Prefer concise and focused search queries. Use specific keywords and phrases to narrow down results.
9.  [Add any additional specialized instructions here]
```

### Environment Variables

You can also set a default prompt type using the environment variable:

```
PROMPT_TYPE=architect
```

Or specify a path to a custom prompt file:

```
CUSTOM_PROMPT=/path/to/your/prompt.txt
```

The command-line option takes precedence over the environment variable.
Provider-specific URLs always take precedence over the generic LLM_BASE_URL.

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