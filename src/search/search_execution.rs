use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::Path as StdPath;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use tiktoken_rs::p50k_base;
use tiktoken_rs::CoreBPE;

use crate::language::get_language;
use crate::models::{LimitedSearchResults, SearchLimits, SearchResult};
use crate::search::file_processing::{process_file_by_filename, process_file_with_results};
use crate::search::file_search::{
    find_files_with_pattern, find_matching_filenames, search_file_for_pattern,
};
use crate::search::query::{create_term_patterns, preprocess_query};
use crate::search::result_ranking::rank_search_results;

/// Function to format and print search results according to the specified format
pub fn format_and_print_search_results(results: &[SearchResult]) {
    // Check if debug mode is enabled
    let debug_mode = std::env::var("CODE_SEARCH_DEBUG").unwrap_or_default() == "1";

    for result in results {
        // Get file extension
        let file_path = StdPath::new(&result.file);
        let extension = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        // Check if this is a full file or partial file
        let is_full_file = result.node_type == "file";

        if is_full_file {
            // Format for full file
            println!("File: {}", result.file);
            println!("```{}", extension);
            println!("{}", result.code);
            println!("```");
        } else {
            // Format for partial file
            println!("File: {}", result.file);
            println!("Lines: {}-{}", result.lines.0, result.lines.1);
            println!("```{}", extension);
            println!("{}", result.code);
            println!("```");
        }

        // Only print metadata in debug mode
        if debug_mode {
            if let Some(rank) = result.rank {
                println!("Rank: {}", rank);

                if let Some(score) = result.score {
                    println!("Combined Score: {:.4}", score);
                }

                if let Some(tfidf_score) = result.tfidf_score {
                    println!("TF-IDF Score: {:.4}", tfidf_score);
                }

                if let Some(tfidf_rank) = result.tfidf_rank {
                    println!("TF-IDF Rank: {}", tfidf_rank);
                }

                if let Some(bm25_score) = result.bm25_score {
                    println!("BM25 Score: {:.4}", bm25_score);
                }

                if let Some(bm25_rank) = result.bm25_rank {
                    println!("BM25 Rank: {}", bm25_rank);
                }

                println!("Type: {}", result.node_type);
            }
        }

        println!("\n");
    }

    println!("Found {} search results", results.len());
}

/// Returns a reference to the tiktoken tokenizer
fn get_tokenizer() -> &'static CoreBPE {
    static TOKENIZER: OnceLock<CoreBPE> = OnceLock::new();
    TOKENIZER.get_or_init(|| p50k_base().expect("Failed to initialize tiktoken tokenizer"))
}

/// Helper function to count tokens in a string using tiktoken (same tokenizer as GPT models)
fn count_tokens(text: &str) -> usize {
    let tokenizer = get_tokenizer();
    tokenizer.encode_with_special_tokens(text).len()
}

/// Helper function to apply limits to search results
fn apply_limits(
    mut results: Vec<SearchResult>,
    max_results: Option<usize>,
    max_bytes: Option<usize>,
    max_tokens: Option<usize>,
) -> LimitedSearchResults {
    // If no limits are set, return all results
    if max_results.is_none() && max_bytes.is_none() && max_tokens.is_none() {
        return LimitedSearchResults {
            results,
            skipped_files: Vec::new(),
            limits_applied: None,
        };
    }

    // Sort results by rank if available
    results.sort_by(|a, b| match (a.rank, b.rank) {
        (Some(a_rank), Some(b_rank)) => a_rank.cmp(&b_rank),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });

    let mut limited_results = Vec::new();
    let mut skipped_files = Vec::new();
    let mut total_bytes = 0;
    let mut total_tokens = 0;

    // Apply limits
    for result in results {
        let result_bytes = result.code.len();
        let result_tokens = count_tokens(&result.code);

        // Check if adding this result would exceed any limit
        let would_exceed_results =
            max_results.map_or(false, |limit| limited_results.len() >= limit);
        let would_exceed_bytes =
            max_bytes.map_or(false, |limit| total_bytes + result_bytes > limit);
        let would_exceed_tokens =
            max_tokens.map_or(false, |limit| total_tokens + result_tokens > limit);

        if would_exceed_results || would_exceed_bytes || would_exceed_tokens {
            // Skip this result if it would exceed any limit
            // Only track skipped files with non-zero ranks
            if result.rank.is_some()
                && (result.tfidf_score.unwrap_or(0.0) > 0.0
                    || result.bm25_score.unwrap_or(0.0) > 0.0)
            {
                skipped_files.push(result);
            }
        } else {
            // Add this result
            total_bytes += result_bytes;
            total_tokens += result_tokens;
            limited_results.push(result);
        }
    }

    LimitedSearchResults {
        results: limited_results,
        skipped_files,
        limits_applied: Some(SearchLimits {
            max_results,
            max_bytes,
            max_tokens,
            total_bytes,
            total_tokens,
        }),
    }
}

/// Function to perform code search and return results in a structured format
pub fn perform_code_search(
    path: &Path,
    queries: &[String],
    files_only: bool,
    custom_ignores: &[String],
    include_filenames: bool,
    reranker: &str,
    frequency_search: bool, // Parameter for frequency-based search
    max_results: Option<usize>,
    max_bytes: Option<usize>,
    max_tokens: Option<usize>,
) -> Result<LimitedSearchResults> {
    // Check if debug mode is enabled
    let debug_mode = std::env::var("CODE_SEARCH_DEBUG").unwrap_or_default() == "1";

    if debug_mode {
        println!("DEBUG: Starting code search with parameters:");
        println!("DEBUG:   Path: {:?}", path);
        println!("DEBUG:   Queries: {:?}", queries);
        println!("DEBUG:   Files only: {}", files_only);
        println!("DEBUG:   Custom ignores: {:?}", custom_ignores);
        println!("DEBUG:   Include filenames: {}", include_filenames);
        println!("DEBUG:   Reranker: {}", reranker);
        println!("DEBUG:   Frequency search: {}", frequency_search);
    }

    // If frequency-based search is enabled and we have exactly one query
    if frequency_search && queries.len() == 1 {
        if debug_mode {
            println!(
                "DEBUG: Using frequency-based search for query: {}",
                queries[0]
            );
        }
        return perform_frequency_search(
            path,
            &queries[0],
            files_only,
            custom_ignores,
            include_filenames,
            reranker,
            max_results,
            max_bytes,
            max_tokens,
        );
    }

    // Collect matches
    let mut matches_by_file: HashMap<PathBuf, HashSet<usize>> = HashMap::new();

    println!("Searching for {} queries in {:?}...", queries.len(), path);

    // Process each query
    for query in queries {
        // Create a case-insensitive regex matcher with the query
        let matcher = RegexMatcherBuilder::new()
            .case_insensitive(true)
            .build(query)
            .context(format!("Failed to create regex matcher for: {}", query))?;

        // Configure the searcher
        let mut searcher = SearcherBuilder::new()
            .binary_detection(BinaryDetection::quit(b'\x00'))
            .build();

        // Create a WalkBuilder that respects .gitignore files and common ignore patterns
        let mut builder = WalkBuilder::new(path);

        // Configure the builder to respect .gitignore files
        builder.git_ignore(true);
        builder.git_global(true);
        builder.git_exclude(true);

        // Add common directories to ignore
        let common_ignores = [
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
            "*.pyc",
            "*.pyo",
            "*.class",
            "*.o",
            "*.obj",
            "*.a",
            "*.lib",
            "*.so",
            "*.dylib",
            "*.dll",
            "*.exe",
            "*.out",
            "*.app",
            "*.jar",
            "*.war",
            "*.ear",
            "*.zip",
            "*.tar.gz",
            "*.rar",
            "*.log",
            "*.tmp",
            "*.temp",
            "*.swp",
            "*.swo",
            "*.bak",
            "*.orig",
            "*.DS_Store",
            "Thumbs.db",
        ];

        for pattern in &common_ignores {
            builder.add_custom_ignore_filename(pattern);
        }

        // Add custom ignore patterns
        for pattern in custom_ignores {
            // Create an override builder for glob patterns
            let mut override_builder = ignore::overrides::OverrideBuilder::new(path);
            override_builder.add(&format!("!{}", pattern)).unwrap();
            let overrides = override_builder.build().unwrap();
            builder.overrides(overrides);
        }

        // Recursively walk the directory and search each file
        for result in builder.build() {
            let entry = match result {
                Ok(entry) => entry,
                Err(err) => {
                    eprintln!("Error walking directory: {}", err);
                    continue;
                }
            };

            // Skip directories
            if !entry.file_type().map_or(false, |ft| ft.is_file()) {
                continue;
            }

            let file_path = entry.path();

            // Skip files that we don't support parsing (if AST mode is enabled)
            if !files_only {
                let extension = file_path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .unwrap_or("");

                if get_language(extension).is_none() {
                    continue;
                }
            }

            // Search the file
            let path_clone = file_path.to_owned();
            if let Err(err) = searcher.search_path(
                &matcher,
                file_path,
                UTF8(|line_number, _line| {
                    matches_by_file
                        .entry(path_clone.clone())
                        .or_insert_with(HashSet::new)
                        .insert(line_number as usize);
                    Ok(true)
                }),
            ) {
                eprintln!("Error searching file {:?}: {}", file_path, err);
                continue;
            }
        }
    }

    // If no matches found in content and filename matching is disabled, return empty results
    if matches_by_file.is_empty() && !include_filenames {
        println!("No matches found.");
        return Ok(LimitedSearchResults {
            results: Vec::new(),
            skipped_files: Vec::new(),
            limits_applied: None,
        });
    }

    println!("Found matches in {} files.", matches_by_file.len());

    if debug_mode {
        println!("DEBUG: Raw search results - matches by file:");
        for (file, line_numbers) in &matches_by_file {
            println!("DEBUG:   File: {:?}", file);
            println!("DEBUG:     Line numbers: {:?}", line_numbers);
        }
    }

    // If files_only mode is enabled, just return file paths
    if files_only {
        println!("\nMatching files:");
        let mut results = Vec::new();
        for path in matches_by_file.keys() {
            results.push(SearchResult {
                file: path.to_string_lossy().to_string(),
                lines: (0, 0),
                node_type: "file".to_string(),
                code: "".to_string(),
                matched_by_filename: None,
                rank: None,
                score: None,
                tfidf_score: None,
                bm25_score: None,
                tfidf_rank: None,
                bm25_rank: None,
            });
        }

        // If filename matching is enabled, find files whose names match query words
        if include_filenames {
            let already_found_files: HashSet<PathBuf> = matches_by_file.keys().cloned().collect();
            let matching_files =
                find_matching_filenames(path, queries, &already_found_files, custom_ignores)?;

            for file_path in matching_files {
                results.push(SearchResult {
                    file: file_path.to_string_lossy().to_string(),
                    lines: (0, 0),
                    node_type: "file".to_string(),
                    code: "".to_string(),
                    matched_by_filename: Some(true),
                    rank: None,
                    score: None,
                    tfidf_score: None,
                    bm25_score: None,
                    tfidf_rank: None,
                    bm25_rank: None,
                });
            }
        }

        // For files_only mode, we can't rank by content since we don't have code content
        // Instead, we'll rank by filename relevance to the query
        if !results.is_empty() {
            // Assign simple ranks based on filename match with query
            for (i, result) in results.iter_mut().enumerate() {
                result.rank = Some(i + 1);
                // Simple score based on position
                result.score = Some(1.0 / (i as f64 + 1.0));

                // For files_only mode, we'll use the same score for TF-IDF and BM25
                // This is a simplification since we don't have actual content to score
                result.tfidf_score = Some(result.score.unwrap());
                result.bm25_score = Some(result.score.unwrap());
            }

            // Create separate rankings for TF-IDF and BM25 scores (in files_only mode, these will be the same)
            // First, create a copy of the results with their original indices
            let mut tfidf_ranking: Vec<(usize, f64)> = results
                .iter()
                .enumerate()
                .filter_map(|(idx, r)| r.tfidf_score.map(|score| (idx, score)))
                .collect();

            let mut bm25_ranking: Vec<(usize, f64)> = results
                .iter()
                .enumerate()
                .filter_map(|(idx, r)| r.bm25_score.map(|score| (idx, score)))
                .collect();

            // Sort by scores in descending order
            tfidf_ranking
                .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            bm25_ranking.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            // Assign ranks
            for (rank, (idx, _)) in tfidf_ranking.iter().enumerate() {
                if let Some(result) = results.get_mut(*idx) {
                    result.tfidf_rank = Some(rank + 1); // 1-based rank
                }
            }

            for (rank, (idx, _)) in bm25_ranking.iter().enumerate() {
                if let Some(result) = results.get_mut(*idx) {
                    result.bm25_rank = Some(rank + 1); // 1-based rank
                }
            }

            // Print rank information for each file result
            for result in results.iter() {
                if let Some(rank) = result.rank {
                    let score = result.score.unwrap_or(0.0);
                    println!("Rank {}: File {}", rank, result.file);
                    println!("Combined Score: {:.4}", score);

                    // Print TF-IDF and BM25 ranks if available
                    if let Some(tfidf_rank) = result.tfidf_rank {
                        println!("TF-IDF Rank: {}", tfidf_rank);
                    }

                    if let Some(bm25_rank) = result.bm25_rank {
                        println!("BM25 Rank: {}", bm25_rank);
                    }
                }
            }

            println!("Assigned ranks to {} file results", results.len());
        }

        return Ok(apply_limits(results, max_results, max_bytes, max_tokens));
    }

    // Collect the keys of matches_by_file for filename matching later
    let found_files: HashSet<PathBuf> = matches_by_file.keys().cloned().collect();

    // Process each file and collect results
    let mut results = Vec::new();
    for (path, line_numbers) in matches_by_file {
        let file_results = process_file_with_results(&path, &line_numbers)?;
        results.extend(file_results);
    }

    // If filename matching is enabled, find files whose names match query words
    if include_filenames {
        let already_found_files: HashSet<PathBuf> = found_files;
        let matching_files =
            find_matching_filenames(path, queries, &already_found_files, custom_ignores)?;

        for file_path in matching_files {
            match process_file_by_filename(&file_path) {
                Ok(result) => results.push(result),
                Err(err) => eprintln!("Error processing file {:?}: {}", file_path, err),
            }
        }
    }

    // Rank the results if there are any
    if !results.is_empty() {
        rank_search_results(&mut results, queries, reranker);
    }

    // Apply limits and return
    Ok(apply_limits(results, max_results, max_bytes, max_tokens))
}

/// Performs a search using frequency-based approach with stemming and stopword removal
pub fn perform_frequency_search(
    path: &Path,
    query: &str,
    files_only: bool,
    custom_ignores: &[String],
    include_filenames: bool,
    reranker: &str,
    max_results: Option<usize>,
    max_bytes: Option<usize>,
    max_tokens: Option<usize>,
) -> Result<LimitedSearchResults> {
    // Check if debug mode is enabled
    let debug_mode = std::env::var("CODE_SEARCH_DEBUG").unwrap_or_default() == "1";

    println!("Performing frequency-based search for query: {}", query);

    if debug_mode {
        println!("DEBUG: Starting frequency-based search with parameters:");
        println!("DEBUG:   Path: {:?}", path);
        println!("DEBUG:   Query: {}", query);
        println!("DEBUG:   Files only: {}", files_only);
        println!("DEBUG:   Custom ignores: {:?}", custom_ignores);
        println!("DEBUG:   Include filenames: {}", include_filenames);
        println!("DEBUG:   Reranker: {}", reranker);
    }

    // 1. Preprocess the query into original/stemmed pairs
    let term_pairs = preprocess_query(query);

    if term_pairs.is_empty() {
        println!("No valid search terms after preprocessing");
        return Ok(LimitedSearchResults {
            results: Vec::new(),
            skipped_files: Vec::new(),
            limits_applied: None,
        });
    }

    // 2. Create regex patterns for each term
    let term_patterns = create_term_patterns(&term_pairs);
    println!("Frequency search enabled");
    println!("Original query: {}", query);
    println!("After preprocessing:");
    for (i, (original, stemmed)) in term_pairs.iter().enumerate() {
        if original == stemmed {
            println!("  Term {}: {} (stemmed same as original)", i + 1, original);
        } else {
            println!("  Term {}: {} (stemmed to {})", i + 1, original, stemmed);
        }
    }
    println!("Search patterns: {:?}", term_patterns);

    // 3. Find all files and their match frequencies
    let mut file_match_counts: HashMap<PathBuf, usize> = HashMap::new();
    let mut file_matched_lines: HashMap<PathBuf, HashSet<usize>> = HashMap::new();

    // First, find all files matching the first pattern
    let initial_files = find_files_with_pattern(path, &term_patterns[0], custom_ignores)?;
    println!(
        "Found {} files matching first term pattern",
        initial_files.len()
    );

    // Initialize with zero matches
    for file in &initial_files {
        file_match_counts.insert(file.clone(), 0);
        file_matched_lines.insert(file.clone(), HashSet::new());
    }

    // For each pattern, search all files and update match counts
    for (i, pattern) in term_patterns.iter().enumerate() {
        let files_to_search = if i == 0 {
            // For first pattern, we already have the files
            initial_files.clone()
        } else {
            // For subsequent patterns, search only in files that matched at least one previous pattern
            file_match_counts.keys().cloned().collect()
        };

        println!(
            "Searching {} files for pattern {}: {}",
            files_to_search.len(),
            i + 1,
            pattern
        );

        for file in &files_to_search {
            // Search for this pattern in the file
            match search_file_for_pattern(file, pattern) {
                Ok((matched, line_numbers)) => {
                    if matched {
                        // Increment match count for this file
                        *file_match_counts.entry(file.clone()).or_insert(0) += 1;

                        // Add matched line numbers
                        if let Some(lines) = file_matched_lines.get_mut(file) {
                            lines.extend(line_numbers);
                        }
                    }
                }
                Err(err) => eprintln!("Error searching file {:?}: {}", file, err),
            }
        }
    }

    // 4. Sort files by match frequency (descending)
    let mut files_by_frequency: Vec<(PathBuf, usize)> = file_match_counts.into_iter().collect();
    files_by_frequency.sort_by(|a, b| b.1.cmp(&a.1));

    println!(
        "Found {} files with the following match frequencies:",
        files_by_frequency.len()
    );
    for (i, (file, count)) in files_by_frequency.iter().enumerate().take(10) {
        println!("  {}: {:?} - {} term matches", i + 1, file, count);
    }

    if debug_mode {
        println!("DEBUG: Raw search results - all files by frequency:");
        for (i, (file, count)) in files_by_frequency.iter().enumerate() {
            println!("DEBUG:   {}. {:?} - {} term matches", i + 1, file, count);
        }

        println!("DEBUG: Raw search results - matched lines by file:");
        for (file, lines) in &file_matched_lines {
            println!("DEBUG:   File: {:?}", file);
            println!("DEBUG:     Line numbers: {:?}", lines);
        }
    }

    // 5. Process the top N files (or all if fewer)
    let top_n = 100; // Configurable
    let top_files: Vec<(PathBuf, usize)> = files_by_frequency
        .into_iter()
        .filter(|(_, count)| *count > 0) // Only include files that matched at least one term
        .take(top_n)
        .collect();

    // 6. Process results
    let mut results = Vec::new();

    for (file_path, match_count) in &top_files {
        if files_only {
            // For files-only mode, just return the file paths
            results.push(SearchResult {
                file: file_path.to_string_lossy().to_string(),
                lines: (0, 0),
                node_type: "file".to_string(),
                code: "".to_string(),
                matched_by_filename: None,
                rank: None,
                score: Some(*match_count as f64), // Use match count as score
                tfidf_score: None,
                bm25_score: None,
                tfidf_rank: None,
                bm25_rank: None,
            });
        } else {
            // Process the file with the matching line numbers
            if let Some(line_numbers) = file_matched_lines.get(file_path) {
                match process_file_with_results(&file_path, line_numbers) {
                    Ok(file_results) => {
                        // Add match count as a score for each result from this file
                        let mut scored_results = file_results;
                        for result in &mut scored_results {
                            // Store the match count as an initial score
                            result.score = Some(*match_count as f64);
                        }
                        results.extend(scored_results);
                    }
                    Err(err) => eprintln!("Error processing file {:?}: {}", file_path, err),
                }
            }
        }
    }

    // 7. Add filename matches if requested
    if include_filenames {
        // Create a HashSet of file paths from top_files
        let already_found_files: HashSet<PathBuf> =
            top_files.iter().map(|(path, _)| path.clone()).collect();
        let filename_matches = find_matching_filenames(
            path,
            &[query.to_string()],
            &already_found_files,
            custom_ignores,
        )?;

        for file_path in filename_matches {
            if files_only {
                results.push(SearchResult {
                    file: file_path.to_string_lossy().to_string(),
                    lines: (0, 0),
                    node_type: "file".to_string(),
                    code: "".to_string(),
                    matched_by_filename: Some(true),
                    rank: None,
                    score: Some(0.5), // Lower score for filename matches
                    tfidf_score: None,
                    bm25_score: None,
                    tfidf_rank: None,
                    bm25_rank: None,
                });
            } else {
                match process_file_by_filename(&file_path) {
                    Ok(mut result) => {
                        result.score = Some(0.5); // Lower score for filename matches
                        results.push(result);
                    }
                    Err(err) => eprintln!("Error processing file {:?}: {}", file_path, err),
                }
            }
        }
    }

    // 8. Final ranking using existing ranking logic
    if !results.is_empty() {
        // Use the existing ranking function, but it will start with our frequency-based scores
        rank_search_results(&mut results, &[query.to_string()], reranker);
    }

    // Apply limits and return
    Ok(apply_limits(results, max_results, max_bytes, max_tokens))
}

// Import necessary types for the implementation
use anyhow::Context;
use grep::regex::RegexMatcherBuilder;
use grep::searcher::sinks::UTF8;
use grep::searcher::{BinaryDetection, SearcherBuilder};
use ignore::WalkBuilder;
