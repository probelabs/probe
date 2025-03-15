# Probe Web Interface

A web interface for the Probe code search tool.

## Installation

```bash
npm install -g @buger/probe-web
```

## Usage

After installing the package globally, you can start the web interface by running:

```bash
probe-web
```

This will start a local web server that provides a user-friendly interface for using Probe to search your codebase.

## Features

- Interactive web interface for Probe code search
- Support for both Anthropic and OpenAI models
- Syntax highlighting for search results
- Easy-to-use query interface
- AI-powered code exploration

## Configuration

The web interface can be configured using environment variables:

- `PORT`: The port to run the web server on (default: 3000)
- `ANTHROPIC_API_KEY`: Your Anthropic API key
- `OPENAI_API_KEY`: Your OpenAI API key
- `MODEL_NAME`: The model to use (default depends on available API keys)

You can set these variables in a `.env` file in your current directory.

## Requirements

- Node.js 18 or later
- Probe CLI tool installed and available in your PATH

## License

Apache-2.0