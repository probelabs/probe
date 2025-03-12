use anyhow::Result;
use clap::Parser as ClapParser;
use colored::*;
use std::path::PathBuf;
use std::time::Instant;

mod chat;
mod cli;
mod language;
mod models;
mod query;
mod ranking;
mod search;

use cli::{Args, Commands};
use search::{format_and_print_search_results, perform_probe, SearchOptions};

struct SearchParams {
    pattern: String,
    paths: Vec<PathBuf>,
    files_only: bool,
    ignore: Vec<String>,
    exclude_filenames: bool,
    reranker: String,
    frequency_search: bool,
    exact: bool,
    max_results: Option<usize>,
    max_bytes: Option<usize>,
    max_tokens: Option<usize>,
    allow_tests: bool,
    no_merge: bool,
    merge_threshold: Option<usize>,
    dry_run: bool,
    format: String,
}

fn handle_search(params: SearchParams) -> Result<()> {
    let use_frequency = if params.exact {
        false
    } else {
        params.frequency_search
    };

    println!("{} {}", "Pattern:".bold().green(), params.pattern);
    println!(
        "{} {}",
        "Path:".bold().green(),
        params.paths.first().unwrap().display()
    );

    // Show advanced options if they differ from defaults
    let mut advanced_options = Vec::<String>::new();
    if params.files_only {
        advanced_options.push("Files only".to_string());
    }
    if params.exclude_filenames {
        advanced_options.push("Exclude filenames".to_string());
    }
    if params.reranker != "hybrid" {
        advanced_options.push(format!("Reranker: {}", params.reranker));
    }
    if !use_frequency {
        advanced_options.push("Frequency search disabled".to_string());
    }
    if params.exact {
        advanced_options.push("Exact match".to_string());
    }
    if params.allow_tests {
        advanced_options.push("Including tests".to_string());
    }
    if params.no_merge {
        advanced_options.push("No block merging".to_string());
    }
    if let Some(threshold) = params.merge_threshold {
        advanced_options.push(format!("Merge threshold: {}", threshold));
    }
    if params.dry_run {
        advanced_options.push("Dry run (file names and lines only)".to_string());
    }

    if !advanced_options.is_empty() {
        println!(
            "{} {}",
            "Options:".bold().green(),
            advanced_options.join(", ")
        );
    }

    let start_time = Instant::now();

    // Create a vector with the pattern
    let query = vec![params.pattern.clone()];

    let search_options = SearchOptions {
        path: params.paths.first().unwrap(),
        queries: &query,
        files_only: params.files_only,
        custom_ignores: &params.ignore,
        exclude_filenames: params.exclude_filenames,
        reranker: &params.reranker,
        frequency_search: use_frequency,
        exact: params.exact,
        max_results: params.max_results,
        max_bytes: params.max_bytes,
        max_tokens: params.max_tokens,
        allow_tests: params.allow_tests,
        no_merge: params.no_merge,
        merge_threshold: params.merge_threshold,
        dry_run: params.dry_run,
    };

    let limited_results = perform_probe(&search_options)?;

    // Calculate search time
    let duration = start_time.elapsed();

    if limited_results.results.is_empty() {
        println!("{}", "No results found.".yellow().bold());
        println!("Search completed in {:.2?}", duration);
    } else {
        println!("Search completed in {:.2?}", duration);
        println!();

        // Pass the query plan to the format_and_print_search_results function
        // We need to recreate the query plan here since we don't have access to it from perform_probe
        let query_plan = if search_options.queries.len() > 1 {
            // Join multiple queries with AND
            let combined_query = search_options.queries.join(" AND ");
            crate::search::query::create_query_plan(&combined_query, search_options.exact).ok()
        } else {
            crate::search::query::create_query_plan(
                &search_options.queries[0],
                search_options.exact,
            )
            .ok()
        };

        format_and_print_search_results(
            &limited_results.results,
            search_options.dry_run,
            &params.format,
            query_plan.as_ref(),
        );

        if !limited_results.skipped_files.is_empty() {
            if let Some(limits) = &limited_results.limits_applied {
                println!();
                println!("{}", "Limits applied:".yellow().bold());
                if let Some(max_results) = limits.max_results {
                    println!("  {} {}", "Max results:".yellow(), max_results);
                }
                if let Some(max_bytes) = limits.max_bytes {
                    println!("  {} {}", "Max bytes:".yellow(), max_bytes);
                }
                if let Some(max_tokens) = limits.max_tokens {
                    println!("  {} {}", "Max tokens:".yellow(), max_tokens);
                }

                println!();
                println!(
                    "{} {}",
                    "Skipped files due to limits:".yellow().bold(),
                    limited_results.skipped_files.len()
                );
            }
        }
    }

    Ok(())
}

use regex::Regex;
use std::collections::HashSet;
use std::io::{self, Read};

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
fn extract_file_paths_from_text(text: &str) -> Vec<(PathBuf, Option<usize>, Option<usize>)> {
    let mut results = Vec::new();
    let mut processed_paths = HashSet::new();

    // First, try to match file paths with line ranges (e.g., file.rs:1-60)
    let file_range_regex =
        Regex::new(r"(?:^|\s)([a-zA-Z0-9_\-./]+\.[a-zA-Z0-9]+):(\d+)-(\d+)").unwrap();

    for cap in file_range_regex.captures_iter(text) {
        let file_path = cap.get(1).unwrap().as_str();
        let start_line = cap.get(2).and_then(|m| m.as_str().parse::<usize>().ok());
        let end_line = cap.get(3).and_then(|m| m.as_str().parse::<usize>().ok());

        if let (Some(start), Some(end)) = (start_line, end_line) {
            processed_paths.insert(file_path.to_string());
            results.push((PathBuf::from(file_path), Some(start), Some(end)));
        }
    }

    // Then, try to match file paths with single line numbers (and optional column numbers)
    let file_line_regex =
        Regex::new(r"(?:^|\s)([a-zA-Z0-9_\-./]+\.[a-zA-Z0-9]+):(\d+)(?::\d+)?").unwrap();

    for cap in file_line_regex.captures_iter(text) {
        let file_path = cap.get(1).unwrap().as_str();

        // Skip if we've already processed this path with a line range
        if processed_paths.contains(file_path) {
            continue;
        }

        let line_num = cap.get(2).and_then(|m| m.as_str().parse::<usize>().ok());

        processed_paths.insert(file_path.to_string());
        results.push((PathBuf::from(file_path), line_num, None));
    }

    // Finally, match file paths without line numbers
    // We use a simpler regex and filter out paths we've already processed
    let simple_file_regex = Regex::new(r"(?:^|\s)([a-zA-Z0-9_\-./]+\.[a-zA-Z0-9]+)").unwrap();

    for cap in simple_file_regex.captures_iter(text) {
        let file_path = cap.get(1).unwrap().as_str();

        // Skip if we've already processed this path with a line number or range
        if !processed_paths.contains(file_path) {
            results.push((PathBuf::from(file_path), None, None));
            processed_paths.insert(file_path.to_string());
        }
    }

    results
}

/// Parse a file path with optional line number or range (e.g., "file.rs:10" or "file.rs:1-60")
fn parse_file_with_line(input: &str) -> (PathBuf, Option<usize>, Option<usize>) {
    if let Some((file_part, rest)) = input.split_once(':') {
        // Extract the line specification from the rest (which might contain more colons)
        let line_spec = rest.split(':').next().unwrap_or("");

        // Check if it's a range (contains a hyphen)
        if let Some((start_str, end_str)) = line_spec.split_once('-') {
            let start_num = start_str.parse::<usize>().ok();
            let end_num = end_str.parse::<usize>().ok();

            if let (Some(start), Some(end)) = (start_num, end_num) {
                return (PathBuf::from(file_part), Some(start), Some(end));
            }
        } else {
            // Try to parse as a single line number
            let line_num = line_spec.parse::<usize>().ok();

            if let Some(num) = line_num {
                return (PathBuf::from(file_part), Some(num), None);
            }
        }
    }
    (PathBuf::from(input), None, None)
}

/// Handle the extract command
fn handle_extract(
    files: Vec<String>,
    allow_tests: bool,
    context_lines: usize,
    format: String,
) -> Result<()> {
    use colored::*;
    use probe::extract::{format_and_print_extraction_results, process_file_for_extraction};

    let mut file_paths = Vec::new();

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
            let (path, start_line, end_line) = parse_file_with_line(&file);
            file_paths.push((path, start_line, end_line));
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

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        // When no subcommand provided, fallback to search mode
        None => {
            // Use provided pattern or default to empty string
            let pattern = args.pattern.unwrap_or_else(String::new);

            // Use provided paths or default to current directory
            let paths = if args.paths.is_empty() {
                vec![std::path::PathBuf::from(".")]
            } else {
                args.paths
            };

            handle_search(SearchParams {
                pattern,
                paths,
                files_only: args.files_only,
                ignore: args.ignore,
                exclude_filenames: args.exclude_filenames,
                reranker: args.reranker,
                frequency_search: args.frequency_search,
                exact: args.exact,
                max_results: args.max_results,
                max_bytes: args.max_bytes,
                max_tokens: args.max_tokens,
                allow_tests: args.allow_tests,
                no_merge: args.no_merge,
                merge_threshold: args.merge_threshold,
                dry_run: args.dry_run,
                format: args.format,
            })?
        }
        Some(Commands::Search {
            pattern,
            paths,
            files_only,
            ignore,
            exclude_filenames,
            reranker,
            frequency_search,
            exact,
            max_results,
            max_bytes,
            max_tokens,
            allow_tests,
            no_merge,
            merge_threshold,
            dry_run,
            format,
        }) => handle_search(SearchParams {
            pattern,
            paths,
            files_only,
            ignore,
            exclude_filenames,
            reranker,
            frequency_search,
            exact,
            max_results,
            max_bytes,
            max_tokens,
            allow_tests,
            no_merge,
            merge_threshold,
            dry_run,
            format,
        })?,
        Some(Commands::Extract {
            files,
            allow_tests,
            context_lines,
            format,
        }) => handle_extract(files, allow_tests, context_lines, format)?,
        Some(Commands::Query {
            pattern,
            path,
            language,
            ignore,
            allow_tests,
            max_results,
            format,
        }) => query::handle_query(
            &pattern,
            &path,
            language.as_deref(),
            &ignore,
            allow_tests,
            max_results,
            &format,
        )?,
        Some(Commands::Chat) => probe::handle_chat().await?,
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_file_with_line() {
        // Test with no line number
        let (path, start, end) = parse_file_with_line("src/main.rs");
        assert_eq!(path, PathBuf::from("src/main.rs"));
        assert_eq!(start, None);
        assert_eq!(end, None);

        // Test with line number
        let (path, start, end) = parse_file_with_line("src/main.rs:42");
        assert_eq!(path, PathBuf::from("src/main.rs"));
        assert_eq!(start, Some(42));
        assert_eq!(end, None);

        // Test with line range
        let (path, start, end) = parse_file_with_line("src/main.rs:1-60");
        assert_eq!(path, PathBuf::from("src/main.rs"));
        assert_eq!(start, Some(1));
        assert_eq!(end, Some(60));

        // Test with invalid line number
        let (path, start, end) = parse_file_with_line("src/main.rs:abc");
        assert_eq!(path, PathBuf::from("src/main.rs:abc"));
        assert_eq!(start, None);
        assert_eq!(end, None);

        // Test with multiple colons (should extract the first number after the first colon)
        let (path, start, end) = parse_file_with_line("src/main.rs:42:10");
        assert_eq!(path, PathBuf::from("src/main.rs"));
        assert_eq!(start, Some(42));
        assert_eq!(end, None);

        // Test with the format from compiler/editor error messages (file:line:column)
        let (path, start, end) = parse_file_with_line("tests/extract_command_tests.rs:214:41");
        assert_eq!(path, PathBuf::from("tests/extract_command_tests.rs"));
        assert_eq!(start, Some(214));
        assert_eq!(end, None);
    }

    #[test]
    fn test_extract_file_paths_from_text() {
        // Test with error message
        let text = "Error in file src/main.rs:42: something went wrong";
        let paths = extract_file_paths_from_text(text);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].0, PathBuf::from("src/main.rs"));
        assert_eq!(paths[0].1, Some(42));
        assert_eq!(paths[0].2, None);

        // Test with line range
        let text = "Extract lines src/main.rs:1-60 for the example";
        let paths = extract_file_paths_from_text(text);
        assert_eq!(paths.len(), 1);
        assert_eq!(paths[0].0, PathBuf::from("src/main.rs"));
        assert_eq!(paths[0].1, Some(1));
        assert_eq!(paths[0].2, Some(60));

        // Test with backtrace
        let text = r#"
        Backtrace:
        0: core::panicking::panic_fmt
        1: src/lib.rs:15
        2: src/main.rs:42
        3: src/cli.rs:10: in function parse_args
        "#;
        let paths = extract_file_paths_from_text(text);
        assert_eq!(paths.len(), 3);
        assert_eq!(paths[0].0, PathBuf::from("src/lib.rs"));
        assert_eq!(paths[0].1, Some(15));
        assert_eq!(paths[0].2, None);
        assert_eq!(paths[1].0, PathBuf::from("src/main.rs"));
        assert_eq!(paths[1].1, Some(42));
        assert_eq!(paths[1].2, None);
        assert_eq!(paths[2].0, PathBuf::from("src/cli.rs"));
        assert_eq!(paths[2].1, Some(10));
        assert_eq!(paths[2].2, None);

        // Test with no file paths
        let text = "This text contains no file paths";
        let paths = extract_file_paths_from_text(text);
        assert_eq!(paths.len(), 0);

        // Test with multiple file paths on one line
        let text = "Files: src/main.rs:10 src/cli.rs:20 src/lib.rs";
        let paths = extract_file_paths_from_text(text);
        assert_eq!(paths.len(), 3);
        assert_eq!(paths[0].0, PathBuf::from("src/main.rs"));
        assert_eq!(paths[0].1, Some(10));
        assert_eq!(paths[0].2, None);
        assert_eq!(paths[1].0, PathBuf::from("src/cli.rs"));
        assert_eq!(paths[1].1, Some(20));
        assert_eq!(paths[1].2, None);
        assert_eq!(paths[2].0, PathBuf::from("src/lib.rs"));
        assert_eq!(paths[2].1, None);
        assert_eq!(paths[2].2, None);

        // Test with file:line:column format (common in compiler/editor error messages)
        let text = "Error at tests/extract_command_tests.rs:214:41: unexpected token";
        let paths = extract_file_paths_from_text(text);
        assert!(paths.iter().any(|(path, line, _)| path
            == &PathBuf::from("tests/extract_command_tests.rs")
            && *line == Some(214)));

        // Test with multiple file:line:column formats
        let text = r#"
        Error:
        - tests/extract_command_tests.rs:214:41: unexpected token
        - src/main.rs:42:10: missing semicolon
        "#;
        let paths = extract_file_paths_from_text(text);
        assert_eq!(paths.len(), 2);
        assert!(paths.iter().any(|(path, line, _)| path
            == &PathBuf::from("tests/extract_command_tests.rs")
            && *line == Some(214)));
        assert!(paths
            .iter()
            .any(|(path, line, _)| path == &PathBuf::from("src/main.rs") && *line == Some(42)));

        // Test with mixed line numbers and ranges
        let text = r#"
        Files to process:
        - src/main.rs:1-60
        - src/lib.rs:42
        - src/cli.rs
        "#;
        let paths = extract_file_paths_from_text(text);
        assert_eq!(paths.len(), 3);
        assert!(paths
            .iter()
            .any(|(path, start, end)| path == &PathBuf::from("src/main.rs")
                && *start == Some(1)
                && *end == Some(60)));
        assert!(paths
            .iter()
            .any(|(path, line, end)| path == &PathBuf::from("src/lib.rs")
                && *line == Some(42)
                && end.is_none()));
        assert!(paths
            .iter()
            .any(|(path, line, end)| path == &PathBuf::from("src/cli.rs")
                && line.is_none()
                && end.is_none()));
    }
}
