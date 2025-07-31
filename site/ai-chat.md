# AI Chat Mode

Probe's AI Chat mode provides an interactive interface where you can ask questions about your codebase and get AI-powered responses. This mode combines Probe's powerful code search capabilities with large language models to help you understand and navigate your codebase more effectively.

> **Note**: For comprehensive documentation on all AI integration features, including the AI chat mode, MCP server integration, and Node.js SDK, see the [AI Integration](./ai-integration.md) page.

## Unified Interface

Probe now features a unified interface that combines both CLI and web functionality in a single package. The `@buger/probe-chat` package supports both CLI mode (default) and web mode (with the `--web` flag).

For detailed information about the web interface mode, see the [Web Interface](./web-interface.md) documentation.

## Getting Started

The AI chat functionality is available as a standalone npm package that can be run directly with npx.

### Using npx (Recommended)

```bash
# Run directly with npx in CLI mode (no installation needed)
npx -y @buger/probe-chat@latest

# Run in web interface mode
npx -y @buger/probe-chat@latest --web

# Set your API key first
export ANTHROPIC_API_KEY=your_api_key
# Or for OpenAI
# export OPENAI_API_KEY=your_api_key
# Or for Google
# export GOOGLE_API_KEY=your_api_key

# Or specify a directory to search
npx -y @buger/probe-chat@latest /path/to/your/project
```

### Using the npm package

```bash
# Install globally
npm install -g @buger/probe-chat@latest

# Start the chat interface in CLI mode
probe-chat

# Start the chat interface in web mode
probe-chat --web
```

### Using the example code

```bash
# Navigate to the examples directory
cd examples/chat

# Install dependencies
npm install

# Set your API key
export ANTHROPIC_API_KEY=your_api_key
# Or for OpenAI
# export OPENAI_API_KEY=your_api_key
# Or for Google
# export GOOGLE_API_KEY=your_api_key

# Start the chat interface in CLI mode
node index.js

# Start the chat interface in web mode
node index.js --web
```

## Features

### AI-Powered Search

The AI Chat mode uses large language models to understand your questions and search your codebase intelligently. It can:

- Find relevant code based on natural language descriptions
- Explain how different parts of your codebase work together
- Identify patterns and architectural decisions
- Help you understand complex code

### Multi-Model Support

Probe's AI Chat mode supports multiple AI providers:

- **Anthropic Claude**: Provides excellent code understanding and explanation capabilities
- **OpenAI GPT**: Offers strong general-purpose capabilities
- **Google Gemini**: Delivers fast responses and efficient code search

The default model is selected based on which API key you provide, or you can force a specific provider using the `--force-provider` option.

#### Force Provider Option

You can force the chat to use a specific provider regardless of which API keys are available:

```bash
# Force using Anthropic Claude
probe-chat --force-provider anthropic

# Force using OpenAI
probe-chat --force-provider openai

# Force using Google Gemini
probe-chat --force-provider google
```

This is useful when you have multiple API keys configured but want to use a specific provider for certain tasks.

### Token Tracking

The AI Chat mode monitors token usage for both requests and responses, helping you keep track of your API usage:

```
Token Usage: Request: 1245 Response: 1532 (Current message only: ~1532)
Total: 2777 tokens (Cumulative for entire session)
```

### OpenTelemetry Tracing

Probe Chat includes built-in OpenTelemetry tracing support to monitor AI model interactions, performance metrics, and usage patterns:

```bash
# Enable file-based tracing
probe-chat --trace-file ./traces.jsonl

# Enable remote tracing to an OpenTelemetry collector
probe-chat --trace-remote http://localhost:4318/v1/traces

# Enable console tracing for debugging
probe-chat --trace-console

# Combine multiple tracing options
probe-chat --trace-file --trace-remote --trace-console
```

#### What Gets Traced

The tracing system captures comprehensive data about AI interactions:

- **AI Model Calls**: Complete request/response cycles with timing
- **Token Usage**: Detailed token consumption (prompt, completion, total)
- **Performance Metrics**: Response times, request durations, and throughput
- **Session Information**: Session IDs, iteration counts, and conversation flow
- **Model Configuration**: Provider, model name, temperature, and other parameters
- **Error Tracking**: Failed requests, timeouts, and error details

#### File Tracing

File tracing saves traces to a JSON Lines format file:

```bash
# Basic file tracing
probe-chat --trace-file

# Custom file path
probe-chat --trace-file ./debug-traces.jsonl
```

Trace files contain one JSON object per line, making them easy to analyze:

```json
{
  "traceId": "abc123...",
  "name": "ai.generateText",
  "attributes": {
    "ai.model.id": "claude-3-7-sonnet-20250219",
    "ai.model.provider": "anthropic",
    "ai.usage.prompt_tokens": "15",
    "ai.usage.completion_tokens": "8",
    "ai.usage.total_tokens": "23"
  },
  "events": [
    {
      "name": "ai.request.start",
      "attributes": {
        "ai.request.messages": "[{\"role\":\"user\",\"content\":\"Hello\"}]"
      }
    }
  ]
}
```

#### Remote Tracing

Send traces to any OpenTelemetry-compatible collector:

```bash
# Jaeger
probe-chat --trace-remote http://localhost:4318/v1/traces

# Custom collector
probe-chat --trace-remote https://your-collector.com/v1/traces
```

For Jaeger setup:

```bash
# Start Jaeger with Docker
docker run -d --name jaeger \
  -p 16686:16686 \
  -p 4318:4318 \
  jaegertracing/all-in-one:latest

# Use probe-chat with remote tracing
probe-chat --trace-remote http://localhost:4318/v1/traces

# View traces at http://localhost:16686
```

#### Environment Variables

Configure tracing through environment variables:

```bash
# Enable file tracing
export OTEL_ENABLE_FILE=true
export OTEL_FILE_PATH=./traces.jsonl

# Enable remote tracing
export OTEL_ENABLE_REMOTE=true
export OTEL_EXPORTER_OTLP_TRACES_ENDPOINT=http://localhost:4318/v1/traces

# Enable console tracing
export OTEL_ENABLE_CONSOLE=true

# Service information
export OTEL_SERVICE_NAME=probe-chat
export OTEL_SERVICE_VERSION=1.0.0
```

#### Analyzing Traces

Use standard tools to analyze trace data:

```bash
# View all traces
cat traces.jsonl | jq '.'

# Count successful vs failed requests
cat traces.jsonl | jq -r '.status.code' | sort | uniq -c

# Extract token usage statistics
cat traces.jsonl | jq -r '.events[]? | select(.name == "ai.response.complete") | .attributes."ai.usage.total_tokens"'

# Calculate average response time
cat traces.jsonl | jq -r '((.endTimeUnixNano - .startTimeUnixNano) / 1000000)' | \
  awk '{sum+=$1} END {print sum/NR " ms"}'
```

### Conversation History

The chat maintains context across multiple interactions, allowing for follow-up questions and deeper exploration of topics.

### Colored Output

The terminal interface provides user-friendly colored output with syntax highlighting for code blocks.

### Code Editing (Experimental)

Probe can help you modify your code in two ways:

#### Local Code Editing (`--allow-edit`)

When you use the `--allow-edit` flag, Probe can make changes directly to your code files.

*   **What it does**: Ask Probe to "fix this bug" or "add error handling" and it will modify your files.
*   **What you need**: Install a code editing backend tool:
     - **Claude Code** (default if available): `npm install -g @anthropic-ai/claude-code`
     - **Aider** (fallback): `pip install aider-chat`
*   **Backend selection**: Probe automatically detects which tool is available. You can override this with the `implement_tool_backend` environment variable:
     ```bash
     # Force Claude Code
     export implement_tool_backend=claude
     probe-chat --allow-edit
     
     # Force Aider
     export implement_tool_backend=aider
     probe-chat --allow-edit
     ```

#### GitHub Integration

If you're using Probe with GitHub Actions, you can set up code suggestions instead of direct changes.

*   **How it works**: Probe creates suggested changes that appear in your pull requests, just like human code reviews.
*   **Setup**: See the [GitHub Actions Integration](./integrations/github-actions.md#code-modification-options) guide for configuration details.

#### Stay Safe

*   **Always check changes** before keeping them
*   **Test your code** after Probe makes modifications
*   **Start with small requests** to see how it works
*   **GitHub suggestions are safer** because you review changes before applying them

For complete usage instructions, see the [CLI Reference](./cli-mode.md#code-editing---allow-edit).

## Configuration

You can configure the AI Chat mode using environment variables or command-line options:

### Command-Line Options

```bash
# Enable debug mode
node index.js --debug

# Specify a model
node index.js --model claude-3-7-sonnet-latest

# Force a specific provider
node index.js --force-provider anthropic

# Run in web mode
node index.js --web

# Specify a port for web mode
node index.js --web --port 3000

# Specify a directory to search
node index.js /path/to/your/project

# Enable tracing
node index.js --trace-file ./traces.jsonl
node index.js --trace-remote http://localhost:4318/v1/traces
node index.js --trace-console
```

### Environment Variables

```bash
# Override the default model
export MODEL_NAME=claude-3-opus-20240229
probe-chat

# Force a specific provider
export FORCE_PROVIDER=anthropic  # Options: anthropic, openai, google
probe-chat

# Override API URLs (useful for proxies or enterprise deployments)
export ANTHROPIC_API_URL=https://your-anthropic-proxy.com
export OPENAI_API_URL=https://your-openai-proxy.com/v1
export GOOGLE_API_URL=https://your-google-proxy.com
probe-chat

# Enable debug mode for detailed logging
export DEBUG=1 probe-chat
```

## Example Usage

Here's an example of how you might use the AI Chat mode:

```
❯ How does the ranking algorithm work?

─ Response ─────────────────────────────────────────────────────────
I'll explain how the ranking algorithm works in the codebase.

The ranking system in Probe uses multiple algorithms to sort search results by relevance. The main ranking algorithms are:

1. TF-IDF (Term Frequency-Inverse Document Frequency)
2. BM25 (Best Matching 25)
3. Hybrid (a combination approach)

Let me break down each approach:

## TF-IDF Ranking

TF-IDF weighs terms based on how frequently they appear in a document versus how common they are across all documents.

Key implementation details:
- Term frequency (TF) measures how often a term appears in a document
- Inverse document frequency (IDF) measures how rare a term is across all documents
- Final score is calculated as TF × IDF

## BM25 Ranking

BM25 is an advanced ranking function that improves upon TF-IDF by adding document length normalization.

Key implementation details:
- Uses parameters k1 (term frequency saturation) and b (document length normalization)
- Handles edge cases like empty documents and rare terms
- Provides better results for longer documents

## Hybrid Ranking

The hybrid approach combines multiple ranking signals for better results:

1. Combines scores from both TF-IDF and BM25
2. Considers document length and term positions
3. Applies normalization to ensure fair comparison

The default reranker is "hybrid" which provides the best overall results for code search.

The ranking implementation can be found in `src/search/result_ranking.rs`.
─────────────────────────────────────────────────────────────────────
Token Usage: Request: 1245 Response: 1532 (Current message only: ~1532)
Total: 2777 tokens (Cumulative for entire session)
─────────────────────────────────────────────────────────────────────
```

## CLI Commands

While in the chat interface, you can use these commands:

- `help`: Display help information
- `quit` or `exit`: Exit the assistant
- `usage`: Display token usage statistics
- `clear`: Clear the chat history and start a new session

## Tips for Effective Use

1. **Be Specific**: Ask specific questions about your codebase for more targeted answers
2. **Start with Overview Questions**: Begin with high-level questions to understand the structure before diving into details
3. **Follow Up**: Use follow-up questions to explore topics in more depth
4. **Reference Files**: Mention specific files or directories if you want to focus on a particular area
5. **Ask for Explanations**: The AI is particularly good at explaining complex code or concepts
6. **Request Examples**: Ask for examples if you're trying to understand how to use a particular feature or API

## Limitations

- The AI's knowledge is based on the code it can find in your repository
- Very large codebases may need multiple targeted questions rather than broad ones
- The AI may occasionally make mistakes in its understanding or explanations
- Token limits may restrict the amount of code that can be analyzed at once
