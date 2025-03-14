# Adding Support for New Languages

Probe is designed to be extensible, making it relatively straightforward to add support for new programming languages. This guide provides a step-by-step walkthrough of the process.

> For a comprehensive overview of how Probe's language support system works, see the [Language Support Overview](/language-support-overview) page.

## Overview

Adding a new language to Probe involves several steps:

1. Adding the tree-sitter grammar for the language
2. Creating a language module
3. Implementing the LanguageImpl trait
4. Registering the language in the factory
5. Testing the implementation
6. Adding test detection support
7. Documenting the new language

## 1. Adding the Tree-sitter Grammar

First, add the tree-sitter grammar for the new language as a dependency in `Cargo.toml`:

```toml
[dependencies]
# Existing dependencies...
tree-sitter = "0.20.10"
tree-sitter-rust = "0.20.4"
# Add your new language
tree-sitter-mylanguage = "0.1.0"
```

### Finding Existing Tree-sitter Grammars

Many languages already have tree-sitter grammars available. You can find them:

1. On [GitHub](https://github.com/topics/tree-sitter-parser)
2. In the [tree-sitter organization](https://github.com/tree-sitter)
3. By searching for `tree-sitter-[language]` on crates.io

### Creating a New Tree-sitter Grammar

If you need to create a new grammar:

1. Install the tree-sitter CLI: `npm install -g tree-sitter-cli`
2. Generate a new grammar: `tree-sitter init`
3. Define your grammar in `grammar.js`
4. Generate and test your parser: `tree-sitter generate && tree-sitter test`

## 2. Creating a Language Module

Create a new file in the `src/language/` directory for your language:

```rust
// src/language/mylanguage.rs
use crate::language::language_trait::LanguageImpl;
use tree_sitter::{Language as TSLanguage, Node};

pub struct MyLanguage;

impl Default for MyLanguage {
    fn default() -> Self {
        Self::new()
    }
}

impl MyLanguage {
    pub fn new() -> Self {
        MyLanguage
    }
}

// Implementation will go here...
```

Also, update `src/language/mod.rs` to include your new module:

```rust
// Existing modules...
pub mod rust;
pub mod javascript;
// Add your new language
pub mod mylanguage;
```

## 3. Implementing the LanguageImpl Trait

Implement the `LanguageImpl` trait for your new language:

```rust
impl LanguageImpl for MyLanguage {
    fn get_tree_sitter_language(&self) -> TSLanguage {
        // Return the tree-sitter language for your language
        tree_sitter_mylanguage::language()
    }

    fn is_acceptable_parent(&self, node: &Node) -> bool {
        // Determine if a node is an acceptable container/parent entity
        match node.kind() {
            "function_definition" | "class_definition" | "method_definition" => true,
            // Add other relevant node types for your language
            _ => false,
        }
    }

    fn is_test_node(&self, node: &Node, source: &[u8]) -> bool {
        // Determine if a node represents test code
        let node_type = node.kind();
        
        if node_type == "function_definition" {
            // Example: Check if function name contains "test"
            let name_node = node.child_by_field_name("name");
            if let Some(name) = name_node {
                if let Ok(name_text) = name.utf8_text(source) {
                    return name_text.contains("test");
                }
            }
        }
        
        false
    }

    fn get_extension(&self) -> &'static str {
        // Return the primary file extension for your language
        ".ml" // Example for OCaml
    }

    // Optional: Override these methods if needed for your language
    
    fn find_topmost_struct_type<'a>(&self, node: Node<'a>) -> Option<Node<'a>> {
        // Default implementation returns the node itself
        Some(node)
    }

    fn find_parent_function<'a>(&self, _node: Node<'a>) -> Option<Node<'a>> {
        // Default implementation returns None
        None
    }
}
```

### Understanding the AST Structure

To implement `is_acceptable_parent` effectively, you need to understand the AST structure of your language. The tree-sitter playground is an invaluable tool for this:

1. Visit the [tree-sitter playground](https://tree-sitter.github.io/tree-sitter/playground)
2. Select your language
3. Enter some sample code
4. Examine the generated AST

For example, for a simple Python function:

```python
def hello(name):
    return f"Hello, {name}!"
```

The AST might look like:

```
module
  function_definition
    name: identifier
    parameters
      identifier
    block
      return_statement
        string
```

From this, you can determine that `function_definition` is an acceptable parent node type.

### Key Methods to Implement

#### `get_tree_sitter_language()`

Returns the tree-sitter language for your language:

```rust
fn get_tree_sitter_language(&self) -> TSLanguage {
    tree_sitter_mylanguage::language()
}
```

#### `is_acceptable_parent()`

Determines if a node is an acceptable container/parent entity. This is crucial for code block extraction:

```rust
fn is_acceptable_parent(&self, node: &Node) -> bool {
    match node.kind() {
        "function_definition" | "class_definition" | "method_definition" => true,
        // Add other relevant node types for your language
        _ => false,
    }
}
```

Common acceptable parent types include:
- Functions/methods
- Classes/structs/interfaces
- Modules/namespaces
- Type definitions
- Top-level declarations

#### `is_test_node()`

Determines if a node represents test code. This allows Probe to filter out test code when desired:

```rust
fn is_test_node(&self, node: &Node, source: &[u8]) -> bool {
    let node_type = node.kind();
    
    if node_type == "function_definition" {
        // Check for test indicators in the function name
        let name_node = node.child_by_field_name("name");
        if let Some(name) = name_node {
            if let Ok(name_text) = name.utf8_text(source) {
                return name_text.starts_with("test_") || name_text.contains("_test");
            }
        }
    }
    
    false
}
```

#### `get_extension()`

Returns the primary file extension for your language:

```rust
fn get_extension(&self) -> &'static str {
    ".ml" // Example for OCaml
}
```

## 4. Registering the Language in the Factory

Update the language factory in `src/language/factory.rs` to include your new language:

```rust
// Add this to the imports
use crate::language::mylanguage::MyLanguage;

// Add your language to the get_language_impl function
pub fn get_language_impl(extension: &str) -> Option<Box<dyn LanguageImpl>> {
    match extension {
        // Existing languages...
        "rs" => Some(Box::new(RustLanguage::new())),
        "js" | "jsx" => Some(Box::new(JavaScriptLanguage::new())),
        // Add your language
        "ml" | "mli" => Some(Box::new(MyLanguage::new())),
        _ => None,
    }
}
```

Make sure to map all relevant file extensions for your language.

## 5. Testing the Implementation

Create tests for your language implementation:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::language::mylanguage::MyLanguage;
    use crate::language::language_trait::LanguageImpl;
    use tree_sitter::Parser;

    #[test]
    fn test_mylanguage_extension() {
        let lang = MyLanguage::new();
        assert_eq!(lang.get_extension(), ".ml");
    }

    #[test]
    fn test_mylanguage_acceptable_parent() {
        let lang = MyLanguage::new();
        let source = r#"
            function add(x, y) {
                return x + y;
            }
        "#;
        
        let mut parser = Parser::new();
        parser.set_language(lang.get_tree_sitter_language()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root_node = tree.root_node();
        
        // Find a function node and test if it's an acceptable parent
        let function_node = root_node.named_child(0).unwrap();
        assert!(lang.is_acceptable_parent(&function_node));
    }

    #[test]
    fn test_mylanguage_test_node() {
        let lang = MyLanguage::new();
        let source = r#"
            function test_add() {
                assert.equal(add(1, 2), 3);
            }
        "#;
        
        let mut parser = Parser::new();
        parser.set_language(lang.get_tree_sitter_language()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root_node = tree.root_node();
        
        // Find a test function node and test if it's recognized as a test
        let function_node = root_node.named_child(0).unwrap();
        assert!(lang.is_test_node(&function_node, source.as_bytes()));
    }
}
```

### Testing with Real Code

It's important to test your implementation with real code examples:

1. Create sample files in the `tests/mocks/` directory
2. Write integration tests that parse and extract code blocks from these files
3. Verify that the extracted blocks match your expectations

## 6. Adding Test Detection Support

Update the `is_test_file` function in `src/language/test_detection.rs` to include patterns for your language:

```rust
pub fn is_test_file(path: &Path) -> bool {
    // Existing code...
    
    // Check file name patterns
    if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
        // Existing patterns...
        
        // MyLanguage: test_*.ml, *_test.ml
        if file_name.starts_with("test_") && file_name.ends_with(".ml")
            || file_name.ends_with("_test.ml")
        {
            return true;
        }
    }
    
    // Existing code...
}
```

## 7. Documenting the New Language

Update the documentation to include your new language:

1. Add your language to the table in `site/supported-languages.md`
2. Add language-specific features to the "Language-Specific Features" section
3. Add language-specific patterns to the "Pattern Matching" section

## Best Practices

When adding support for a new language, follow these best practices:

1. **Study the AST**: Use tools like [tree-sitter playground](https://tree-sitter.github.io/tree-sitter/playground) to understand the AST structure.
2. **Look at Existing Implementations**: Use similar languages as a reference.
3. **Test Thoroughly**: Create comprehensive tests with various code examples.
4. **Handle Edge Cases**: Consider unusual syntax, comments, and language-specific features.
5. **Document Your Implementation**: Add comments explaining language-specific logic.
6. **Optimize Performance**: Ensure your implementation is efficient, especially for large files.

## Common Challenges and Solutions

### Challenge: Complex AST Structures

Some languages have complex AST structures that make it difficult to identify acceptable parent nodes.

**Solution**: Use the tree-sitter playground to explore the AST and identify patterns. Look for node types that represent meaningful code blocks.

### Challenge: Associating Comments with Code

Comments are often separate nodes in the AST, making it challenging to associate them with the code they document.

**Solution**: Implement custom logic to find the nearest code node to a comment, considering both preceding and following nodes.

### Challenge: Handling Preprocessor Directives

Languages like C/C++ have preprocessor directives that can affect the code structure.

**Solution**: Include relevant preprocessor directives in your `is_acceptable_parent` implementation and handle them specially if needed.

## Example: Adding Support for OCaml

Here's a simplified example of adding support for OCaml:

```rust
// src/language/ocaml.rs
use crate::language::language_trait::LanguageImpl;
use tree_sitter::{Language as TSLanguage, Node};

pub struct OCamlLanguage;

impl Default for OCamlLanguage {
    fn default() -> Self {
        Self::new()
    }
}

impl OCamlLanguage {
    pub fn new() -> Self {
        OCamlLanguage
    }
}

impl LanguageImpl for OCamlLanguage {
    fn get_tree_sitter_language(&self) -> TSLanguage {
        tree_sitter_ocaml::language_ocaml()
    }

    fn get_extension(&self) -> &'static str {
        ".ml"
    }

    fn is_acceptable_parent(&self, node: &Node) -> bool {
        matches!(
            node.kind(),
            "let_binding"
                | "type_definition"
                | "module_definition"
                | "module_type_definition"
                | "class_definition"
                | "method_definition"
                | "external"
        )
    }

    fn is_test_node(&self, node: &Node, source: &[u8]) -> bool {
        let node_type = node.kind();
        
        if node_type == "let_binding" {
            // Check for test function names
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "value_name" {
                    if let Ok(name) = child.utf8_text(source) {
                        return name.starts_with("test_") || name.contains("_test");
                    }
                }
            }
        }
        
        false
    }
}
```

## Contributing Your Language

Once you've implemented support for a new language:

1. **Write Tests**: Ensure your implementation is well-tested.
2. **Update Documentation**: Add your language to the supported languages list.
3. **Submit a Pull Request**: Contribute your implementation to the Probe project.

For more detailed guidance, check the [CONTRIBUTING.md](https://github.com/buger/probe/blob/main/CONTRIBUTING.md) file in the Probe repository.