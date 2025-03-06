use anyhow::Result;
use clap::Parser as ClapParser;

mod cli;
mod language;
mod models;
mod ranking;
mod search;

use cli::Args;
use search::{format_and_print_search_results, perform_probe, SearchOptions};

fn main() -> Result<()> {
    let args = Args::parse();

    // If exact is specified, override frequency_search
    let use_frequency = if args.exact {
        false
    } else {
        args.frequency_search
    };

    let query = vec![args.pattern];
    let search_options = SearchOptions {
        path: &args.paths[0], // First path or "." by default
        queries: &query,
        files_only: args.files_only,
        custom_ignores: &args.ignore,
        include_filenames: args.include_filenames,
        reranker: &args.reranker,
        frequency_search: use_frequency,
        max_results: args.max_results,
        max_bytes: args.max_bytes,
        max_tokens: args.max_tokens,
        allow_tests: args.allow_tests,
        any_term: args.any_term,
        exact: args.exact,
        no_merge: args.no_merge,
        merge_threshold: args.merge_threshold,
    };

    let limited_results = perform_probe(&search_options)?;

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
