<p align="center">
  <img src="logo.png?2" alt="Probe Logo" width="400">
</p>

# Probe

Probe is an **AI-friendly, fully local, semantic code search** tool designed to power the next generation of AI coding assistants. By combining the speed of [ripgrep](https://github.com/BurntSushi/ripgrep) with the code-aware parsing of [tree-sitter](https://tree-sitter.github.io/tree-sitter/), Probe delivers precise results with complete code blocks—perfect for large codebases and AI-driven development workflows.

---

## Table of Contents

- [Quick Start](#quick-start)
- [Features](#features)
- [Installation](#installation)
  - [Quick Installation](#quick-installation)
  - [Requirements](#requirements)
  - [Manual Installation](#manual-installation)
  - [Building from Source](#building-from-source)
  - [Verifying the Installation](#verifying-the-installation)
  - [Troubleshooting](#troubleshooting)
  - [Uninstalling](#uninstalling)
- [Usage](#usage)
  - [CLI Mode](#cli-mode)
  - [MCP Server Mode](#mcp-server-mode)
  - [AI Chat Mode](#ai-chat-mode)
  - [Web Interface](#web-interface)
- [Supported Languages](#supported-languages)
- [How It Works](#how-it-works)
- [Adding Support for New Languages](#adding-support-for-new-languages)
- [Releasing New Versions](#releasing-new-versions)

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

**Interactive AI Chat**
Use the built-in AI assistant to ask questions about your codebase:

~~~bash
# Set your API key first
export ANTHROPIC_API_KEY=your_api_key
# Then start the chat interface
probe chat
~~~

**MCP server**

Integrate with any AI editor:

  ~~~json
  {
    "mcpServers": {
      "memory": {
        "command": "npx",
        "args": [
          "-y",
          "@buger/probe-mcp"
        ]
      }
    }
  }
  ~~~

Example queries:
> "Do the probe and search my codebase for implementations of the ranking algorithm"
>
> "Using probe find all functions related to error handling in the src directory"

---

## Features

- **AI-Friendly**: Extracts **entire functions, classes, or structs** so AI models get full context.
- **Fully Local**: Keeps your code on your machine—no external APIs.
- **Powered by ripgrep**: Extremely fast scanning of large codebases.
- **Tree-sitter Integration**: Parses and understands code structure accurately.
- **Re-Rankers & NLP**: Uses tokenization, stemming, BM25, TF-IDF, or hybrid ranking methods for better search results.
- **Multi-Language**: Works with popular languages like Rust, Python, JavaScript, TypeScript, Java, Go, C/C++, Swift, C#, and more.
- **Interactive AI Chat**: Built-in AI assistant that can answer questions about your codebase using Claude or GPT models.
- **Flexible**: Run as a CLI tool, an MCP server, or an interactive AI chat.

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

Probe can be used in three main modes:

1. **CLI Mode**: Direct code search from the command line
2. **MCP Server Mode**: Run as a server exposing search functionality via MCP
3. **AI Chat Mode**: Interactive AI assistant for code exploration

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
~~~

### MCP Server

Add the following to your AI editor's MCP configuration file:
  
  ~~~json
  {
    "mcpServers": {
      "memory": {
        "command": "npx",
        "args": [
          "-y",
          "@buger/probe-mcp"
        ]
      }
    }
  }
  ~~~
  
- **Example Usage in AI Editors**:
  
  Once configured, you can ask your AI assistant to search your codebase with natural language queries like:
  
  > "Do the probe and search my codebase for implementations of the ranking algorithm"
  >
  > "Using probe find all functions related to error handling in the src directory"

### AI Chat Mode

Run Probe as an interactive AI assistant:

~~~bash
probe chat
~~~

This starts an interactive CLI interface where you can ask questions about your codebase and get AI-powered responses.

#### Features

- **AI-Powered Search**: Uses LLMs to understand your questions and search the codebase intelligently
- **Multi-Model Support**: Works with both Anthropic's Claude and OpenAI's GPT models
- **Token Tracking**: Monitors token usage for both requests and responses
- **Conversation History**: Maintains context across multiple interactions
- **Colored Output**: Provides a user-friendly terminal interface with syntax highlighting

#### Configuration

Configure the chat using environment variables:

~~~bash
# Use Claude models (recommended)
export ANTHROPIC_API_KEY=your_api_key
probe chat

# Or use OpenAI models
export OPENAI_API_KEY=your_api_key
probe chat

# Override the default model
export MODEL_NAME=claude-3-opus-20240229
probe chat

# Override API URLs (useful for proxies or enterprise deployments)
export ANTHROPIC_API_URL=https://your-anthropic-proxy.com
export OPENAI_API_URL=https://your-openai-proxy.com/v1
probe chat

# Enable debug mode for detailed logging
export DEBUG=1 probe chat
~~~

#### Example Usage

~~~
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
~~~

### Web Interface

Probe includes a web-based chat interface that provides a user-friendly way to interact with your codebase using AI. The web interface is located in the `web/` folder and offers a modern UI for code search and AI-powered code exploration.

#### Features

- **Interactive Chat UI**: Clean, modern interface with markdown and syntax highlighting
- **AI-Powered Code Search**: Uses Claude AI to search and explain your codebase
- **Mermaid Diagram Support**: Renders visual diagrams for code architecture and flows
- **Configurable Search Paths**: Define which directories can be searched via environment variables

#### Setup and Configuration

1. **Navigate to the web directory**:
   ```bash
   cd web
   ```

2. **Install dependencies**:
   ```bash
   npm install
   ```

3. **Configure environment variables**:
   Create or edit the `.env` file in the web directory:
   ```
   ANTHROPIC_API_KEY=your_anthropic_api_key
   PORT=8080
   ALLOWED_FOLDERS=/path/to/folder1,/path/to/folder2
   ```

4. **Start the server**:
   ```bash
   npm start
   ```

5. **Access the web interface**:
   Open your browser and navigate to `http://localhost:8080`

#### Technical Details

- Built with vanilla JavaScript and Node.js
- Uses the Vercel AI SDK for Claude integration
- Executes Probe commands via the probeTool.js module
- Renders markdown with Marked.js and syntax highlighting with Highlight.js
- Supports Mermaid.js for diagram generation and visualization

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
- **Swift** (`.swift`)
- **C#** (`.cs`)
- **Markdown** (`.md`, `.markdown`)

---

## How It Works

Probe combines **fast file scanning** with **deep code parsing** to provide highly relevant, context-aware results:

1. **Ripgrep Scanning**  
   Probe uses ripgrep to quickly search across your files, identifying lines that match your query. Ripgrep's efficiency allows it to handle massive codebases at lightning speed.

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
4. **Factory Update**: Register your new language in Probe's detection mechanism.

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

We believe that **local, privacy-focused, semantic code search** is essential for the future of AI-assisted development. Probe is built to empower developers and AI alike to navigate and comprehend large codebases more effectively.

For questions or contributions, please open an issue on [GitHub](https://github.com/buger/probe/issues). Happy coding—and searching!
