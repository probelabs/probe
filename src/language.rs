use anyhow::{Context, Result};
use std::collections::HashSet;
use tree_sitter::{Language, Node, Parser as TSParser};

use crate::models::CodeBlock;

// Function to get the appropriate tree-sitter language based on file extension
pub fn get_language(extension: &str) -> Option<Language> {
    match extension {
        "rs" => Some(tree_sitter_rust::language()),
        "js" | "jsx" => Some(tree_sitter_javascript::language()),
        "ts" => Some(tree_sitter_typescript::language_typescript()),
        "tsx" => Some(tree_sitter_typescript::language_tsx()),
        "py" => Some(tree_sitter_python::language()),
        "go" => Some(tree_sitter_go::language()),
        "c" | "h" => Some(tree_sitter_c::language()),
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => Some(tree_sitter_cpp::language()),
        "java" => Some(tree_sitter_java::language()),
        "rb" => Some(tree_sitter_ruby::language()),
        "php" => Some(tree_sitter_php::language()),
        // Removed Markdown support to fix version conflicts
        _ => None,
    }
}

// Function to determine if a node is an acceptable container/parent entity
pub fn is_acceptable_parent(node: &Node, extension: &str) -> bool {
    let node_type = node.kind();

    match extension {
        "rs" => {
            matches!(
                node_type,
                "function_item"
                    | "struct_item"
                    | "impl_item"
                    | "trait_item"
                    | "enum_item"
                    | "mod_item"
                    | "macro_definition"
            )
        }
        "js" | "jsx" | "ts" | "tsx" => {
            matches!(
                node_type,
                "function_declaration"
                    | "method_definition"
                    | "class_declaration"
                    | "arrow_function"
                    | "function"
                    | "export_statement"
                    | "variable_declaration"
                    | "lexical_declaration"
            )
        }
        "py" => {
            matches!(node_type, "function_definition" | "class_definition")
        }
        "go" => {
            matches!(
                node_type,
                "function_declaration" |
                "method_declaration" |
                "type_declaration" |
                "struct_type" |
                "interface_type" |
                // Added node types for better Go support
                "const_declaration" |
                "var_declaration" |
                "const_spec" |
                "var_spec" |
                "short_var_declaration" |
                "type_spec" // Added for type definitions
            )
        }
        "c" | "h" | "cpp" | "cc" | "cxx" | "hpp" | "hxx" => {
            matches!(
                node_type,
                "function_definition"
                    | "declaration"
                    | "struct_specifier"
                    | "class_specifier"
                    | "enum_specifier"
                    | "namespace_definition"
            )
        }
        "java" => {
            matches!(
                node_type,
                "method_declaration"
                    | "class_declaration"
                    | "interface_declaration"
                    | "enum_declaration"
                    | "constructor_declaration"
            )
        }
        "rb" => {
            matches!(
                node_type,
                "method" | "class" | "module" | "singleton_method"
            )
        }
        "php" => {
            matches!(
                node_type,
                "function_definition"
                    | "method_declaration"
                    | "class_declaration"
                    | "interface_declaration"
                    | "trait_declaration"
            )
        }
        _ => false,
    }
}

// Function to find the closest acceptable parent entity that encompasses a given line
pub fn find_code_structure<'a>(node: Node<'a>, line: usize, extension: &str) -> Option<Node<'a>> {
    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;

    if line < start_line || line > end_line {
        return None;
    }

    let debug_mode = std::env::var("CODE_SEARCH_DEBUG").unwrap_or_default() == "1";
    let target_node = find_most_specific_node(node, line);

    if debug_mode {
        println!(
            "DEBUG: Most specific node for line {}: type='{}', lines={}-{}",
            line,
            target_node.kind(),
            target_node.start_position().row + 1,
            target_node.end_position().row + 1
        );
    }

    // Skip comments explicitly
    if target_node.kind() == "comment" {
        if debug_mode {
            println!("DEBUG: Skipping comment node at line {}", line);
        }
        return None;
    }

    // First check if the target node itself is an acceptable parent
    if is_acceptable_parent(&target_node, extension) {
        if debug_mode {
            println!(
                "DEBUG: Target node is an acceptable parent: type='{}', lines={}-{}",
                target_node.kind(),
                target_node.start_position().row + 1,
                target_node.end_position().row + 1
            );
        }
        return Some(target_node);
    }

    // Traverse up to find the closest acceptable parent
    let mut current_node = target_node;
    while let Some(parent) = current_node.parent() {
        if is_acceptable_parent(&parent, extension) {
            if debug_mode {
                println!(
                    "DEBUG: Found acceptable parent: type='{}', lines={}-{}",
                    parent.kind(),
                    parent.start_position().row + 1,
                    parent.end_position().row + 1
                );
            }
            return Some(parent);
        }
        current_node = parent;
    }

    if debug_mode {
        println!("DEBUG: No acceptable parent found for line {}", line);
    }
    None // Fallback to line-based context if no parent found
}

// Helper function to find the most specific node that contains a given line
fn find_most_specific_node<'a>(node: Node<'a>, line: usize) -> Node<'a> {
    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;

    // Check if the node contains the line
    if line < start_line || line > end_line {
        return node;
    }

    // Check children for a more specific match
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let child_start = child.start_position().row + 1;
        let child_end = child.end_position().row + 1;

        if line >= child_start && line <= child_end {
            // Recursively check this child
            return find_most_specific_node(child, line);
        }
    }

    // If no child contains the line, this is the most specific node
    node
}

// Helper function to collect all node types in the AST
fn collect_node_types(node: Node, node_types: &mut HashSet<String>) {
    node_types.insert(node.kind().to_string());

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_node_types(child, node_types);
    }
}

// Function to parse a file and extract code blocks for the given line numbers
pub fn parse_file_for_code_blocks(
    content: &str,
    extension: &str,
    line_numbers: &HashSet<usize>,
    allow_tests: bool,
) -> Result<Vec<CodeBlock>> {
    // Get the appropriate language
    let language =
        get_language(extension).context(format!("Unsupported file type: {}", extension))?;

    // Parse the file
    let mut parser = TSParser::new();
    parser.set_language(language)?;

    let tree = parser
        .parse(content, None)
        .context("Failed to parse the file")?;

    let root_node = tree.root_node();

    // Check for debug mode
    let debug_mode = std::env::var("CODE_SEARCH_DEBUG").unwrap_or_default() == "1";

    if debug_mode {
        println!("DEBUG: Parsing file with extension: {}", extension);
        println!("DEBUG: Root node type: {}", root_node.kind());

        // Log all node types in the file
        let mut node_types = HashSet::new();
        collect_node_types(root_node, &mut node_types);
        println!("DEBUG: All node types in file: {:?}", node_types);
    }

    // Collect all code blocks
    let mut code_blocks: Vec<CodeBlock> = Vec::new();
    let mut seen_nodes: HashSet<(usize, usize)> = HashSet::new();

    // Process each line number
    for &line in line_numbers {
        if let Some(node) = find_code_structure(root_node, line, extension) {
            let start_pos = node.start_position();
            let end_pos = node.end_position();

            // Skip if we've already seen this node
            let node_key = (start_pos.row, end_pos.row);
            if seen_nodes.contains(&node_key) {
                continue;
            }

            seen_nodes.insert(node_key);
            
            // Skip test nodes unless allow_tests is true
            if !allow_tests && is_test_node(&node, extension, content.as_bytes()) {
                if debug_mode {
                    println!(
                        "DEBUG: Skipping test node at line {}, type: {}",
                        line,
                        node.kind()
                    );
                }
                continue;
            }

            if debug_mode {
                println!(
                    "DEBUG: Match at line {}, found node type: {}",
                    line,
                    node.kind()
                );
                println!(
                    "DEBUG: Node spans lines {}-{}",
                    start_pos.row + 1,
                    end_pos.row + 1
                );
            }

            // Ensure we never have an empty node_type
            let node_kind = node.kind();
            let node_type = if node_kind.is_empty() {
                // Fallback for empty node types
                if debug_mode {
                    println!("DEBUG: Empty node type detected, using fallback");
                }
                "unknown_node".to_string()
            } else {
                node_kind.to_string()
            };

            code_blocks.push(CodeBlock {
                start_row: start_pos.row,
                end_row: end_pos.row,
                start_byte: node.start_byte(),
                end_byte: node.end_byte(),
                node_type,
            });
        } else if debug_mode {
            println!("DEBUG: No node found for line {}", line);
        }
    }

    // Sort code blocks by start position
    code_blocks.sort_by_key(|block| block.start_row);

    Ok(code_blocks)
}

// Function to merge overlapping code blocks
pub fn merge_code_blocks(code_blocks: Vec<CodeBlock>) -> Vec<CodeBlock> {
    let mut merged_blocks: Vec<CodeBlock> = Vec::new();
    let debug_mode = std::env::var("CODE_SEARCH_DEBUG").unwrap_or_default() == "1";

    for block in code_blocks {
        if let Some(last) = merged_blocks.last_mut() {
            let threshold = if block.node_type == "struct_type" || last.node_type == "struct_type" {
                50 // Larger threshold for structs
            } else {
                10
            };

            if block.start_row <= last.end_row + threshold
                || (block.node_type == last.node_type && block.node_type == "struct_type")
            {
                if debug_mode {
                    println!(
                        "DEBUG: Merging blocks: {} ({}-{}) with {} ({}-{})",
                        last.node_type,
                        last.start_row + 1,
                        last.end_row + 1,
                        block.node_type,
                        block.start_row + 1,
                        block.end_row + 1
                    );
                }
                last.end_row = last.end_row.max(block.end_row);
                last.end_byte = last.end_byte.max(block.end_byte);
                last.start_row = last.start_row.min(block.start_row);
                last.start_byte = last.start_byte.min(block.start_byte);
                continue;
            }
        }
        merged_blocks.push(block);
    }

    if debug_mode {
        println!("DEBUG: After merging: {} blocks", merged_blocks.len());
        for (i, block) in merged_blocks.iter().enumerate() {
            println!(
                "DEBUG:   Block {}: type={}, lines={}-{}",
                i + 1,
                block.node_type,
                block.start_row + 1,
                block.end_row + 1
            );
        }
    }
    merged_blocks
}

// Function to determine if a file is a test file based on common naming conventions and directory patterns
pub fn is_test_file(path: &std::path::Path) -> bool {
    let debug_mode = std::env::var("CODE_SEARCH_DEBUG").unwrap_or_default() == "1";
    
    // Check file name patterns
    if let Some(file_name) = path.file_name().and_then(|f| f.to_str()) {
        // Rust: *_test.rs, *_tests.rs, test_*.rs, tests.rs
        if file_name.ends_with("_test.rs") || file_name.ends_with("_tests.rs") || 
           file_name.starts_with("test_") || file_name == "tests.rs" {
            if debug_mode {
                println!("DEBUG: Test file detected (Rust): {}", file_name);
            }
            return true;
        }
        
        // JavaScript/TypeScript: *.spec.js, *.test.js, *.spec.ts, *.test.ts
        if file_name.ends_with(".spec.js") || file_name.ends_with(".test.js") ||
           file_name.ends_with(".spec.ts") || file_name.ends_with(".test.ts") ||
           file_name.ends_with(".spec.jsx") || file_name.ends_with(".test.jsx") ||
           file_name.ends_with(".spec.tsx") || file_name.ends_with(".test.tsx") {
            if debug_mode {
                println!("DEBUG: Test file detected (JS/TS): {}", file_name);
            }
            return true;
        }
        
        // Python: test_*.py
        if file_name.starts_with("test_") && file_name.ends_with(".py") {
            if debug_mode {
                println!("DEBUG: Test file detected (Python): {}", file_name);
            }
            return true;
        }
        
        // Go: *_test.go
        if file_name.ends_with("_test.go") {
            if debug_mode {
                println!("DEBUG: Test file detected (Go): {}", file_name);
            }
            return true;
        }
        
        // C/C++: test_*.c, test_*.cpp, *_test.c, *_test.cpp
        if (file_name.starts_with("test_") || file_name.ends_with("_test.c") || 
            file_name.ends_with("_test.cpp") || file_name.ends_with("_test.cc") || 
            file_name.ends_with("_test.cxx")) {
            if debug_mode {
                println!("DEBUG: Test file detected (C/C++): {}", file_name);
            }
            return true;
        }
        
        // Java: *Test.java
        if file_name.ends_with("Test.java") {
            if debug_mode {
                println!("DEBUG: Test file detected (Java): {}", file_name);
            }
            return true;
        }
        
        // Ruby: *_test.rb, test_*.rb, *_spec.rb
        if file_name.ends_with("_test.rb") || file_name.starts_with("test_") && file_name.ends_with(".rb") || 
           file_name.ends_with("_spec.rb") {
            if debug_mode {
                println!("DEBUG: Test file detected (Ruby): {}", file_name);
            }
            return true;
        }
        
        // PHP: *Test.php, test_*.php
        if file_name.ends_with("Test.php") || (file_name.starts_with("test_") && file_name.ends_with(".php")) {
            if debug_mode {
                println!("DEBUG: Test file detected (PHP): {}", file_name);
            }
            return true;
        }
    }
    
    // Check directory patterns
    let path_str = path.to_string_lossy();
    
    // Common test directories across languages
    if path_str.contains("/tests/") || path_str.contains("/test/") || 
       path_str.contains("/__tests__/") || path_str.contains("/__test__/") ||
       path_str.contains("/spec/") || path_str.contains("/specs/") {
        if debug_mode {
            println!("DEBUG: Test file detected (directory pattern): {}", path_str);
        }
        return true;
    }
    
    // Check for files in a tests directory at the root level
    if let Some(parent) = path.parent() {
        if let Some(dir_name) = parent.file_name().and_then(|d| d.to_str()) {
            if dir_name == "tests" {
                if debug_mode {
                    println!("DEBUG: Test file detected (in tests directory): {}", path_str);
                }
                return true;
            }
        }
    }
    
    false
}

// Function to identify test code blocks in the AST using tree-sitter nodes
pub fn is_test_node(node: &Node, extension: &str, source: &[u8]) -> bool {
    let debug_mode = std::env::var("CODE_SEARCH_DEBUG").unwrap_or_default() == "1";
    let node_type = node.kind();
    
    match extension {
        "rs" => {
            // Rust: Check for #[test] attribute or test_ prefix on function_item nodes
            if node_type == "function_item" {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "attribute" {
                        let attr_text = child.utf8_text(source).unwrap_or("");
                        if attr_text.contains("#[test]") || attr_text.contains("#[cfg(test)]") {
                            if debug_mode {
                                println!("DEBUG: Test node detected (Rust): #[test] or #[cfg(test)] function");
                            }
                            return true;
                        }
                    } else if child.kind() == "identifier" {
                        let name = child.utf8_text(source).unwrap_or("");
                        if name.starts_with("test_") {
                            if debug_mode {
                                println!("DEBUG: Test node detected (Rust): test_ function");
                            }
                            return true;
                        }
                    }
                }
            } else if node_type == "mod_item" {
                // Check for test modules
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "identifier" {
                        let name = child.utf8_text(source).unwrap_or("");
                        if name == "tests" {
                            if debug_mode {
                                println!("DEBUG: Test node detected (Rust): tests module");
                            }
                            return true;
                        }
                    } else if child.kind() == "attribute" {
                        let attr_text = child.utf8_text(source).unwrap_or("");
                        if attr_text.contains("#[cfg(test)]") {
                            if debug_mode {
                                println!("DEBUG: Test node detected (Rust): #[cfg(test)] module");
                            }
                            return true;
                        }
                    }
                }
            }
        }
        "js" | "jsx" | "ts" | "tsx" => {
            // JavaScript/TypeScript: Check call_expression nodes with describe, test, or it identifiers
            if node_type == "call_expression" {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "identifier" {
                        let name = child.utf8_text(source).unwrap_or("");
                        if name == "describe" || name == "test" || name == "it" || name == "suite" || 
                           name == "context" || name == "expect" {
                            if debug_mode {
                                println!("DEBUG: Test node detected (JS/TS): {} function", name);
                            }
                            return true;
                        }
                    }
                }
            }
        }
        "py" => {
            // Python: Check function_definition nodes with names starting with test_
            if node_type == "function_definition" {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "identifier" {
                        let name = child.utf8_text(source).unwrap_or("");
                        if name.starts_with("test_") {
                            if debug_mode {
                                println!("DEBUG: Test node detected (Python): test_ function");
                            }
                            return true;
                        }
                    }
                }
            }
        }
        "go" => {
            // Go: Check function_declaration nodes with names starting with Test
            if node_type == "function_declaration" {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "identifier" {
                        let name = child.utf8_text(source).unwrap_or("");
                        if name.starts_with("Test") {
                            if debug_mode {
                                println!("DEBUG: Test node detected (Go): Test function");
                            }
                            return true;
                        }
                    }
                }
            }
        }
        "c" | "h" | "cpp" | "cc" | "cxx" | "hpp" | "hxx" => {
            // C/C++: Check function_definition nodes with test in the name
            if node_type == "function_definition" {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "function_declarator" {
                        let mut subcursor = child.walk();
                        for subchild in child.children(&mut subcursor) {
                            if subchild.kind() == "identifier" {
                                let name = subchild.utf8_text(source).unwrap_or("");
                                if name.contains("test") || name.contains("Test") {
                                    if debug_mode {
                                        println!("DEBUG: Test node detected (C/C++): test function");
                                    }
                                    return true;
                                }
                            }
                        }
                    }
                }
            }
        }
        "java" => {
            // Java: Check method_declaration nodes with @Test annotation
            if node_type == "method_declaration" {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "modifiers" {
                        let mut subcursor = child.walk();
                        for annotation in child.children(&mut subcursor) {
                            if annotation.kind() == "annotation" {
                                let annotation_text = annotation.utf8_text(source).unwrap_or("");
                                if annotation_text.contains("@Test") {
                                    if debug_mode {
                                        println!("DEBUG: Test node detected (Java): @Test method");
                                    }
                                    return true;
                                }
                            }
                        }
                    }
                }
            }
        }
        "rb" => {
            // Ruby: Check method nodes with test_ prefix or describe/it blocks
            if node_type == "method" {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "identifier" {
                        let name = child.utf8_text(source).unwrap_or("");
                        if name.starts_with("test_") {
                            if debug_mode {
                                println!("DEBUG: Test node detected (Ruby): test_ method");
                            }
                            return true;
                        }
                    }
                }
            } else if node_type == "call" {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "identifier" {
                        let name = child.utf8_text(source).unwrap_or("");
                        if name == "describe" || name == "it" || name == "context" || name == "specify" {
                            if debug_mode {
                                println!("DEBUG: Test node detected (Ruby): {} block", name);
                            }
                            return true;
                        }
                    }
                }
            }
        }
        "php" => {
            // PHP: Check method_declaration nodes with test prefix or PHPUnit annotations
            if node_type == "method_declaration" {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "name" {
                        let name = child.utf8_text(source).unwrap_or("");
                        if name.starts_with("test") {
                            if debug_mode {
                                println!("DEBUG: Test node detected (PHP): test method");
                            }
                            return true;
                        }
                    } else if child.kind() == "comment" {
                        let comment = child.utf8_text(source).unwrap_or("");
                        if comment.contains("@test") {
                            if debug_mode {
                                println!("DEBUG: Test node detected (PHP): @test annotation");
                            }
                            return true;
                        }
                    }
                }
            }
        }
        _ => {}
    }
    
    false
}
