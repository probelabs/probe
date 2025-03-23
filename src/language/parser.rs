use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use tree_sitter::{Node, Parser as TSParser};

use crate::language::factory::get_language_impl;
use crate::language::language_trait::LanguageImpl;
use crate::language::tree_cache;
use crate::models::CodeBlock;

/// Structure to hold node information for a specific line
#[derive(Clone, Copy)]
struct NodeInfo<'a> {
    node: Node<'a>,
    is_comment: bool,
    context_node: Option<Node<'a>>,
    is_test: bool,
    // Track the specificity of this node assignment
    // Lower values mean more specific (e.g., smaller node)
    specificity: usize,
}

/// Helper function to determine if we should update the line map for a given line
fn should_update_line_map<'a>(
    line_map: &[Option<NodeInfo<'a>>],
    line: usize,
    node: Node<'a>,
    is_comment: bool,
    context_node: Option<Node<'a>>,
    specificity: usize,
) -> bool {
    match &line_map[line] {
        None => true, // No existing node, always update
        Some(current) => {
            // Special case: If current node is a comment with context, and new node is the context,
            // don't replace it (preserve the comment+context relationship)
            if current.is_comment && current.context_node.is_some() {
                if let Some(ctx) = current.context_node {
                    if ctx.id() == node.id() {
                        return false;
                    }
                }
            }

            // Special case: If new node is a comment with context, and current node is the context,
            // replace it (comment with context is more specific)
            if is_comment && context_node.is_some() {
                if let Some(ctx) = context_node {
                    if ctx.id() == current.node.id() {
                        return true;
                    }
                }
            }

            // Otherwise use specificity to decide
            specificity < current.specificity
        }
    }
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

/// Find the nearest acceptable ancestor for a node
/// This traverses up the AST to find the first parent that is an acceptable parent
fn find_nearest_acceptable_ancestor<'a>(
    node: Node<'a>,
    language_impl: &dyn LanguageImpl,
) -> Option<Node<'a>> {
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    // Check if the current node is acceptable
    if language_impl.is_acceptable_parent(&node) {
        if debug_mode {
            println!(
                "DEBUG: Node is already an acceptable parent: type='{}', lines={}-{}",
                node.kind(),
                node.start_position().row + 1,
                node.end_position().row + 1
            );
        }
        return Some(node);
    }

    // Traverse up the parent chain
    let mut current = node;
    while let Some(parent) = current.parent() {
        if language_impl.is_acceptable_parent(&parent) {
            return Some(parent);
        }
        current = parent;
    }

    None
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

/// Helper function to find the context node for a comment.
/// This is a comprehensive implementation that handles all comment context finding strategies.
fn find_comment_context_node<'a>(
    comment_node: Node<'a>,
    language_impl: &dyn LanguageImpl,
    debug_mode: bool,
) -> Option<Node<'a>> {
    let start_row = comment_node.start_position().row;

    if debug_mode {
        println!(
            "DEBUG: Finding context for comment at lines {}-{}: {}",
            comment_node.start_position().row + 1,
            comment_node.end_position().row + 1,
            comment_node.kind()
        );
    }

    // Strategy 1: Try to find next non-comment sibling first (most common case for doc comments)
    let mut current_sibling = comment_node.next_sibling();

    // Skip over any comment siblings to find the next non-comment sibling
    while let Some(sibling) = current_sibling {
        if sibling.kind() == "comment"
            || sibling.kind() == "line_comment"
            || sibling.kind() == "block_comment"
            || sibling.kind() == "doc_comment"
            || sibling.kind() == "//"
        {
            // This is another comment, move to the next sibling
            current_sibling = sibling.next_sibling();
            continue;
        }

        // Found a non-comment sibling
        if language_impl.is_acceptable_parent(&sibling) {
            if debug_mode {
                println!(
                    "DEBUG: Found next non-comment sibling for comment at line {}: type='{}', lines={}-{}",
                    start_row + 1,
                    sibling.kind(),
                    sibling.start_position().row + 1,
                    sibling.end_position().row + 1
                );
            }
            return Some(sibling);
        } else {
            // If next sibling isn't acceptable, check its children
            if let Some(child) = find_acceptable_child(sibling, language_impl) {
                if debug_mode {
                    println!(
                        "DEBUG: Found acceptable child in next non-comment sibling for comment at line {}: type='{}', lines={}-{}",
                        start_row + 1,
                        child.kind(),
                        child.start_position().row + 1,
                        child.end_position().row + 1
                    );
                }
                return Some(child);
            }
        }

        // If we get here, this non-comment sibling wasn't acceptable, try the next one
        current_sibling = sibling.next_sibling();
    }

    // Strategy 2: If no acceptable next sibling, try previous sibling (for trailing comments)
    // But only if the comment is at the end of a block or if there's no next sibling
    // This helps ensure comments are associated with the code that follows them when possible
    let has_next_sibling = comment_node.next_sibling().is_some();

    if !has_next_sibling {
        if let Some(prev_sibling) = find_prev_sibling(comment_node) {
            if language_impl.is_acceptable_parent(&prev_sibling) {
                if debug_mode {
                    println!(
                        "DEBUG: Found previous sibling for comment at line {}: type='{}', lines={}-{}",
                        start_row + 1,
                        prev_sibling.kind(),
                        prev_sibling.start_position().row + 1,
                        prev_sibling.end_position().row + 1
                    );
                }
                return Some(prev_sibling);
            } else {
                // If previous sibling isn't acceptable, check its children
                if let Some(child) = find_acceptable_child(prev_sibling, language_impl) {
                    if debug_mode {
                        println!(
                            "DEBUG: Found acceptable child in previous sibling for comment at line {}: type='{}', lines={}-{}",
                            start_row + 1,
                            child.kind(),
                            child.start_position().row + 1,
                            child.end_position().row + 1
                        );
                    }
                    return Some(child);
                }
            }
        }
    }

    // Strategy 3: Check parent chain
    let mut current = comment_node;
    while let Some(parent) = current.parent() {
        if language_impl.is_acceptable_parent(&parent) {
            if debug_mode {
                println!(
                    "DEBUG: Found parent for comment at line {}: type='{}', lines={}-{}",
                    start_row + 1,
                    parent.kind(),
                    parent.start_position().row + 1,
                    parent.end_position().row + 1
                );
            }
            return Some(parent);
        }
        current = parent;
    }

    // Strategy 4: Look for any immediate next node
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

        // Look for acceptable child in the next node
        if let Some(child) = find_acceptable_child(next_node, language_impl) {
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

    if debug_mode {
        println!("DEBUG: No related node found for the comment");
    }
    None
}

/// Process a node and its children in a single pass, building a comprehensive line-to-node map.
/// This is the core of our unified AST traversal strategy.
fn process_node<'a>(
    node: Node<'a>,
    line_map: &mut Vec<Option<NodeInfo<'a>>>,
    _extension: &str,
    language_impl: &dyn LanguageImpl,
    content: &[u8],
    allow_tests: bool,
    debug_mode: bool,
) {
    let start_row = node.start_position().row;
    let end_row = node.end_position().row;

    // Skip nodes that are outside the file bounds
    if start_row >= line_map.len() {
        return;
    }

    // Determine node type
    let is_comment = node.kind() == "comment"
        || node.kind() == "line_comment"
        || node.kind() == "block_comment"
        || node.kind() == "doc_comment"
        || node.kind() == "//";

    let is_test = !allow_tests && language_impl.is_test_node(&node, content);

    // Calculate node specificity (smaller is more specific)
    // We use line coverage as the primary metric for specificity
    let line_coverage = end_row.saturating_sub(start_row) + 1;
    let byte_coverage = node.end_byte().saturating_sub(node.start_byte());

    // Combine both metrics, with line coverage being more important
    let specificity = line_coverage * 1000 + (byte_coverage / 100);

    // For comments, find the related code node immediately during traversal
    // For non-comments, find the nearest acceptable ancestor
    let context_node = if is_comment {
        find_comment_context_node(node, language_impl, debug_mode)
    } else {
        // For non-comment nodes, find the nearest acceptable ancestor
        // This ensures that each line is associated with an acceptable parent node
        if !language_impl.is_acceptable_parent(&node) {
            find_nearest_acceptable_ancestor(node, language_impl)
        } else {
            None // Node is already acceptable
        }
    };

    // Update the line map for each line covered by this node
    for line in start_row..=end_row {
        if line >= line_map.len() {
            break;
        }

        // Determine if we should update the line map for this line
        let should_update =
            should_update_line_map(line_map, line, node, is_comment, context_node, specificity);

        if should_update {
            line_map[line] = Some(NodeInfo {
                node,
                is_comment,
                context_node,
                is_test,
                specificity,
            });
        }
    }

    // Process children (depth-first traversal)
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        process_node(
            child,
            line_map,
            _extension,
            language_impl,
            content,
            allow_tests,
            debug_mode,
        );
    }
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

        if debug_mode {
            println!("DEBUG: Processing line {}", line);
        }

        // Skip if line is out of bounds
        if line_idx >= line_map.len() {
            if debug_mode {
                println!("DEBUG: Line {} is out of bounds", line);
            }
            continue;
        }

        // Get the node info for this line
        if let Some(info) = &line_map[line_idx] {
            if debug_mode {
                println!(
                    "DEBUG: Found node for line {}: type='{}', lines={}-{}",
                    line,
                    info.node.kind(),
                    info.node.start_position().row + 1,
                    info.node.end_position().row + 1
                );
            }
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

            // Check if we have a context node (nearest acceptable ancestor) for this node
            if let Some(context_node) = info.context_node {
                let rel_start_pos = context_node.start_position();
                let rel_end_pos = context_node.end_position();
                let rel_key = (rel_start_pos.row, rel_end_pos.row);

                // Skip test nodes unless allow_tests is true
                if !allow_tests && language_impl.is_test_node(&context_node, content.as_bytes()) {
                    if debug_mode {
                        println!(
                            "DEBUG: Skipping test node at lines {}-{}, type: {}",
                            rel_start_pos.row + 1,
                            rel_end_pos.row + 1,
                            context_node.kind()
                        );
                    }
                } else {
                    if debug_mode {
                        println!(
                            "DEBUG: Using context node for line {}: type='{}', lines={}-{}",
                            line,
                            context_node.kind(),
                            rel_start_pos.row + 1,
                            rel_end_pos.row + 1
                        );
                    }

                    // Mark the context node as seen
                    seen_nodes.insert(rel_key);

                    // Add the context node to the code blocks
                    code_blocks.push(CodeBlock {
                        start_row: rel_start_pos.row,
                        end_row: rel_end_pos.row,
                        start_byte: context_node.start_byte(),
                        end_byte: context_node.end_byte(),
                        node_type: context_node.kind().to_string(),
                        parent_node_type: None,
                        parent_start_row: None,
                        parent_end_row: None,
                    });

                    continue;
                }
            }

            // Check if this node is an acceptable parent
            if language_impl.is_acceptable_parent(&target_node) {
                if debug_mode {
                    println!(
                        "DEBUG: Adding acceptable parent node for line {}: type='{}', lines={}-{}",
                        line,
                        target_node.kind(),
                        start_pos.row + 1,
                        end_pos.row + 1
                    );
                }

                // Add the node to the code blocks
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

                continue;
            }

            // No need for fallback exact line matching - the line_map is comprehensive
            // and should already contain the best node for this line

            // If no exact match found, use the line_map directly
            // No need for a fallback approach since we've built a comprehensive line_map
            if let Some(node_info) = &line_map[line_idx] {
                let node = node_info.node;
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
