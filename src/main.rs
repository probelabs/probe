use anyhow::Result;
use clap::Parser as ClapParser;

mod cli;
mod language;
mod models;
mod ranking;
mod search;

use cli::Args;
use search::{format_and_print_search_results, perform_probe};

fn main() -> Result<()> {
    let args = Args::parse();

    // If exact is specified, override frequency_search
    let use_frequency = if args.exact {
        false
    } else {
        args.frequency_search
    };

    let query = vec![args.pattern];
    let limited_results = perform_probe(
        &args.paths[0], // First path or "." by default
        &query,
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
        args.merge_blocks,
        args.merge_threshold,
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
