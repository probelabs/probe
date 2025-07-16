//! Functions for extracting file paths from text.
//!
//! This module provides functions for parsing file paths with optional line numbers,
//! line ranges, or symbol references from text input.

use crate::language::is_test_file;
use glob::glob;
use ignore::WalkBuilder;
use probe::path_resolver::resolve_path;
use regex::Regex;
use std::collections::HashSet;
use std::path::PathBuf;

/// Represents a file path with optional line numbers and symbol information
///
/// - `PathBuf`: The path to the file
/// - First `Option<usize>`: Optional start line number
/// - Second `Option<usize>`: Optional end line number
/// - `Option<String>`: Optional symbol name
/// - `Option<HashSet<usize>>`: Optional set of specific line numbers
pub type FilePathInfo = (
    PathBuf,
    Option<usize>,
    Option<usize>,
    Option<String>,
    Option<HashSet<usize>>,
);
/// Check if content is in git diff format
///
/// This function checks if the content starts with "diff --git" which indicates
/// it's in git diff format.
pub fn is_git_diff_format(content: &str) -> bool {
    content.trim_start().starts_with("diff --git")
}

/// Extract file paths from git diff format
///
/// This function takes a string of text in git diff format and extracts file paths
/// with line ranges. It's used when the extract command is run with the --diff option.
///
/// The function looks for patterns like:
/// - diff --git a/path/to/file.rs b/path/to/file.rs
/// - @@ -45,7 +45,7 @@ (hunk header)
///
/// It extracts the file path from the diff header and the line range from the hunk header.
/// We don't add arbitrary context lines - instead we rely on the AST parser to find
/// the full function or code block that contains the changed lines.
///
/// If allow_tests is false, test files will be filtered out.
pub fn extract_file_paths_from_git_diff(text: &str, allow_tests: bool) -> Vec<FilePathInfo> {
    let mut results = Vec::new();
    let mut processed_files = HashSet::new();
    let mut current_file: Option<PathBuf> = None;
    let mut current_file_lines = HashSet::new();

    // Check if debug mode is enabled
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    // Split the text into lines
    let lines: Vec<&str> = text.lines().collect();

    // Regex for diff header: diff --git a/path/to/file.rs b/path/to/file.rs
    let diff_header_regex = Regex::new(r"^diff --git a/(.*) b/(.*)$").unwrap();

    // Regex for hunk header capturing start+len for old and new lines:
    //   @@ -oldStart,oldLen +newStart,newLen @@
    // The length part may be omitted if 1 (in which case the diff might display e.g. @@ -10 +20 @@).
    // We'll default missing length to 1.
    let hunk_header_regex = Regex::new(r"^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@").unwrap();

    // Helper function to finalize a file (add to results if it has changes)
    let finalize_file = |results: &mut Vec<FilePathInfo>,
                         processed_files: &mut HashSet<String>,
                         file_path: &PathBuf,
                         changed_lines: &HashSet<usize>,
                         allow_tests: bool,
                         debug_mode: bool| {
        // Only process if we have lines and haven't processed this file yet
        if !changed_lines.is_empty()
            && !processed_files.contains(&file_path.to_string_lossy().to_string())
        {
            // Skip test files if allow_tests is false
            let is_test = is_test_file(file_path);
            if !is_ignored_by_gitignore(file_path) && (allow_tests || !is_test) {
                if debug_mode {
                    println!(
                        "[DEBUG] Adding file with {} changed lines: {:?}",
                        changed_lines.len(),
                        file_path
                    );
                }
                // Use the min and max values in the HashSet for start and end lines
                let start_line = changed_lines.iter().min().cloned();
                let end_line = changed_lines.iter().max().cloned();

                // Pass both the start/end line numbers and the full set of lines
                results.push((
                    file_path.clone(),
                    start_line,
                    end_line,
                    None,
                    Some(changed_lines.clone()),
                ));
                processed_files.insert(file_path.to_string_lossy().to_string());
            } else if debug_mode {
                if is_ignored_by_gitignore(file_path) {
                    println!("[DEBUG] Skipping ignored file: {file_path:?}");
                } else if !allow_tests && is_test {
                    println!("[DEBUG] Skipping test file: {file_path:?}");
                }
            }
        }
    };

    // Use a manual index to process the lines
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];

        // Check for diff header
        if let Some(cap) = diff_header_regex.captures(line) {
            // When we find a new file, process any lines from the previous file
            if let Some(file_path) = &current_file {
                finalize_file(
                    &mut results,
                    &mut processed_files,
                    file_path,
                    &current_file_lines,
                    allow_tests,
                    debug_mode,
                );
            }

            // Use the 'b' path (new file) as the current file
            let file_path = cap.get(2).unwrap().as_str();
            current_file = Some(PathBuf::from(file_path));
            current_file_lines = HashSet::new(); // Reset lines for the new file

            if debug_mode {
                println!("[DEBUG] Found file in git diff: {file_path:?}");
            }

            i += 1;
            continue;
        }
        // Check for hunk header
        else if let Some(cap) = hunk_header_regex.captures(line) {
            if let Some(file_path) = &current_file {
                // Get the line numbers from the hunk header
                let new_start: usize = cap.get(3).unwrap().as_str().parse().unwrap_or(1);
                let _new_len: usize = cap
                    .get(4)
                    .map(|m| m.as_str().parse().unwrap_or(1))
                    .unwrap_or(1);

                if debug_mode {
                    println!(
                        "[DEBUG] Found hunk for file {file_path:?}: parsing for actual changed lines"
                    );
                }

                // Move to the next line after the hunk header
                i += 1;

                // Process lines within this hunk
                let mut current_line = new_start;
                while i < lines.len() {
                    let hunk_line = lines[i];

                    // Check if we've reached the next hunk or next diff
                    if hunk_line.starts_with("@@") || hunk_line.starts_with("diff --git") {
                        // Do not increment i here, so the outer loop sees this line
                        break;
                    }

                    // Process lines within the hunk
                    if hunk_line.starts_with('+') && !hunk_line.starts_with("+++") {
                        // This is an added/modified line in the new version
                        if debug_mode {
                            println!("[DEBUG] Found changed line at {current_line}: {hunk_line}");
                        }
                        current_file_lines.insert(current_line);
                    }

                    // Advance the line counter for all lines except removed lines
                    if !hunk_line.starts_with('-') {
                        current_line += 1;
                    }

                    i += 1;
                }

                // We've processed this hunk, continue to the next line
                continue;
            }
        }

        // If not a diff header or hunk header, just move on
        i += 1;
    }

    // Process any lines from the last file
    if let Some(file_path) = &current_file {
        finalize_file(
            &mut results,
            &mut processed_files,
            file_path,
            &current_file_lines,
            allow_tests,
            debug_mode,
        );
    }

    results
}

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
/// - File paths with symbol references (e.g., file.rs#function_name)
/// - Paths can be wrapped in backticks, single quotes, or double quotes
///
/// If allow_tests is false, test files will be filtered out.
pub fn extract_file_paths_from_text(text: &str, allow_tests: bool) -> Vec<FilePathInfo> {
    let mut results = Vec::new();
    let mut processed_paths = HashSet::new();

    // Check if debug mode is enabled
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    // Preprocess the text to handle paths wrapped in backticks or quotes
    // This replaces backticks, single quotes, and double quotes with spaces
    // around the path, making it easier to match with our regex patterns
    let mut preprocessed_text = String::with_capacity(text.len());
    let mut in_quote = false;
    let mut quote_char = ' ';
    let mut prev_char = ' ';

    for (i, c) in text.chars().enumerate() {
        let next_char = text.chars().nth(i + 1).unwrap_or(' ');

        // Check if this is an apostrophe within a word (like in "Here's")
        // An apostrophe is likely part of a word if:
        // 1. It's surrounded by alphanumeric characters (e.g., "don't", "O'Reilly")
        // 2. It's not at the beginning or end of the text
        let is_apostrophe_in_word =
            c == '\'' && prev_char.is_alphanumeric() && next_char.is_alphanumeric();

        if !in_quote && (c == '`' || c == '"' || (c == '\'' && !is_apostrophe_in_word)) {
            // Start of a quoted section
            in_quote = true;
            quote_char = c;
            preprocessed_text.push(' '); // Add space before the quoted content
        } else if in_quote && c == quote_char {
            // End of a quoted section
            in_quote = false;
            preprocessed_text.push(' '); // Add space after the quoted content
        } else {
            // Regular character
            preprocessed_text.push(c);
        }

        prev_char = c;
    }

    // Use the preprocessed text for regex matching
    let text = &preprocessed_text;

    // First, try to match file paths with symbol references (e.g., file.rs#function_name)
    let file_symbol_regex =
        Regex::new(r"(?:^|[\s\r\n])([a-zA-Z0-9_\-./\*\{\}]+\.[a-zA-Z0-9]+)#([a-zA-Z0-9_]+)")
            .unwrap();

    for cap in file_symbol_regex.captures_iter(text) {
        let file_path = cap.get(1).unwrap().as_str();
        let symbol = cap.get(2).unwrap().as_str();

        // We don't skip symbol references for the same file path
        // This allows multiple symbols from the same file to be extracted

        // Handle glob pattern
        if file_path.contains('*') || file_path.contains('{') {
            if let Ok(paths) = glob(file_path) {
                for entry in paths.flatten() {
                    // Check if the file should be ignored or is a test file
                    let is_test = is_test_file(&entry);
                    let should_include =
                        !is_ignored_by_gitignore(&entry) && (allow_tests || !is_test);
                    if should_include {
                        let path_str = entry.to_string_lossy().to_string();
                        processed_paths.insert(path_str.clone());
                        // Pass the symbol name directly instead of using environment variables
                        results.push((entry, None, None, Some(symbol.to_string()), None));
                    } else if debug_mode {
                        if is_ignored_by_gitignore(&entry) {
                            println!("DEBUG: Skipping ignored file: {entry:?}");
                        } else if !allow_tests && is_test {
                            println!("DEBUG: Skipping test file: {entry:?}");
                        }
                    }
                }
            }
        } else {
            // Check if the path needs special resolution
            match resolve_path(file_path) {
                Ok(resolved_path) => {
                    let is_test = is_test_file(&resolved_path);
                    if !is_ignored_by_gitignore(&resolved_path) && (allow_tests || !is_test) {
                        processed_paths.insert(file_path.to_string());
                        // Pass the symbol name directly instead of using environment variables
                        results.push((resolved_path, None, None, Some(symbol.to_string()), None));
                    } else if debug_mode {
                        if is_ignored_by_gitignore(&resolved_path) {
                            println!("DEBUG: Skipping ignored file: {file_path:?}");
                        } else if !allow_tests && is_test {
                            println!("DEBUG: Skipping test file: {file_path:?}");
                        }
                    }
                }
                Err(err) => {
                    if debug_mode {
                        println!("DEBUG: Failed to resolve path '{file_path}': {err}");
                    }

                    // Fall back to the original path
                    let path = PathBuf::from(file_path);
                    let is_test = is_test_file(&path);
                    if !is_ignored_by_gitignore(&path) && (allow_tests || !is_test) {
                        processed_paths.insert(file_path.to_string());
                        // Pass the symbol name directly instead of using environment variables
                        results.push((path, None, None, Some(symbol.to_string()), None));
                    } else if debug_mode {
                        if is_ignored_by_gitignore(&path) {
                            println!("DEBUG: Skipping ignored file: {file_path:?}");
                        } else if !allow_tests && is_test {
                            println!("DEBUG: Skipping test file: {file_path:?}");
                        }
                    }
                }
            }
        }
    }

    // Next, try to match file paths with line ranges (e.g., file.rs:1-60)
    let file_range_regex =
        Regex::new(r"(?:^|[\s\r\n])([a-zA-Z0-9_\-./\*\{\}]+\.[a-zA-Z0-9]+):(\d+)-(\d+)").unwrap();

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
                        // Check if the file should be ignored or is a test file
                        let is_test = is_test_file(&entry);
                        let should_include =
                            !is_ignored_by_gitignore(&entry) && (allow_tests || !is_test);
                        if should_include {
                            processed_paths.insert(entry.to_string_lossy().to_string());
                            results.push((entry, Some(start), Some(end), None, None));
                        } else if debug_mode {
                            if is_ignored_by_gitignore(&entry) {
                                println!("DEBUG: Skipping ignored file: {entry:?}");
                            } else if !allow_tests && is_test {
                                println!("DEBUG: Skipping test file: {entry:?}");
                            }
                        }
                    }
                }
            } else {
                // Check if the path needs special resolution
                match resolve_path(file_path) {
                    Ok(resolved_path) => {
                        let is_test = is_test_file(&resolved_path);
                        if !is_ignored_by_gitignore(&resolved_path) && (allow_tests || !is_test) {
                            processed_paths.insert(file_path.to_string());
                            results.push((resolved_path, Some(start), Some(end), None, None));
                        } else if debug_mode {
                            if is_ignored_by_gitignore(&resolved_path) {
                                println!("DEBUG: Skipping ignored file: {file_path:?}");
                            } else if !allow_tests && is_test {
                                println!("DEBUG: Skipping test file: {file_path:?}");
                            }
                        }
                    }
                    Err(err) => {
                        if debug_mode {
                            println!("DEBUG: Failed to resolve path '{file_path}': {err}");
                        }

                        // Fall back to the original path
                        let path = PathBuf::from(file_path);
                        let is_test = is_test_file(&path);
                        if !is_ignored_by_gitignore(&path) && (allow_tests || !is_test) {
                            processed_paths.insert(file_path.to_string());
                            results.push((path, Some(start), Some(end), None, None));
                        } else if debug_mode {
                            if is_ignored_by_gitignore(&path) {
                                println!("DEBUG: Skipping ignored file: {file_path:?}");
                            } else if !allow_tests && is_test {
                                println!("DEBUG: Skipping test file: {file_path:?}");
                            }
                        }
                    }
                }
            }
        }
    }

    // Then, try to match file paths with single line numbers (and optional column numbers)
    let file_line_regex =
        Regex::new(r"(?:^|[\s\r\n])([a-zA-Z0-9_\-./\*\{\}]+\.[a-zA-Z0-9]+):(\d+)(?::\d+)?")
            .unwrap();

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
                        // Check if the file should be ignored or is a test file
                        let is_test = is_test_file(&entry);
                        let should_include =
                            !is_ignored_by_gitignore(&entry) && (allow_tests || !is_test);
                        if should_include {
                            processed_paths.insert(path_str);
                            results.push((entry, line_num, None, None, None));
                        } else if debug_mode {
                            if is_ignored_by_gitignore(&entry) {
                                println!("DEBUG: Skipping ignored file: {entry:?}");
                            } else if !allow_tests && is_test {
                                println!("DEBUG: Skipping test file: {entry:?}");
                            }
                        }
                    }
                }
            }
        } else {
            // Check if the path needs special resolution
            match resolve_path(file_path) {
                Ok(path) => {
                    let is_test = is_test_file(&path);
                    if !is_ignored_by_gitignore(&path) && (allow_tests || !is_test) {
                        processed_paths.insert(file_path.to_string());
                        results.push((path, line_num, None, None, None));
                    } else if debug_mode {
                        if is_ignored_by_gitignore(&path) {
                            println!("DEBUG: Skipping ignored file: {file_path:?}");
                        } else if !allow_tests && is_test {
                            println!("DEBUG: Skipping test file: {file_path:?}");
                        }
                    }
                }
                Err(err) => {
                    if debug_mode {
                        println!("DEBUG: Failed to resolve path '{file_path}': {err}");
                    }

                    // Fall back to the original path
                    let path = PathBuf::from(file_path);
                    let is_test = is_test_file(&path);
                    if !is_ignored_by_gitignore(&path) && (allow_tests || !is_test) {
                        processed_paths.insert(file_path.to_string());
                        results.push((path, line_num, None, None, None));
                    } else if debug_mode {
                        if is_ignored_by_gitignore(&path) {
                            println!("DEBUG: Skipping ignored file: {file_path:?}");
                        } else if !allow_tests && is_test {
                            println!("DEBUG: Skipping test file: {file_path:?}");
                        }
                    }
                }
            }
        }
    }

    // Finally, match file paths without line numbers or symbols
    // But only if they haven't been processed already
    let simple_file_regex =
        Regex::new(r"(?:^|[\s\r\n])([a-zA-Z0-9_\-./\*\{\}]+\.[a-zA-Z0-9]+)").unwrap();

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
                            // Check if the file should be ignored or is a test file
                            let is_test = is_test_file(&entry);
                            let should_include =
                                !is_ignored_by_gitignore(&entry) && (allow_tests || !is_test);
                            if should_include {
                                processed_paths.insert(path_str);
                                results.push((entry, None, None, None, None));
                            } else if debug_mode {
                                if is_ignored_by_gitignore(&entry) {
                                    println!("DEBUG: Skipping ignored file: {entry:?}");
                                } else if !allow_tests && is_test {
                                    println!("DEBUG: Skipping test file: {entry:?}");
                                }
                            }
                        }
                    }
                }
            } else {
                // Check if the path needs special resolution
                match resolve_path(file_path) {
                    Ok(path) => {
                        let is_test = is_test_file(&path);
                        if !is_ignored_by_gitignore(&path) && (allow_tests || !is_test) {
                            results.push((path, None, None, None, None));
                            processed_paths.insert(file_path.to_string());
                        } else if debug_mode {
                            if is_ignored_by_gitignore(&path) {
                                println!("DEBUG: Skipping ignored file: {file_path:?}");
                            } else if !allow_tests && is_test {
                                println!("DEBUG: Skipping test file: {file_path:?}");
                            }
                        }
                    }
                    Err(err) => {
                        if debug_mode {
                            println!("DEBUG: Failed to resolve path '{file_path}': {err}");
                        }

                        // Fall back to the original path
                        let path = PathBuf::from(file_path);
                        let is_test = is_test_file(&path);
                        if !is_ignored_by_gitignore(&path) && (allow_tests || !is_test) {
                            results.push((path, None, None, None, None));
                            processed_paths.insert(file_path.to_string());
                        } else if debug_mode {
                            if is_ignored_by_gitignore(&path) {
                                println!("DEBUG: Skipping ignored file: {file_path:?}");
                            } else if !allow_tests && is_test {
                                println!("DEBUG: Skipping test file: {file_path:?}");
                            }
                        }
                    }
                }
            }
        }
    }

    results
}

/// Parse a file path with optional line number or range (e.g., "file.rs:10" or "file.rs:1-60")
///
/// If allow_tests is false, test files will be filtered out.
pub fn parse_file_with_line(input: &str, allow_tests: bool) -> Vec<FilePathInfo> {
    let mut results = Vec::new();

    // Remove any surrounding backticks or quotes, but not apostrophes within words
    // First check if the input starts and ends with the same quote character
    let first_char = input.chars().next().unwrap_or(' ');
    let last_char = input.chars().last().unwrap_or(' ');

    let cleaned_input = if (first_char == '`' || first_char == '\'' || first_char == '"')
        && first_char == last_char
    {
        // If the input is fully wrapped in quotes, remove them
        &input[1..input.len() - 1]
    } else {
        // Otherwise just trim any quotes at the beginning or end
        input.trim_matches(|c| c == '`' || c == '"')
    };

    // Check if the input contains a symbol reference (file#symbol or file#parent.child)
    if let Some((file_part, symbol)) = cleaned_input.split_once('#') {
        // For symbol references, we don't have line numbers yet
        // We'll need to find the symbol in the file later
        match resolve_path(file_part) {
            Ok(path) => {
                let is_test = is_test_file(&path);
                if allow_tests || !is_test {
                    // Symbol can be a simple name or a dot-separated path (e.g., "Class.method")
                    results.push((path, None, None, Some(symbol.to_string()), None));
                }
            }
            Err(err) => {
                if std::env::var("DEBUG").unwrap_or_default() == "1" {
                    println!("DEBUG: Failed to resolve path '{file_part}': {err}");
                }

                // Fall back to the original path
                let path = PathBuf::from(file_part);
                let is_test = is_test_file(&path);
                if allow_tests || !is_test {
                    // Symbol can be a simple name or a dot-separated path (e.g., "Class.method")
                    results.push((path, None, None, Some(symbol.to_string()), None));
                }
            }
        }
        return results;
    } else if let Some((file_part, rest)) = cleaned_input.split_once(':') {
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
                            // Check if the file should be ignored or is a test file
                            let is_test = is_test_file(&entry);
                            let should_include =
                                !is_ignored_by_gitignore(&entry) && (allow_tests || !is_test);
                            if should_include {
                                results.push((entry, Some(start), Some(end), None, None));
                            }
                        }
                    }
                } else {
                    // Check if the path needs special resolution
                    match resolve_path(file_part) {
                        Ok(path) => {
                            let is_test = is_test_file(&path);
                            if !is_ignored_by_gitignore(&path) && (allow_tests || !is_test) {
                                results.push((path, Some(start), Some(end), None, None));
                            }
                        }
                        Err(err) => {
                            if std::env::var("DEBUG").unwrap_or_default() == "1" {
                                println!("DEBUG: Failed to resolve path '{file_part}': {err}");
                            }

                            // Fall back to the original path
                            let path = PathBuf::from(file_part);
                            let is_test = is_test_file(&path);
                            if !is_ignored_by_gitignore(&path) && (allow_tests || !is_test) {
                                results.push((path, Some(start), Some(end), None, None));
                            }
                        }
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
                            // Check if the file should be ignored or is a test file
                            let is_test = is_test_file(&entry);
                            let should_include =
                                !is_ignored_by_gitignore(&entry) && (allow_tests || !is_test);
                            if should_include {
                                // Create a HashSet with just this line number
                                let mut lines_set = HashSet::new();
                                lines_set.insert(num);
                                results.push((entry, Some(num), None, None, Some(lines_set)));
                            }
                        }
                    }
                } else {
                    // Check if the path needs special resolution
                    match resolve_path(file_part) {
                        Ok(path) => {
                            let is_test = is_test_file(&path);
                            if !is_ignored_by_gitignore(&path) && (allow_tests || !is_test) {
                                // Create a HashSet with just this line number
                                let mut lines_set = HashSet::new();
                                lines_set.insert(num);
                                results.push((path, Some(num), None, None, Some(lines_set)));
                            }
                        }
                        Err(err) => {
                            if std::env::var("DEBUG").unwrap_or_default() == "1" {
                                println!("DEBUG: Failed to resolve path '{file_part}': {err}");
                            }

                            // Fall back to the original path
                            let path = PathBuf::from(file_part);
                            let is_test = is_test_file(&path);
                            if !is_ignored_by_gitignore(&path) && (allow_tests || !is_test) {
                                // Create a HashSet with just this line number
                                let mut lines_set = HashSet::new();
                                lines_set.insert(num);
                                results.push((path, Some(num), None, None, Some(lines_set)));
                            }
                        }
                    }
                }
            }
        }
    } else {
        // No line number or symbol specified, just a file path
        // Handle glob pattern
        if cleaned_input.contains('*') || cleaned_input.contains('{') {
            if let Ok(paths) = glob(cleaned_input) {
                for entry in paths.flatten() {
                    // Check if the file should be ignored or is a test file
                    let is_test = is_test_file(&entry);
                    let should_include =
                        !is_ignored_by_gitignore(&entry) && (allow_tests || !is_test);
                    if should_include {
                        results.push((entry, None, None, None, None));
                    }
                }
            }
        } else {
            // Check if the path needs special resolution (e.g., go:github.com/user/repo)
            match resolve_path(cleaned_input) {
                Ok(path) => {
                    let is_test = is_test_file(&path);
                    if !is_ignored_by_gitignore(&path) && (allow_tests || !is_test) {
                        results.push((path, None, None, None, None));
                    }
                }
                Err(err) => {
                    // If resolution fails, log the error and try with the original path
                    if std::env::var("DEBUG").unwrap_or_default() == "1" {
                        println!("DEBUG: Failed to resolve path '{cleaned_input}': {err}");
                    }

                    // Fall back to the original path
                    let path = PathBuf::from(cleaned_input);
                    let is_test = is_test_file(&path);
                    if !is_ignored_by_gitignore(&path) && (allow_tests || !is_test) {
                        results.push((path, None, None, None, None));
                    }
                }
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
