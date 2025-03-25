# Using Probe in the CLI for Advanced AI

This guide explains how to use Probe's CLI-based AI chat capabilities for powerful code exploration and understanding directly from your terminal.

## Overview

Probe's AI Chat mode combines the power of large language models with Probe's code search capabilities, all in a convenient command-line interface. This integration allows you to have natural conversations about your codebase without leaving your terminal.

Key benefits:

- **Natural Language Understanding**: Ask questions about your code in plain English
- **Contextual Awareness**: The AI maintains conversation history for follow-up questions
- **Code-Aware Responses**: Get explanations that reference specific files and line numbers
- **Intelligent Search**: The AI automatically formulates optimal search queries based on your questions
- **Local Search Engine**: Probe searches your code locally without requiring embedding generation, but search results are sent to the AI service

## Getting Started

### Quick Setup

The fastest way to get started is using npx:

```bash
# Set your API key first
export ANTHROPIC_API_KEY=your_api_key
# Or for OpenAI
# export OPENAI_API_KEY=your_api_key

# Run directly with npx (no installation needed)
npx -y @buger/probe-chat@latest

# Or specify a directory to search
npx -y @buger/probe-chat@latest /path/to/your/project
```

### Global Installation

For regular use, you might prefer a global installation:

```bash
# Install globally
npm install -g @buger/probe-chat

# Start the chat interface
probe-chat
```

## Common Workflows

### 1. Exploring a New Codebase

When you're new to a project, the AI chat can help you quickly understand its structure and functionality:

```
❯ What's the overall architecture of this project?

❯ What are the main components and how do they interact?

❯ How is data flowing through the system?
```

### 2. Understanding Complex Code

When you encounter complex code that's difficult to understand:

```
❯ Can you explain how the ranking algorithm in src/ranking.rs works?

❯ What's the purpose of the cache implementation in the search module?

❯ Walk me through the authentication flow step by step.
```

### 3. Finding Implementation Details

When you need to locate specific functionality:

```
❯ Where is the error handling implemented for API requests?

❯ How does the system validate user input?

❯ Show me all the places where database connections are created.
```

### 4. Learning Best Practices

When you want to understand the patterns and practices used in the codebase:

```
❯ What design patterns are used in this project?

❯ How is testing implemented across the codebase?

❯ What's the error handling strategy used in this project?
```

### 5. Planning Changes

When you need to make changes to the codebase:

```
❯ If I wanted to add a new authentication method, what files would I need to modify?

❯ What would be the impact of changing the database schema for users?

❯ How could I improve the performance of the search functionality?
```

## Advanced Configuration

### Model Selection

You can choose which AI model to use:

```bash
# Override the default model
export MODEL_NAME=claude-3-opus-20240229
probe-chat
```

### API URLs

For enterprise deployments or when using proxies:

```bash
# Override API URLs
export ANTHROPIC_API_URL=https://your-anthropic-proxy.com
export OPENAI_API_URL=https://your-openai-proxy.com/v1
probe-chat
```

### Debug Mode

For troubleshooting or development:

```bash
# Enable debug mode for detailed logging
export DEBUG=1 probe-chat
```

### Session-Based Caching

Probe uses session-based caching to avoid showing the same code blocks multiple times in a conversation:

```bash
# First search generates a session ID
probe search "authentication" --session ""
# Session: a1b2 (example output)

# Subsequent searches reuse the session ID
probe search "login" --session "a1b2"
# Will skip code blocks already shown in the previous search
```

## Integration with Other CLI Tools

Probe's AI chat can be combined with other command-line tools for even more powerful workflows:

### Piping Compiler Errors

Feed compiler errors directly to the AI for explanation:

```bash
# Compile and pipe errors to AI chat
rustc main.rs 2>&1 | probe-chat
```

### Using Extract with LLM Prompts

Extract code with built-in prompt templates for LLM integration:

```bash
# Extract with engineer prompt template
probe extract src/auth.rs#authenticate --prompt engineer --instructions "Explain this authentication function"

# Extract with architect prompt template
probe extract src/api.js --prompt architect --instructions "Analyze this API module"

# Extract and pipe to AI chat
probe extract src/main.rs:42 --prompt engineer --instructions "Explain this function" | probe-chat
```

### Processing Search Results

Use standard Unix tools to process search results:

```bash
# Find then filter
probe search "database" | grep "connection" | probe-chat
```

### Scripting with AI Chat

You can create scripts that use the AI chat for automated tasks:

```bash
#!/bin/bash
# Example script to analyze code quality
echo "What are the potential security issues in this codebase?" | probe-chat > security_report.txt
```

## Best Practices

1. **Be Specific**: Ask specific questions about your codebase for more targeted answers

2. **Start with Overview Questions**: Begin with high-level questions to understand the structure before diving into details

3. **Follow Up**: Use follow-up questions to explore topics in more depth

4. **Reference Files**: Mention specific files or directories if you want to focus on a particular area

5. **Ask for Explanations**: The AI is particularly good at explaining complex code or concepts

6. **Request Examples**: Ask for examples if you're trying to understand how to use a particular feature or API

7. **Use Multiple Queries**: If you don't find what you're looking for, try reformulating your question

8. **Combine with Regular CLI**: Use the AI chat alongside regular Probe CLI commands for maximum effectiveness

## Example Conversation

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

## Limitations and Considerations

- **Token Limits**: Very large codebases may need multiple targeted questions rather than broad ones
- **API Costs**: Be mindful of token usage, especially with larger models
- **Privacy**: When using Probe with AI services, code snippets found by Probe are sent to external AI providers (Anthropic, OpenAI, etc.) as part of your queries
- **Accuracy**: The AI may occasionally make mistakes in its understanding or explanations

## Next Steps

- Learn about [Probe's CLI Reference](/cli-mode) for more advanced command-line usage
- Explore [using Probe with AI Code Editors](/use-cases/ai-code-editors) for integrated IDE experiences
- Check out [building custom tools](/use-cases/nodejs-sdk) with the Node.js SDK