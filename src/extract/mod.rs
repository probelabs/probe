//! Extract command functionality for extracting code blocks from files.
//!
//! This module provides functions for extracting code blocks from files based on file paths
//! and optional line numbers. When a line number is specified, it uses tree-sitter to find
//! the closest suitable parent node (function, struct, class, etc.) for that line.

mod file_paths;
mod formatter;
mod processor;
mod symbol_finder;

// Re-export public functions
#[allow(unused_imports)]
pub use formatter::format_and_print_extraction_results;
#[allow(unused_imports)]
pub use processor::process_file_for_extraction;

use crate::extract::file_paths::FilePathInfo;
use anyhow::Result;
use std::io::Read;
#[allow(unused_imports)]
use std::path::PathBuf;

/// Handle the extract command
pub fn handle_extract(
    files: Vec<String>,
    allow_tests: bool,
    context_lines: usize,
    format: String,
    from_clipboard: bool,
    to_clipboard: bool,
) -> Result<()> {
    use arboard::Clipboard;
    use colored::*;

    // Check if debug mode is enabled
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    if debug_mode {
        println!("\n[DEBUG] ===== Extract Command Started =====");
        println!("[DEBUG] Files to process: {:?}", files);
        println!("[DEBUG] Allow tests: {}", allow_tests);
        println!("[DEBUG] Context lines: {}", context_lines);
        println!("[DEBUG] Output format: {}", format);
        println!("[DEBUG] Read from clipboard: {}", from_clipboard);
        println!("[DEBUG] Write to clipboard: {}", to_clipboard);
    }

    let mut file_paths: Vec<FilePathInfo> = Vec::new();

    if from_clipboard {
        // Read from clipboard
        println!("{}", "Reading from clipboard...".bold().blue());
        let mut clipboard = Clipboard::new()?;
        let buffer = clipboard.get_text()?;

        if debug_mode {
            println!(
                "[DEBUG] Reading from clipboard, content length: {} bytes",
                buffer.len()
            );
        }

        file_paths = file_paths::extract_file_paths_from_text(&buffer);

        if debug_mode {
            println!(
                "[DEBUG] Extracted {} file paths from clipboard",
                file_paths.len()
            );
            for (path, start, end, symbol) in &file_paths {
                println!(
                    "[DEBUG]   - {:?} (lines: {:?}-{:?}, symbol: {:?})",
                    path, start, end, symbol
                );
            }
        }

        if file_paths.is_empty() {
            println!("{}", "No file paths found in clipboard.".yellow().bold());
            return Ok(());
        }
    } else if files.is_empty() {
        // Read from stdin
        println!("{}", "Reading from stdin...".bold().blue());
        let mut buffer = String::new();
        std::io::stdin().read_to_string(&mut buffer)?;

        if debug_mode {
            println!(
                "[DEBUG] Reading from stdin, content length: {} bytes",
                buffer.len()
            );
        }

        file_paths = file_paths::extract_file_paths_from_text(&buffer);

        if debug_mode {
            println!(
                "[DEBUG] Extracted {} file paths from stdin",
                file_paths.len()
            );
            for (path, start, end, symbol) in &file_paths {
                println!(
                    "[DEBUG]   - {:?} (lines: {:?}-{:?}, symbol: {:?})",
                    path, start, end, symbol
                );
            }
        }

        if file_paths.is_empty() {
            println!("{}", "No file paths found in stdin.".yellow().bold());
            return Ok(());
        }
    } else {
        // Parse command-line arguments
        if debug_mode {
            println!("[DEBUG] Parsing command-line arguments");
        }

        for file in &files {
            if debug_mode {
                println!("[DEBUG] Parsing file argument: {}", file);
            }

            let paths = file_paths::parse_file_with_line(file);

            if debug_mode {
                println!(
                    "[DEBUG] Parsed {} paths from argument '{}'",
                    paths.len(),
                    file
                );
                for (path, start, end, symbol) in &paths {
                    println!(
                        "[DEBUG]   - {:?} (lines: {:?}-{:?}, symbol: {:?})",
                        path, start, end, symbol
                    );
                }
            }

            file_paths.extend(paths);
        }
    }

    // Only print file information for non-JSON/XML formats
    if format != "json" && format != "xml" {
        println!("{}", "Files to extract:".bold().green());

        for (path, start_line, end_line, symbol) in &file_paths {
            if let (Some(start), Some(end)) = (start_line, end_line) {
                println!("  {} (lines {}-{})", path.display(), start, end);
            } else if let Some(line_num) = start_line {
                println!("  {} (line {})", path.display(), line_num);
            } else if let Some(sym) = symbol {
                println!("  {} (symbol: {})", path.display(), sym);
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
    }

    let mut results = Vec::new();
    let mut errors = Vec::new();

    // Process each file
    for (path, start_line, end_line, symbol) in file_paths {
        if debug_mode {
            println!("\n[DEBUG] Processing file: {:?}", path);
            println!("[DEBUG] Start line: {:?}", start_line);
            println!("[DEBUG] End line: {:?}", end_line);
            println!("[DEBUG] Symbol: {:?}", symbol);

            // Check if file exists
            if path.exists() {
                println!("[DEBUG] File exists: Yes");

                // Get file extension and language
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    let language = formatter::get_language_from_extension(ext);
                    println!("[DEBUG] File extension: {}", ext);
                    println!(
                        "[DEBUG] Detected language: {}",
                        if language.is_empty() {
                            "unknown"
                        } else {
                            language
                        }
                    );
                } else {
                    println!("[DEBUG] File has no extension");
                }
            } else {
                println!("[DEBUG] File exists: No");
            }
        }

        match processor::process_file_for_extraction(
            &path,
            start_line,
            end_line,
            symbol.as_deref(),
            allow_tests,
            context_lines,
        ) {
            Ok(result) => {
                if debug_mode {
                    println!("[DEBUG] Successfully extracted code from {:?}", path);
                    println!("[DEBUG] Extracted lines: {:?}", result.lines);
                    println!("[DEBUG] Node type: {}", result.node_type);
                    println!("[DEBUG] Code length: {} bytes", result.code.len());
                    println!(
                        "[DEBUG] Estimated tokens: {}",
                        crate::search::search_tokens::count_tokens(&result.code)
                    );
                }
                results.push(result);
            }
            Err(e) => {
                let error_msg = format!("Error processing file {:?}: {}", path, e);
                if debug_mode {
                    println!("[DEBUG] Error: {}", error_msg);
                }
                // Only print error messages for non-JSON/XML formats
                if format != "json" && format != "xml" {
                    eprintln!("{}", error_msg.red());
                }
                errors.push(error_msg);
            }
        }
    }

    if debug_mode {
        println!("\n[DEBUG] ===== Extraction Summary =====");
        println!("[DEBUG] Total results: {}", results.len());
        println!("[DEBUG] Total errors: {}", errors.len());
        println!("[DEBUG] Output format: {}", format);
    }

    // Format the results
    let res = {
        // Temporarily disable colors if writing to clipboard
        let colors_enabled = if to_clipboard {
            let was_enabled = colored::control::SHOULD_COLORIZE.should_colorize();
            colored::control::set_override(false);
            was_enabled
        } else {
            false
        };

        // Format the results
        let result = formatter::format_extraction_results(&results, &format);

        // Restore color settings if they were changed
        if to_clipboard && colors_enabled {
            colored::control::set_override(true);
        }

        result
    };
    match res {
        Ok(formatted_output) => {
            if to_clipboard {
                // Write to clipboard
                let mut clipboard = Clipboard::new()?;
                clipboard.set_text(&formatted_output)?;
                println!("{}", "Results copied to clipboard.".green().bold());

                if debug_mode {
                    println!(
                        "[DEBUG] Wrote {} bytes to clipboard",
                        formatted_output.len()
                    );
                }
            } else {
                // Print to stdout
                println!("{}", formatted_output);
            }
        }
        Err(e) => {
            // Only print error messages for non-JSON/XML formats
            if format != "json" && format != "xml" {
                eprintln!("{}", format!("Error formatting results: {}", e).red());
            }
            if debug_mode {
                println!("[DEBUG] Error formatting results: {}", e);
            }
        }
    }

    // Print summary of errors if any (only for non-JSON/XML formats)
    if !errors.is_empty() && format != "json" && format != "xml" {
        println!();
        println!(
            "{} {} {}",
            "Encountered".red().bold(),
            errors.len(),
            if errors.len() == 1 { "error" } else { "errors" }
        );
    }

    if debug_mode {
        println!("[DEBUG] ===== Extract Command Completed =====");
    }

    Ok(())
}
