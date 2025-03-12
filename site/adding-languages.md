# Adding Support for New Languages

Probe is designed to be extensible, making it relatively straightforward to add support for new programming languages. This guide walks through the process of adding a new language to Probe.

## Overview

Adding a new language to Probe involves several steps:

1. Adding the tree-sitter grammar for the language
2. Creating a language module
3. Implementing the Language trait
4. Registering the language in the factory
5. Testing the implementation

Let's go through each step in detail.

## 1. Adding the Tree-sitter Grammar

First, you need to add the tree-sitter grammar for the new language as a dependency in `Cargo.toml`:

```toml
[dependencies]
# Existing dependencies...
tree-sitter = "0.20.10"
tree-sitter-rust = "0.20.4"
# Add your new language
tree-sitter-mylanguage = "0.1.0"
```

If a tree-sitter grammar doesn't exist for your language, you may need to create one or adapt an existing one. See the [tree-sitter documentation](https://tree-sitter.github.io/tree-sitter/creating-parsers) for details on creating parsers.

## 2. Creating a Language Module

Create a new file in the `src/language/` directory for your language. For example, if you're adding support for a language called "MyLanguage", create `src/language/mylanguage.rs`:

```rust
use crate::language::language_trait::Language;
use crate::language::common::{extract_block_with_context, find_parent_node_of_type};
use tree_sitter::{Node, Parser, Query, QueryCursor, Tree};

pub struct MyLanguage;

// Implementation will go here...
```

## 3. Implementing the Language Trait

Implement the `Language` trait for your new language. This involves implementing several methods:

```rust
impl Language for MyLanguage {
    fn get_language() -> tree_sitter::Language {
        // Return the tree-sitter language for your language
        tree_sitter_mylanguage::language()
    }

    fn get_file_extensions() -> Vec<&'static str> {
        // Return the file extensions for your language
        vec![".ml", ".mli"] // Example for OCaml
    }

    fn is_test_file(file_path: &str) -> bool {
        // Determine if a file is a test file based on naming conventions
        file_path.contains("_test.") || file_path.contains("test_") || file_path.contains("/tests/")
    }

    fn is_test_node(node: &Node, source: &str) -> bool {
        // Determine if a node represents test code
        // This is language-specific and depends on testing conventions
        let node_type = node.kind();
        
        if node_type == "function_definition" {
            // Example: Check if function name contains "test"
            let name_node = node.child_by_field_name("name");
            if let Some(name) = name_node {
                let name_text = name.utf8_text(source.as_bytes()).unwrap_or("");
                return name_text.contains("test");
            }
        }
        
        false
    }

    fn parse(source: &str, parser: &mut Parser) -> Option<Tree> {
        // Parse the source code using the tree-sitter parser
        parser.set_language(Self::get_language()).ok()?;
        parser.parse(source, None)
    }

    fn extract_block(node: Node, source: &str, with_context: bool, context_lines: usize) -> String {
        // Extract a code block for the given node
        // This is often similar across languages, so you can use common utilities
        extract_block_with_context(node, source, with_context, context_lines)
    }

    fn find_closest_parent_block(node: Node, source: &str) -> Option<Node> {
        // Find the closest parent node that represents a complete code block
        // This is language-specific and depends on the AST structure
        
        // Example: Look for function definitions, class definitions, etc.
        find_parent_node_of_type(
            node,
            &[
                "function_definition",
                "class_definition",
                "method_definition",
                // Add other relevant node types for your language
            ],
        )
    }
}
```

### Key Methods to Implement

- **`get_language()`**: Returns the tree-sitter language for your language.
- **`get_file_extensions()`**: Returns the file extensions associated with your language.
- **`is_test_file()`**: Determines if a file is a test file based on naming conventions.
- **`is_test_node()`**: Determines if a node represents test code.
- **`parse()`**: Parses the source code using the tree-sitter parser.
- **`extract_block()`**: Extracts a code block for the given node.
- **`find_closest_parent_block()`**: Finds the closest parent node that represents a complete code block.

## 4. Registering the Language in the Factory

Update the language factory in `src/language/factory.rs` to include your new language:

```rust
// Add this to the imports
use crate::language::mylanguage::MyLanguage;

// Add your language to the get_language_for_file function
pub fn get_language_for_file(file_path: &str) -> Option<Box<dyn Language>> {
    let extension = Path::new(file_path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("");

    // Check file extension
    match extension {
        // Existing languages...
        "rs" => Some(Box::new(RustLanguage)),
        "js" | "jsx" => Some(Box::new(JavaScriptLanguage)),
        // Add your language
        "ml" | "mli" => Some(Box::new(MyLanguage)),
        _ => None,
    }
}
```

Also update the `get_all_languages` function to include your language:

```rust
pub fn get_all_languages() -> Vec<Box<dyn Language>> {
    vec![
        // Existing languages...
        Box::new(RustLanguage),
        Box::new(JavaScriptLanguage),
        // Add your language
        Box::new(MyLanguage),
    ]
}
```

## 5. Testing the Implementation

Create tests for your language implementation in `src/language/tests.rs` or a separate test file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::mylanguage::MyLanguage;
    use crate::language::language_trait::Language;

    #[test]
    fn test_mylanguage_file_extensions() {
        let extensions = MyLanguage::get_file_extensions();
        assert!(extensions.contains(&".ml"));
        assert!(extensions.contains(&".mli"));
    }

    #[test]
    fn test_mylanguage_parsing() {
        let source = r#"
            let add x y = x + y
            
            let test_add () =
              assert_equal 3 (add 1 2)
        "#;
        
        let mut parser = Parser::new();
        let tree = MyLanguage::parse(source, &mut parser);
        assert!(tree.is_some());
    }

    #[test]
    fn test_mylanguage_block_extraction() {
        // Test that blocks are correctly extracted
        // ...
    }

    #[test]
    fn test_mylanguage_test_detection() {
        // Test that test code is correctly identified
        // ...
    }
}
```

## Advanced Language Features

Depending on the complexity of your language, you may need to implement additional features:

### Custom Query Patterns

For complex AST traversal, you can define custom tree-sitter queries:

```rust
fn get_functions_query() -> Query {
    Query::new(
        Self::get_language(),
        "(function_definition name: (identifier) @function_name) @function"
    ).expect("Invalid query")
}
```

### Special Syntax Handling

Some languages have special syntax that requires custom handling:

```rust
fn handle_special_syntax(node: Node, source: &str) -> String {
    // Custom handling for language-specific syntax
    // ...
}
```

### Documentation Extraction

To extract documentation comments:

```rust
fn extract_documentation(node: Node, source: &str) -> Option<String> {
    // Find and extract documentation comments
    // ...
}
```

## Best Practices

When adding support for a new language, follow these best practices:

1. **Study the AST**: Use tools like [tree-sitter playground](https://tree-sitter.github.io/tree-sitter/playground) to understand the AST structure.
2. **Look at Existing Implementations**: Use similar languages as a reference.
3. **Test Thoroughly**: Create comprehensive tests with various code examples.
4. **Handle Edge Cases**: Consider unusual syntax, comments, and language-specific features.
5. **Document Your Implementation**: Add comments explaining language-specific logic.

## Contributing Your Language

Once you've implemented support for a new language:

1. **Write Tests**: Ensure your implementation is well-tested.
2. **Update Documentation**: Add your language to the supported languages list.
3. **Submit a Pull Request**: Contribute your implementation to the Probe project.

For more detailed guidance, check the [CONTRIBUTING.md](https://github.com/buger/probe/blob/main/CONTRIBUTING.md) file in the Probe repository.