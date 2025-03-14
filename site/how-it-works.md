# How Probe Works

Probe combines efficient text search with code intelligence to find relevant code in your codebase.

## System Architecture

Probe operates in six stages:

1. **Scan**: File identification using ripgrep
2. **Parse**: Code structure analysis via Abstract Syntax Trees
3. **Process**: Query enhancement with NLP techniques
4. **Rank**: Result prioritization
5. **Extract**: Code block isolation
6. **Format**: Output generation

## Search Workflow

Here's how a search for "error handling" works:

1. **Query Processing**:
   - Tokenize → [error, handling]
   - Stem → [error, handl]
   - Generate patterns → `\berror\b`, `\bhandl\w*\b`
   - Parse query syntax → AND(error, handling)

2. **File Scanning**:
   - Identify potential matches using ripgrep
   - Filter based on .gitignore and custom patterns
   - Exclude test files (unless specified otherwise)

3. **Code Analysis**:
   - Parse matching files into ASTs
   - Identify complete code blocks containing matches
   - Extract metadata (function names, parameters, etc.)

4. **Result Ranking**:
   - Calculate relevance scores
   - Apply position weights for terms in identifiers
   - Sort by combined relevance metrics
   - Filter based on session cache (if enabled)

5. **Block Extraction**:
   - Extract complete functions/methods with matches
   - Merge related code blocks when appropriate
   - Apply context lines if requested

6. **Result Delivery**:
   - Format with syntax highlighting
   - Apply token/size constraints
   - Generate structured output

## Rapid Scanning

The foundation of Probe's speed:

- **Ripgrep Engine**: Fast line scanning at core
- **Parallel Processing**: Utilizes all CPU cores
- **Smart Filtering**: Respects .gitignore patterns
- **Stream Processing**: Minimal memory footprint
- **Incremental Matching**: Stops scanning when limits are reached

## Code Structure Parsing

Where Probe becomes more than just text search:

- **Tree-sitter**: Industry-standard parsing technology
- **AST Generation**: Builds complete code structure map
- **Language-specific**: Understands each language's unique patterns
- **Robust Handling**: Works with partial or imperfect code
- **Symbol Resolution**: Maps identifiers to declarations

## Session-based Caching

Avoiding duplicate results across multiple searches:

- **Unique Identifiers**: Cache keys based on file path and line numbers
- **Result Tracking**: Remembers which blocks have been shown
- **Session Management**: Generates and maintains session IDs
- **Cache Invalidation**: Clears cache when appropriate

## Output Strategies

Delivering results in the most useful format:

- **Markdown/Syntax**: Rich, readable code presentation
- **JSON**: Structured for programmatic use
- **Token Limiting**: Fits within AI context windows
- **Priority Handling**: Most relevant results survive limits
- **Streaming**: Real-time output for interactive use

## Integration Architecture

How Probe connects with other tools:

### MCP Server

- **STDIO Transport**: Communicates via standard input/output
- **JSON Protocol**: Structured message format
- **Tool Definitions**: Exposes search, query, and extract capabilities

### Node.js SDK

- **JS Bindings**: JavaScript interface to core functionality
- **Vercel AI SDK**: Integration for streaming AI responses
- **Promise-based**: Async/await compatible API
- **Type Definitions**: TypeScript support

### Web Interface

- **Express Backend**: Node.js server for API endpoints
- **Vanilla JS Frontend**: No framework dependencies
- **Streaming Responses**: Real-time AI output
- **Markdown Rendering**: Rich text and code formatting

For detailed information on specific features, see:
- [Search Functionality](search-functionality.md)
- [Code Extraction](code-extraction.md)
- [Feature Overview](features.md)
