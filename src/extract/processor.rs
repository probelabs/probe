//! Functions for processing files and extracting code blocks.
//!
//! This module provides functions for processing files and extracting code blocks
//! based on file paths and optional line numbers.

use crate::extract::symbol_finder::find_symbol_in_file;
use crate::language::parser::parse_file_for_code_blocks;
use crate::models::SearchResult;
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// Process a single file and extract the specified code block
///
/// This function takes a file path, optional line numbers, and options for
/// extraction. It returns a SearchResult containing the extracted code.
///
/// If a single line number is specified, the function uses tree-sitter to find the
/// closest suitable parent node for that line. If a line range is specified (start and end),
/// it extracts exactly those lines. If no line number is specified, the entire file is extracted.
///
/// # Arguments
///
/// * `path` - The path to the file to extract from
/// * `start_line` - Optional start line number to extract a specific code block or range
/// * `end_line` - Optional end line number for extracting a range of lines
/// * `symbol` - Optional symbol name to look for in the file
/// * `allow_tests` - Whether to include test files and test code blocks
/// * `context_lines` - Number of context lines to include before and after the specified line
///
/// # Returns
///
/// A SearchResult containing the extracted code, or an error if the file
/// couldn't be read or the line number is out of bounds.
pub fn process_file_for_extraction(
    path: &Path,
    start_line: Option<usize>,
    end_line: Option<usize>,
    symbol: Option<&str>,
    allow_tests: bool,
    context_lines: usize,
) -> Result<SearchResult> {
    // Check if debug mode is enabled (used by tree-sitter parser)
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    if debug_mode {
        println!("\n[DEBUG] ===== Processing File for Extraction =====");
        println!("[DEBUG] File path: {:?}", path);
        println!("[DEBUG] Start line: {:?}", start_line);
        println!("[DEBUG] End line: {:?}", end_line);
        println!("[DEBUG] Symbol: {:?}", symbol);
        println!("[DEBUG] Allow tests: {}", allow_tests);
        println!("[DEBUG] Context lines: {}", context_lines);
    }

    // Check if the file exists
    if !path.exists() {
        if debug_mode {
            println!("[DEBUG] Error: File does not exist");
        }
        return Err(anyhow::anyhow!("File does not exist: {:?}", path));
    }

    // Read the file content
    let content = fs::read_to_string(path).context(format!("Failed to read file: {:?}", path))?;
    let lines: Vec<&str> = content.lines().collect();

    if debug_mode {
        println!("[DEBUG] File read successfully");
        println!("[DEBUG] File size: {} bytes", content.len());
        println!("[DEBUG] Line count: {}", lines.len());
    }

    // If we have a symbol, find it in the file
    if let Some(symbol_name) = symbol {
        if debug_mode {
            println!("[DEBUG] Looking for symbol: {}", symbol_name);
        }
        // Find the symbol in the file
        return find_symbol_in_file(path, symbol_name, &content, allow_tests, context_lines);
    }

    // If both start and end lines are specified, extract that exact range
    if let (Some(start), Some(end)) = (start_line, end_line) {
        if debug_mode {
            println!("[DEBUG] Extracting exact line range: {}-{}", start, end);
        }

        // Ensure line numbers are within bounds
        if start == 0 || start > lines.len() || end == 0 || end > lines.len() || start > end {
            let error_msg = format!(
                "Line range {}-{} is invalid (file has {} lines)",
                start,
                end,
                lines.len()
            );
            if debug_mode {
                println!("[DEBUG] Error: {}", error_msg);
            }
            return Err(anyhow::anyhow!(error_msg));
        }

        // Extract the specified range (adjusting for 0-indexed arrays)
        let start_idx = start - 1;
        let end_idx = end;
        let range_content = lines[start_idx..end_idx].join("\n");

        if debug_mode {
            println!("[DEBUG] Extracted {} lines of code", end_idx - start_idx);
            println!("[DEBUG] Content size: {} bytes", range_content.len());
        }

        Ok(SearchResult {
            file: path.to_string_lossy().to_string(),
            lines: (start, end),
            node_type: "range".to_string(),
            code: range_content,
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
        })
    } else if let Some(line_num) = start_line {
        // Single line specified, extract the code block
        if debug_mode {
            println!("[DEBUG] Single line specified: {}", line_num);
            println!(
                "[DEBUG] Will attempt to find the most specific code block containing this line"
            );
        }

        let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");

        if debug_mode {
            println!("[DEBUG] File extension: {}", extension);
            let language = crate::extract::formatter::get_language_from_extension(extension);
            println!(
                "[DEBUG] Detected language: {}",
                if language.is_empty() {
                    "unknown"
                } else {
                    language
                }
            );
        }

        // Create a set with the specified line number
        let mut line_set = HashSet::new();
        line_set.insert(line_num);

        // Use the existing parse_file_for_code_blocks function
        if debug_mode {
            println!(
                "[DEBUG] Parsing file for code blocks containing line {}",
                line_num
            );
        }

        match parse_file_for_code_blocks(&content, extension, &line_set, allow_tests, None) {
            Ok(code_blocks) if !code_blocks.is_empty() => {
                // Use the first code block found
                let block = &code_blocks[0];
                let start_line = block.start_row + 1;
                let end_line = block.end_row + 1;

                if debug_mode {
                    println!("[DEBUG] Found code block containing line {}", line_num);
                    println!("[DEBUG] Block type: {}", block.node_type);
                    println!("[DEBUG] Block range: {}-{}", start_line, end_line);
                    println!("[DEBUG] Block size: {} lines", end_line - start_line + 1);
                }

                // Extract the code for this block
                let block_content = content
                    .lines()
                    .skip(block.start_row)
                    .take(block.end_row - block.start_row + 1)
                    .collect::<Vec<_>>()
                    .join("\n");

                if debug_mode {
                    println!(
                        "[DEBUG] Extracted content size: {} bytes",
                        block_content.len()
                    );
                }

                // Use the node type from the language-specific implementation
                let node_type = block.node_type.clone();

                Ok(SearchResult {
                    file: path.to_string_lossy().to_string(),
                    lines: (start_line, end_line),
                    node_type,
                    code: block_content,
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
                })
            }
            Ok(_) | Err(_) => {
                // Fallback: If no code block found or parsing failed, extract context around the line
                if debug_mode {
                    println!("[DEBUG] No specific code block found or parsing failed");
                    println!("[DEBUG] Falling back to context-based extraction");
                }

                // Ensure line_num is within bounds
                if line_num == 0 || line_num > lines.len() {
                    let error_msg = format!(
                        "Line number {} is out of bounds (file has {} lines)",
                        line_num,
                        lines.len()
                    );
                    if debug_mode {
                        println!("[DEBUG] Error: {}", error_msg);
                    }
                    return Err(anyhow::anyhow!(error_msg));
                }

                // Extract context (configurable number of lines before and after)
                let start_line = line_num.saturating_sub(context_lines);
                let end_line = std::cmp::min(line_num + context_lines, lines.len());

                if debug_mode {
                    println!("[DEBUG] Using context-based extraction");
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

                Ok(SearchResult {
                    file: path.to_string_lossy().to_string(),
                    lines: (start_line, end_line),
                    node_type: "context".to_string(),
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
                })
            }
        }
    } else {
        // No line specified, return the entire file
        if debug_mode {
            println!("[DEBUG] No line specified, extracting entire file");
            println!("[DEBUG] File size: {} bytes", content.len());
            println!("[DEBUG] Line count: {}", lines.len());
        }

        Ok(SearchResult {
            file: path.to_string_lossy().to_string(),
            lines: (1, content.lines().count()),
            node_type: "file".to_string(),
            code: content,
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
        })
    }
}
