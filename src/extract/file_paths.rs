//! Functions for extracting file paths from text.
//!
//! This module provides functions for parsing file paths with optional line numbers,
//! line ranges, or symbol references from text input.

use glob::glob;
use ignore::WalkBuilder;
use regex::Regex;
use std::collections::HashSet;
use std::path::PathBuf;

/// Represents a file path with optional line numbers and symbol information
///
/// - `PathBuf`: The path to the file
/// - First `Option<usize>`: Optional start line number
/// - Second `Option<usize>`: Optional end line number
/// - `Option<String>`: Optional symbol name
pub type FilePathInfo = (PathBuf, Option<usize>, Option<usize>, Option<String>);

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
/// - File paths with symbol references (e.g., file.rs#function_name)
pub fn extract_file_paths_from_text(text: &str) -> Vec<FilePathInfo> {
    let mut results = Vec::new();
    let mut processed_paths = HashSet::new();

    // Check if debug mode is enabled
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    // First, try to match file paths with symbol references (e.g., file.rs#function_name)
    let file_symbol_regex =
        Regex::new(r"(?:^|\s)([a-zA-Z0-9_\-./\*\{\}]+\.[a-zA-Z0-9]+)#([a-zA-Z0-9_]+)").unwrap();

    for cap in file_symbol_regex.captures_iter(text) {
        let file_path = cap.get(1).unwrap().as_str();
        let symbol = cap.get(2).unwrap().as_str();

        // Check if this file path has already been processed
        if processed_paths.contains(file_path) {
            continue;
        }

        // Handle glob pattern
        if file_path.contains('*') || file_path.contains('{') {
            if let Ok(paths) = glob(file_path) {
                for entry in paths.flatten() {
                    // Check if the file should be ignored
                    let should_include = !is_ignored_by_gitignore(&entry);
                    if should_include {
                        let path_str = entry.to_string_lossy().to_string();
                        processed_paths.insert(path_str.clone());
                        // Pass the symbol name directly instead of using environment variables
                        results.push((entry, None, None, Some(symbol.to_string())));
                    } else if debug_mode {
                        println!("DEBUG: Skipping ignored file: {:?}", entry);
                    }
                }
            }
        } else {
            let path = PathBuf::from(file_path);
            if !is_ignored_by_gitignore(&path) {
                processed_paths.insert(file_path.to_string());
                // Pass the symbol name directly instead of using environment variables
                results.push((path, None, None, Some(symbol.to_string())));
            } else if debug_mode {
                println!("DEBUG: Skipping ignored file: {:?}", file_path);
            }
        }
    }

    // Next, try to match file paths with line ranges (e.g., file.rs:1-60)
    let file_range_regex =
        Regex::new(r"(?:^|\s)([a-zA-Z0-9_\-./\*\{\}]+\.[a-zA-Z0-9]+):(\d+)-(\d+)").unwrap();

    for cap in file_range_regex.captures_iter(text) {
        let file_path = cap.get(1).unwrap().as_str();

        // Skip if we've already processed this path with a symbol reference
        if processed_paths.contains(file_path) {
            continue;
        }

        let start_line = cap.get(2).and_then(|m| m.as_str().parse::<usize>().ok());
        let end_line = cap.get(3).and_then(|m| m.as_str().parse::<usize>().ok());

        if let (Some(start), Some(end)) = (start_line, end_line) {
            // Handle glob pattern
            if file_path.contains('*') || file_path.contains('{') {
                if let Ok(paths) = glob(file_path) {
                    for entry in paths.flatten() {
                        // Check if the file should be ignored
                        let should_include = !is_ignored_by_gitignore(&entry);
                        if should_include {
                            processed_paths.insert(entry.to_string_lossy().to_string());
                            results.push((entry, Some(start), Some(end), None));
                        } else if debug_mode {
                            println!("DEBUG: Skipping ignored file: {:?}", entry);
                        }
                    }
                }
            } else {
                let path = PathBuf::from(file_path);
                if !is_ignored_by_gitignore(&path) {
                    processed_paths.insert(file_path.to_string());
                    results.push((path, Some(start), Some(end), None));
                } else if debug_mode {
                    println!("DEBUG: Skipping ignored file: {:?}", file_path);
                }
            }
        }
    }

    // Then, try to match file paths with single line numbers (and optional column numbers)
    let file_line_regex =
        Regex::new(r"(?:^|\s)([a-zA-Z0-9_\-./\*\{\}]+\.[a-zA-Z0-9]+):(\d+)(?::\d+)?").unwrap();

    for cap in file_line_regex.captures_iter(text) {
        let file_path = cap.get(1).unwrap().as_str();

        // Skip if we've already processed this path with a symbol reference or line range
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
                        // Check if the file should be ignored
                        let should_include = !is_ignored_by_gitignore(&entry);
                        if should_include {
                            processed_paths.insert(path_str);
                            results.push((entry, line_num, None, None));
                        } else if debug_mode {
                            println!("DEBUG: Skipping ignored file: {:?}", entry);
                        }
                    }
                }
            }
        } else {
            let path = PathBuf::from(file_path);
            if !is_ignored_by_gitignore(&path) {
                processed_paths.insert(file_path.to_string());
                results.push((path, line_num, None, None));
            } else if debug_mode {
                println!("DEBUG: Skipping ignored file: {:?}", file_path);
            }
        }
    }

    // Finally, match file paths without line numbers or symbols
    // But only if they haven't been processed already
    let simple_file_regex = Regex::new(r"(?:^|\s)([a-zA-Z0-9_\-./\*\{\}]+\.[a-zA-Z0-9]+)").unwrap();

    for cap in simple_file_regex.captures_iter(text) {
        let file_path = cap.get(1).unwrap().as_str();

        // Skip if we've already processed this path with a symbol, line number, or range
        if !processed_paths.contains(file_path) {
            // Handle glob pattern
            if file_path.contains('*') || file_path.contains('{') {
                if let Ok(paths) = glob(file_path) {
                    for entry in paths.flatten() {
                        let path_str = entry.to_string_lossy().to_string();
                        if !processed_paths.contains(&path_str) {
                            // Check if the file should be ignored
                            let should_include = !is_ignored_by_gitignore(&entry);
                            if should_include {
                                processed_paths.insert(path_str);
                                results.push((entry, None, None, None));
                            } else if debug_mode {
                                println!("DEBUG: Skipping ignored file: {:?}", entry);
                            }
                        }
                    }
                }
            } else {
                let path = PathBuf::from(file_path);
                if !is_ignored_by_gitignore(&path) {
                    results.push((path, None, None, None));
                    processed_paths.insert(file_path.to_string());
                } else if debug_mode {
                    println!("DEBUG: Skipping ignored file: {:?}", file_path);
                }
            }
        }
    }

    results
}

/// Parse a file path with optional line number or range (e.g., "file.rs:10" or "file.rs:1-60")
pub fn parse_file_with_line(input: &str) -> Vec<FilePathInfo> {
    let mut results = Vec::new();

    // Check if the input contains a symbol reference (file#symbol)
    if let Some((file_part, symbol)) = input.split_once('#') {
        // For symbol references, we don't have line numbers yet
        // We'll need to find the symbol in the file later
        results.push((
            PathBuf::from(file_part),
            None,
            None,
            Some(symbol.to_string()),
        ));
        return results;
    } else if let Some((file_part, rest)) = input.split_once(':') {
        // Extract the line specification from the rest (which might contain more colons)
        let line_spec = rest.split(':').next().unwrap_or("");

        // Check if it's a range (contains a hyphen)
        if let Some((start_str, end_str)) = line_spec.split_once('-') {
            let start_num = start_str.parse::<usize>().ok();
            let end_num = end_str.parse::<usize>().ok();

            if let (Some(start), Some(end)) = (start_num, end_num) {
                // Handle glob pattern
                if file_part.contains('*') || file_part.contains('{') {
                    // Use WalkBuilder to respect .gitignore
                    let base_dir = std::path::Path::new(".");
                    let mut builder = WalkBuilder::new(base_dir);
                    builder.git_ignore(true);
                    builder.git_global(true);
                    builder.git_exclude(true);

                    // Also try glob for backward compatibility
                    if let Ok(paths) = glob(file_part) {
                        for entry in paths.flatten() {
                            // Check if the file should be ignored
                            let should_include = !is_ignored_by_gitignore(&entry);
                            if should_include {
                                results.push((entry, Some(start), Some(end), None));
                            }
                        }
                    }
                } else {
                    let path = PathBuf::from(file_part);
                    if !is_ignored_by_gitignore(&path) {
                        results.push((path, Some(start), Some(end), None));
                    }
                }
            }
        } else {
            // Try to parse as a single line number
            let line_num = line_spec.parse::<usize>().ok();

            if let Some(num) = line_num {
                // Handle glob pattern
                if file_part.contains('*') || file_part.contains('{') {
                    // Use WalkBuilder to respect .gitignore
                    if let Ok(paths) = glob(file_part) {
                        for entry in paths.flatten() {
                            // Check if the file should be ignored
                            let should_include = !is_ignored_by_gitignore(&entry);
                            if should_include {
                                results.push((entry, Some(num), None, None));
                            }
                        }
                    }
                } else {
                    let path = PathBuf::from(file_part);
                    if !is_ignored_by_gitignore(&path) {
                        results.push((path, Some(num), None, None));
                    }
                }
            }
        }
    } else {
        // No line number or symbol specified, just a file path
        // Handle glob pattern
        if input.contains('*') || input.contains('{') {
            if let Ok(paths) = glob(input) {
                for entry in paths.flatten() {
                    // Check if the file should be ignored
                    let should_include = !is_ignored_by_gitignore(&entry);
                    if should_include {
                        results.push((entry, None, None, None));
                    }
                }
            }
        } else {
            let path = PathBuf::from(input);
            if !is_ignored_by_gitignore(&path) {
                results.push((path, None, None, None));
            }
        }
    }

    results
}

// Thread-local storage for the custom ignore patterns
thread_local! {
    static CUSTOM_IGNORES: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Set custom ignore patterns for the current thread
pub fn set_custom_ignores(patterns: &[String]) {
    CUSTOM_IGNORES.with(|cell| {
        let mut ignores = cell.borrow_mut();
        ignores.clear();
        ignores.extend(patterns.iter().cloned());
    });
}

/// Check if a file should be ignored according to .gitignore rules
fn is_ignored_by_gitignore(path: &PathBuf) -> bool {
    // Check if debug mode is enabled
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    // Simple check for common ignore patterns in the path
    let path_str = path.to_string_lossy().to_lowercase();

    // Check for common ignore patterns directly in the path
    let common_ignore_patterns = [
        "node_modules",
        "vendor",
        "target",
        "dist",
        "build",
        ".git",
        ".svn",
        ".hg",
        ".idea",
        ".vscode",
        "__pycache__",
    ];

    // Get custom ignore patterns
    let mut custom_patterns = Vec::new();
    CUSTOM_IGNORES.with(|cell| {
        let ignores = cell.borrow();
        custom_patterns.extend(ignores.iter().cloned());
    });

    // Check if the path contains any of the common ignore patterns
    for pattern in &common_ignore_patterns {
        if path_str.contains(pattern) {
            if debug_mode {
                println!(
                    "DEBUG: File {:?} is ignored (contains pattern '{}')",
                    path, pattern
                );
            }
            return true;
        }
    }

    // Check if the path contains any of the custom ignore patterns
    for pattern in &custom_patterns {
        if path_str.contains(pattern) {
            if debug_mode {
                println!(
                    "DEBUG: File {:?} is ignored (contains custom pattern '{}')",
                    path, pattern
                );
            }
            return true;
        }
    }

    false
}
