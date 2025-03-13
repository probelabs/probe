//! Extract command functionality for extracting code blocks from files.
//!
//! This module provides functions for extracting code blocks from files based on file paths
//! and optional line numbers. When a line number is specified, it uses tree-sitter to find
//! the closest suitable parent node (function, struct, class, etc.) for that line.

use crate::language::parser::parse_file_for_code_blocks;
use crate::models::SearchResult;
use crate::search::search_tokens::count_tokens;
use anyhow::{Context, Result};
use glob::glob;
use regex::Regex;
use std::collections::HashSet;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// Extract file paths from text (for stdin mode)
///
/// This function takes a string of text and extracts file paths with optional
/// line numbers or ranges. It's used when the extract command receives input from stdin.
///
/// The function looks for patterns like:
/// - File paths with extensions (e.g., file.rs, path/to/file.go)
/// - Optional line numbers after a colon (e.g., file.rs:10)
/// - Optional line ranges after a colon (e.g., file.rs:1-60)
/// - File paths with line and column numbers (e.g., file.rs:10:42)
pub fn extract_file_paths_from_text(text: &str) -> Vec<(PathBuf, Option<usize>, Option<usize>)> {
    let mut results = Vec::new();
    let mut processed_paths = HashSet::new();

    // First, try to match file paths with line ranges (e.g., file.rs:1-60)
    let file_range_regex =
        Regex::new(r"(?:^|\s)([a-zA-Z0-9_\-./\*\{\}]+\.[a-zA-Z0-9]+):(\d+)-(\d+)").unwrap();

    for cap in file_range_regex.captures_iter(text) {
        let file_path = cap.get(1).unwrap().as_str();
        let start_line = cap.get(2).and_then(|m| m.as_str().parse::<usize>().ok());
        let end_line = cap.get(3).and_then(|m| m.as_str().parse::<usize>().ok());

        if let (Some(start), Some(end)) = (start_line, end_line) {
            // Handle glob pattern
            if file_path.contains('*') || file_path.contains('{') {
                if let Ok(paths) = glob(file_path) {
                    for entry in paths.flatten() {
                        processed_paths.insert(entry.to_string_lossy().to_string());
                        results.push((entry, Some(start), Some(end)));
                    }
                }
            } else {
                processed_paths.insert(file_path.to_string());
                results.push((PathBuf::from(file_path), Some(start), Some(end)));
            }
        }
    }

    // Then, try to match file paths with single line numbers (and optional column numbers)
    let file_line_regex =
        Regex::new(r"(?:^|\s)([a-zA-Z0-9_\-./\*\{\}]+\.[a-zA-Z0-9]+):(\d+)(?::\d+)?").unwrap();

    for cap in file_line_regex.captures_iter(text) {
        let file_path = cap.get(1).unwrap().as_str();

        // Skip if we've already processed this path with a line range
        if processed_paths.contains(file_path) {
            continue;
        }

        let line_num = cap.get(2).and_then(|m| m.as_str().parse::<usize>().ok());

        // Handle glob pattern
        if file_path.contains('*') || file_path.contains('{') {
            if let Ok(paths) = glob(file_path) {
                for entry in paths.flatten() {
                    let path_str = entry.to_string_lossy().to_string();
                    if !processed_paths.contains(&path_str) {
                        processed_paths.insert(path_str);
                        results.push((entry, line_num, None));
                    }
                }
            }
        } else {
            processed_paths.insert(file_path.to_string());
            results.push((PathBuf::from(file_path), line_num, None));
        }
    }

    // Finally, match file paths without line numbers
    let simple_file_regex = Regex::new(r"(?:^|\s)([a-zA-Z0-9_\-./\*\{\}]+\.[a-zA-Z0-9]+)").unwrap();

    for cap in simple_file_regex.captures_iter(text) {
        let file_path = cap.get(1).unwrap().as_str();

        // Skip if we've already processed this path with a line number or range
        if !processed_paths.contains(file_path) {
            // Handle glob pattern
            if file_path.contains('*') || file_path.contains('{') {
                if let Ok(paths) = glob(file_path) {
                    for entry in paths.flatten() {
                        let path_str = entry.to_string_lossy().to_string();
                        if !processed_paths.contains(&path_str) {
                            processed_paths.insert(path_str);
                            results.push((entry, None, None));
                        }
                    }
                }
            } else {
                results.push((PathBuf::from(file_path), None, None));
                processed_paths.insert(file_path.to_string());
            }
        }
    }

    results
}

/// Parse a file path with optional line number or range (e.g., "file.rs:10" or "file.rs:1-60")
pub fn parse_file_with_line(input: &str) -> Vec<(PathBuf, Option<usize>, Option<usize>)> {
    let mut results = Vec::new();

    if let Some((file_part, rest)) = input.split_once(':') {
        // Extract the line specification from the rest (which might contain more colons)
        let line_spec = rest.split(':').next().unwrap_or("");

        // Check if it's a range (contains a hyphen)
        if let Some((start_str, end_str)) = line_spec.split_once('-') {
            let start_num = start_str.parse::<usize>().ok();
            let end_num = end_str.parse::<usize>().ok();

            if let (Some(start), Some(end)) = (start_num, end_num) {
                // Handle glob pattern
                if file_part.contains('*') || file_part.contains('{') {
                    if let Ok(paths) = glob(file_part) {
                        for entry in paths.flatten() {
                            results.push((entry, Some(start), Some(end)));
                        }
                    }
                } else {
                    results.push((PathBuf::from(file_part), Some(start), Some(end)));
                }
            }
        } else {
            // Try to parse as a single line number
            let line_num = line_spec.parse::<usize>().ok();

            if let Some(num) = line_num {
                // Handle glob pattern
                if file_part.contains('*') || file_part.contains('{') {
                    if let Ok(paths) = glob(file_part) {
                        for entry in paths.flatten() {
                            results.push((entry, Some(num), None));
                        }
                    }
                } else {
                    results.push((PathBuf::from(file_part), Some(num), None));
                }
            }
        }
    } else {
        // Handle glob pattern for file without line numbers
        if input.contains('*') || input.contains('{') {
            if let Ok(paths) = glob(input) {
                for entry in paths.flatten() {
                    results.push((entry, None, None));
                }
            }
        } else {
            results.push((PathBuf::from(input), None, None));
        }
    }

    results
}

// Internal module for formatting functions
mod format {
    use super::*;

    pub(super) fn format_and_print_terminal_results(results: &[SearchResult]) {
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

    pub(super) fn format_and_print_markdown_results(results: &[SearchResult]) {
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

    pub(super) fn format_and_print_plain_results(results: &[SearchResult]) {
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

    pub(super) fn format_and_print_json_results(results: &[SearchResult]) -> Result<()> {
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

    pub(super) fn format_and_print_color_results(results: &[SearchResult]) {
        use colored::*;
        use regex::Regex;
        use std::collections::HashSet;

        if results.is_empty() {
            println!("No results found.");
            return;
        }

        // Extract search terms from the results
        // We'll use the unique terms from the results if available
        let mut search_terms = HashSet::new();
        for result in results {
            if let Some(terms) = &result.file_unique_terms {
                if *terms > 0 {
                    // If we have unique terms data, we can try to extract terms from the code
                    // This is a simple approach - in a real implementation, you might want to
                    // get the actual search terms from the search query
                    let words: Vec<&str> = result.code.split_whitespace().collect();
                    for word in words {
                        // Clean up the word (remove punctuation, etc.)
                        let clean_word = word.trim_matches(|c: char| !c.is_alphanumeric());
                        if !clean_word.is_empty() {
                            search_terms.insert(clean_word.to_lowercase());
                        }
                    }
                }
            }
        }

        // Use the search terms we extracted, or an empty list if none were found
        // This removes the default highlighting of common programming terms
        let default_terms: Vec<String> = search_terms.into_iter().collect();

        // Create regex patterns for the terms
        let mut patterns = Vec::new();
        for term in &default_terms {
            // Create a case-insensitive regex for the term
            // We use word boundaries to match whole words
            if let Ok(regex) = Regex::new(&format!(r"(?i)\b{}\b", regex::escape(term))) {
                patterns.push(regex);
            }
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

            // Process the code line by line to highlight matching terms
            for line in result.code.lines() {
                let mut highlighted_line = line.to_string();

                // Apply highlighting for each pattern
                for pattern in &patterns {
                    // Use a temporary string to build the highlighted line
                    let mut temp_line = String::new();
                    let mut last_end = 0;

                    // Find all matches in the line
                    for mat in pattern.find_iter(&highlighted_line) {
                        // Add the text before the match
                        temp_line.push_str(&highlighted_line[last_end..mat.start()]);

                        // Add the highlighted match
                        temp_line.push_str(&mat.as_str().yellow().bold().to_string());

                        last_end = mat.end();
                    }

                    // Add the remaining text
                    temp_line.push_str(&highlighted_line[last_end..]);

                    highlighted_line = temp_line;
                }

                println!("{}", highlighted_line);
            }

            println!("```");
            println!();
        }
    }

    pub(super) fn get_language_from_extension(extension: &str) -> &'static str {
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
}

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
    let lines: Vec<&str> = content.lines().collect();

    // If both start and end lines are specified, extract that exact range
    if let (Some(start), Some(end)) = (start_line, end_line) {
        // Ensure line numbers are within bounds
        if start == 0 || start > lines.len() || end == 0 || end > lines.len() || start > end {
            return Err(anyhow::anyhow!(
                "Line range {}-{} is invalid (file has {} lines)",
                start,
                end,
                lines.len()
            ));
        }

        // Extract the specified range (adjusting for 0-indexed arrays)
        let start_idx = start - 1;
        let end_idx = end;
        let range_content = lines[start_idx..end_idx].join("\n");

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
                    matched_keywords: None,
                })
            }
            Ok(_) | Err(_) => {
                // Fallback: If no code block found or parsing failed, extract context around the line
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
                    matched_keywords: None,
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
            matched_keywords: None,
        })
    }
}

/// Format and print the extraction results in the specified format
///
/// # Arguments
///
/// * `results` - The search results to format and print
/// * `format` - The output format (terminal, markdown, plain, json, or color)
pub fn format_and_print_extraction_results(results: &[SearchResult], format: &str) -> Result<()> {
    match format {
        "markdown" => format::format_and_print_markdown_results(results),
        "plain" => format::format_and_print_plain_results(results),
        "json" => format::format_and_print_json_results(results)?,
        "color" => format::format_and_print_color_results(results),
        _ => format::format_and_print_terminal_results(results),
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

                // Calculate and display total bytes and tokens
                let total_bytes: usize = results.iter().map(|r| r.code.len()).sum();
                let total_tokens: usize = results.iter().map(|r| count_tokens(&r.code)).sum();
                println!("Total bytes returned: {}", total_bytes);
                println!("Total tokens returned: {}", total_tokens);
            }
        }
    }

    Ok(())
}

/// Handle the extract command
pub fn handle_extract(
    files: Vec<String>,
    allow_tests: bool,
    context_lines: usize,
    format: String,
) -> Result<()> {
    use colored::*;

    let mut file_paths: Vec<(PathBuf, Option<usize>, Option<usize>)> = Vec::new();

    if files.is_empty() {
        // Read from stdin
        println!("{}", "Reading from stdin...".bold().blue());
        let mut buffer = String::new();
        io::stdin().read_to_string(&mut buffer)?;

        file_paths = extract_file_paths_from_text(&buffer);

        if file_paths.is_empty() {
            println!("{}", "No file paths found in stdin.".yellow().bold());
            return Ok(());
        }
    } else {
        // Parse command-line arguments
        for file in files {
            let paths = parse_file_with_line(&file);
            file_paths.extend(paths);
        }
    }

    println!("{}", "Files to extract:".bold().green());

    for (path, start_line, end_line) in &file_paths {
        if let (Some(start), Some(end)) = (start_line, end_line) {
            println!("  {} (lines {}-{})", path.display(), start, end);
        } else if let Some(line_num) = start_line {
            println!("  {} (line {})", path.display(), line_num);
        } else {
            println!("  {}", path.display());
        }
    }

    if allow_tests {
        println!("{}", "Including test files and blocks".yellow());
    }

    if context_lines > 0 {
        println!("Context lines: {}", context_lines);
    }

    println!("Format: {}", format);
    println!();

    let mut results = Vec::new();
    let mut errors = Vec::new();

    // Process each file
    for (path, start_line, end_line) in file_paths {
        match process_file_for_extraction(&path, start_line, end_line, allow_tests, context_lines) {
            Ok(result) => results.push(result),
            Err(e) => {
                let error_msg = format!("Error processing file {:?}: {}", path, e);
                eprintln!("{}", error_msg.red());
                errors.push(error_msg);
            }
        }
    }

    // Format and print the results
    if let Err(e) = format_and_print_extraction_results(&results, &format) {
        eprintln!("{}", format!("Error formatting results: {}", e).red());
    }

    // Print summary of errors if any
    if !errors.is_empty() {
        println!();
        println!(
            "{} {} {}",
            "Encountered".red().bold(),
            errors.len(),
            if errors.len() == 1 { "error" } else { "errors" }
        );
    }

    Ok(())
}
