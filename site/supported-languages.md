# Supported Languages

Probe provides language-aware code search and extraction for a wide range of programming languages. This page details the supported languages and their specific features.

## Language Support Overview

Probe currently supports the following languages:

| Language | File Extensions | AST Parsing | Block Extraction |
|----------|----------------|-------------|-----------------|
| Rust | `.rs` | ✅ | ✅ |
| JavaScript / JSX | `.js`, `.jsx` | ✅ | ✅ |
| TypeScript / TSX | `.ts`, `.tsx` | ✅ | ✅ |
| Python | `.py` | ✅ | ✅ |
| Go | `.go` | ✅ | ✅ |
| C / C++ | `.c`, `.h`, `.cpp`, `.cc`, `.cxx`, `.hpp`, `.hxx` | ✅ | ✅ |
| Java | `.java` | ✅ | ✅ |
| Ruby | `.rb` | ✅ | ✅ |
| PHP | `.php` | ✅ | ✅ |
| Swift | `.swift` | ✅ | ✅ |
| C# | `.cs` | ✅ | ✅ |
| Markdown | `.md`, `.markdown` | ✅ | ✅ |

## Language-Specific Features

Each language has specific features and capabilities in Probe:

### Rust

- **Function Extraction**: Extracts complete function definitions including attributes and documentation
- **Struct/Enum Extraction**: Extracts complete struct and enum definitions
- **Impl Block Extraction**: Extracts implementation blocks for types
- **Macro Handling**: Properly handles macro definitions and invocations
- **Module Awareness**: Understands Rust's module system

### JavaScript / TypeScript

- **Function Extraction**: Extracts function and arrow function definitions
- **Class Extraction**: Extracts complete class definitions with methods
- **JSX/TSX Support**: Properly handles JSX and TSX syntax
- **Module Awareness**: Understands ES modules and CommonJS
- **Type Definitions**: Extracts TypeScript interfaces and type definitions

### Python

- **Function Extraction**: Extracts function definitions with docstrings
- **Class Extraction**: Extracts complete class definitions with methods
- **Decorator Handling**: Properly handles decorated functions and classes
- **Indentation Awareness**: Understands Python's significant whitespace
- **Module Docstrings**: Includes module-level docstrings in relevant extractions

### Go

- **Function Extraction**: Extracts function definitions with documentation
- **Struct Extraction**: Extracts complete struct definitions
- **Interface Extraction**: Extracts interface definitions
- **Method Extraction**: Extracts methods associated with types
- **Comment Handling**: Properly associates comments with code blocks

### C / C++

- **Function Extraction**: Extracts function definitions and declarations
- **Class/Struct Extraction**: Extracts complete class and struct definitions
- **Template Handling**: Properly handles template definitions
- **Namespace Awareness**: Understands C++ namespaces
- **Preprocessor Handling**: Includes relevant preprocessor directives

### Java

- **Method Extraction**: Extracts method definitions with annotations
- **Class Extraction**: Extracts complete class definitions
- **Interface Extraction**: Extracts interface definitions
- **Annotation Handling**: Properly handles annotated elements
- **Package Awareness**: Understands Java's package system

### Ruby

- **Method Extraction**: Extracts method definitions
- **Class/Module Extraction**: Extracts complete class and module definitions
- **Block Handling**: Properly handles Ruby blocks
- **Mixin Awareness**: Understands Ruby's include and extend
- **Documentation**: Includes RDoc comments in extractions

### PHP

- **Function Extraction**: Extracts function definitions
- **Class Extraction**: Extracts complete class definitions with methods
- **Namespace Awareness**: Understands PHP namespaces
- **Attribute Handling**: Properly handles PHP 8 attributes
- **Documentation**: Includes PHPDoc comments in extractions

### Swift

- **Function Extraction**: Extracts function and method definitions
- **Class/Struct Extraction**: Extracts complete class and struct definitions
- **Protocol Extraction**: Extracts protocol definitions
- **Extension Handling**: Properly handles Swift extensions
- **Attribute Handling**: Includes relevant attributes in extractions

### C#

- **Method Extraction**: Extracts method definitions with attributes
- **Class Extraction**: Extracts complete class definitions
- **Interface Extraction**: Extracts interface definitions
- **Namespace Awareness**: Understands C# namespaces
- **Attribute Handling**: Properly handles C# attributes

### Markdown

- **Section Extraction**: Extracts complete sections based on headings
- **Code Block Extraction**: Extracts fenced code blocks
- **List Extraction**: Extracts complete lists
- **Table Extraction**: Extracts complete tables
- **Frontmatter Handling**: Properly handles YAML frontmatter

## Language Detection

Probe automatically detects the language of a file based on its extension. This detection is used to:

1. **Select the appropriate parser**: Each language has a specialized parser
2. **Apply language-specific extraction rules**: Different languages have different code structures
3. **Handle language-specific features**: Such as Python's significant whitespace or Rust's macros

## Test Detection

For each supported language, Probe can detect test code based on language-specific patterns:

- **Rust**: Test modules and functions marked with `#[test]`
- **JavaScript/TypeScript**: Test functions using Jest, Mocha, or other frameworks
- **Python**: Test functions using unittest, pytest, or other frameworks
- **Go**: Test functions with the `Test` prefix
- **Java**: Classes and methods using JUnit annotations
- **And more...**

Test detection allows you to include or exclude test code from search results using the `--allow-tests` flag.

## Adding Support for New Languages

Probe's architecture makes it relatively easy to add support for new languages. See the [Adding New Languages](/adding-languages) page for details on how to contribute support for additional languages.