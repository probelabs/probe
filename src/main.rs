use anyhow::Result;
use clap::Parser as ClapParser;

mod cli;
mod language;
mod models;
mod ranking;
mod search;

use cli::{Args, Command};
use search::{format_and_print_search_results, perform_code_search};

// Main function that handles CLI mode
fn main() -> Result<()> {
    // Parse arguments
    let args = Args::parse();

    match args.command {
        Some(Command::Cli {
            path,
            query,
            files_only,
            ignore,
            include_filenames,
            reranker,
            frequency_search,
            exact,
            max_results,
            max_bytes,
            max_tokens,
            allow_tests,
            any_term,
        }) => {
            println!("Running in CLI mode (subcommand)");
            // If exact is specified, override frequency_search
            let use_frequency = if exact { false } else { frequency_search };
            let limited_results = perform_code_search(
                &path,
                &query,
                files_only,
                &ignore,
                include_filenames,
                &reranker,
                use_frequency,
                max_results,
                max_bytes,
                max_tokens,
                allow_tests,
                any_term,
                exact,
            )?;

            if limited_results.results.is_empty() {
                println!("No results found.");
            } else {
                format_and_print_search_results(&limited_results.results);

                // Print information about limits only if some results were skipped
                if !limited_results.skipped_files.is_empty() {
                    if let Some(limits) = &limited_results.limits_applied {
                        println!("\nLimits applied:");
                        if let Some(max_results) = limits.max_results {
                            println!("  Max results: {}", max_results);
                        }
                        if let Some(max_bytes) = limits.max_bytes {
                            println!("  Max bytes: {}", max_bytes);
                        }
                        if let Some(max_tokens) = limits.max_tokens {
                            println!("  Max tokens: {}", max_tokens);
                        }

                        println!(
                            "\nSkipped {} files due to limits",
                            limited_results.skipped_files.len()
                        );
                    }
                }
            }

            Ok(())
        }
        None => {
            // Check if we have path and query for direct CLI mode
            if let Some(path) = args.path {
                if !args.query.is_empty() {
                    println!("Running in CLI mode (direct arguments)");
                    // Run in CLI mode with direct arguments
                    // If exact is specified, override frequency_search
                    let use_frequency = if args.exact { false } else { args.frequency_search };
                    let limited_results = perform_code_search(
                        &path,
                        &args.query,
                        args.files_only,
                        &args.ignore,
                        args.include_filenames,
                        &args.reranker,
                        use_frequency,
                        args.max_results,
                        args.max_bytes,
                        args.max_tokens,
                        args.allow_tests,
                        args.any_term,
                        args.exact,
                    )?;

                    if limited_results.results.is_empty() {
                        println!("No results found.");
                    } else {
                        format_and_print_search_results(&limited_results.results);

                        // Print information about limits only if some results were skipped
                        if !limited_results.skipped_files.is_empty() {
                            if let Some(limits) = &limited_results.limits_applied {
                                println!("\nLimits applied:");
                                if let Some(max_results) = limits.max_results {
                                    println!("  Max results: {}", max_results);
                                }
                                if let Some(max_bytes) = limits.max_bytes {
                                    println!("  Max bytes: {}", max_bytes);
                                }
                                if let Some(max_tokens) = limits.max_tokens {
                                    println!("  Max tokens: {}", max_tokens);
                                }

                                println!(
                                    "\nSkipped {} files due to limits",
                                    limited_results.skipped_files.len()
                                );
                            }
                        }
                    }

                    return Ok(());
                }
            }

            // No valid command or arguments
            println!("No valid command or required arguments specified. Use --help for usage information.");
            println!("To run in CLI mode: code-search --path <PATH> --query <QUERY>");
            println!("Or with subcommand: code-search cli --path <PATH> --query <QUERY>");
            Ok(())
        }
    }
}
