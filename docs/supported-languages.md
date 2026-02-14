# Supported Languages

Probe provides language-aware code search and extraction for a wide range of programming languages. This page details the supported languages and their specific features.

> For a comprehensive overview of how Probe's language support system works, see the [Language Support Overview](/language-support-overview) page.

## Language Support Table

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
| YAML | `.yaml`, `.yml` | ✅ | ✅ |

## Language Detection

Probe automatically detects the language of a file based on its extension. This detection is used to:

1. **Select the appropriate parser**: Each language has a specialized parser
2. **Apply language-specific extraction rules**: Different languages have different code structures
3. **Handle language-specific features**: Such as Python's significant whitespace or Rust's macros

## Language-Specific Features

Each language has specific features and capabilities in Probe:

### Rust

- **Function Extraction**: Extracts complete function definitions including attributes and documentation
- **Struct/Enum Extraction**: Extracts complete struct and enum definitions
- **Impl Block Extraction**: Extracts implementation blocks for types
- **Macro Handling**: Properly handles macro definitions and invocations
- **Module Awareness**: Understands Rust's module system
- **Test Detection**: Identifies test functions marked with `#[test]` attribute or functions with names starting with `test_`

### JavaScript / TypeScript

- **Function Extraction**: Extracts function and arrow function definitions
- **Class Extraction**: Extracts complete class definitions with methods
- **JSX/TSX Support**: Properly handles JSX and TSX syntax
- **Module Awareness**: Understands ES modules and CommonJS
- **Type Definitions**: Extracts TypeScript interfaces and type definitions
- **Test Detection**: Identifies test functions using Jest, Mocha, or other frameworks

TypeScript adds additional support for interfaces, type aliases, enums, and namespaces.

### Python

- **Function Extraction**: Extracts function definitions with docstrings
- **Class Extraction**: Extracts complete class definitions with methods
- **Decorator Handling**: Properly handles decorated functions and classes
- **Indentation Awareness**: Understands Python's significant whitespace
- **Module Docstrings**: Includes module-level docstrings in relevant extractions
- **Test Detection**: Identifies test functions using unittest, pytest, or other frameworks

### Go

- **Function Extraction**: Extracts function definitions with documentation
- **Struct Extraction**: Extracts complete struct definitions
- **Interface Extraction**: Extracts interface definitions
- **Method Extraction**: Extracts methods associated with types
- **Comment Handling**: Properly associates comments with code blocks
- **Test Detection**: Identifies test functions with the `Test` prefix

Go also implements special handling for nested struct types.

### C / C++

- **Function Extraction**: Extracts function definitions and declarations
- **Class/Struct Extraction**: Extracts complete class and struct definitions
- **Template Handling**: Properly handles template definitions
- **Namespace Awareness**: Understands C++ namespaces
- **Preprocessor Handling**: Includes relevant preprocessor directives
- **Test Detection**: Identifies test functions based on naming conventions

### Java

- **Method Extraction**: Extracts method definitions with annotations
- **Class Extraction**: Extracts complete class definitions
- **Interface Extraction**: Extracts interface definitions
- **Annotation Handling**: Properly handles annotated elements
- **Package Awareness**: Understands Java's package system
- **Test Detection**: Identifies test classes and methods using JUnit annotations

### Ruby

- **Method Extraction**: Extracts method definitions
- **Class/Module Extraction**: Extracts complete class and module definitions
- **Block Handling**: Properly handles Ruby blocks
- **Mixin Awareness**: Understands Ruby's include and extend
- **Documentation**: Includes RDoc comments in extractions
- **Test Detection**: Identifies test methods in Test::Unit, RSpec, or Minitest

### PHP

- **Function Extraction**: Extracts function definitions
- **Class Extraction**: Extracts complete class definitions with methods
- **Namespace Awareness**: Understands PHP namespaces
- **Attribute Handling**: Properly handles PHP 8 attributes
- **Documentation**: Includes PHPDoc comments in extractions
- **Test Detection**: Identifies test classes and methods using PHPUnit conventions

### Swift

- **Function Extraction**: Extracts function and method definitions
- **Class/Struct Extraction**: Extracts complete class and struct definitions
- **Protocol Extraction**: Extracts protocol definitions
- **Extension Handling**: Properly handles Swift extensions
- **Attribute Handling**: Includes relevant attributes in extractions
- **Test Detection**: Identifies test classes and methods using XCTest conventions

### C#

- **Method Extraction**: Extracts method definitions with attributes
- **Class Extraction**: Extracts complete class definitions
- **Interface Extraction**: Extracts interface definitions
- **Namespace Awareness**: Understands C# namespaces
- **Attribute Handling**: Properly handles C# attributes
- **Test Detection**: Identifies test classes and methods using NUnit, MSTest, or xUnit conventions

### Markdown

- **Section Extraction**: Extracts complete sections based on headings
- **Code Block Extraction**: Extracts fenced code blocks
- **List Extraction**: Extracts complete lists
- **Table Extraction**: Extracts complete tables
- **Frontmatter Handling**: Properly handles YAML frontmatter

### YAML

- **Key-Value Pair Extraction**: Extracts mapping pairs with keys and values
- **Sequence Extraction**: Extracts arrays and lists
- **Scalar Extraction**: Extracts literal and folded block scalars
- **Comment Extraction**: Extracts comments and preserves them in context
- **Document Structure**: Handles multi-document YAML streams
- **Anchor and Alias Support**: Properly handles YAML anchors and aliases
- **Test Detection**: Identifies test-related configuration keys and values

## Test Detection

Probe can detect test code at two levels:

1. **File-level detection**: Identifies test files based on naming conventions and directory patterns
2. **Node-level detection**: Identifies test functions and classes within files

This allows users to include or exclude test code from search results using the `--allow-tests` flag.

### File-level Test Detection

Probe identifies test files based on common naming patterns:

- **Rust**: `*_test.rs`, `*_tests.rs`, `test_*.rs`, `tests.rs`
- **JavaScript/TypeScript**: `*.test.js`, `*.spec.js`, `*.test.ts`, `*.spec.ts`
- **Python**: `test_*.py`, `*_test.py`
- **Go**: `*_test.go`
- **Java**: `*Test.java`, `*Tests.java`

Probe also identifies test files based on directory patterns:
- `/test/`, `/tests/`, `/spec/`, `/specs/`, `/__tests__/`

### Node-level Test Detection

Each language implementation has specific logic to identify test functions and classes:

- **Rust**: Functions with `#[test]` attribute or names starting with `test_`
- **JavaScript/TypeScript**: Functions using Jest, Mocha, or other test frameworks
- **Python**: Functions using unittest, pytest, or other test frameworks
- **Go**: Functions with the `Test` prefix
- **Java**: Classes and methods with JUnit annotations

## Pattern Matching Examples

Probe supports language-specific pattern matching. Here are some examples:

### Rust Patterns
```
fn $NAME($$$PARAMS) $$$BODY
struct $NAME { $$$FIELDS }
impl $TYPE { $$$METHODS }
```

### JavaScript/TypeScript Patterns
```
function $NAME($$$PARAMS) { $$$BODY }
const $NAME = ($$$PARAMS) => $$$BODY
class $NAME { $$$METHODS }
```

### Python Patterns
```
def $NAME($$$PARAMS): $$$BODY
class $NAME: $$$METHODS
@$DECORATOR\ndef $NAME($$$PARAMS): $$$BODY
```

### Go Patterns
```
func $NAME($$$PARAMS) $$$BODY
type $NAME struct { $$$FIELDS }
func ($RECEIVER $TYPE) $NAME($$$PARAMS) $$$BODY
```

## Adding Support for New Languages

If you're interested in adding support for a language that's not currently supported, see the [Adding New Languages](/adding-languages) page for a detailed guide.