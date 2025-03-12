# AI Chat Mode

Probe's AI Chat mode provides an interactive CLI interface where you can ask questions about your codebase and get AI-powered responses. This mode combines Probe's powerful code search capabilities with large language models to help you understand and navigate your codebase more effectively.

## Getting Started

To use the AI Chat mode, you'll need an API key for either Anthropic's Claude or OpenAI's GPT models.

### Setting Up API Keys

For Claude models (recommended):

```bash
export ANTHROPIC_API_KEY=your_api_key
```

For OpenAI models:

```bash
export OPENAI_API_KEY=your_api_key
```

### Starting the Chat

Once you've set up your API key, you can start the chat:

```bash
probe chat
```

This will launch an interactive CLI interface where you can ask questions about your codebase.

## Features

### AI-Powered Search

The AI Chat mode uses large language models to understand your questions and search your codebase intelligently. It can:

- Find relevant code based on natural language descriptions
- Explain how different parts of your codebase work together
- Identify patterns and architectural decisions
- Help you understand complex code

### Multi-Model Support

Probe's AI Chat mode supports both Anthropic's Claude and OpenAI's GPT models:

- **Claude Models**: Provide excellent code understanding and explanation capabilities
- **GPT Models**: Offer strong general-purpose capabilities

The default model is selected based on which API key you provide.

### Token Tracking

The AI Chat mode monitors token usage for both requests and responses, helping you keep track of your API usage:

```
Token Usage: Request: 1245 Response: 1532 (Current message only: ~1532)
Total: 2777 tokens (Cumulative for entire session)
```

### Conversation History

The chat maintains context across multiple interactions, allowing for follow-up questions and deeper exploration of topics.

### Colored Output

The terminal interface provides user-friendly colored output with syntax highlighting for code blocks.

## Configuration

You can configure the AI Chat mode using environment variables:

### Model Selection

```bash
# Override the default model
export MODEL_NAME=claude-3-opus-20240229
probe chat
```

### API URLs

```bash
# Override API URLs (useful for proxies or enterprise deployments)
export ANTHROPIC_API_URL=https://your-anthropic-proxy.com
export OPENAI_API_URL=https://your-openai-proxy.com/v1
probe chat
```

### Debug Mode

```bash
# Enable debug mode for detailed logging
export DEBUG=1 probe chat
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

## Tips for Effective Use

1. **Be Specific**: Ask specific questions about your codebase for more targeted answers
2. **Start with Overview Questions**: Begin with high-level questions to understand the structure before diving into details
3. **Follow Up**: Use follow-up questions to explore topics in more depth
4. **Reference Files**: Mention specific files or directories if you want to focus on a particular area
5. **Ask for Explanations**: The AI is particularly good at explaining complex code or concepts
6. **Request Examples**: Ask for examples if you're trying to understand how to use a particular feature or API

## CLI Commands

While in the chat interface, you can use these commands:

- `help`: Display help information
- `quit`: Exit the assistant

## Limitations

- The AI's knowledge is based on the code it can find in your repository
- Very large codebases may need multiple targeted questions rather than broad ones
- The AI may occasionally make mistakes in its understanding or explanations
- Token limits may restrict the amount of code that can be analyzed at once