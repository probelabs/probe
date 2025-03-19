use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use tree_sitter::{Node, Parser as TSParser};

use crate::language::common::find_most_specific_node;
use crate::language::factory::get_language_impl;
use crate::language::language_trait::LanguageImpl;
use crate::language::tree_cache;
use crate::models::CodeBlock;

/// Function to find the closest acceptable parent entity that encompasses a given line.
/// When a comment is encountered, it attempts to find the next related code node.
pub fn find_code_structure<'a>(node: Node<'a>, line: usize, extension: &str) -> Option<Node<'a>> {
    let start_line = node.start_position().row + 1;
    let end_line = node.end_position().row + 1;

    if line < start_line || line > end_line {
        return None;
    }

    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";
    let target_node = find_most_specific_node(node, line);

    // Check if this is a comment node
    if target_node.kind() == "comment"
        || target_node.kind() == "line_comment"
        || target_node.kind() == "block_comment"
        || target_node.kind() == "//"
    // Add support for comment token in new tree-sitter
    {
        if debug_mode {
            println!(
                "DEBUG: Found comment node at line {}, looking for related code node",
                line
            );
        }

        // Get the language implementation for this extension
        let language_impl = get_language_impl(extension)?;

        // Try to find related code node using AST traversal
        let mut found_node = None;

        // First check next siblings and their subtrees
        if let Some(next_sibling) = target_node.next_sibling() {
            if language_impl.is_acceptable_parent(&next_sibling) {
                found_node = Some(next_sibling);
            } else {
                // Look in next sibling's subtree
                found_node = find_acceptable_child(next_sibling, language_impl.as_ref());
            }
        }

        // If no next sibling found, check previous siblings
        if found_node.is_none() {
            if let Some(prev_sibling) = find_prev_sibling(target_node) {
                if language_impl.is_acceptable_parent(&prev_sibling) {
                    found_node = Some(prev_sibling);
                } else {
                    // Look in previous sibling's subtree
                    found_node = find_acceptable_child(prev_sibling, language_impl.as_ref());
                }
            }
        }

        // If we found a sibling node, return it
        if let Some(node) = found_node {
            if debug_mode {
                println!(
                    "DEBUG: Found acceptable sibling node: type='{}', lines={}-{}",
                    node.kind(),
                    node.start_position().row + 1,
                    node.end_position().row + 1
                );
            }
            return Some(node);
        }

        // If no siblings are acceptable, check if we're nested in an acceptable parent
        let mut current = target_node;
        while let Some(parent) = current.parent() {
            if language_impl.is_acceptable_parent(&parent) {
                if debug_mode {
                    println!(
                        "DEBUG: Found enclosing acceptable parent: type='{}', lines={}-{}",
                        parent.kind(),
                        parent.start_position().row + 1,
                        parent.end_position().row + 1
                    );
                }
                return Some(parent);
            }
            current = parent;
        }

        if debug_mode {
            println!("DEBUG: No related node found for comment at line {}", line);
        }
        return None;
    }

    // Get the language implementation for this extension
    let language_impl = get_language_impl(extension)?;

    // First check if the target node itself is an acceptable parent
    if language_impl.is_acceptable_parent(&target_node) {
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
        if language_impl.is_acceptable_parent(&parent) {
            if debug_mode {
                println!(
                    "DEBUG: Found acceptable parent: type='{}', lines={}-{}",
                    parent.kind(),
                    parent.start_position().row + 1,
                    parent.end_position().row + 1
                );
            }

            // Special case for struct_type in Go
            if parent.kind() == "struct_type" && extension == "go" {
                // Use the language-specific helper to find the topmost struct_type
                if let Some(top_struct) = language_impl.find_topmost_struct_type(parent) {
                    if debug_mode {
                        println!(
                            "DEBUG: Found nested struct_type chain, using topmost parent: type='{}', lines={}-{}",
                            top_struct.kind(),
                            top_struct.start_position().row + 1,
                            top_struct.end_position().row + 1
                        );
                    }
                    return Some(top_struct);
                }
            }

            return Some(parent);
        }
        current_node = parent;
    }

    if debug_mode {
        println!(
            "DEBUG: No acceptable parent found for line {}, trying additional strategies",
            line
        );
        println!("DEBUG: Target node type: {}", target_node.kind());
    }

    // Additional strategy: If the target node is a doc_comment, try to find the next sibling
    // which might be the struct_item or function_item we're looking for
    if target_node.kind() == "doc_comment" || target_node.kind() == "line_comment" {
        if let Some(next_sibling) = target_node.next_sibling() {
            if debug_mode {
                println!(
                    "DEBUG: Found next sibling after comment: type='{}', lines={}-{}",
                    next_sibling.kind(),
                    next_sibling.start_position().row + 1,
                    next_sibling.end_position().row + 1
                );
            }

            // Check if the next sibling is an acceptable parent
            if language_impl.is_acceptable_parent(&next_sibling) {
                if debug_mode {
                    println!("DEBUG: Next sibling is an acceptable parent, using it");
                }
                return Some(next_sibling);
            }

            // If not, try to find an acceptable parent in the next sibling's children
            if let Some(child) = find_acceptable_child(next_sibling, language_impl.as_ref()) {
                if debug_mode {
                    println!(
                        "DEBUG: Found acceptable child in next sibling: type='{}', lines={}-{}",
                        child.kind(),
                        child.start_position().row + 1,
                        child.end_position().row + 1
                    );
                }
                return Some(child);
            }
        }
    }

    // FINAL FALLBACK: If we never found an "acceptable parent,"
    // just return the original node anyway instead of returning None
    if debug_mode {
        println!("DEBUG: All strategies failed, falling back to target node");
    }
    Some(target_node)
}

/// Gets the context for a comment node, which can be either:
/// 1. An acceptable parent node if the comment is inside a code block
/// 2. The next acceptable node if the comment is at the root level
pub fn get_comment_context<'a>(comment_node: Node<'a>, extension: &str) -> Option<Node<'a>> {
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    // Get the language implementation for this extension
    let language_impl = get_language_impl(extension)?;

    if debug_mode {
        println!(
            "DEBUG: Finding context for comment at lines {}-{}: {}",
            comment_node.start_position().row + 1,
            comment_node.end_position().row + 1,
            comment_node.kind()
        );
    }

    // Priority 1: Check if comment has an acceptable parent
    let mut current = comment_node;
    while let Some(parent) = current.parent() {
        if language_impl.is_acceptable_parent(&parent) {
            if debug_mode {
                println!(
                    "DEBUG: Found enclosing acceptable parent: type='{}', lines={}-{}",
                    parent.kind(),
                    parent.start_position().row + 1,
                    parent.end_position().row + 1
                );
            }
            return Some(parent);
        }
        current = parent;
    }

    // Priority 2: If no acceptable parent, look for next acceptable node
    find_related_code_node(comment_node, extension)
}

/// Finds the immediate next node that follows a given node in the AST
fn find_immediate_next_node(node: Node<'_>) -> Option<Node<'_>> {
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    // First try direct next sibling
    if let Some(next) = node.next_sibling() {
        if debug_mode {
            println!(
                "DEBUG: Found immediate next sibling: type='{}', lines={}-{}",
                next.kind(),
                next.start_position().row + 1,
                next.end_position().row + 1
            );
        }
        return Some(next);
    }

    // If no direct sibling, check parent's next sibling
    if let Some(parent) = node.parent() {
        if let Some(next_parent) = parent.next_sibling() {
            if debug_mode {
                println!(
                    "DEBUG: Found parent's next sibling: type='{}', lines={}-{}",
                    next_parent.kind(),
                    next_parent.start_position().row + 1,
                    next_parent.end_position().row + 1
                );
            }
            return Some(next_parent);
        }
    }

    if debug_mode {
        println!("DEBUG: No immediate next node found");
    }
    None
}

pub fn find_related_code_node<'a>(comment_node: Node<'a>, extension: &str) -> Option<Node<'a>> {
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    // Get the language implementation for this extension
    let language_impl = get_language_impl(extension)?;

    if debug_mode {
        println!(
            "DEBUG: Finding related node for comment at lines {}-{}: {}",
            comment_node.start_position().row + 1,
            comment_node.end_position().row + 1,
            comment_node.kind()
        );
    }

    // Priority 1: Check immediate next node
    if let Some(next_node) = find_immediate_next_node(comment_node) {
        if language_impl.is_acceptable_parent(&next_node) {
            if debug_mode {
                println!(
                    "DEBUG: Using immediate next acceptable node: type='{}', lines={}-{}",
                    next_node.kind(),
                    next_node.start_position().row + 1,
                    next_node.end_position().row + 1
                );
            }
            return Some(next_node);
        }
    }

    // Priority 2: Look for acceptable child in the next node
    if let Some(next_node) = find_immediate_next_node(comment_node) {
        if let Some(child) = find_acceptable_child(next_node, language_impl.as_ref()) {
            if debug_mode {
                println!(
                    "DEBUG: Found acceptable child in next node: type='{}', lines={}-{}",
                    child.kind(),
                    child.start_position().row + 1,
                    child.end_position().row + 1
                );
            }
            return Some(child);
        }
    }

    // Priority 3: Check previous siblings (fallback for when comments follow the code they document)
    if let Some(prev_sibling) = comment_node.prev_sibling() {
        if language_impl.is_acceptable_parent(&prev_sibling) {
            if debug_mode {
                println!(
                    "DEBUG: Using previous sibling node: type='{}', lines={}-{}",
                    prev_sibling.kind(),
                    prev_sibling.start_position().row + 1,
                    prev_sibling.end_position().row + 1
                );
            }
            return Some(prev_sibling);
        }

        // Look in previous sibling's subtree
        if let Some(child) = find_acceptable_child(prev_sibling, language_impl.as_ref()) {
            if debug_mode {
                println!(
                    "DEBUG: Found acceptable child in previous sibling: type='{}', lines={}-{}",
                    child.kind(),
                    child.start_position().row + 1,
                    child.end_position().row + 1
                );
            }
            return Some(child);
        }
    }

    // Priority 4: Check parent chain
    let mut current = comment_node;
    while let Some(parent) = current.parent() {
        if language_impl.is_acceptable_parent(&parent) {
            if debug_mode {
                println!(
                    "DEBUG: Found enclosing acceptable parent: type='{}', lines={}-{}",
                    parent.kind(),
                    parent.start_position().row + 1,
                    parent.end_position().row + 1
                );
            }
            return Some(parent);
        }
        current = parent;
    }

    if debug_mode {
        println!("DEBUG: No related node found for the comment");
    }
    None
}

/// Gets the previous sibling of a node in the AST
fn find_prev_sibling(node: Node<'_>) -> Option<Node<'_>> {
    let parent = node.parent()?;

    let mut cursor = parent.walk();
    let mut prev_child = None;

    for child in parent.children(&mut cursor) {
        if child.id() == node.id() {
            return prev_child;
        }
        prev_child = Some(child);
    }

    None // No previous sibling found
}

/// Find first acceptable node in a subtree
fn find_acceptable_child<'a>(node: Node<'a>, language_impl: &dyn LanguageImpl) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if language_impl.is_acceptable_parent(&child) {
            return Some(child);
        }

        // Recursive search
        if let Some(acceptable) = find_acceptable_child(child, language_impl) {
            return Some(acceptable);
        }
    }

    None // No acceptable child found
}

/// Structure to hold node information for a specific line
#[derive(Clone, Copy)]
struct NodeInfo<'a> {
    node: Node<'a>,
    is_comment: bool,
    context_node: Option<Node<'a>>,
    is_test: bool,
}

/// Function to parse a file and extract code blocks for the given line numbers
pub fn parse_file_for_code_blocks(
    content: &str,
    extension: &str,
    line_numbers: &HashSet<usize>,
    allow_tests: bool,
    _term_matches: Option<&HashMap<usize, HashSet<usize>>>, // Query index to line numbers
) -> Result<Vec<CodeBlock>> {
    // Get the appropriate language implementation
    let language_impl = match get_language_impl(extension) {
        Some(lang) => lang,
        None => {
            return Err(anyhow::anyhow!(format!(
                "Unsupported file type: {}",
                extension
            )))
        }
    };

    // Get the tree-sitter language
    let language = language_impl.get_tree_sitter_language();

    // Parse the file
    let mut parser = TSParser::new();
    parser.set_language(&language)?;

    // Use the tree cache to get or parse the tree
    // We use a stable identifier for the file
    let cache_key = format!("file_{}", extension);
    let tree = tree_cache::get_or_parse_tree(&cache_key, content, &mut parser)
        .context("Failed to parse the file")?;

    let root_node = tree.root_node();

    // Check for debug mode
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    if debug_mode {
        println!("DEBUG: Parsing file with extension: {}", extension);
        println!("DEBUG: Root node type: {}", root_node.kind());

        // Log all node types in the file
        let mut node_types = HashSet::new();
        super::common::collect_node_types(root_node, &mut node_types);
        println!("DEBUG: All node types in file: {:?}", node_types);
    }

    // Create a line-to-node map for the entire file
    let line_count = content.lines().count();
    let mut line_map: Vec<Option<NodeInfo>> = vec![None; line_count];

    // Build the line-to-node map with a single traversal
    if debug_mode {
        println!("DEBUG: Building line-to-node map with a single traversal");
    }

    // Helper function to process a node and its children
    fn process_node<'a>(
        node: Node<'a>,
        line_map: &mut Vec<Option<NodeInfo<'a>>>,
        extension: &str,
        language_impl: &dyn LanguageImpl,
        content: &[u8],
        allow_tests: bool,
        _debug_mode: bool,
    ) {
        let start_row = node.start_position().row;
        let end_row = node.end_position().row;

        // Skip nodes that are outside the file bounds
        if start_row >= line_map.len() {
            return;
        }

        let is_comment = node.kind() == "comment"
            || node.kind() == "line_comment"
            || node.kind() == "block_comment"
            || node.kind() == "//";

        let is_test = !allow_tests && language_impl.is_test_node(&node, content);

        // Get context node for comments
        let context_node = if is_comment {
            get_comment_context(node, extension)
        } else {
            None
        };

        // Update the line map for each line covered by this node
        for line in start_row..=end_row {
            if line >= line_map.len() {
                break;
            }

            // Only update if this node is more specific (smaller) than the current one
            let should_update = match &line_map[line] {
                None => true,
                Some(current) => {
                    let current_size = current.node.end_byte() - current.node.start_byte();
                    let this_size = node.end_byte() - node.start_byte();
                    this_size < current_size
                }
            };

            if should_update {
                line_map[line] = Some(NodeInfo {
                    node,
                    is_comment,
                    context_node,
                    is_test,
                });
            }
        }

        // Process children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            process_node(
                child,
                line_map,
                extension,
                language_impl,
                content,
                allow_tests,
                _debug_mode,
            );
        }
    }

    // For large files, we could parallelize the processing, but due to thread-safety
    // constraints with the language implementation, we'll use a sequential approach
    // that's still efficient for most cases
    if debug_mode {
        println!("DEBUG: Using sequential processing for AST nodes");
    }

    // Start the traversal from the root node
    process_node(
        root_node,
        &mut line_map,
        extension,
        language_impl.as_ref(),
        content.as_bytes(),
        allow_tests,
        debug_mode,
    );

    if debug_mode {
        println!("DEBUG: Line-to-node map built successfully");
    }

    // Collect all code blocks
    let mut code_blocks: Vec<CodeBlock> = Vec::new();
    let mut seen_nodes: HashSet<(usize, usize)> = HashSet::new();

    // Process each line number using the precomputed map
    for &line in line_numbers {
        // Adjust for 0-based indexing
        let line_idx = line - 1;

        // Skip if line is out of bounds
        if line_idx >= line_map.len() {
            if debug_mode {
                println!("DEBUG: Line {} is out of bounds", line);
            }
            continue;
        }

        // Get the node info for this line
        if let Some(info) = &line_map[line_idx] {
            let target_node = info.node;
            let start_pos = target_node.start_position();
            let end_pos = target_node.end_position();
            let node_key = (start_pos.row, end_pos.row);

            // Skip if we've already processed this node
            if seen_nodes.contains(&node_key) {
                if debug_mode {
                    println!(
                        "DEBUG: Already processed node at lines {}-{}, type: {}",
                        start_pos.row + 1,
                        end_pos.row + 1,
                        target_node.kind()
                    );
                }
                continue;
            }

            // Mark this node as seen
            seen_nodes.insert(node_key);

            // Special handling for comments
            if info.is_comment {
                if debug_mode {
                    println!(
                        "DEBUG: Found comment node at line {}: {}",
                        line,
                        target_node.kind()
                    );
                }

                // If we have a context node for this comment
                if let Some(context_node) = info.context_node {
                    let rel_start_pos = context_node.start_position();
                    let rel_end_pos = context_node.end_position();
                    let rel_key = (rel_start_pos.row, rel_end_pos.row);

                    // Skip test nodes unless allow_tests is true
                    if !allow_tests && language_impl.is_test_node(&context_node, content.as_bytes())
                    {
                        if debug_mode {
                            println!(
                                "DEBUG: Skipping test node at lines {}-{}, type: {}",
                                rel_start_pos.row + 1,
                                rel_end_pos.row + 1,
                                context_node.kind()
                            );
                        }
                    } else {
                        // Create a merged block that includes both the comment and its context
                        let merged_start_row = std::cmp::min(start_pos.row, rel_start_pos.row);
                        let merged_end_row = std::cmp::max(end_pos.row, rel_end_pos.row);
                        let merged_start_byte =
                            std::cmp::min(target_node.start_byte(), context_node.start_byte());
                        let merged_end_byte =
                            std::cmp::max(target_node.end_byte(), context_node.end_byte());

                        // Use the context node's type as the merged block's type
                        let merged_node_type = context_node.kind().to_string();

                        // Mark both the comment and context as seen
                        seen_nodes.insert(rel_key);

                        // Add the merged block
                        code_blocks.push(CodeBlock {
                            start_row: merged_start_row,
                            end_row: merged_end_row,
                            start_byte: merged_start_byte,
                            end_byte: merged_end_byte,
                            node_type: merged_node_type.clone(),
                            parent_node_type: None,
                            parent_start_row: None,
                            parent_end_row: None,
                        });

                        if debug_mode {
                            println!(
                                "DEBUG: Added merged block (comment + context) at lines {}-{}, type: {}",
                                merged_start_row + 1,
                                merged_end_row + 1,
                                merged_node_type
                            );
                        }

                        continue;
                    }
                }

                // If we didn't add the comment as part of a merged block, add it individually
                code_blocks.push(CodeBlock {
                    start_row: start_pos.row,
                    end_row: end_pos.row,
                    start_byte: target_node.start_byte(),
                    end_byte: target_node.end_byte(),
                    node_type: target_node.kind().to_string(),
                    parent_node_type: None,
                    parent_start_row: None,
                    parent_end_row: None,
                });

                if debug_mode {
                    println!(
                        "DEBUG: Added comment block at lines {}-{}, type: {}",
                        start_pos.row + 1,
                        end_pos.row + 1,
                        target_node.kind()
                    );
                }
                continue;
            }

            // Skip test nodes unless allow_tests is true
            if info.is_test {
                if debug_mode {
                    println!(
                        "DEBUG: Skipping test node at line {}, type: {}",
                        line,
                        target_node.kind()
                    );
                }
                continue;
            }

            // For non-comments, first check if this line is within any existing block
            let mut existing_block = false;
            for block in &code_blocks {
                if line > block.start_row + 1 && line <= block.end_row + 1 {
                    if debug_mode {
                        println!(
                            "DEBUG: Line {} is within existing block: type='{}', lines={}-{}",
                            line,
                            block.node_type,
                            block.start_row + 1,
                            block.end_row + 1
                        );
                    }
                    existing_block = true;
                    break;
                }
            }

            if existing_block {
                continue;
            }

            // Special case for line-based extraction: check if there's a node that starts exactly at this line
            let exact_line_match = if target_node.start_position().row + 1 == line
                && language_impl.is_acceptable_parent(&target_node)
            {
                Some(target_node)
            } else {
                // Try to find a node that starts exactly at the target line
                fn find_node_at_exact_line<'a>(
                    node: Node<'a>,
                    line: usize,
                    debug_mode: bool,
                    language_impl: &dyn LanguageImpl,
                ) -> Option<Node<'a>> {
                    let start_line = node.start_position().row + 1;

                    // Check if this node starts exactly at the target line
                    if start_line == line && language_impl.is_acceptable_parent(&node) {
                        if debug_mode {
                            println!(
                                "DEBUG: Found node that starts exactly at line {}: type='{}', lines={}-{}",
                                line,
                                node.kind(),
                                start_line,
                                node.end_position().row + 1
                            );
                        }
                        return Some(node);
                    }

                    // Recursively check children
                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        if let Some(found) =
                            find_node_at_exact_line(child, line, debug_mode, language_impl)
                        {
                            return Some(found);
                        }
                    }

                    None
                }

                find_node_at_exact_line(root_node, line, debug_mode, language_impl.as_ref())
            };

            // If we found an exact match, use it
            if let Some(exact_match) = exact_line_match {
                let start_pos = exact_match.start_position();
                let end_pos = exact_match.end_position();
                let node_key = (start_pos.row, end_pos.row);

                if !seen_nodes.contains(&node_key) {
                    seen_nodes.insert(node_key);

                    if debug_mode {
                        println!(
                            "DEBUG: Using exact line match at line {}: type='{}', lines={}-{}",
                            line,
                            exact_match.kind(),
                            start_pos.row + 1,
                            end_pos.row + 1
                        );
                    }

                    code_blocks.push(CodeBlock {
                        start_row: start_pos.row,
                        end_row: end_pos.row,
                        start_byte: exact_match.start_byte(),
                        end_byte: exact_match.end_byte(),
                        node_type: exact_match.kind().to_string(),
                        parent_node_type: None,
                        parent_start_row: None,
                        parent_end_row: None,
                    });

                    continue;
                }
            }

            // If no exact match found, fall back to the standard approach
            if let Some(node) = find_code_structure(root_node, line, extension) {
                let start_pos = node.start_position();
                let end_pos = node.end_position();
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
                let node_type = node.kind().to_string();

                // Check if this node has a parent that is a function or method
                let parent_info = if node_type == "struct_type" {
                    if let Some(parent_node) = language_impl.find_parent_function(node) {
                        let parent_type = parent_node.kind().to_string();
                        let parent_start = parent_node.start_position().row;
                        let parent_end = parent_node.end_position().row;

                        if debug_mode {
                            println!(
                                "DEBUG: Found parent {} for struct_type at lines {}-{}, parent at {}-{}",
                                parent_type, start_pos.row + 1, end_pos.row + 1,
                                parent_start + 1, parent_end + 1
                            );
                        }

                        Some((parent_type, parent_start, parent_end))
                    } else {
                        None
                    }
                } else {
                    None
                };

                code_blocks.push(CodeBlock {
                    start_row: start_pos.row,
                    end_row: end_pos.row,
                    start_byte: node.start_byte(),
                    end_byte: node.end_byte(),
                    node_type,
                    parent_node_type: parent_info.as_ref().map(|(t, _, _)| t.clone()),
                    parent_start_row: parent_info.as_ref().map(|(_, s, _)| *s),
                    parent_end_row: parent_info.as_ref().map(|(_, _, e)| *e),
                });
            }
        } else if debug_mode {
            println!("DEBUG: No node found for line {}", line);
        }
    }

    // Sort code blocks by start position
    code_blocks.sort_by_key(|block| block.start_row);

    // Deduplicate blocks with overlapping spans
    let mut deduplicated_blocks: Vec<CodeBlock> = Vec::new();

    // First add all comment blocks (we want to keep these)
    for block in code_blocks
        .iter()
        .filter(|b| b.node_type.contains("comment"))
    {
        deduplicated_blocks.push(block.clone());
    }

    // Then add non-comment blocks that don't overlap
    for block in code_blocks
        .into_iter()
        .filter(|b| !b.node_type.contains("comment"))
    {
        let mut should_add = true;

        // Check if this block overlaps with any of the previous blocks
        for prev_block in &deduplicated_blocks {
            if !prev_block.node_type.contains("comment") && // Only check overlap with non-comment blocks
               ((block.start_row >= prev_block.start_row && block.start_row <= prev_block.end_row) ||
                (block.end_row >= prev_block.start_row && block.end_row <= prev_block.end_row))
            {
                if debug_mode {
                    println!(
                        "DEBUG: Skipping overlapping block: type='{}', lines={}-{} (overlaps with type='{}', lines={}-{})",
                        block.node_type,
                        block.start_row + 1,
                        block.end_row + 1,
                        prev_block.node_type,
                        prev_block.start_row + 1,
                        prev_block.end_row + 1
                    );
                }
                should_add = false;
                break;
            }
        }

        if should_add {
            deduplicated_blocks.push(block);
        }
    }

    // Final sort to maintain correct order
    deduplicated_blocks.sort_by_key(|block| block.start_row);
    Ok(deduplicated_blocks)
}
