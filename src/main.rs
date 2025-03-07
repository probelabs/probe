use anyhow::Result;
use clap::Parser as ClapParser;
use colored::*;
use std::path::PathBuf;
use std::time::Instant;

mod chat;
mod cli;
mod language;
mod models;
mod ranking;
mod search;

use cli::{Args, Commands};
use search::{format_and_print_search_results, perform_probe, SearchOptions};

struct SearchParams {
    pattern: String,
    paths: Vec<PathBuf>,
    files_only: bool,
    ignore: Vec<String>,
    include_filenames: bool,
    reranker: String,
    frequency_search: bool,
    exact: bool,
    max_results: Option<usize>,
    max_bytes: Option<usize>,
    max_tokens: Option<usize>,
    allow_tests: bool,
    any_term: bool,
    no_merge: bool,
    merge_threshold: Option<usize>,
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
    if params.include_filenames {
        advanced_options.push("Include filenames".to_string());
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
    if params.any_term {
        advanced_options.push("Match any term".to_string());
    }
    if params.no_merge {
        advanced_options.push("No block merging".to_string());
    }
    if let Some(threshold) = params.merge_threshold {
        advanced_options.push(format!("Merge threshold: {}", threshold));
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
        include_filenames: params.include_filenames,
        reranker: &params.reranker,
        frequency_search: use_frequency,
        exact: params.exact,
        max_results: params.max_results,
        max_bytes: params.max_bytes,
        max_tokens: params.max_tokens,
        allow_tests: params.allow_tests,
        any_term: params.any_term,
        no_merge: params.no_merge,
        merge_threshold: params.merge_threshold,
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

        format_and_print_search_results(&limited_results.results);

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

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    match args.command {
        // When no subcommand provided, default to search with empty pattern
        None => handle_search(SearchParams {
            pattern: String::new(),
            paths: vec![std::path::PathBuf::from(".")],
            files_only: false,
            ignore: Vec::new(),
            include_filenames: false,
            reranker: String::from("hybrid"),
            frequency_search: true,
            exact: false,
            max_results: None,
            max_bytes: None,
            max_tokens: None,
            allow_tests: false,
            any_term: false,
            no_merge: false,
            merge_threshold: None,
        })?,
        Some(Commands::Search {
            pattern,
            paths,
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
            no_merge,
            merge_threshold,
        }) => handle_search(SearchParams {
            pattern,
            paths,
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
            no_merge,
            merge_threshold,
        })?,
        Some(Commands::Chat) => chat::handle_chat().await?,
    }

    Ok(())
}
