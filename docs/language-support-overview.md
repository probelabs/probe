# Language Support in Probe

This document provides an overview of Probe's language-aware code search and extraction capabilities.

## Introduction

One of Probe's most powerful features is its language-aware code search and extraction. Unlike traditional text-based search tools, Probe understands the structure of code in different programming languages, allowing it to:

- Extract meaningful code blocks (functions, classes, methods, etc.)
- Associate comments with their related code
- Filter out test code when desired
- Provide context-aware search results

This language awareness is made possible by Probe's language support system, which uses [tree-sitter](https://tree-sitter.github.io/tree-sitter/) to parse code into Abstract Syntax Trees (ASTs) and extract structured information.

## Architecture Overview

Probe's language support system consists of several key components:

1. **Language Trait**: A common interface (`LanguageImpl`) that all language implementations must follow
2. **Language Implementations**: Concrete implementations for each supported language
3. **Language Factory**: A factory that selects the appropriate language implementation based on file extension
4. **Parser**: A system that uses the language implementations to parse and extract code blocks
5. **Test Detection**: A system for identifying test code at both the file and node level

### Language Trait

The `LanguageImpl` trait defines the interface that all language implementations must follow:

```rust
pub trait LanguageImpl {
    // Get the tree-sitter language for parsing
    fn get_tree_sitter_language(&self) -> TSLanguage;

    // Check if a node is an acceptable container/parent entity
    fn is_acceptable_parent(&self, node: &Node) -> bool;

    // Check if a node represents a test
    fn is_test_node(&self, node: &Node, source: &[u8]) -> bool;

    // Get the file extension for this language
    fn get_extension(&self) -> &'static str;

    // Find the topmost struct type (mainly for Go)
    fn find_topmost_struct_type<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        // Default implementation returns the node itself
        Some(node)
    }

    // Find the parent function or method declaration for a node (if any)
    fn find_parent_function<'a>(&self, _node: Node<'a>) -> Option<Node<'a>> {
        // Default implementation returns None
        None
    }
}
```

## How It Works

When you search for code using Probe, the following process occurs:

1. **File Selection**: Probe identifies files that match your search criteria
2. **Language Detection**: For each file, Probe determines the language based on the file extension
3. **Parsing**: Probe parses the file using the appropriate tree-sitter grammar
4. **Pattern Matching**: Probe searches for your query patterns in the file
5. **Block Extraction**: For each match, Probe extracts the surrounding code block using language-specific rules
6. **Test Filtering**: If requested, Probe filters out test code
7. **Result Ranking**: Probe ranks the results based on relevance
8. **Output**: Probe displays the results, highlighting the matches within their context

## Benefits of Language-Aware Search

### 1. Precise Code Block Extraction

Traditional search tools often return individual lines or arbitrary chunks of code. Probe returns complete, meaningful code blocks such as:

- Functions and methods
- Classes and structs
- Interfaces and traits
- Modules and namespaces

This makes it much easier to understand the context and purpose of the code.

### 2. Intelligent Comment Handling

Comments are often crucial for understanding code, but they can be difficult to associate with the code they document. Probe intelligently associates comments with their related code blocks, ensuring that you see the complete context.

### 3. Test Code Filtering

Test code can often clutter search results when you're looking for implementation details. Probe allows you to easily filter out test code, focusing on the core implementation.

### 4. Language-Specific Features

Different languages have different structures and conventions. Probe understands these differences and handles each language appropriately, providing more relevant and useful results.

## Supported Languages

Probe currently supports a wide range of programming languages, including:

- Rust
- JavaScript / JSX
- TypeScript / TSX
- Python
- Go
- C / C++
- Java
- Ruby
- PHP
- Swift
- C#
- Markdown

For a complete list of supported languages and their specific features, see the [Supported Languages](/supported-languages) page.

## Adding New Languages

Probe's architecture makes it relatively easy to add support for new languages. If you're interested in contributing support for a language that's not currently supported, see the [Adding New Languages](/adding-languages) page for a detailed guide.

## Technical Details

### Abstract Syntax Trees (ASTs)

Probe uses tree-sitter to parse code into Abstract Syntax Trees (ASTs). An AST is a tree representation of the syntactic structure of source code. Each node in the tree represents a construct in the source code.

For example, a simple function in JavaScript:

```javascript
function add(a, b) {
    return a + b;
}
```

Might be represented by an AST like:

```
program
  function_declaration
    name: identifier "add"
    parameters
      identifier "a"
      identifier "b"
    body: block
      return_statement
        binary_expression
          left: identifier "a"
          operator: "+"
          right: identifier "b"
```

Probe uses these ASTs to understand the structure of code and extract meaningful blocks.

### Tree-sitter

[Tree-sitter](https://tree-sitter.github.io/tree-sitter/) is a parser generator tool and an incremental parsing library. It can build a concrete syntax tree for a source file and efficiently update the syntax tree as the source file is edited.

Tree-sitter provides several advantages for Probe:

1. **Incremental Parsing**: Tree-sitter can efficiently update the AST when the code changes
2. **Error Tolerance**: Tree-sitter can parse code with syntax errors, providing a best-effort AST
3. **Language Agnostic**: Tree-sitter supports a wide range of programming languages through grammar definitions
4. **Query Language**: Tree-sitter provides a query language for searching the AST

## Related Documentation

- [Supported Languages](/supported-languages): Detailed information about each supported language
- [Adding New Languages](/adding-languages): Guide for adding support for new languages
- [Search Functionality](/search-functionality): How to use Probe's search features
- [Code Extraction](/code-extraction): Details on how Probe extracts code blocks