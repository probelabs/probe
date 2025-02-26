# Code Search

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)

A powerful semantic code search tool that combines ripgrep's speed with tree-sitter's code understanding to find and extract complete code blocks based on search patterns.

## 🚀 Features

- **Semantic Code Search**: Finds and extracts entire functions, classes, structs, and other code structures rather than just matching lines
- **Intelligent Ranking**: Ranks results using advanced NLP techniques (TF-IDF, BM25, or hybrid mode)
- **Multi-Language Support**: Works with Rust, JavaScript, TypeScript, Python, Go, C/C++, Java, Ruby, and PHP
- **Smart Extraction**: Ensures complete, usable code blocks with proper context
- **Dual Mode**: Works as a CLI tool or as an MCP server
- **Frequency-based Search**: Advanced mode with stemming and stopword removal for more accurate results
- **AST Parsing**: Leverages tree-sitter to understand code structure across languages

## 📋 Installation

### Prerequisites

1. Install Rust and Cargo (if not already installed):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

### From Source

1. Clone the repository:
   ```bash
   git clone https://github.com/yourusername/code-search.git
   cd code-search
   ```

2. Build the project:
   ```bash
   cargo build --release
   ```

3. (Optional) Install globally:
   ```bash
   cargo install --path .
   ```

## 🔍 Basic Usage

### CLI Mode

```bash
# Basic search
code-search --path <DIRECTORY_PATH> --query <SEARCH_PATTERN>

# Search for "setTools" in the current directory
code-search --path . --query setTools

# Search for "impl" in the src directory
code-search --path ./src --query impl
```

### Output Example

```
File: src/models.rs
Lines: 10-25
```rust
struct SearchResult {
    pub file: String,
    pub lines: (usize, usize),
    pub node_type: String,
    pub code: String,
    pub matched_by_filename: Option<bool>,
    pub rank: Option<usize>,
    pub score: Option<f64>,
    pub tfidf_score: Option<f64>,
    pub bm25_score: Option<f64>,
    pub tfidf_rank: Option<usize>,
    pub bm25_rank: Option<usize>,
}
```

## 🔧 Advanced Usage

### Search Modes

```bash
# Find files only (no code blocks)
code-search --path . --query search --files-only

# Include files whose names match the query
code-search --path . --query search --include-filenames

# Use frequency-based search with stemming (better for large codebases)
code-search --path . --query search --frequency
```

### Ranking Options

```bash
# Use TF-IDF ranking
code-search --path . --query search --reranker tfidf

# Use BM25 ranking
code-search --path . --query search --reranker bm25

# Use hybrid ranking (default)
code-search --path . --query search --reranker hybrid
```

### Search Limits

```bash
# Limit to 10 results
code-search --path . --query search --max-results 10

# Limit to 10KB of content
code-search --path . --query search --max-bytes 10240

# Limit to 500 tokens (for AI usage)
code-search --path . --query search --max-tokens 500
```

### Custom Ignore Patterns

```bash
# Ignore specific file types
code-search --path . --query search --ignore "*.py" --ignore "*.js"
```

## 🌐 Supported Languages

Currently, the tool supports:
- Rust (.rs)
- JavaScript (.js, .jsx)
- TypeScript (.ts, .tsx)
- Python (.py)
- Go (.go)
- C (.c, .h)
- C++ (.cpp, .cc, .cxx, .hpp, .hxx)
- Java (.java)
- Ruby (.rb)
- PHP (.php)

## 🏗 Architecture

Code Search works in the following way:

1. **Search**: Uses ripgrep to quickly find files containing the search pattern
2. **Parse**: For each matching file, parses it with tree-sitter to build an AST
3. **Extract**: Identifies the smallest code block (function, class, etc.) that contains the match
4. **Rank**: Ranks results using TF-IDF, BM25, or a hybrid approach
5. **Output**: Returns complete, properly formatted code blocks

### Components

- **CLI Interface**: Handles user input and displays results
- **Search Engine**: Wraps ripgrep for efficient file searching
- **Language Parser**: Uses tree-sitter for language-specific parsing
- **Code Extractor**: Identifies code structures in the parse tree
- **Result Ranker**: Analyzes and sorts results by relevance

## 🔌 Server Mode

Code Search can also run as an MCP (Model Context Protocol) server that exposes a `search_code` tool:

```bash
# Start the server
code-search server
```

When running as a server, it implements the ServerHandler trait from the MCP Rust SDK, providing:

1. `initialize` - Handles client connection and capabilities negotiation
2. `handle_method` - Processes method calls like list_tools, call_tool, etc.
3. `shutdown` - Handles graceful server shutdown

### MCP Tool: search_code

Input schema:
```json
{
  "path": "Directory path to search in",
  "query": ["Query patterns to search for"],
  "files_only": false
}
```

## 🧑‍💻 For Developers

### Project Structure

The project is organized into the following directories:

- `src/` - Source code for the application
  - `search/` - Code search implementation modules
  - `language.rs` - Language-specific parsing
  - `models.rs` - Data structures
  - `ranking.rs` - Result ranking algorithms
  - `cli.rs` - Command-line interface
- `tests/` - Test files and utilities
  - `mocks/` - Mock data files for testing

### Building and Testing

```bash
# Build in debug mode
cargo build

# Build in release mode
cargo build --release

# Run all tests
cargo test

# Run specific test
cargo test test_search_single_term
```

### Adding Support for New Languages

To add support for a new programming language:

1. Add the tree-sitter grammar as a dependency in `Cargo.toml`:
   ```toml
   [dependencies]
   tree-sitter-newlang = "0.20"
   ```

2. Update the `get_language` function in `src/language.rs`:
   ```rust
   pub fn get_language(extension: &str) -> Option<Language> {
       match extension {
           // ... existing languages
           "nl" => Some(tree_sitter_newlang::language()),
           _ => None,
       }
   }
   ```

3. Update the `is_acceptable_parent` function to identify code structures for the language:
   ```rust
   pub fn is_acceptable_parent(node: &Node, extension: &str) -> bool {
       let node_type = node.kind();
       
       match extension {
           // ... existing languages
           "nl" => {
               matches!(node_type,
                   "function_declaration" |
                   "class_declaration" |
                   "other_structure"
               )
           },
           _ => false,
       }
   }
   ```

## 🔍 How It Works

1. The tool scans files using ripgrep's highly efficient search algorithm
2. For each match, it parses the file with tree-sitter to build an AST
3. It identifies the smallest AST node that:
   - Contains the matching line
   - Represents a complete code block (function, class, struct, etc.)
4. Extracts and ranks these code blocks based on relevance to the query
5. Presents the results with appropriate formatting and context

## 🚩 Troubleshooting

- **No matches found**: Verify your search pattern and check if there are matches using the regular ripgrep tool
- **File parsing errors**: Some files may have syntax errors or use language features not supported by the tree-sitter grammar
- **Missing code blocks**: Update the `is_acceptable_parent` function in `src/language.rs` to support additional node types

## 📄 License

This project is licensed under the MIT License - see the LICENSE file for details.

## 🤝 Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request