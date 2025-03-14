# Code Extraction Reference

Complete reference documentation for Probe's code extraction capabilities, including AST parsing, language-specific extraction, and advanced usage techniques.

## EXTRACT COMMAND

```bash
probe extract <FILES> [OPTIONS]
```

### CORE PARAMETERS

| Parameter | Description |
|-----------|-------------|
| `<FILES>` | **Required**: Files to extract from (e.g., `main.rs:42` or `main.rs#function_name`) |

### KEY OPTIONS

| Option | Description | Default |
|--------|-------------|---------|
| `--allow-tests` | Include test code blocks | Off |
| `-c, --context <N>` | Add N context lines | 0 |
| `-f, --format <TYPE>` | Output as: `markdown`, `plain`, `json`, `xml`, `color` | `color` |
| `--to-clipboard` | Copy results to clipboard | Off |
| `--from-clipboard` | Read file paths from clipboard | Off |

For complete option details, see `probe extract --help`.

## FILE PATH SYNTAX

Probe supports several ways to specify what code to extract:

- **Entire file**: `file.rs`
- **Specific line**: `file.rs:42`
- **Line range**: `file.rs:10-20`
- **Symbol name**: `file.rs#handle_extract`
- **Multiple files**: `file1.rs:10 file2.go:15`
- **Glob patterns**: `src/*.rs:42`

## AST PARSING

Probe uses Abstract Syntax Tree (AST) parsing to understand code structure:

### EXTRACTION PROCESS

When you specify a line number, Probe:

1. **Parses** the file using tree-sitter to generate an AST
2. **Locates** the node in the AST containing the specified line
3. **Navigates** to find the smallest complete code unit (function, class, etc.)
4. **Extracts** the entire code block with proper formatting

This approach ensures you get complete, syntactically valid code blocks rather than arbitrary line ranges.

```
File Content → Tree-sitter Parser → AST Generation → Node Location → Parent Node Identification → Code Block Extraction
```

### BENEFITS

- **Structural Understanding**: Recognizes code structures, not just text
- **Complete Units**: Extracts entire functions, classes, or blocks
- **Language Awareness**: Applies language-specific extraction rules
- **Context Preservation**: Maintains the full context of code elements
- **Documentation Inclusion**: Captures associated comments and documentation

### FALLBACK MECHANISMS

If AST parsing fails or isn't available for a particular language:

1. **Context-Based Fallback**: Extracts the specified line with configurable context
2. **Line Range Extraction**: For explicit line ranges, extracts exactly those lines
3. **Symbol Text Search**: For symbol references, falls back to text-based search

## LANGUAGE-SPECIFIC EXTRACTION

Probe provides specialized extraction for each supported language:

### RUST

- Function definitions with attributes and documentation
- Struct and enum definitions
- Implementation blocks
- Macro definitions

```bash
# Extract a Rust function
probe extract src/main.rs#handle_request

# Extract an impl block
probe extract src/models.rs:42
```

### JAVASCRIPT / TYPESCRIPT

- Function and arrow function definitions
- Class definitions with methods
- JSX/TSX components
- TypeScript interfaces and type definitions

```bash
# Extract a JavaScript class
probe extract src/components/Button.js#Button

# Extract a React component
probe extract src/App.tsx:15
```

### PYTHON

- Function definitions with docstrings
- Class definitions with methods
- Decorated functions and classes
- Indentation-aware extraction

```bash
# Extract a Python class
probe extract src/models.py#UserModel

# Extract a decorated function
probe extract src/views.py:42
```

### GO

- Function definitions with documentation
- Struct and interface definitions
- Methods associated with types
- Comment association

```bash
# Extract a Go struct
probe extract pkg/models/user.go#User

# Extract a method
probe extract pkg/handlers/auth.go:42
```

### OTHER LANGUAGES

Probe supports extraction for many other languages including:

- **C/C++**: Functions, classes, structs, templates, namespaces
- **Java**: Methods, classes, interfaces, annotations
- **Ruby**: Methods, classes, modules, blocks
- **PHP**: Functions, classes, namespaces, attributes
- **Swift**: Functions, classes, structs, protocols, extensions
- **C#**: Methods, classes, interfaces, namespaces, attributes
- **Markdown**: Sections, code blocks, lists, tables, frontmatter

Each language implementation understands the unique syntax and structures of that language.

## ADVANCED USAGE TECHNIQUES

### EXTRACTING FROM ERROR MESSAGES

Feed compiler errors directly to extract relevant code:

```bash
# Extract code from compiler errors
rustc main.rs 2>&1 | probe extract

# Pull code from test failures
go test ./... | probe extract
```

### PIPELINE INTEGRATION

Chain with other tools for powerful workflows:

```bash
# Find and extract error handlers
probe search "error handling" --files-only | xargs -I{} probe extract {} --format markdown

# Extract specific functions with context
grep -n "handleRequest" ./src/*.js | cut -d':' -f1,2 | probe extract --context 3

# Extract all functions matching a pattern
find . -name "*.py" | xargs grep -l "def test_" | xargs -I{} probe extract {}#test_
```

### SYMBOL EXTRACTION EXAMPLES

Extract code by symbol name across different languages:

```bash
# Extract a Rust function
probe extract src/main.rs#handle_extract

# Extract a JavaScript class method
probe extract src/components/User.js#User.authenticate

# Extract a Python class method
probe extract src/models.py#UserModel.save

# Extract a Go interface
probe extract pkg/service/interface.go#UserService
```

### MULTI-FILE EXTRACTION

Extract code from multiple files in a single command:

```bash
# Extract from multiple specific files
probe extract src/auth.js:15 src/api.js:27 src/models.rs:42

# Extract using glob patterns
probe extract src/*.rs:42

# Extract multiple symbols
probe extract src/main.rs#handle_request src/models.rs#User
```

## PRACTICAL APPLICATIONS

### CODE REVIEW WORKFLOWS

```bash
# Extract changes for review
git diff --name-only | grep "\.rs$" | xargs -I{} probe extract {}

# Extract functions modified in a PR
git diff --name-only origin/main | xargs grep -l "fn " | xargs -I{} probe extract {}
```

### DOCUMENTATION GENERATION

```bash
# Extract public API functions for documentation
find . -name "*.rs" | xargs grep -l "pub fn" | xargs -I{} probe extract {} --format markdown > api_docs.md

# Extract class definitions for API reference
find . -name "*.py" | xargs grep -l "class " | xargs -I{} probe extract {} --format markdown > classes.md
```

### AI INTEGRATION

```bash
# Extract code for AI context
probe extract src/main.rs:42 --format json | jq '.results[0].code' | ai-assistant "Explain this code"

# Extract multiple related functions for AI analysis
probe extract src/auth.rs#authenticate src/auth.rs#validate --format json --max-tokens 4000
```

### DEBUGGING ASSISTANCE

```bash
# Extract code from error stack trace
cat error.log | probe extract

# Extract function with additional context
probe extract src/api.js:27 --context 10
```

## BENEFITS OF CODE EXTRACTION

- **COMPLETE CONTEXT**: Get entire functions or classes, not just fragments
- **LANGUAGE AWARENESS**: Extracts code according to language-specific rules
- **PRECISE TARGETING**: Extract exactly what you need by line or symbol name
- **FORMAT FLEXIBILITY**: Output in various formats for different workflows
- **TOOL INTEGRATION**: Works seamlessly with other command-line tools
- **INTELLIGENT FALLBACKS**: Gracefully handles cases where AST parsing isn't possible

For more information on how Probe works internally, see [How Probe Works](how-it-works.md).
For details on search capabilities, see [Search Functionality](search-functionality.md).