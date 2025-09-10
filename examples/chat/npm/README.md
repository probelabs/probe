# @probelabs/probe-chat

A CLI chat interface for the [probe](https://github.com/probelabs/probe) code search tool.

## Installation

### Global Installation (Recommended)

```bash
npm install -g @probelabs/probe-chat
```

### Local Installation

```bash
npm install @probelabs/probe-chat
```

## Features

- **Interactive Chat**: Talk to an AI assistant that can search your codebase
- **Code Search**: Uses the powerful Probe code search tool under the hood
- **Multiple LLM Support**: Works with both Anthropic Claude and OpenAI models
- **AI SDK Integration**: Built on the [Vercel AI SDK](https://sdk.vercel.ai/) for robust AI capabilities
- **CLI Interface**: Easy-to-use command-line interface
- **Session Management**: Maintains chat history and provides usage statistics
- **Customizable**: Configure search folders, models, and more

## Usage

### Command Line Interface

```bash
# Basic usage (searches in current directory)
probe-chat

# Search in a specific directory
probe-chat /path/to/your/project

# Enable debug mode
probe-chat --debug /path/to/your/project

# Specify a model to use
probe-chat --model claude-3-7-sonnet-latest /path/to/your/project
```

### Environment Variables

You can configure the chat using environment variables:

- `ANTHROPIC_API_KEY`: Your Anthropic API key
- `OPENAI_API_KEY`: Your OpenAI API key
- `MODEL_NAME`: The model to use (e.g., 'claude-3-7-sonnet-latest', 'gpt-4o-2024-05-13')
- `ALLOWED_FOLDERS`: Comma-separated list of folders to search in
- `DEBUG`: Set to 'true' to enable debug mode
- `ANTHROPIC_API_URL`: Custom Anthropic API URL (optional)
- `OPENAI_API_URL`: Custom OpenAI API URL (optional)

### Chat Commands

While in the chat, you can use the following commands:

- `exit` or `quit`: End the chat session
- `usage`: Display token usage statistics
- `clear`: Clear the chat history

## Programmatic Usage

You can also use the package programmatically in your Node.js applications:

```javascript
import { ProbeChat, tools } from '@probelabs/probe-chat';

// Create a new chat instance
const chat = new ProbeChat({
  debug: true,
  model: 'claude-3-7-sonnet-latest',
  anthropicApiKey: 'your-api-key',
  allowedFolders: ['/path/to/your/project']
});

// Get a response from the chat
const response = await chat.chat('How is the authentication implemented in this codebase?');
console.log(response);

// Get token usage statistics
const usage = chat.getTokenUsage();
console.log(`Request tokens: ${usage.request}`);
console.log(`Response tokens: ${usage.response}`);
console.log(`Total tokens: ${usage.total}`);

// Clear the chat history
chat.clearHistory();
```

## API Reference

### ProbeChat Class

```javascript
import { ProbeChat } from '@probelabs/probe-chat';

// Create a new chat instance
const chat = new ProbeChat(options);
```

#### Constructor Options

- `debug`: Enable debug mode (boolean)
- `model`: Model name to use (string)
- `anthropicApiKey`: Anthropic API key (string)
- `openaiApiKey`: OpenAI API key (string)
- `anthropicApiUrl`: Custom Anthropic API URL (string)
- `openaiApiUrl`: Custom OpenAI API URL (string)
- `allowedFolders`: Folders to search in (array of strings)

#### Methods

- `chat(message)`: Process a user message and get a response
- `getSessionId()`: Get the session ID
- `getTokenUsage()`: Get token usage statistics
- `clearHistory()`: Clear the chat history

### Tools

The package also exports the tools from `@probelabs/probe` for convenience:

```javascript
import { tools } from '@probelabs/probe-chat';

// Access the tools
const { searchTool, queryTool, extractTool } = tools;

// Access the default system message
const systemMessage = tools.DEFAULT_SYSTEM_MESSAGE;
```

## Supported Models

### Anthropic Models
- `claude-3-7-sonnet-latest` (default)
- `claude-3-7-opus-latest`
- `claude-3-5-sonnet-20241022`
- `claude-3-5-sonnet-20240620`
- `claude-3-opus-20240229`
- `claude-3-sonnet-20240229`
- `claude-3-haiku-20240307`

### OpenAI Models
- `gpt-4o-2024-05-13` (default)
- `gpt-4o`
- `gpt-4-turbo`
- `gpt-4`

## Requirements

- Node.js 18.0.0 or higher
- An API key from either Anthropic or OpenAI

## License

ISC

## Related Projects

- [probe](https://github.com/probelabs/probe) - The core probe code search tool
- [@probelabs/probe](https://www.npmjs.com/package/@probelabs/probe) - Node.js wrapper for the probe tool
- [Vercel AI SDK](https://sdk.vercel.ai/) - The AI SDK used for model integration 