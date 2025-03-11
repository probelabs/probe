//! Extract command functionality for extracting code blocks from files.
//!
//! This module provides functions for extracting code blocks from files based on file paths
//! and optional line numbers. When a line number is specified, it uses tree-sitter to find
//! the closest suitable parent node (function, struct, class, etc.) for that line.

use crate::language::parser::parse_file_for_code_blocks;
use crate::models::SearchResult;
use anyhow::{Context, Result};
use serde_json;
use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// Process a single file and extract the specified code block
///
/// This function takes a file path, an optional line number, and options for
/// extraction. It returns a SearchResult containing the extracted code.
///
/// If a line number is specified, the function uses tree-sitter to find the
/// closest suitable parent node for that line. If no line number is specified,
/// the entire file is extracted.
///
/// # Arguments
///
/// * `path` - The path to the file to extract from
/// * `line` - Optional line number to extract a specific code block
/// * `allow_tests` - Whether to include test files and test code blocks
/// * `context_lines` - Number of context lines to include before and after the specified line
///
/// # Returns
///
/// A SearchResult containing the extracted code, or an error if the file
/// couldn't be read or the line number is out of bounds.
pub fn process_file_for_extraction(
    path: &Path,
    line: Option<usize>,
    allow_tests: bool,
    context_lines: usize,
) -> Result<SearchResult> {
    // Check if debug mode is enabled (used by tree-sitter parser)
    let _debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    // Check if the file exists
    if !path.exists() {
        return Err(anyhow::anyhow!("File does not exist: {:?}", path));
    }

    // Read the file content
    let content = fs::read_to_string(path).context(format!("Failed to read file: {:?}", path))?;

    if let Some(line_num) = line {
        // Line specified, extract the code block
        let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");

        // Create a set with the specified line number
        let mut line_set = HashSet::new();
        line_set.insert(line_num);

        // Use the existing parse_file_for_code_blocks function
        match parse_file_for_code_blocks(&content, extension, &line_set, allow_tests, None) {
            Ok(code_blocks) if !code_blocks.is_empty() => {
                // Use the first code block found
                let block = &code_blocks[0];
                let start_line = block.start_row + 1;
                let end_line = block.end_row + 1;

                // Extract the code for this block
                let block_content = content
                    .lines()
                    .skip(block.start_row)
                    .take(block.end_row - block.start_row + 1)
                    .collect::<Vec<_>>()
                    .join("\n");

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
                })
            }
            Ok(_) | Err(_) => {
                // Fallback: If no code block found or parsing failed, extract context around the line
                let lines: Vec<&str> = content.lines().collect();

                // Ensure line_num is within bounds
                if line_num == 0 || line_num > lines.len() {
                    return Err(anyhow::anyhow!(
                        "Line number {} is out of bounds (file has {} lines)",
                        line_num,
                        lines.len()
                    ));
                }

                // Extract context (configurable number of lines before and after)
                let start_line = line_num.saturating_sub(context_lines);
                let end_line = std::cmp::min(line_num + context_lines, lines.len());

                // Adjust start_line to be at least 1 (1-indexed)
                let start_idx = if start_line > 0 { start_line - 1 } else { 0 };

                let context = lines[start_idx..end_line].join("\n");

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
                })
            }
        }
    } else {
        // No line specified, return the entire file
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
        })
    }
}

/// Format and print the extraction results in the specified format
///
/// # Arguments
///
/// * `results` - The search results to format and print
/// * `format` - The output format (terminal, markdown, plain, or json)
pub fn format_and_print_extraction_results(results: &[SearchResult], format: &str) -> Result<()> {
    match format {
        "markdown" => format_and_print_markdown_results(results),
        "plain" => format_and_print_plain_results(results),
        "json" => format_and_print_json_results(results)?,
        _ => format_and_print_terminal_results(results),
    }

    // Print summary
    if !results.is_empty() {
        match format {
            "json" => {} // No summary for JSON format
            _ => {
                use colored::*;
                println!();
                println!(
                    "{} {} {}",
                    "Extracted".green().bold(),
                    results.len(),
                    if results.len() == 1 {
                        "result"
                    } else {
                        "results"
                    }
                );
            }
        }
    }

    Ok(())
}

/// Format and print results in terminal format (with colors)
fn format_and_print_terminal_results(results: &[SearchResult]) {
    use colored::*;

    if results.is_empty() {
        println!("{}", "No results found.".yellow().bold());
        return;
    }

    for result in results {
        // Get file extension
        let file_path = Path::new(&result.file);
        let extension = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        // Print file info
        println!("File: {}", result.file.yellow());

        // Print lines if not a full file
        if result.node_type != "file" {
            println!("Lines: {}-{}", result.lines.0, result.lines.1);
        }

        // Print node type if available and not "file" or "context"
        if result.node_type != "file" && result.node_type != "context" {
            println!("Type: {}", result.node_type.cyan());
        }

        // Determine the language for syntax highlighting
        let language = get_language_from_extension(extension);

        // Print the code with syntax highlighting
        if !language.is_empty() {
            println!("```{}", language);
        } else {
            println!("```");
        }

        println!("{}", result.code);
        println!("```");
        println!();
    }
}

/// Format and print results in markdown format
fn format_and_print_markdown_results(results: &[SearchResult]) {
    if results.is_empty() {
        println!("No results found.");
        return;
    }

    for result in results {
        // Get file extension
        let file_path = Path::new(&result.file);
        let extension = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        // Print file info
        println!("## File: {}", result.file);

        // Print lines if not a full file
        if result.node_type != "file" {
            println!("Lines: {}-{}", result.lines.0, result.lines.1);
        }

        // Print node type if available and not "file" or "context"
        if result.node_type != "file" && result.node_type != "context" {
            println!("Type: {}", result.node_type);
        }

        // Determine the language for syntax highlighting
        let language = get_language_from_extension(extension);

        // Print the code with syntax highlighting
        if !language.is_empty() {
            println!("```{}", language);
        } else {
            println!("```");
        }

        println!("{}", result.code);
        println!("```");
        println!();
    }
}

/// Format and print results in plain text format (no colors or markdown)
fn format_and_print_plain_results(results: &[SearchResult]) {
    if results.is_empty() {
        println!("No results found.");
        return;
    }

    for result in results {
        // Print file info
        println!("File: {}", result.file);

        // Print lines if not a full file
        if result.node_type != "file" {
            println!("Lines: {}-{}", result.lines.0, result.lines.1);
        }

        // Print node type if available and not "file" or "context"
        if result.node_type != "file" && result.node_type != "context" {
            println!("Type: {}", result.node_type);
        }

        println!();
        println!("{}", result.code);
        println!();
        println!("----------------------------------------");
        println!();
    }
}

/// Format and print results in JSON format
fn format_and_print_json_results(results: &[SearchResult]) -> Result<()> {
    if results.is_empty() {
        println!("[]");
        return Ok(());
    }

    // Create a simplified version of the results for JSON output
    #[derive(serde::Serialize)]
    struct JsonResult<'a> {
        file: &'a str,
        lines: (usize, usize),
        node_type: &'a str,
        code: &'a str,
    }

    let json_results: Vec<JsonResult> = results
        .iter()
        .map(|r| JsonResult {
            file: &r.file,
            lines: r.lines,
            node_type: &r.node_type,
            code: &r.code,
        })
        .collect();

    let json = serde_json::to_string_pretty(&json_results)
        .context("Failed to serialize results to JSON")?;

    println!("{}", json);

    Ok(())
}

/// Get the language name for syntax highlighting based on file extension
fn get_language_from_extension(extension: &str) -> &'static str {
    match extension {
        "rs" => "rust",
        "py" => "python",
        "js" => "javascript",
        "ts" => "typescript",
        "go" => "go",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" => "cpp",
        "java" => "java",
        "rb" => "ruby",
        "php" => "php",
        "sh" => "bash",
        "md" => "markdown",
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "html" => "html",
        "css" => "css",
        "sql" => "sql",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "scala" => "scala",
        "dart" => "dart",
        "ex" | "exs" => "elixir",
        "hs" => "haskell",
        "clj" => "clojure",
        "lua" => "lua",
        "r" => "r",
        "pl" | "pm" => "perl",
        "proto" => "protobuf",
        _ => "",
    }
}
