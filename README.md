<p align="center">
  <img src="logo.png" alt="Probe Logo" width="600">
</p>

# Probe

Probe is an **AI-friendly, fully local, semantic code search** tool designed to power the next generation of AI coding assistants. By combining the speed of [ripgrep](https://github.com/BurntSushi/ripgrep) with the code-aware parsing of [tree-sitter](https://tree-sitter.github.io/tree-sitter/), Probe delivers precise results with complete code blocks—perfect for large codebases and AI-driven development workflows.

---

## Quick Start

**Basic Search Example**  
Search for code containing the phrase "llm pricing" in the current directory:

~~~bash
probe "llm pricing" ./
~~~

**Advanced Search (with Token Limiting)**  
Search for "partial prompt injection" in the current directory but limit the total tokens to 10000 (useful for AI tools with context window constraints):

~~~bash
probe "prompt injection" ./ --max-tokens 10000
~~~

---

## Features

- **AI-Friendly**: Extracts **entire functions, classes, or structs** so AI models get full context.  
- **Fully Local**: Keeps your code on your machine—no external APIs.  
- **Powered by ripgrep**: Extremely fast scanning of large codebases.  
- **Tree-sitter Integration**: Parses and understands code structure accurately.  
- **Re-Rankers & NLP**: Uses tokenization, stemming, BM25, TF-IDF, or hybrid ranking methods for better search results.  
- **Multi-Language**: Works with popular languages like Rust, Python, JavaScript, TypeScript, Java, Go, C/C++, etc.  
- **Flexible**: Run as a CLI tool or an MCP server for advanced AI integrations.

---

## Installation

### Quick Installation

You can install Probe with a single command:

~~~bash
curl -fsSL https://raw.githubusercontent.com/buger/probe/main/install.sh | bash
~~~

**What this script does**:

1. Detects your operating system and architecture  
2. Fetches the latest release from GitHub  
3. Downloads the appropriate binary for your system  
4. Verifies the checksum for security  
5. Installs the binary to `/usr/local/bin`

### Requirements

- **Operating Systems**: macOS, Linux, or Windows (with MSYS/Git Bash/WSL)  
- **Architectures**: x86_64 (all platforms) or ARM64 (macOS only)  
- **Tools**: `curl`, `bash`, and `sudo`/root privileges  

### Manual Installation

1. Download the appropriate binary for your platform from the [GitHub Releases](https://github.com/buger/probe/releases) page:
   - `probe-x86_64-linux.tar.gz` for Linux (x86_64)
   - `probe-x86_64-darwin.tar.gz` for macOS (Intel)
   - `probe-aarch64-darwin.tar.gz` for macOS (Apple Silicon)
   - `probe-x86_64-windows.zip` for Windows
2. Extract the archive:
   ~~~bash
   # For Linux/macOS
   tar -xzf probe-*-*.tar.gz
   
   # For Windows
   unzip probe-x86_64-windows.zip
   ~~~
3. Move the binary to a location in your PATH:
   ~~~bash
   # For Linux/macOS
   sudo mv probe /usr/local/bin/
   
   # For Windows
   # Move probe.exe to a directory in your PATH
   ~~~

### Building from Source

1. Install Rust and Cargo (if not already installed):
   ~~~bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ~~~
2. Clone this repository:
   ~~~bash
   git clone https://github.com/buger/probe.git
   cd code-search
   ~~~
3. Build the project:
   ~~~bash
   cargo build --release
   ~~~
4. (Optional) Install globally:
   ~~~bash
   cargo install --path .
   ~~~

### Verifying the Installation

~~~bash
probe --version
~~~

### Troubleshooting

- **Permissions**: Ensure you can write to `/usr/local/bin`.  
- **System Requirements**: Double-check your OS/architecture.  
- **Manual Install**: If the quick install script fails, try [Manual Installation](#manual-installation).  
- **GitHub Issues**: Report issues on the [GitHub repository](https://github.com/buger/probe/issues).

### Uninstalling

~~~bash
sudo rm /usr/local/bin/probe
~~~

---

## Usage

### CLI Mode

~~~bash
probe <SEARCH_PATTERN> [OPTIONS]
~~~

#### Key Options

- `<SEARCH_PATTERN>`: Pattern to search for (required)  
- `--paths`: Directories to search (defaults to current directory)  
- `--files-only`: Skip AST parsing; only list files with matches  
- `--ignore`: Custom ignore patterns (in addition to `.gitignore`)  
- `--include-filenames, -n`: Include files whose names match query words  
- `--reranker, -r`: Choose a re-ranking algorithm (`hybrid`, `hybrid2`, `bm25`, `tfidf`)  
- `--frequency, -s`: Frequency-based search (tokenization, stemming, stopword removal)  
- `--exact`: Exact matching (overrides frequency search)  
- `--max-results`: Maximum number of results to return  
- `--max-bytes`: Maximum total bytes of code to return  
- `--max-tokens`: Maximum total tokens of code to return (useful for AI)  
- `--allow-tests`: Include test files and test code blocks  
- `--any-term`: Match files containing **any** query terms (default is **all** terms)  
- `--no-merge`: Disable merging of adjacent code blocks after ranking (merging enabled by default)
- `--merge-threshold`: Max lines between code blocks to consider them adjacent for merging (default: 5)

#### Examples

~~~bash
# 1) Search for "setTools" in the current directory with frequency-based search
probe "setTools"

# 2) Search for "impl" in ./src with exact matching
probe "impl" --paths ./src --exact

# 3) Search for "search" returning only the top 5 results
probe "search" --max-tokens 10000

# 4) Search for "function" and disable merging of adjacent code blocks
probe "function" --no-merge
~~~

### MCP Server Mode

Run Probe as an MCP server:

~~~bash
cd mcp && npm run build && node build/index.js
~~~

This starts a server exposing a `search_code` tool for use with the [Model Context Protocol (MCP)](https://github.com/multiprocessio/mcp).

#### MCP Tool: `search_code`

- **Purpose**: Search code blocks based on various parameters.  
- **Input Schema** (JSON):
  ~~~json
  {
    "path": "Directory path to search in",
    "query": ["Query patterns to search for"],
    "filesOnly": false,
    "ignore": ["Patterns to ignore"],
    "includeFilenames": false,
    "reranker": "hybrid",
    "frequencySearch": true,
    "exact": false,
    "maxResults": null,
    "maxBytes": null,
    "maxTokens": null,
    "allowTests": false,
    "noMerge": false,
    "mergeThreshold": 5
  }
  ~~~

- **Usage Example** (MCP client in Rust):
  ~~~rust
  use std::sync::Arc;
  use mcp_rust_sdk::{Client, transport::stdio::StdioTransport};
  use serde_json::json;

  #[tokio::main]
  async fn main() -> Result<(), Box<dyn std::error::Error>> {
      let (transport, _) = StdioTransport::new();
      let client = Client::new(transport);

      let response = client.request(
          "call_tool",
          Some(json!({
              "name": "search_code",
              "arguments": {
                  "path": "./src",
                  "query": ["impl", "fn"],
                  "filesOnly": false,
                  "exact": true
              }
          }))
      ).await?;

      println!("Search results: {:?}", response);
      Ok(())
  }
  ~~~

The MCP server implements:
- `initialize`  
- `handle_method` (for `list_tools`, `call_tool`, etc.)  
- `shutdown`

---

## Supported Languages

Probe currently supports:

- **Rust** (`.rs`)  
- **JavaScript / JSX** (`.js`, `.jsx`)  
- **TypeScript / TSX** (`.ts`, `.tsx`)  
- **Python** (`.py`)  
- **Go** (`.go`)  
- **C / C++** (`.c`, `.h`, `.cpp`, `.cc`, `.cxx`, `.hpp`, `.hxx`)  
- **Java** (`.java`)  
- **Ruby** (`.rb`)  
- **PHP** (`.php`)  
- **Markdown** (`.md`, `.markdown`)

---

## How It Works

Probe combines **fast file scanning** with **deep code parsing** to provide highly relevant, context-aware results:

1. **Ripgrep Scanning**  
   Probe uses ripgrep to quickly search across your files, identifying lines that match your query. Ripgrep’s efficiency allows it to handle massive codebases at lightning speed.

2. **AST Parsing with Tree-sitter**  
   For each file containing matches, Probe uses tree-sitter to parse the file into an Abstract Syntax Tree (AST). This process ensures that code blocks (functions, classes, structs) can be identified precisely.

3. **NLP & Re-Rankers**  
   Next, Probe applies classical NLP methods—tokenization, stemming, and stopword removal—alongside re-rankers such as **BM25**, **TF-IDF**, or the **hybrid** approach (combining multiple ranking signals). This step elevates the most relevant code blocks to the top, especially helpful for AI-driven searches.

4. **Block Extraction**  
   Probe identifies the smallest complete AST node containing each match (e.g., a full function or class). It extracts these code blocks and aggregates them into search results.

5. **Context for AI**  
   Finally, these structured blocks can be returned directly or fed into an AI system. By providing the full context of each code segment, Probe helps AI models navigate large codebases and produce more accurate insights.

---

## Adding Support for New Languages

1. **Tree-sitter Grammar**: In `Cargo.toml`, add the tree-sitter parser for the new language.  
2. **Language Module**: Create a new file in `src/language/` for parsing logic.  
3. **Implement Language Trait**: Adapt the parse method for the new language constructs.  
4. **Factory Update**: Register your new language in Probe’s detection mechanism.

---

## Releasing New Versions

Probe uses GitHub Actions for multi-platform builds and releases.

1. **Update `Cargo.toml`** with the new version.  
2. **Create a new Git tag**:
   ~~~bash
   git tag -a vX.Y.Z -m "Release vX.Y.Z"
   git push origin vX.Y.Z
   ~~~
3. **GitHub Actions** will build, package, and draft a new release with checksums.

Each release includes:
- Linux binary (x86_64)  
- macOS binaries (x86_64 and aarch64)  
- Windows binary (x86_64)  
- SHA256 checksums  

---

## Project Structure

.
├── src/
│   ├── language/           # Language-specific parsing modules
│   ├── search/             # Search implementation modules
│   └── main.rs             # CLI entry point
├── tests/
│   └── mocks/              # Mock data for testing
├── mcp/                    # MCP server implementation
├── target/                 # Cargo build artifacts
└── .github/workflows/      # GitHub Actions CI/CD workflows

---


We believe that **local, privacy-focused, semantic code search** is essential for the future of AI-assisted development. Probe is built to empower developers and AI alike to navigate and comprehend large codebases more effectively.

For questions or contributions, please open an issue on [GitHub](https://github.com/buger/probe/issues). Happy coding—and searching!
