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
pub use formatter::format_extraction_dry_run;
#[allow(unused_imports)]
pub use processor::process_file_for_extraction;
#[allow(unused_imports)]
pub use file_paths::{extract_file_paths_from_git_diff, extract_file_paths_from_text, is_git_diff_format, parse_file_with_line};

use crate::extract::file_paths::{set_custom_ignores, FilePathInfo};
use anyhow::Result;
use std::io::Read;
#[allow(unused_imports)]
use std::path::PathBuf;

/// Options for the extract command
pub struct ExtractOptions {
    /// Files to extract from
    pub files: Vec<String>,
    /// Custom patterns to ignore
    pub custom_ignores: Vec<String>,
    /// Number of context lines to include
    pub context_lines: usize,
    /// Output format
    pub format: String,
    /// Whether to read from clipboard
    pub from_clipboard: bool,
    /// Whether to write to clipboard
    pub to_clipboard: bool,
    /// Whether to perform a dry run
    pub dry_run: bool,
    /// Whether to parse input as git diff format
    pub diff: bool,
    /// Whether to allow test files and test code blocks
    pub allow_tests: bool,
}

/// Handle the extract command
pub fn handle_extract(options: ExtractOptions) -> Result<()> {
    use arboard::Clipboard;
    use colored::*;

    // Check if debug mode is enabled
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    if debug_mode {
        println!("\n[DEBUG] ===== Extract Command Started =====");
        println!("[DEBUG] Files to process: {:?}", options.files);
        println!("[DEBUG] Custom ignores: {:?}", options.custom_ignores);
        println!("[DEBUG] Context lines: {}", options.context_lines);
        println!("[DEBUG] Output format: {}", options.format);
        println!("[DEBUG] Read from clipboard: {}", options.from_clipboard);
        println!("[DEBUG] Write to clipboard: {}", options.to_clipboard);
        println!("[DEBUG] Dry run: {}", options.dry_run);
        println!("[DEBUG] Parse as git diff: {}", options.diff);
        println!("[DEBUG] Allow tests: {}", options.allow_tests);
    }

    // Set custom ignore patterns
    set_custom_ignores(&options.custom_ignores);

    let mut file_paths: Vec<FilePathInfo> = Vec::new();

    if options.from_clipboard {
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

        // Auto-detect git diff format or use explicit flag
        let is_diff_format = options.diff || is_git_diff_format(&buffer);
        
        if is_diff_format {
            // Parse as git diff format
            if debug_mode {
                println!("[DEBUG] Parsing clipboard content as git diff format");
            }
            file_paths = extract_file_paths_from_git_diff(&buffer, options.allow_tests);
        } else {
            // Parse as regular text
            file_paths = file_paths::extract_file_paths_from_text(&buffer, options.allow_tests);
        }

        if debug_mode {
            println!(
                "[DEBUG] Extracted {} file paths from clipboard",
                file_paths.len()
            );
            for (path, start, end, symbol, lines) in &file_paths {
                println!(
                    "[DEBUG]   - {:?} (lines: {:?}-{:?}, symbol: {:?}, specific lines: {:?})",
                    path, start, end, symbol, lines.as_ref().map(|l| l.len())
                );
            }
        }

        if file_paths.is_empty() {
            println!("{}", "No file paths found in clipboard.".yellow().bold());
            return Ok(());
        }
    } else if options.files.is_empty() {
        // Check if stdin is available (not a terminal)
        let is_stdin_available = !atty::is(atty::Stream::Stdin);
        
        if is_stdin_available {
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

            // Auto-detect git diff format or use explicit flag
            let is_diff_format = options.diff || is_git_diff_format(&buffer);
            
            if is_diff_format {
                // Parse as git diff format
                if debug_mode {
                    println!("[DEBUG] Parsing stdin content as git diff format");
                }
                file_paths = extract_file_paths_from_git_diff(&buffer, options.allow_tests);
            } else {
                // Parse as regular text
                file_paths = file_paths::extract_file_paths_from_text(&buffer, options.allow_tests);
            }
        } else {
            // No arguments and no stdin, show help
            println!("{}", "No files specified and no stdin input detected.".yellow().bold());
            println!("{}", "Use --help for usage information.".blue());
            return Ok(());
        }

        if debug_mode {
            println!(
                "[DEBUG] Extracted {} file paths from stdin",
                file_paths.len()
            );
            for (path, start, end, symbol, lines) in &file_paths {
                println!(
                    "[DEBUG]   - {:?} (lines: {:?}-{:?}, symbol: {:?}, specific lines: {:?})",
                    path, start, end, symbol, lines.as_ref().map(|l| l.len())
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

        for file in &options.files {
            if debug_mode {
                println!("[DEBUG] Parsing file argument: {}", file);
            }

            let paths = file_paths::parse_file_with_line(file, options.allow_tests);

            if debug_mode {
                println!(
                    "[DEBUG] Parsed {} paths from argument '{}'",
                    paths.len(),
                    file
                );
                for (path, start, end, symbol, lines) in &paths {
                    println!(
                        "[DEBUG]   - {:?} (lines: {:?}-{:?}, symbol: {:?}, specific lines: {:?})",
                        path, start, end, symbol, lines.as_ref().map(|l| l.len())
                    );
                }
            }

            file_paths.extend(paths);
        }
    }

    // Only print file information for non-JSON/XML formats
    if options.format != "json" && options.format != "xml" {
        println!("{}", "Files to extract:".bold().green());

        for (path, start_line, end_line, symbol, lines) in &file_paths {
            if let (Some(start), Some(end)) = (start_line, end_line) {
                println!("  {} (lines {}-{})", path.display(), start, end);
            } else if let Some(line_num) = start_line {
                println!("  {} (line {})", path.display(), line_num);
            } else if let Some(sym) = symbol {
                println!("  {} (symbol: {})", path.display(), sym);
            } else if let Some(lines_set) = lines {
                println!("  {} (specific lines: {} lines)", path.display(), lines_set.len());
            } else {
                println!("  {}", path.display());
            }
        }

        if options.context_lines > 0 {
            println!("Context lines: {}", options.context_lines);
        }

        if options.dry_run {
            println!("{}", "Dry run (file names and lines only)".yellow());
        }

        println!("Format: {}", options.format);
        println!();
    }

    let mut results = Vec::new();
    let mut errors = Vec::new();

    // Process each file
    for (path, start_line, end_line, symbol, specific_lines) in file_paths {
        if debug_mode {
            println!("\n[DEBUG] Processing file: {:?}", path);
            println!("[DEBUG] Start line: {:?}", start_line);
            println!("[DEBUG] End line: {:?}", end_line);
            println!("[DEBUG] Symbol: {:?}", symbol);
            println!("[DEBUG] Specific lines: {:?}", specific_lines.as_ref().map(|l| l.len()));

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

        // The allow_tests check is now handled in the file path extraction functions
        // We only need to check if this is a test file for debugging purposes
        if debug_mode && crate::language::is_test_file(&path) && !options.allow_tests {
            println!("[DEBUG] Test file detected: {:?}", path);
        }
        
        match processor::process_file_for_extraction(
            &path,
            start_line,
            end_line,
            symbol.as_deref(),
            options.allow_tests,
            options.context_lines,
            specific_lines.as_ref(),
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
                if options.format != "json" && options.format != "xml" {
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
        println!("[DEBUG] Output format: {}", options.format);
        println!("[DEBUG] Dry run: {}", options.dry_run);
    }

    // Format the results
    let res = {
        // Temporarily disable colors if writing to clipboard
        let colors_enabled = if options.to_clipboard {
            let was_enabled = colored::control::SHOULD_COLORIZE.should_colorize();
            colored::control::set_override(false);
            was_enabled
        } else {
            false
        };

        // Format the results
        let result = if options.dry_run {
            formatter::format_extraction_dry_run(&results, &options.format)
        } else {
            formatter::format_extraction_results(&results, &options.format)
        };

        // Restore color settings if they were changed
        if options.to_clipboard && colors_enabled {
            colored::control::set_override(true);
        }

        result
    };
    match res {
        Ok(formatted_output) => {
            if options.to_clipboard {
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
            if options.format != "json" && options.format != "xml" {
                eprintln!("{}", format!("Error formatting results: {}", e).red());
            }
            if debug_mode {
                println!("[DEBUG] Error formatting results: {}", e);
            }
        }
    }

    // Print summary of errors if any (only for non-JSON/XML formats)
    if !errors.is_empty() && options.format != "json" && options.format != "xml" {
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
