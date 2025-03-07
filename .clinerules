# Probe Project Guidelines

This file provides guidelines for AI assistants and developers working on the probe project.

## Project Overview

Probe is a tool for searching code repositories with powerful filtering and ranking capabilities. It provides:
- Regex-based code search with stemming and stopword removal
- Language-aware code block extraction
- Frequency-based search for better relevance
- Flexible term matching with boundary control
- Result ranking using TF-IDF and BM25 algorithms

## Project Structure

```
probe/
├── src/                    # Main source code
│   ├── language/           # Language-specific parsing
│   ├── search/             # Core search functionality
│   │   ├── file_processing.rs    # File content processing
│   │   ├── file_search.rs        # File searching logic
│   │   ├── query.rs              # Query processing
│   │   ├── result_ranking.rs     # Result ranking algorithms
│   │   └── search_execution.rs   # Main search execution
│   ├── agent.rs            # AI agent implementation
│   ├── lib.rs              # Library entry point
│   └── main.rs             # CLI application entry point
├── tests/                  # Integration tests
│   ├── mocks/              # Mock files for testing
│   └── ...                 # Various test files
└── mcp/                    # MCP server for probe
```

## Code Style Guide

### Rust Conventions

1. **Naming**:
   - Use `snake_case` for variables, functions, and modules
   - Use `CamelCase` for types and traits
   - Use `SCREAMING_SNAKE_CASE` for constants

2. **Documentation**:
   - All public functions, structs, and modules should have doc comments
   - Use `///` for doc comments
   - Include examples where appropriate

3. **Error Handling**:
   - Use `Result<T, E>` for functions that can fail
   - Prefer `?` operator for error propagation
   - Use `anyhow::Result` for functions that can return multiple error types

4. **Formatting**:
   - Follow standard Rust formatting (rustfmt)
   - Use 4 spaces for indentation
   - Maximum line length of 100 characters

### Project-Specific Conventions

1. **Pattern Generation**:
   - When generating regex patterns, use `HashSet` to avoid duplicates
   - For term patterns, generate three variants: start boundary, end boundary, and no boundary
   - For multi-term queries, generate concatenated combinations

2. **File Processing**:
   - Use AST parsing when possible for more accurate code block extraction
   - Fall back to line-based context when AST parsing fails
   - Apply the 80% rule: if more than 80% of a file is matched, return the entire file

## Testing Approach

### Test Organization

1. **Unit Tests**:
   - Place unit tests in the same file as the code they test
   - Use `#[cfg(test)]` module at the end of each file
   - For complex modules, use a separate `*_tests.rs` file in the same directory
   - Include the tests using `include!("module_tests.rs")` in a `#[cfg(test)]` module

2. **Integration Tests**:
   - Place integration tests in the `tests/` directory
   - Use descriptive names for test files
   - Group related tests in the same file

### Running Tests

1. **Unit Tests**:
   ```bash
   cargo test --lib
   ```

2. **Integration Tests**:
   ```bash
   cargo test --test integration_tests
   ```

3. **All Tests**:
   ```bash
   cargo test
   ```

4. **Specific Tests**:
   ```bash
   cargo test test_name
   ```

### Test Coverage

- Aim for high test coverage, especially for core functionality
- Include tests for edge cases and error conditions
- Use property-based testing for functions with complex input domains

## Common Commands

### Building

```bash
cargo build
```

### Running

```bash
cargo run -- search "query" path/to/search
```

### Debug Mode

Enable debug mode to see detailed logging:

```bash
DEBUG=1 cargo run -- search "query" path/to/search
```

### MCP Server

Start the MCP server:

```bash
cd mcp && npm run build && node build/index.js
```

## Dependency Management

1. **Adding Dependencies**:
   - Add new dependencies to `Cargo.toml`
   - Prefer well-maintained crates with good documentation
   - Consider the impact on build time and binary size

2. **Versioning**:
   - Use semantic versioning for dependencies
   - Specify version constraints to avoid breaking changes

## File Organization

1. **Module Structure**:
   - Use `mod.rs` files for module organization
   - Group related functionality in the same module
   - Export public items from the module root

2. **Code Organization**:
   - Place related functions and types together
   - Use private helper functions for complex logic
   - Keep functions focused on a single responsibility

## Making Changes

When making changes to the codebase:

1. **Pattern Generation**:
   - When modifying `create_term_patterns`, ensure it handles:
     - Individual terms with flexible boundaries
     - Concatenated term combinations for multi-term queries
     - Proper regex escaping for special characters

2. **Search Execution**:
   - When modifying `perform_probe`, ensure it:
     - Properly maps patterns to terms
     - Handles both "any term" and "all terms" modes
     - Correctly processes filename matches

3. **Result Ranking**:
   - When modifying ranking algorithms, ensure they:
     - Properly calculate TF-IDF and BM25 scores
     - Handle edge cases (empty documents, rare terms)
     - Maintain backward compatibility with existing code
## Debugging Tips

1. **Debug Mode**:
   - Set `DEBUG=1` to enable debug logging
   - Debug logs include detailed information about:
     - Pattern generation
     - File matching
     - Term frequencies
     - Result ranking

2. **Common Issues**:
   - If search returns no results, check:
     - Query preprocessing (stemming, stopwords)
     - Pattern generation
     - File matching logic
   - If ranking seems incorrect, check:
     - Term frequency calculation
     - Document length normalization
     - Score combination logic

## AI Agent

The `agent.rs` file implements an AI-powered assistant that can interact with the code search functionality.

### Agent Overview

- **Purpose**: Provides an interactive AI assistant that can search code repositories and answer questions about the codebase
- **Models**: Supports both Anthropic's Claude and OpenAI's GPT models
- **Features**:
  - Interactive CLI interface
  - Token usage tracking
  - Conversation history management
  - Tool integration with code search functionality
  - Colored output formatting

### Components

1. **ProbeSearch Tool**:
   - Implements the `Tool` trait from the `rig` crate
   - Allows the AI to search code using the core probe functionality
   - Configurable with various search parameters (pattern, path, exact matching, etc.)
   - Returns formatted search results to the AI

2. **ModelType Enum**:
   - Represents different LLM backends:
     - `Anthropic`: Uses Claude models via Anthropic's API
     - `OpenAI`: Uses GPT models via OpenAI's API

3. **ProbeAgent Struct**:
   - Main agent implementation
   - Handles model initialization based on available API keys
   - Tracks token usage for requests and responses
   - Implements conversation handling

4. **Chat Implementation**:
   - Implements the `RigChat` trait for handling conversations
   - Manages conversation history with a limit to prevent context explosion
   - Processes tool calls within AI responses
   - Handles continuation prompts after tool execution

### Usage

1. **Starting the Agent**:
   ```bash
   cargo run -- agent
   ```

2. **Environment Variables**:
   - `ANTHROPIC_API_KEY`: API key for Anthropic (Claude models)
   - `OPENAI_API_KEY`: API key for OpenAI (GPT models)
   - `MODEL_NAME`: Override the default model (optional)
   - `ANTHROPIC_API_URL`: Override the default Anthropic API URL (optional)
   - `OPENAI_API_URL`: Override the default OpenAI API URL (optional)
   - `DEBUG`: Enable debug output (set to any value)

3. **CLI Commands**:
   - `help`: Display help information
   - `quit`: Exit the assistant

### Implementation Details

1. **Token Tracking**:
   - Uses `tiktoken_rs::cl100k_base` for token counting
   - Tracks request and response tokens separately
   - Displays token usage information after each interaction

2. **Conversation Management**:
   - Limits history to `MAX_HISTORY_MESSAGES` (4) to prevent context explosion
   - Filters out tool outputs from history to save tokens
   - Uses continuation prompts to maintain context after tool execution

3. **Tool Execution**:
   - Detects "tool:" calls in AI responses
   - Executes the ProbeSearch tool with provided parameters
   - Returns results to the AI for further processing
   - Handles errors and provides appropriate error messages

4. **Model-Specific Handling**:
   - Different prompt formats for Anthropic vs OpenAI
   - Adjusts token limits based on model capabilities
   - Handles API-specific requirements and response formats
     - Score combination logic
