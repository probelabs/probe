// Note on debug output terminology:
// - "pattern matches" refers to the number of regex patterns that matched in a file
//   (each term can have multiple patterns: word boundaries, stemmed variants, etc.)
// - The actual number of unique terms matched is shown in the Content/Filename matches lists

use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use rayon::prelude::*;
use tiktoken_rs::p50k_base;
use tiktoken_rs::CoreBPE;

// Removed unused import
use crate::models::{LimitedSearchResults, SearchLimits, SearchResult};
use crate::search::file_processing::{process_file_by_filename, process_file_with_results};
use crate::search::file_search::{
    find_files_with_pattern, find_matching_filenames, get_filename_matched_queries,
    get_filename_matched_queries_compat, search_file_for_pattern,
};
use crate::search::query::{create_term_patterns, preprocess_query};
use crate::search::result_ranking::rank_search_results;

/// Struct to hold timing information for different stages of the search process
struct SearchTimings {
    query_preprocessing: Option<Duration>,
    file_searching: Option<Duration>,
    result_processing: Option<Duration>,
    result_ranking: Option<Duration>,
    limit_application: Option<Duration>,
}

/// Function to format and print search results according to the specified format
pub fn format_and_print_search_results(results: &[SearchResult]) {
    // Check if debug mode is enabled
    let debug_mode = std::env::var("CODE_SEARCH_DEBUG").unwrap_or_default() == "1";

    for result in results {
        // Get file extension
        let file_path = Path::new(&result.file);
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

                // Display file-level statistics
                if let Some(file_unique_terms) = result.file_unique_terms {
                    println!("File Unique Terms: {}", file_unique_terms);
                }

                if let Some(file_total_matches) = result.file_total_matches {
                    println!("File Total Matches: {}", file_total_matches);
                }

                if let Some(file_match_rank) = result.file_match_rank {
                    println!("File Match Rank: {}", file_match_rank);
                }

                println!("Type: {}", result.node_type);
            }
        }

        println!("\n");
    }

    println!("Found {} search results", results.len());

    // Calculate and print total bytes and tokens
    let total_bytes: usize = results.iter().map(|r| r.code.len()).sum();
    let total_tokens: usize = results.iter().map(|r| count_tokens(&r.code)).sum();
    println!("Total bytes returned: {}", total_bytes);
    println!("Total tokens returned: {}", total_tokens);
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
    results: Vec<SearchResult>,
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
    let mut results = results;
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
    allow_tests: bool, // Parameter to control test file/node inclusion
    any_term: bool,    // Parameter to control multi-term search behavior
    exact: bool,       // Parameter to control exact matching (no stemming/stopwords)
) -> Result<LimitedSearchResults> {
    // Check if debug mode is enabled
    let debug_mode = std::env::var("CODE_SEARCH_DEBUG").unwrap_or_default() == "1";

    // Initialize timing structure
    let mut timings = SearchTimings {
        query_preprocessing: None,
        file_searching: None,
        result_processing: None,
        result_ranking: None,
        limit_application: None,
    };

    // Start timing query preprocessing
    let query_preprocessing_start = Instant::now();

    if debug_mode {
        println!("DEBUG: Starting code search with parameters:");
        println!("DEBUG:   Path: {:?}", path);
        println!("DEBUG:   Queries: {:?}", queries);
        println!("DEBUG:   Files only: {}", files_only);
        println!("DEBUG:   Custom ignores: {:?}", custom_ignores);
        println!("DEBUG:   Include filenames: {}", include_filenames);
        println!("DEBUG:   Reranker: {}", reranker);
        println!("DEBUG:   Frequency search: {}", frequency_search);
        println!(
            "DEBUG:   Match mode: {}",
            if any_term { "any term" } else { "all terms" }
        );
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
            allow_tests,
            any_term,
            exact,
        );
    }

    // Process each query string into multiple terms and store the term pairs for later use
    let mut all_patterns = Vec::new();
    // Create a mapping from pattern index to the terms it represents
    let mut pattern_to_terms: Vec<HashSet<usize>> = Vec::new();
    // Store the term pairs for each query for filename matching
    let queries_terms: Vec<Vec<(String, String)>> =
        queries.iter().map(|q| preprocess_query(q, exact)).collect();

    // Create a mapping from pattern index to query index (for backward compatibility)
    let mut pattern_to_query: Vec<usize> = Vec::new();

    // Calculate term offsets for each query to map to flattened term indices
    let mut term_offset: Vec<usize> = queries_terms
        .iter()
        .scan(0, |state, terms| {
            let offset = *state;
            *state += terms.len();
            Some(offset)
        })
        .collect();

    // Add the total number of terms as the final entry
    let total_terms: usize = queries_terms.iter().map(|t| t.len()).sum();
    term_offset.push(total_terms);

    for (query_idx, _query) in queries.iter().enumerate() {
        // Split query into terms and create patterns with word boundaries
        let term_pairs = &queries_terms[query_idx];
        let patterns_with_terms = create_term_patterns(term_pairs);

        // For each pattern, adjust term indices to be global (across all queries)
        for (pattern, term_indices) in &patterns_with_terms {
            // Create a new set with adjusted indices
            let mut global_term_indices = HashSet::new();
            let query_offset = term_offset[query_idx];

            // Adjust each term index by adding the query's term offset
            for &term_idx in term_indices {
                global_term_indices.insert(query_offset + term_idx);
            }

            // Add the pattern and its global term indices
            all_patterns.push(pattern.clone());
            pattern_to_terms.push(global_term_indices);

            // For backward compatibility, also record the query index
            pattern_to_query.push(query_idx);
        }
    }

    // Record query preprocessing time
    timings.query_preprocessing = Some(query_preprocessing_start.elapsed());

    // Collect matches for each term
    let _matches_by_file_and_term: HashMap<PathBuf, HashMap<usize, HashSet<usize>>> =
        HashMap::new();

    // Calculate term offsets for each query to map to flattened term indices
    // Include an additional entry for the total number of terms
    let mut term_offset: Vec<usize> = queries_terms
        .iter()
        .scan(0, |state, terms| {
            let offset = *state;
            *state += terms.len();
            Some(offset)
        })
        .collect();

    // Add the total number of terms as the final entry
    let total_terms: usize = queries_terms.iter().map(|t| t.len()).sum();
    term_offset.push(total_terms);

    println!(
        "Searching for {} queries with {} patterns in {:?} (mode: {})...",
        queries.len(),
        all_patterns.len(),
        path,
        if any_term {
            "any term matches"
        } else {
            "all terms must match"
        }
    );

    // Start timing file searching phase
    let file_searching_start = Instant::now();

    // Create a thread-safe container for collecting results
    let matches_by_file_and_term: Arc<Mutex<HashMap<PathBuf, HashMap<usize, HashSet<usize>>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    // Group patterns by term indices
    let mut single_term_patterns: HashMap<usize, Vec<String>> = HashMap::new();
    let mut combo_patterns: Vec<(String, HashSet<usize>)> = Vec::new();

    for (pattern_idx, pattern) in all_patterns.iter().enumerate() {
        let term_indices = &pattern_to_terms[pattern_idx];

        if term_indices.len() == 1 {
            // This is a single-term pattern
            let term_idx = *term_indices.iter().next().unwrap();
            single_term_patterns
                .entry(term_idx)
                .or_default()
                .push(pattern.clone());
        } else {
            // This is a combination pattern
            combo_patterns.push((pattern.clone(), term_indices.clone()));
        }
    }

    if debug_mode {
        println!(
            "DEBUG: Grouped {} patterns into {} single-term groups and {} combination patterns",
            all_patterns.len(),
            single_term_patterns.len(),
            combo_patterns.len()
        );
    }

    // Helper function to search for a group of patterns and collect results
    let search_group = |patterns: &[String], term_indices: &HashSet<usize>| {
        let local_matches: Arc<Mutex<HashMap<PathBuf, HashMap<usize, HashSet<usize>>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        patterns.par_iter().for_each(|pattern| {
            if let Ok(matching_files) =
                find_files_with_pattern(path, pattern, custom_ignores, allow_tests)
            {
                for file_path in matching_files {
                    if let Ok((matched, line_numbers)) =
                        search_file_for_pattern(&file_path, pattern, true)
                    {
                        if matched {
                            // Store the matches for each term this pattern represents
                            let mut file_matches_lock = local_matches.lock().unwrap();
                            let file_matches =
                                file_matches_lock.entry(file_path.clone()).or_default();
                            for &term_idx in term_indices {
                                file_matches
                                    .entry(term_idx)
                                    .or_default()
                                    .extend(line_numbers.clone());
                            }
                        }
                    }
                }
            }
        });

        // Return the collected matches
        Arc::try_unwrap(local_matches)
            .expect("Failed to unwrap Arc")
            .into_inner()
            .expect("Failed to unwrap Mutex")
    };

    // Process single-term patterns in parallel
    let single_term_results: Vec<_> = single_term_patterns
        .par_iter()
        .map(|(&term_idx, patterns)| {
            let term_indices = HashSet::from([term_idx]);
            let matches = search_group(patterns, &term_indices);
            (matches, term_indices)
        })
        .collect();

    // Process combination patterns
    let combo_results = if !combo_patterns.is_empty() {
        let combo_pattern_strings: Vec<String> =
            combo_patterns.iter().map(|(p, _)| p.clone()).collect();
        let combo_term_indices: HashSet<usize> = combo_patterns
            .iter()
            .flat_map(|(_, indices)| indices.iter().cloned())
            .collect();

        search_group(&combo_pattern_strings, &combo_term_indices)
    } else {
        HashMap::new()
    };

    // Merge all results into the main matches_by_file_and_term
    {
        let mut main_matches = matches_by_file_and_term.lock().unwrap();

        // Merge single-term results
        for (matches, _term_indices) in single_term_results {
            for (file_path, term_line_matches) in matches {
                let file_entry = main_matches.entry(file_path).or_default();
                for (term_idx, lines) in term_line_matches {
                    file_entry.entry(term_idx).or_default().extend(lines);
                }
            }
        }

        // Merge combination results
        for (file_path, term_line_matches) in combo_results {
            let file_entry = main_matches.entry(file_path).or_default();
            for (term_idx, lines) in term_line_matches {
                file_entry.entry(term_idx).or_default().extend(lines);
            }
        }
    }

    // Extract the results from the Arc<Mutex<...>>
    let matches_by_file_and_term = Arc::try_unwrap(matches_by_file_and_term)
        .expect("Failed to unwrap Arc")
        .into_inner()
        .expect("Failed to unwrap Mutex");

    // Collect all files with matches (both content and filename matches)
    let content_files: HashSet<PathBuf> = matches_by_file_and_term.keys().cloned().collect();
    let mut all_files = content_files.clone();

    // Find files that match by filename if filename matching is enabled
    let filename_matching_files;
    if include_filenames {
        filename_matching_files =
            find_matching_filenames(path, queries, &content_files, custom_ignores, allow_tests)?;
        all_files.extend(filename_matching_files.iter().cloned());
    }

    // Compute statistics for each file
    let mut file_stats: HashMap<String, (usize, usize, usize)> = HashMap::new();
    let mut file_total_matches: Vec<(String, usize)> = Vec::new();

    for file in &all_files {
        // Get content matches for this file
        let content_terms: HashSet<usize> = matches_by_file_and_term
            .get(file)
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();

        // Get filename matches for this file
        let filename = file
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        let filename_terms = get_filename_matched_queries_compat(&filename, &queries_terms);

        // Calculate unique terms (union of content and filename matches)
        let __unique_terms = content_terms.union(&filename_terms).count();

        // Calculate total matches (content lines + filename matches)
        let content_lines = matches_by_file_and_term
            .get(file)
            .map(|m| m.values().flatten().collect::<HashSet<_>>().len())
            .unwrap_or(0);
        let total_matches = content_lines + filename_terms.len();

        // Store the total matches for ranking
        file_total_matches.push((file.to_string_lossy().to_string(), total_matches));
    }

    // Rank files by total matches (descending order)
    file_total_matches.sort_by(|a, b| b.1.cmp(&a.1));

    // Assign ranks based on sorted order
    for (rank, (file, total_matches)) in file_total_matches.iter().enumerate() {
        // Get content matches for this file
        let file_path = PathBuf::from(file);
        let content_terms: HashSet<usize> = matches_by_file_and_term
            .get(&file_path)
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();

        // Get filename matches for this file
        let filename = PathBuf::from(file)
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        let filename_terms = get_filename_matched_queries_compat(&filename, &queries_terms);

        // Calculate unique terms
        let _unique_terms = content_terms.union(&filename_terms).count();

        // Store statistics: (_unique_terms, total_matches, rank)
        file_stats.insert(file.clone(), (_unique_terms, *total_matches, rank + 1));
    }

    // Combine matches based on the search mode (all terms or any term)
    let mut matches_by_file: HashMap<PathBuf, HashSet<usize>> = HashMap::new();
    let total_files_with_any_match = matches_by_file_and_term.len();

    // First, check which files have terms in their filenames
    let mut filename_matched_terms_by_file: HashMap<PathBuf, HashSet<usize>> = HashMap::new();

    for (file_path, _) in &matches_by_file_and_term {
        // Get the filename for matching
        let filename = file_path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();

        // Determine which terms match in the filename
        let filename_matched_terms = get_filename_matched_queries_compat(&filename, &queries_terms);

        // Store the filename matches if there are any
        if !filename_matched_terms.is_empty() {
            filename_matched_terms_by_file.insert(file_path.clone(), filename_matched_terms);
        }
    }

    for (file_path, term_matches) in &matches_by_file_and_term {
        // Get filename matches for this file
        let filename_matches = filename_matched_terms_by_file
            .get(file_path)
            .cloned()
            .unwrap_or_default();

        // Combine all matched lines for this file
        let mut all_lines = HashSet::new();
        for lines in term_matches.values() {
            all_lines.extend(lines);
        }

        // Determine if this file should be included based on the search mode
        let should_include = if any_term {
            // Any term mode: include if there's at least one match
            !term_matches.is_empty()
        } else {
            // All terms mode: include if all queries have at least one term matched
            queries.iter().enumerate().all(|(q_idx, _)| {
                let start = term_offset[q_idx];
                let total_terms = queries_terms.iter().map(|t| t.len()).sum();
                let end = term_offset.get(q_idx + 1).unwrap_or(&total_terms);

                // Check if any term in this query matches (either in content or filename)
                (start..*end).any(|t_idx| {
                    term_matches.contains_key(&t_idx) || filename_matches.contains(&t_idx)
                })
            })
        };

        if should_include {
            matches_by_file.insert(file_path.clone(), all_lines);
        }
    }

    // Record file searching time
    timings.file_searching = Some(file_searching_start.elapsed());

    // Log filtering statistics
    if queries.len() > 1 {
        println!("Term filtering: {} files matched at least one term, {} files matched the filter criteria ({}).",
                 total_files_with_any_match,
                 matches_by_file.len(),
                 if any_term { "any term" } else { "all terms must match" });
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
                file_unique_terms: None,
                file_total_matches: None,
                file_match_rank: None,
            });
        }

        // If filename matching is enabled, find files whose names match query words
        if include_filenames {
            let already_found_files: HashSet<PathBuf> = matches_by_file.keys().cloned().collect();
            let matching_files = find_matching_filenames(
                path,
                queries,
                &already_found_files,
                custom_ignores,
                allow_tests,
            )?;

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
                    file_unique_terms: None,
                    file_total_matches: None,
                    file_match_rank: None,
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

                    // Display file-level statistics
                    if let Some(file_unique_terms) = result.file_unique_terms {
                        println!("File Unique Terms: {}", file_unique_terms);
                    }

                    if let Some(file_total_matches) = result.file_total_matches {
                        println!("File Total Matches: {}", file_total_matches);
                    }

                    if let Some(file_match_rank) = result.file_match_rank {
                        println!("File Match Rank: {}", file_match_rank);
                    }
                }
            }

            println!("Assigned ranks to {} file results", results.len());
        }

        return Ok(apply_limits(results, max_results, max_bytes, max_tokens));
    }

    // Collect the keys of matches_by_file for filename matching later
    let found_files: HashSet<PathBuf> = matches_by_file.keys().cloned().collect();

    // Start timing result processing phase
    let result_processing_start = Instant::now();

    // Process each file and collect results
    let mut results = Vec::new();
    for (path, line_numbers) in matches_by_file {
        // Get term-specific matches for this file
        let term_matches = matches_by_file_and_term.get(&path);

        // Get the filename for matching
        let filename = path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();

        // Determine which terms match in the filename
        let filename_matched_terms = get_filename_matched_queries_compat(&filename, &queries_terms);

        // Process the file with both content and filename matches
        let mut file_results = process_file_with_results(
            &path,
            &line_numbers,
            allow_tests,
            term_matches,
            any_term,
            queries_terms.iter().map(|terms| terms.len()).sum(),
            filename_matched_terms,
        )?;

        // Assign file-level statistics to each result
        for result in &mut file_results {
            let file_path_str = result.file.clone();
            if let Some(&(unique_terms, total_matches, rank)) = file_stats.get(&file_path_str) {
                result.file_unique_terms = Some(unique_terms);
                result.file_total_matches = Some(total_matches);
                result.file_match_rank = Some(rank);
            }
        }

        results.extend(file_results);
    }

    // If filename matching is enabled, find files whose names match query words
    let filename_matching_files;
    if include_filenames {
        filename_matching_files =
            find_matching_filenames(path, queries, &found_files, custom_ignores, allow_tests)?;

        for file_path in filename_matching_files {
            // For filename matches, we need to determine which queries match the filename
            let filename = file_path
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_default();

            // We don't need to use the filename_matched_queries here since these are already
            // files that matched by filename, but we include it for consistency
            let _filename_matched_queries =
                get_filename_matched_queries_compat(&filename, &queries_terms);

            match process_file_by_filename(&file_path) {
                Ok(mut result) => {
                    // Ensure the matched_by_filename flag is set
                    result.matched_by_filename = Some(true);

                    // Assign file-level statistics
                    let file_path_str = file_path.to_string_lossy().to_string();
                    if let Some(&(unique_terms, total_matches, rank)) =
                        file_stats.get(&file_path_str)
                    {
                        result.file_unique_terms = Some(unique_terms);
                        result.file_total_matches = Some(total_matches);
                        result.file_match_rank = Some(rank);
                    }

                    results.push(result);
                }
                Err(err) => eprintln!("Error processing file {:?}: {}", file_path, err),
            }
        }
    }

    // Record result processing time
    timings.result_processing = Some(result_processing_start.elapsed());

    // Rank the results if there are any
    let result_ranking_start = Instant::now();
    if !results.is_empty() {
        rank_search_results(&mut results, queries, reranker);
    }
    timings.result_ranking = Some(result_ranking_start.elapsed());

    // Apply default token limit of 100k if not specified
    let default_max_tokens = max_tokens.or(Some(100000));

    // Apply limits and return
    let limit_application_start = Instant::now();
    let limited_results = apply_limits(results, max_results, max_bytes, default_max_tokens);
    timings.limit_application = Some(limit_application_start.elapsed());

    // Print timing information in debug mode
    if debug_mode {
        println!("Search Timings:");
        if let Some(dur) = timings.query_preprocessing {
            println!("  Query Preprocessing: {:?}", dur);
        }
        if let Some(dur) = timings.file_searching {
            println!("  File Searching: {:?}", dur);
        }
        if let Some(dur) = timings.result_processing {
            println!("  Result Processing: {:?}", dur);
        }
        if let Some(dur) = timings.result_ranking {
            println!("  Result Ranking: {:?}", dur);
        }
        if let Some(dur) = timings.limit_application {
            println!("  Limit Application: {:?}", dur);
        }
    }

    Ok(limited_results)
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
    allow_tests: bool,
    any_term: bool,
    exact: bool,
) -> Result<LimitedSearchResults> {
    // Check if debug mode is enabled
    let debug_mode = std::env::var("CODE_SEARCH_DEBUG").unwrap_or_default() == "1";

    // Initialize timing structure
    let mut timings = SearchTimings {
        query_preprocessing: None,
        file_searching: None,
        result_processing: None,
        result_ranking: None,
        limit_application: None,
    };

    // Start timing query preprocessing
    let query_preprocessing_start = Instant::now();

    if debug_mode {
        println!("Performing frequency-based search for query: {}", query);
        println!("DEBUG: Starting frequency-based search with parameters:");
        println!("DEBUG:   Path: {:?}", path);
        println!("DEBUG:   Query: {}", query);
        println!("DEBUG:   Files only: {}", files_only);
        println!("DEBUG:   Custom ignores: {:?}", custom_ignores);
        println!("DEBUG:   Include filenames: {}", include_filenames);
        println!("DEBUG:   Reranker: {}", reranker);
    }

    // 1. Preprocess the query into original/stemmed pairs
    let term_pairs = preprocess_query(query, exact);

    if term_pairs.is_empty() {
        println!("No valid search terms after preprocessing");
        return Ok(LimitedSearchResults {
            results: Vec::new(),
            skipped_files: Vec::new(),
            limits_applied: None,
        });
    }

    // 2. Create regex patterns for each term with word boundaries
    let patterns_with_terms = create_term_patterns(&term_pairs);

    // Extract just the patterns for compatibility with existing code
    let term_patterns: Vec<String> = patterns_with_terms
        .iter()
        .map(|(pattern, _)| pattern.clone())
        .collect();

    // Record query preprocessing time
    timings.query_preprocessing = Some(query_preprocessing_start.elapsed());

    // Always print this message for test compatibility
    println!("Frequency search enabled");

    if debug_mode {
        println!("Original query: {}", query);
        println!("After preprocessing:");
        for (i, (original, stemmed)) in term_pairs.iter().enumerate() {
            if original == stemmed {
                println!("  Term {}: {} (stemmed same as original)", i + 1, original);
            } else {
                println!("  Term {}: {} (stemmed to {})", i + 1, original, stemmed);
            }
        }
        println!("Search patterns: {:?}", patterns_with_terms);
    }

    // Start timing file searching phase
    let file_searching_start = Instant::now();

    // 3. Find all files and their matched terms
    let mut file_matched_terms: HashMap<PathBuf, HashSet<usize>> = HashMap::new();
    let _file_total_matches: HashMap<PathBuf, usize> = HashMap::new();
    let mut file_matched_lines: HashMap<PathBuf, HashSet<usize>> = HashMap::new();

    // Collect all files matching any pattern
    let _require_all = !any_term;

    // Use a HashSet to collect unique file paths matching any pattern
    use std::collections::HashSet;
    let mut all_matching_files = HashSet::new();
    for pattern in &term_patterns {
        let matching_files = find_files_with_pattern(path, pattern, custom_ignores, allow_tests)?;

        if debug_mode {
            let matching_count = matching_files.len();
            println!(
                "Found {} files matching pattern: {}",
                matching_count, pattern
            );
        }

        all_matching_files.extend(matching_files);
    }

    // Convert to Vec for further processing
    let initial_files: Vec<PathBuf> = all_matching_files.into_iter().collect();

    if debug_mode {
        println!(
            "Found {} unique files matching all patterns",
            initial_files.len()
        );
    }

    // Initialize with empty sets
    for file in &initial_files {
        file_matched_terms.insert(file.clone(), HashSet::new());
        file_matched_lines.insert(file.clone(), HashSet::new());
    }

    // Create a HashMap to track filename matches
    let mut file_filename_matches: HashMap<PathBuf, HashSet<usize>> = HashMap::new();

    // Process filename matches for all initial files
    for file_path in &initial_files {
        // Convert term_pairs to the format expected by the new function
        // Each (term, _) becomes (term, index)
        let indexed_terms: Vec<(String, usize)> = term_pairs
            .iter()
            .enumerate()
            .map(|(idx, (term, _))| (term.clone(), idx))
            .collect();

        // Use the new function with full path
        let filename_matched_terms =
            get_filename_matched_queries(file_path, path, &[indexed_terms]);
        if !filename_matched_terms.is_empty() {
            file_filename_matches.insert(file_path.clone(), filename_matched_terms);
        }
    }

    // Create a thread-safe container for collecting results
    let file_matched_terms: Arc<Mutex<HashMap<PathBuf, HashSet<usize>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    // Create thread-safe containers for other tracking data
    let _file_total_matches: Arc<Mutex<HashMap<PathBuf, usize>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let file_matched_lines: Arc<Mutex<HashMap<PathBuf, HashSet<usize>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    // Process patterns in parallel
    patterns_with_terms
        .par_iter()
        .for_each(|(pattern, term_indices)| {
            if debug_mode {
                println!("Searching files for pattern: {}", pattern);
            }

            // Find files matching this pattern
            if let Ok(matching_files) =
                find_files_with_pattern(path, pattern, custom_ignores, allow_tests)
            {
                for file_path in matching_files {
                    // Search for this pattern in the file content
                    if let Ok((matched, line_numbers)) =
                        search_file_for_pattern(&file_path, pattern, true)
                    {
                        if matched {
                            // Add the term indices this pattern represents to the file's matched terms
                            {
                                let mut file_matches_lock = file_matched_terms.lock().unwrap();
                                let content_matches =
                                    file_matches_lock.entry(file_path.clone()).or_default();
                                content_matches.extend(term_indices.clone());
                            }

                            // Add the number of matches (line numbers) to the total matches count
                            // {
                            //     let mut file_total_matches_lock = _file_total_matches.lock().unwrap();
                            //     *file_total_matches_lock.entry(file_path.clone()).or_insert(0) += line_numbers.len();
                            // }

                            // Add matched line numbers
                            {
                                let mut file_matched_lines_lock =
                                    file_matched_lines.lock().unwrap();
                                let lines = file_matched_lines_lock
                                    .entry(file_path.clone())
                                    .or_default();
                                lines.extend(line_numbers);
                            }
                        }
                    }
                }
            }
        });

    // Extract the results from the Arc<Mutex<...>>
    let file_matched_terms = Arc::try_unwrap(file_matched_terms)
        .expect("Failed to unwrap Arc")
        .into_inner()
        .expect("Failed to unwrap Mutex");

    // let _file_total_matches = Arc::try_unwrap(_file_total_matches)
    //     .expect("Failed to unwrap Arc")
    //     .into_inner()
    //     .expect("Failed to unwrap Mutex");

    let file_matched_lines = Arc::try_unwrap(file_matched_lines)
        .expect("Failed to unwrap Arc")
        .into_inner()
        .expect("Failed to unwrap Mutex");

    // Record file searching time
    timings.file_searching = Some(file_searching_start.elapsed());

    // 4. Create a combined structure for sorting
    let mut files_by_frequency: Vec<(PathBuf, usize, usize)> = file_matched_terms
        .iter()
        .map(|(path, term_set)| {
            // Get filename matches for this path
            let filename_matches = file_filename_matches.get(path).cloned().unwrap_or_default();

            // Combine content and filename matches
            let all_matches: HashSet<usize> = term_set.union(&filename_matches).cloned().collect();

            let term_count = all_matches.len();
            let total_matches = 0; //file_total_matches.get(path).cloned().unwrap_or(0);
            (path.clone(), term_count, total_matches)
        })
        .collect();

    // Sort by term count first, then by total matches if term counts are equal
    files_by_frequency.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| b.2.cmp(&a.2)));

    // Create a copy of files_by_frequency for debug output
    let all_files_for_debug = if debug_mode {
        files_by_frequency.clone()
    } else {
        Vec::new() // Empty vec if not in debug mode to save memory
    };

    // Take the top files (all files that match all terms, or the top N files that match any term)
    let top_files = if !any_term {
        // For "all terms" mode, only include files that match all terms
        files_by_frequency
            .into_iter()
            .filter(|(path, _, _)| {
                let content_matched = file_matched_terms.get(path).cloned().unwrap_or_default();
                let filename_matched = file_filename_matches.get(path).cloned().unwrap_or_default();
                let all_matched: HashSet<usize> =
                    content_matched.union(&filename_matched).cloned().collect();
                (0..term_pairs.len()).all(|i| all_matched.contains(&i))
            })
            .collect::<Vec<_>>()
    } else {
        // For "any term" mode, include all files that match any term
        files_by_frequency
            .into_iter()
            .filter(|(path, _, _)| {
                file_matched_terms.contains_key(path) || file_filename_matches.contains_key(path)
            })
            .collect::<Vec<_>>()
    };

    // Store a clone of top_files for later use
    let top_files_clone = top_files.clone();

    if debug_mode {
        for (file_path, _term_count, _total_matches) in &all_files_for_debug {
            let content_matched = file_matched_terms
                .get(file_path)
                .cloned()
                .unwrap_or_default();
            let filename_matched = file_filename_matches
                .get(file_path)
                .cloned()
                .unwrap_or_default();
            let all_matched: HashSet<usize> =
                content_matched.union(&filename_matched).cloned().collect();
            let included = if !any_term {
                (0..term_pairs.len()).all(|i| all_matched.contains(&i))
            } else {
                !all_matched.is_empty()
            };
            println!(
                "DEBUG: File {:?} - Content matches: {:?}, Filename matches: {:?}, Included: {}",
                file_path, content_matched, filename_matched, included
            );
        }

        println!(
            "Found {} files matching the frequency search criteria",
            top_files.len()
        );
    }

    // Compute file-level statistics
    let mut file_stats: HashMap<String, (usize, usize, usize)> = HashMap::new();
    let mut file_total_matches: Vec<(String, usize)> = Vec::new();

    // Collect all files with their match counts
    for (file_path, _unique_terms, _) in &top_files {
        // Get content matches (line numbers) for this file
        let content_lines = file_matched_lines
            .get(file_path)
            .map(|lines| lines.len())
            .unwrap_or(0);

        // Get filename matches for this file
        let filename = file_path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        let filename_terms = get_filename_matched_queries_compat(&filename, &[term_pairs.clone()]);

        // Calculate total matches (content lines + filename matches)
        let total_matches = content_lines + filename_terms.len();

        // Store the total matches for ranking
        file_total_matches.push((file_path.to_string_lossy().to_string(), total_matches));
    }

    // Rank files by total matches (descending order)
    file_total_matches.sort_by(|a, b| b.1.cmp(&a.1));

    // Assign ranks based on sorted order
    for (rank, (file, total_matches)) in file_total_matches.iter().enumerate() {
        // Get file path
        let file_path = PathBuf::from(file);

        // Get content matches for this file
        let content_terms = file_matched_terms
            .get(&file_path)
            .cloned()
            .unwrap_or_default();

        // Get filename matches for this file
        let filename = file_path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        let filename_terms = get_filename_matched_queries_compat(&filename, &[term_pairs.clone()]);

        // Calculate unique terms
        let _unique_terms = content_terms.union(&filename_terms).count();

        // Store statistics: (_unique_terms, total_matches, rank)
        file_stats.insert(file.clone(), (_unique_terms, *total_matches, rank + 1));
    }

    // If no matches found and filename matching is disabled, return empty results
    if top_files.is_empty() && !include_filenames {
        println!("No matches found.");
        return Ok(LimitedSearchResults {
            results: Vec::new(),
            skipped_files: Vec::new(),
            limits_applied: None,
        });
    }

    // If files_only mode is enabled, just return file paths
    if files_only {
        println!("\nMatching files:");
        let mut results = Vec::new();
        for (file_path, _term_count, _total_matches) in &top_files {
            results.push(SearchResult {
                file: file_path.to_string_lossy().to_string(),
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
                file_unique_terms: None,
                file_total_matches: None,
                file_match_rank: None,
            });
        }

        // If filename matching is enabled, find files whose names match query words
        if include_filenames {
            let already_found_files: HashSet<PathBuf> =
                top_files_clone.iter().map(|(p, _, _)| p.clone()).collect();
            let matching_files = find_matching_filenames(
                path,
                &[query.to_string()],
                &already_found_files,
                custom_ignores,
                allow_tests,
            )?;

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
                    file_unique_terms: None,
                    file_total_matches: None,
                    file_match_rank: None,
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

                    // Display file-level statistics
                    if let Some(file_unique_terms) = result.file_unique_terms {
                        println!("File Unique Terms: {}", file_unique_terms);
                    }

                    if let Some(file_total_matches) = result.file_total_matches {
                        println!("File Total Matches: {}", file_total_matches);
                    }

                    if let Some(file_match_rank) = result.file_match_rank {
                        println!("File Match Rank: {}", file_match_rank);
                    }
                }
            }

            println!("Assigned ranks to {} file results", results.len());
        }

        return Ok(apply_limits(results, max_results, max_bytes, max_tokens));
    }

    // Process each file to extract matching lines
    let mut results = Vec::new();
    let mut skipped_files = Vec::new();
    let mut total_bytes = 0;
    let mut total_tokens = 0;

    for (file_path, __unique_terms, _total_matches) in top_files {
        // Skip if we've reached the maximum number of results
        if let Some(max_results) = max_results {
            if results.len() >= max_results {
                skipped_files.push(SearchResult {
                    file: file_path.to_string_lossy().to_string(),
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
                    file_unique_terms: Some(__unique_terms),
                    file_total_matches: Some(_total_matches),
                    file_match_rank: None,
                });
                continue;
            }
        }

        // Get the file statistics
        let file_path_str = file_path.to_string_lossy().to_string();
        let (file_unique_terms, file_total_matches, file_match_rank) =
            file_stats.get(&file_path_str).cloned().unwrap_or((0, 0, 0));

        // Read the file content
        let content = match fs::read_to_string(&file_path) {
            Ok(content) => content,
            Err(err) => {
                eprintln!("Error reading file {:?}: {}", file_path, err);
                continue;
            }
        };

        // Skip if the file is too large
        if let Some(max_bytes) = max_bytes {
            let file_bytes = content.len();
            if total_bytes + file_bytes > max_bytes {
                skipped_files.push(SearchResult {
                    file: file_path.to_string_lossy().to_string(),
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
                    file_unique_terms: Some(file_unique_terms),
                    file_total_matches: Some(file_total_matches),
                    file_match_rank: Some(file_match_rank),
                });
                continue;
            }
            total_bytes += file_bytes;
        }

        // Skip if the file has too many tokens
        if let Some(max_tokens) = max_tokens {
            let file_tokens = content.split_whitespace().count();
            if total_tokens + file_tokens > max_tokens {
                skipped_files.push(SearchResult {
                    file: file_path.to_string_lossy().to_string(),
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
                    file_unique_terms: Some(file_unique_terms),
                    file_total_matches: Some(file_total_matches),
                    file_match_rank: Some(file_match_rank),
                });
                continue;
            }
            total_tokens += file_tokens;
        }

        // Get the matched lines for this file
        let matched_lines = file_matched_lines
            .get(&file_path)
            .cloned()
            .unwrap_or_default();

        if matched_lines.is_empty() {
            // If no specific lines matched but the file is in top_files,
            // it means the file matched by filename
            results.push(SearchResult {
                file: file_path.to_string_lossy().to_string(),
                lines: (1, content.lines().count()),
                node_type: "file".to_string(),
                code: content,
                matched_by_filename: Some(true),
                rank: None,
                score: None,
                tfidf_score: None,
                bm25_score: None,
                tfidf_rank: None,
                bm25_rank: None,
                file_unique_terms: Some(file_unique_terms),
                file_total_matches: Some(file_total_matches),
                file_match_rank: Some(file_match_rank),
            });
        } else {
            // Get term-specific matches for this file
            let term_matches = if let Some(terms) = file_matched_terms.get(&file_path) {
                let mut map = HashMap::new();
                for &term_idx in terms {
                    map.insert(term_idx, matched_lines.clone());
                }
                Some(map)
            } else {
                None
            };

            // Get the filename for matching
            let filename = file_path
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_default();

            // Determine which terms match in the filename
            let filename_matched_terms =
                get_filename_matched_queries_compat(&filename, &[term_pairs.clone()]);

            // Process the file with both content and filename matches
            let mut file_results = process_file_with_results(
                &file_path,
                &matched_lines,
                allow_tests,
                term_matches.as_ref(),
                any_term,
                term_pairs.len(),
                filename_matched_terms,
            )?;

            // Assign file-level statistics to each result
            for result in &mut file_results {
                let file_path_str = result.file.clone();
                if let Some(&(unique_terms, total_matches, rank)) = file_stats.get(&file_path_str) {
                    result.file_unique_terms = Some(unique_terms);
                    result.file_total_matches = Some(total_matches);
                    result.file_match_rank = Some(rank);
                }
            }

            results.extend(file_results);
        }
    }

    // If filename matching is enabled, find files whose names match query words
    let filename_matching_files;
    if include_filenames {
        filename_matching_files = find_matching_filenames(
            path,
            &[query.to_string()],
            &top_files_clone.iter().map(|(p, _, _)| p.clone()).collect(),
            custom_ignores,
            allow_tests,
        )?;

        for file_path in filename_matching_files {
            // Process the file that matched by filename
            match process_file_by_filename(&file_path) {
                Ok(mut result) => {
                    // Assign file-level statistics
                    let file_path_str = file_path.to_string_lossy().to_string();
                    if let Some(&(unique_terms, total_matches, rank)) =
                        file_stats.get(&file_path_str)
                    {
                        result.file_unique_terms = Some(unique_terms);
                        result.file_total_matches = Some(total_matches);
                        result.file_match_rank = Some(rank);
                    }
                    results.push(result);
                }
                Err(err) => eprintln!("Error processing file {:?}: {}", file_path, err),
            }
        }
    }

    // Record result processing time
    let result_processing_start = Instant::now();

    // Rank the results if there are any
    let result_ranking_start = Instant::now();
    if !results.is_empty() {
        rank_search_results(&mut results, &[query.to_string()], reranker);
    }
    timings.result_ranking = Some(result_ranking_start.elapsed());
    timings.result_processing = Some(result_processing_start.elapsed());

    // Apply default token limit of 100k if not specified
    let default_max_tokens = max_tokens.or(Some(100000));

    // Apply limits and return
    let limit_application_start = Instant::now();
    let limited_results = apply_limits(results, max_results, max_bytes, default_max_tokens);
    timings.limit_application = Some(limit_application_start.elapsed());

    // Print timing information in debug mode
    if debug_mode {
        println!("Search Timings:");
        if let Some(dur) = timings.query_preprocessing {
            println!("  Query Preprocessing: {:?}", dur);
        }
        if let Some(dur) = timings.file_searching {
            println!("  File Searching: {:?}", dur);
        }
        if let Some(dur) = timings.result_processing {
            println!("  Result Processing: {:?}", dur);
        }
        if let Some(dur) = timings.result_ranking {
            println!("  Result Ranking: {:?}", dur);
        }
        if let Some(dur) = timings.limit_application {
            println!("  Limit Application: {:?}", dur);
        }
    }

    Ok(limited_results)
}
