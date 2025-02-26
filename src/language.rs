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
