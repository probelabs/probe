//! Functions for finding symbols in files.
//!
//! This module provides functions for finding symbols (functions, structs, classes, etc.)
//! in files using tree-sitter.

use crate::models::SearchResult;
use anyhow::Result;
use std::path::Path;

/// Find a symbol (function, struct, class, etc.) in a file by name
///
/// This function searches for a symbol by name in a file and returns the code block
/// containing that symbol. It uses tree-sitter to parse the code and find the symbol.
///
/// # Arguments
///
/// * `path` - The path to the file to search in
/// * `symbol` - The name of the symbol to find
/// * `content` - The content of the file
/// * `allow_tests` - Whether to include test files and test code blocks
/// * `context_lines` - Number of context lines to include
///
/// # Returns
///
/// A SearchResult containing the extracted code block for the symbol, or an error
/// if the symbol couldn't be found.
pub fn find_symbol_in_file(
    path: &Path,
    symbol: &str,
    content: &str,
    allow_tests: bool,
    context_lines: usize,
) -> Result<SearchResult> {
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    if debug_mode {
        println!("\n[DEBUG] ===== Symbol Search =====");
        println!(
            "[DEBUG] Searching for symbol '{}' in file {:?}",
            symbol, path
        );
        println!("[DEBUG] Content size: {} bytes", content.len());
        println!("[DEBUG] Line count: {}", content.lines().count());
    }

    // Get the file extension to determine the language
    let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");

    if debug_mode {
        println!("[DEBUG] File extension: {}", extension);
    }

    // Get the language implementation for this extension
    let language_impl = crate::language::factory::get_language_impl(extension)
        .ok_or_else(|| anyhow::anyhow!("Unsupported language extension: {}", extension))?;

    if debug_mode {
        println!("[DEBUG] Language detected: {}", extension);
        println!("[DEBUG] Using tree-sitter to parse file");
    }

    // Parse the file with tree-sitter
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language_impl.get_tree_sitter_language())
        .map_err(|e| anyhow::anyhow!("Failed to set language: {}", e))?;

    let tree = parser
        .parse(content.as_bytes(), None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse file"))?;

    let root_node = tree.root_node();

    if debug_mode {
        println!("[DEBUG] File parsed successfully");
        println!("[DEBUG] Root node type: {}", root_node.kind());
        println!(
            "[DEBUG] Root node range: {}:{} - {}:{}",
            root_node.start_position().row + 1,
            root_node.start_position().column + 1,
            root_node.end_position().row + 1,
            root_node.end_position().column + 1
        );
        println!("[DEBUG] Searching for symbol '{}' in AST", symbol);
    }

    // Function to recursively search for a node with the given symbol name
    fn find_symbol_node<'a>(
        node: tree_sitter::Node<'a>,
        symbol: &str,
        language_impl: &dyn crate::language::language_trait::LanguageImpl,
        content: &'a [u8],
        debug_mode: bool,
    ) -> Option<tree_sitter::Node<'a>> {
        // Check if this node is an acceptable parent (function, struct, class, etc.)
        if language_impl.is_acceptable_parent(&node) {
            if debug_mode {
                println!(
                    "[DEBUG] Checking node type '{}' at {}:{}",
                    node.kind(),
                    node.start_position().row + 1,
                    node.start_position().column + 1
                );
            }

            // Try to extract the name of this node
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "identifier"
                    || child.kind() == "field_identifier"
                    || child.kind() == "type_identifier"
                    || child.kind() == "function_declarator"
                {
                    // Get the text of this identifier
                    if let Ok(name) = child.utf8_text(content) {
                        if debug_mode {
                            println!(
                                "[DEBUG] Found identifier: '{}' (looking for '{}')",
                                name, symbol
                            );
                        }

                        if name == symbol {
                            if debug_mode {
                                println!(
                                    "[DEBUG] Found symbol '{}' in node type '{}'",
                                    symbol,
                                    node.kind()
                                );
                                println!(
                                    "[DEBUG] Symbol location: {}:{} - {}:{}",
                                    node.start_position().row + 1,
                                    node.start_position().column + 1,
                                    node.end_position().row + 1,
                                    node.end_position().column + 1
                                );
                            }
                            return Some(node);
                        }
                    }

                    // For function_declarator, we need to look deeper
                    if child.kind() == "function_declarator" {
                        if debug_mode {
                            println!("[DEBUG] Checking function_declarator for symbol");
                        }

                        let mut subcursor = child.walk();
                        for subchild in child.children(&mut subcursor) {
                            if subchild.kind() == "identifier" {
                                if let Ok(name) = subchild.utf8_text(content) {
                                    if debug_mode {
                                        println!("[DEBUG] Found function identifier: '{}' (looking for '{}')", name, symbol);
                                    }

                                    if name == symbol {
                                        if debug_mode {
                                            println!(
                                                "[DEBUG] Found symbol '{}' in function_declarator",
                                                symbol
                                            );
                                            println!(
                                                "[DEBUG] Symbol location: {}:{} - {}:{}",
                                                node.start_position().row + 1,
                                                node.start_position().column + 1,
                                                node.end_position().row + 1,
                                                node.end_position().column + 1
                                            );
                                        }
                                        return Some(node);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Recursively search in children
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = find_symbol_node(child, symbol, language_impl, content, debug_mode)
            {
                return Some(found);
            }
        }

        None
    }

    // Search for the symbol in the AST
    if let Some(found_node) = find_symbol_node(
        root_node,
        symbol,
        language_impl.as_ref(),
        content.as_bytes(),
        debug_mode,
    ) {
        let node_start_line = found_node.start_position().row + 1;
        let node_end_line = found_node.end_position().row + 1;

        if debug_mode {
            println!("\n[DEBUG] ===== Symbol Found =====");
            println!(
                "[DEBUG] Found symbol '{}' at lines {}-{}",
                symbol, node_start_line, node_end_line
            );
            println!("[DEBUG] Node type: {}", found_node.kind());
            println!(
                "[DEBUG] Node range: {}:{} - {}:{}",
                found_node.start_position().row + 1,
                found_node.start_position().column + 1,
                found_node.end_position().row + 1,
                found_node.end_position().column + 1
            );
        }

        // Extract the code block
        let node_text = &content[found_node.start_byte()..found_node.end_byte()];

        if debug_mode {
            println!("[DEBUG] Extracted code size: {} bytes", node_text.len());
            println!(
                "[DEBUG] Extracted code lines: {}",
                node_text.lines().count()
            );
        }

        return Ok(SearchResult {
            file: path.to_string_lossy().to_string(),
            lines: (node_start_line, node_end_line),
            node_type: found_node.kind().to_string(),
            code: node_text.to_string(),
            matched_by_filename: None,
            rank: None,
            score: None,
            tfidf_score: None,
            bm25_score: None,
            tfidf_rank: None,
            bm25_rank: None,
            new_score: None,
            hybrid2_rank: None,
            combined_score_rank: None,
            file_unique_terms: None,
            file_total_matches: None,
            file_match_rank: None,
            block_unique_terms: None,
            block_total_matches: None,
            parent_file_id: None,
            block_id: None,
            matched_keywords: None,
        });
    }

    // If we couldn't find the symbol using tree-sitter, try a simple text search as fallback
    if debug_mode {
        println!("\n[DEBUG] ===== Symbol Not Found in AST =====");
        println!("[DEBUG] Symbol '{}' not found in AST", symbol);
        println!("[DEBUG] Trying text search fallback");
    }

    // Simple text search for the symbol
    let lines: Vec<&str> = content.lines().collect();
    let mut found_line = None;

    if debug_mode {
        println!(
            "[DEBUG] Performing text search for '{}' across {} lines",
            symbol,
            lines.len()
        );
    }

    for (i, line) in lines.iter().enumerate() {
        if line.contains(symbol) {
            found_line = Some(i + 1); // 1-indexed line number
            if debug_mode {
                println!(
                    "[DEBUG] Found symbol '{}' in line {}: '{}'",
                    symbol,
                    i + 1,
                    line.trim()
                );
            }
            break;
        }
    }

    if let Some(line_num) = found_line {
        if debug_mode {
            println!("\n[DEBUG] ===== Symbol Found via Text Search =====");
            println!(
                "[DEBUG] Found symbol '{}' using text search at line {}",
                symbol, line_num
            );
        }

        // Extract context around the line
        let start_line = line_num.saturating_sub(context_lines);
        let end_line = std::cmp::min(line_num + context_lines, lines.len());

        if debug_mode {
            println!("[DEBUG] Extracting context around line {}", line_num);
            println!("[DEBUG] Context lines: {}", context_lines);
            println!("[DEBUG] Extracting lines {}-{}", start_line, end_line);
        }

        // Adjust start_line to be at least 1 (1-indexed)
        let start_idx = if start_line > 0 { start_line - 1 } else { 0 };

        let context = lines[start_idx..end_line].join("\n");

        if debug_mode {
            println!("[DEBUG] Extracted {} lines of code", end_line - start_line);
            println!("[DEBUG] Content size: {} bytes", context.len());
        }

        return Ok(SearchResult {
            file: path.to_string_lossy().to_string(),
            lines: (start_line, end_line),
            node_type: "text_search".to_string(),
            code: context,
            matched_by_filename: None,
            rank: None,
            score: None,
            tfidf_score: None,
            bm25_score: None,
            tfidf_rank: None,
            bm25_rank: None,
            new_score: None,
            hybrid2_rank: None,
            combined_score_rank: None,
            file_unique_terms: None,
            file_total_matches: None,
            file_match_rank: None,
            block_unique_terms: None,
            block_total_matches: None,
            parent_file_id: None,
            block_id: None,
            matched_keywords: None,
        });
    }

    // If we get here, we couldn't find the symbol
    if debug_mode {
        println!("\n[DEBUG] ===== Symbol Not Found =====");
        println!("[DEBUG] Symbol '{}' not found in file {:?}", symbol, path);
        println!("[DEBUG] Neither AST parsing nor text search found the symbol");
    }

    Err(anyhow::anyhow!(
        "Symbol '{}' not found in file {:?}",
        symbol,
        path
    ))
}
