use anyhow::Result;
use clap::{CommandFactory, Parser as ClapParser};
use colored::*;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

mod cli;
mod grep;

use cli::{Args, Commands};
use probe_code::{
    extract::{handle_extract, ExtractOptions},
    search::{format_and_print_search_results, perform_probe, SearchOptions},
};

struct SearchParams {
    pattern: String,
    paths: Vec<PathBuf>,
    files_only: bool,
    ignore: Vec<String>,
    exclude_filenames: bool,
    reranker: String,
    frequency_search: bool,
    exact: bool,
    language: Option<String>,
    max_results: Option<usize>,
    max_bytes: Option<usize>,
    max_tokens: Option<usize>,
    allow_tests: bool,
    no_merge: bool,
    merge_threshold: Option<usize>,
    dry_run: bool,
    format: String,
    session: Option<String>,
    timeout: u64,
    question: Option<String>,
    no_gitignore: bool,
}

struct BenchmarkParams {
    bench: Option<String>,
    #[allow(dead_code)]
    sample_size: Option<usize>,
    #[allow(dead_code)]
    format: String,
    output: Option<String>,
    #[allow(dead_code)]
    compare: bool,
    #[allow(dead_code)]
    baseline: Option<String>,
    #[allow(dead_code)]
    fast: bool,
}

fn handle_search(params: SearchParams) -> Result<()> {
    // Print version at the start for text-based formats
    if params.format != "json" && params.format != "xml" {
        println!("Probe version: {}", probe_code::version::get_version());
    }

    let use_frequency = params.frequency_search;

    // Don't print these headers for JSON/XML formats
    if params.format != "json" && params.format != "xml" {
        println!("{} {}", "Pattern:".bold().green(), params.pattern);
        println!(
            "{} {}",
            "Path:".bold().green(),
            params.paths.first().unwrap().display()
        );
    }

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
    if let Some(lang) = &params.language {
        advanced_options.push(format!("Language: {lang}"));
    }
    if params.allow_tests {
        advanced_options.push("Including tests".to_string());
    }
    if params.no_gitignore {
        advanced_options.push("Ignoring .gitignore".to_string());
    }
    if params.no_merge {
        advanced_options.push("No block merging".to_string());
    }
    if let Some(threshold) = params.merge_threshold {
        advanced_options.push(format!("Merge threshold: {threshold}"));
    }
    if params.dry_run {
        advanced_options.push("Dry run (file names and lines only)".to_string());
    }
    if let Some(session) = &params.session {
        advanced_options.push(format!("Session: {session}"));
    }

    // Show timeout if it's not the default value of 30 seconds
    if params.timeout != 30 {
        advanced_options.push(format!("Timeout: {} seconds", params.timeout));
    }

    if !advanced_options.is_empty() && params.format != "json" && params.format != "xml" {
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
        language: params.language.as_deref(),
        max_results: params.max_results,
        max_bytes: params.max_bytes,
        max_tokens: params.max_tokens,
        allow_tests: params.allow_tests,
        no_merge: params.no_merge,
        merge_threshold: params.merge_threshold,
        dry_run: params.dry_run,
        session: params.session.as_deref(),
        timeout: params.timeout,
        question: params.question.as_deref(),
        no_gitignore: params.no_gitignore,
        format: Some(&params.format),
    };

    let limited_results = perform_probe(&search_options)?;

    // Calculate search time
    let duration = start_time.elapsed();

    // Create the query plan regardless of whether we have results
    let query_plan = if search_options.queries.len() > 1 {
        // Join multiple queries with AND
        let combined_query = search_options.queries.join(" AND ");
        probe_code::search::query::create_query_plan(&combined_query, false).ok()
    } else {
        probe_code::search::query::create_query_plan(&search_options.queries[0], false).ok()
    };

    if limited_results.results.is_empty() {
        // For JSON and XML formats, still call format_and_print_search_results
        if params.format == "json" || params.format == "xml" {
            format_and_print_search_results(
                &limited_results.results,
                search_options.dry_run,
                &params.format,
                query_plan.as_ref(),
                Some(&limited_results.skipped_files),
                limited_results.limits_applied.as_ref(),
            );
        } else {
            // For other formats, print the "No results found" message
            println!("{}", "No results found.".yellow().bold());
            println!("Search completed in {duration:.2?}");
        }
    } else {
        // For non-JSON/XML formats, print search time
        if params.format != "json" && params.format != "xml" {
            println!("Search completed in {duration:.2?}");
            println!();
        }

        format_and_print_search_results(
            &limited_results.results,
            search_options.dry_run,
            &params.format,
            query_plan.as_ref(),
            Some(&limited_results.skipped_files),
            limited_results.limits_applied.as_ref(),
        );

        // Don't print skipped files info for JSON/XML/outline-xml formats (they include it in structured output)
        if !limited_results.skipped_files.is_empty()
            && params.format != "json"
            && params.format != "xml"
            && params.format != "outline-xml"
        {
            let use_stderr = false;

            // Helper macro to print to stdout or stderr based on format
            macro_rules! output {
                ($($arg:tt)*) => {
                    if use_stderr {
                        eprintln!($($arg)*);
                    } else {
                        println!($($arg)*);
                    }
                };
            }

            if let Some(limits) = &limited_results.limits_applied {
                output!();
                output!("{}", "Limits applied:".yellow().bold());
                if let Some(max_results) = limits.max_results {
                    output!("  {} {max_results}", "Max results:".yellow());
                }
                if let Some(max_bytes) = limits.max_bytes {
                    output!("  {} {max_bytes}", "Max bytes:".yellow());
                }
                if let Some(max_tokens) = limits.max_tokens {
                    output!("  {} {max_tokens}", "Max tokens:".yellow());
                }

                output!();

                // Calculate total skipped files (results skipped + files not processed)
                let results_skipped = limited_results.skipped_files.len();
                let files_not_processed =
                    limited_results.files_skipped_early_termination.unwrap_or(0);
                let total_skipped = results_skipped + files_not_processed;

                output!(
                    "{} {}",
                    "Skipped files due to limits:".yellow().bold(),
                    total_skipped
                );

                // Show list of skipped files with match counts
                if results_skipped > 0 {
                    output!();
                    output!("{}", "Remaining files not shown:".yellow());

                    // Group skipped files by file path and aggregate match counts
                    let mut file_matches: HashMap<String, (HashSet<String>, usize)> =
                        HashMap::new();

                    for skipped in &limited_results.skipped_files {
                        // Convert to relative path
                        let relative_path =
                            if let Ok(abs_path) = std::fs::canonicalize(&skipped.file) {
                                if let Ok(current_dir) = std::env::current_dir() {
                                    if let Ok(rel) = abs_path.strip_prefix(&current_dir) {
                                        rel.to_string_lossy().to_string()
                                    } else {
                                        skipped.file.clone()
                                    }
                                } else {
                                    skipped.file.clone()
                                }
                            } else {
                                skipped.file.clone()
                            };

                        let entry = file_matches
                            .entry(relative_path)
                            .or_insert((HashSet::new(), 0));

                        // Count unique terms (unique matches)
                        if let Some(keywords) = &skipped.matched_keywords {
                            for keyword in keywords {
                                entry.0.insert(keyword.clone());
                            }
                        }

                        // Count total matches (all occurrences)
                        entry.1 += 1;
                    }

                    // Convert to sorted vec for consistent display
                    let mut file_list: Vec<(String, usize, usize)> = file_matches
                        .into_iter()
                        .map(|(path, (unique, total))| (path, unique.len(), total))
                        .collect();

                    // Sort by unique matches (descending), then by total matches (descending)
                    file_list.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.cmp(&a.2)));

                    // Display the files
                    for (file_path, unique_matches, total_matches) in file_list {
                        output!("  {} <{}> <{}>", file_path, unique_matches, total_matches);
                    }

                    output!();
                    output!(
                        "💡 {} <uniq> = unique search terms matched, <all> = total match blocks",
                        "Hint:".dimmed()
                    );
                }

                // Show guidance message to get more results
                if total_skipped > 0 {
                    output!();
                    if let Some(session_id) = search_options.session {
                        if !session_id.is_empty() && session_id != "new" {
                            output!("💡 To get more results from this search query, repeat it with the same params and session ID: {session_id}");
                        } else {
                            output!("💡 To get more results from this search query, repeat it with the same params and session ID (see above)");
                        }
                    } else {
                        output!("💡 To get more results from this search query, repeat it with the same params and use --session with the session ID shown above");
                    }
                }

                // Show breakdown in debug mode
                if std::env::var("DEBUG").is_ok() && total_skipped > 0 {
                    output!();
                    if results_skipped > 0 {
                        output!(
                            "  {} {}",
                            "Results skipped after processing:".yellow(),
                            results_skipped
                        );
                    }
                    if files_not_processed > 0 {
                        output!(
                            "  {} {}",
                            "Files not processed (early termination):".yellow(),
                            files_not_processed
                        );
                    }
                }
            }
        }

        // Display information about cached blocks
        if let Some(cached_skipped) = limited_results.cached_blocks_skipped {
            if cached_skipped > 0 {
                println!();
                println!(
                    "{} {}",
                    "Skipped blocks due to session cache:".yellow().bold(),
                    cached_skipped
                );
            }
        }
    }

    // Add helpful tip at the very bottom of output (but not for JSON/XML formats)
    if params.format != "json" && params.format != "xml" {
        println!();
        println!("💡 Tip: Use --exact flag when searching for specific function names or variables for more precise results");
    }

    Ok(())
}

fn handle_benchmark(params: BenchmarkParams) -> Result<()> {
    use std::process::Command;

    println!("{}", "Running performance benchmarks...".bold().green());

    let bench_type = params.bench.as_deref().unwrap_or("all");

    // Build the cargo bench command
    let mut cmd = Command::new("cargo");
    cmd.arg("bench");

    // Add specific benchmark if requested
    match bench_type {
        "search" => {
            cmd.arg("--bench").arg("search_benchmarks");
        }
        "timing" => {
            cmd.arg("--bench").arg("timing_benchmarks");
        }
        "parsing" => {
            cmd.arg("--bench").arg("parsing_benchmarks");
        }
        "all" => {
            // Run all benchmarks (default)
        }
        _ => {
            eprintln!("Unknown benchmark type: {bench_type}");
            return Ok(());
        }
    }

    // Add criterion options after --
    let criterion_args: Vec<String> = Vec::new();

    // Note: Criterion benchmarks don't support --sample-size from command line
    // Sample size is configured in the benchmark code itself

    // For now, keep it simple and just run the benchmarks
    // Advanced features like baseline comparison can be added later

    if !criterion_args.is_empty() {
        cmd.arg("--");
        cmd.args(criterion_args);
    }

    // Execute the benchmark
    let output = cmd.output()?;

    if !output.status.success() {
        eprintln!("Benchmark failed:");
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        return Ok(());
    }

    // Print benchmark output
    println!("{}", String::from_utf8_lossy(&output.stdout));

    // Save output to file if requested
    if let Some(output_file) = &params.output {
        use std::fs;
        fs::write(output_file, &output.stdout)?;
        println!("Benchmark results saved to: {output_file}");
    }

    println!();
    println!("{}", "Benchmark completed successfully!".bold().green());
    println!(
        "Results are also available in: {}",
        "target/criterion/".yellow()
    );

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        // When no subcommand provided and no pattern, show help
        None if args.pattern.is_none() || args.pattern.as_ref().unwrap().is_empty() => {
            Args::command().print_help()?;
            return Ok(());
        }
        // When no subcommand but pattern is provided, fallback to search mode
        None => {
            // Use provided pattern
            let pattern = args.pattern.unwrap();

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
                language: None, // Default to None for the no-subcommand case
                max_results: args.max_results,
                max_bytes: args.max_bytes,
                max_tokens: args.max_tokens,
                allow_tests: args.allow_tests,
                no_merge: args.no_merge,
                merge_threshold: args.merge_threshold,
                dry_run: args.dry_run,
                format: args.format,
                session: args.session,
                timeout: args.timeout,
                question: args.question,
                no_gitignore: args.no_gitignore
                    || std::env::var("PROBE_NO_GITIGNORE").unwrap_or_default() == "1",
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
            language,
            max_results,
            max_bytes,
            max_tokens,
            allow_tests,
            no_merge,
            merge_threshold,
            dry_run,
            format,
            session,
            timeout,
            question,
            no_gitignore,
        }) => handle_search(SearchParams {
            pattern,
            paths,
            files_only,
            ignore,
            exclude_filenames,
            reranker,
            frequency_search,
            exact,
            language,
            max_results,
            max_bytes,
            max_tokens,
            allow_tests,
            no_merge,
            merge_threshold,
            dry_run,
            format,
            session,
            timeout,
            question,
            no_gitignore: no_gitignore
                || std::env::var("PROBE_NO_GITIGNORE").unwrap_or_default() == "1",
        })?,
        Some(Commands::Extract {
            files,
            ignore,
            context_lines,
            format,
            from_clipboard,
            input_file,
            to_clipboard,
            dry_run,
            diff,
            allow_tests,
            keep_input,
            prompt,
            instructions,
            no_gitignore,
        }) => handle_extract(ExtractOptions {
            files,
            custom_ignores: ignore,
            context_lines,
            format,
            from_clipboard,
            input_file,
            to_clipboard,
            dry_run,
            diff,
            allow_tests,
            keep_input,
            prompt: prompt.map(|p| {
                probe_code::extract::PromptTemplate::from_str(&p).unwrap_or_else(|e| {
                    eprintln!("Warning: {e}");
                    probe_code::extract::PromptTemplate::Engineer
                })
            }),
            instructions,
            no_gitignore: no_gitignore
                || std::env::var("PROBE_NO_GITIGNORE").unwrap_or_default() == "1",
        })?,
        Some(Commands::Query {
            pattern,
            path,
            language,
            ignore,
            allow_tests,
            max_results,
            format,
            no_gitignore,
        }) => probe_code::query::handle_query(
            &pattern,
            &path,
            language.as_deref().map(|lang| {
                // Normalize language aliases
                match lang.to_lowercase().as_str() {
                    "rs" => "rust",
                    "js" | "jsx" => "javascript",
                    "ts" | "tsx" => "typescript",
                    "py" => "python",
                    "h" => "c",
                    "cc" | "cxx" | "hpp" | "hxx" => "cpp",
                    "rb" => "ruby",
                    "cs" => "csharp",
                    _ => lang, // Return the original language if no alias is found
                }
            }),
            &ignore,
            allow_tests,
            max_results,
            &format,
            no_gitignore || std::env::var("PROBE_NO_GITIGNORE").unwrap_or_default() == "1",
        )?,
        Some(Commands::Benchmark {
            bench,
            sample_size,
            format,
            output,
            compare,
            baseline,
            fast,
        }) => handle_benchmark(BenchmarkParams {
            bench,
            sample_size,
            format,
            output,
            compare,
            baseline,
            fast,
        })?,
        Some(Commands::Grep {
            pattern,
            paths,
            ignore_case,
            line_number,
            count,
            files_with_matches,
            files_without_match,
            invert_match,
            before_context,
            after_context,
            context,
            ignore,
            no_gitignore,
            color,
            max_count,
        }) => grep::handle_grep(grep::GrepParams {
            pattern,
            paths,
            ignore_case,
            line_number,
            count,
            files_with_matches,
            files_without_match,
            invert_match,
            before_context,
            after_context,
            context,
            ignore,
            no_gitignore: no_gitignore
                || std::env::var("PROBE_NO_GITIGNORE").unwrap_or_default() == "1",
            color,
            max_count,
        })?,
    }

    Ok(())
}
