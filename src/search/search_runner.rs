use crate::search::file_list_cache;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
// No need for term_exceptions import

use crate::models::{LimitedSearchResults, SearchResult};
use crate::search::{
    cache,
    // file_list_cache, // Add the new file_list_cache module (unused)
    file_processing::{process_file_with_results, FileProcessingParams},
    query::{create_query_plan, create_structured_patterns, QueryPlan},
    result_ranking::rank_search_results,
    search_limiter::apply_limits,
    search_options::SearchOptions,
    timeout,
};
use crate::path_resolver::resolve_path;

/// Struct to hold timing information for different stages of the search process
pub struct SearchTimings {
    pub query_preprocessing: Option<Duration>,
    pub pattern_generation: Option<Duration>,
    pub file_searching: Option<Duration>,
    pub filename_matching: Option<Duration>,
    pub early_filtering: Option<Duration>,
    pub early_caching: Option<Duration>,
    pub result_processing: Option<Duration>,
    // Granular result processing timings
    pub result_processing_file_io: Option<Duration>,
    pub result_processing_line_collection: Option<Duration>,
    pub result_processing_ast_parsing: Option<Duration>,
    pub result_processing_block_extraction: Option<Duration>,
    pub result_processing_result_building: Option<Duration>,

    // Granular AST parsing sub-step timings
    pub result_processing_ast_parsing_language_init: Option<Duration>,
    pub result_processing_ast_parsing_parser_init: Option<Duration>,
    pub result_processing_ast_parsing_tree_parsing: Option<Duration>,
    pub result_processing_ast_parsing_line_map_building: Option<Duration>,

    // Granular block extraction sub-step timings
    pub result_processing_block_extraction_code_structure: Option<Duration>,
    pub result_processing_block_extraction_filtering: Option<Duration>,
    pub result_processing_block_extraction_result_building: Option<Duration>,

    // Detailed result building timings
    pub result_processing_term_matching: Option<Duration>,
    pub result_processing_compound_processing: Option<Duration>,
    pub result_processing_line_matching: Option<Duration>,
    pub result_processing_result_creation: Option<Duration>,
    pub result_processing_synchronization: Option<Duration>,
    pub result_processing_uncovered_lines: Option<Duration>,

    pub result_ranking: Option<Duration>,
    pub limit_application: Option<Duration>,
    pub block_merging: Option<Duration>,
    pub final_caching: Option<Duration>,
    pub total_search_time: Option<Duration>,
}

/// Helper function to format duration in a human-readable way
pub fn format_duration(duration: Duration) -> String {
    if duration.as_millis() < 1000 {
        let millis = duration.as_millis();
        format!("{millis}ms")
    } else {
        let secs = duration.as_secs_f64();
        format!("{secs:.2}s")
    }
}

/// Helper function to print timing information in debug mode
pub fn print_timings(timings: &SearchTimings) {
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";
    if !debug_mode {
        return;
    }

    println!("\n=== SEARCH TIMING INFORMATION ===");

    if let Some(duration) = timings.query_preprocessing {
        println!("Query preprocessing:   {}", format_duration(duration));
    }

    if let Some(duration) = timings.pattern_generation {
        println!("Pattern generation:    {}", format_duration(duration));
    }

    if let Some(duration) = timings.file_searching {
        println!("File searching:        {}", format_duration(duration));
    }

    if let Some(duration) = timings.filename_matching {
        println!("Filename matching:     {}", format_duration(duration));
    }

    if let Some(duration) = timings.early_filtering {
        println!("Early AST filtering:   {}", format_duration(duration));
    }

    if let Some(duration) = timings.early_caching {
        println!("Early caching:         {}", format_duration(duration));
    }

    if let Some(duration) = timings.result_processing {
        println!("Result processing:     {}", format_duration(duration));

        // Print granular result processing timings if available
        if let Some(duration) = timings.result_processing_file_io {
            println!("  - File I/O:           {}", format_duration(duration));
        }

        if let Some(duration) = timings.result_processing_line_collection {
            println!("  - Line collection:    {}", format_duration(duration));
        }

        if let Some(duration) = timings.result_processing_ast_parsing {
            println!("  - AST parsing:        {}", format_duration(duration));

            // Print granular AST parsing sub-step timings
            if let Some(d) = timings.result_processing_ast_parsing_language_init {
                println!("    - Language init:     {}", format_duration(d));
            }
            if let Some(d) = timings.result_processing_ast_parsing_parser_init {
                println!("    - Parser init:       {}", format_duration(d));
            }
            if let Some(d) = timings.result_processing_ast_parsing_tree_parsing {
                println!("    - Tree parsing:      {}", format_duration(d));
            }
            if let Some(d) = timings.result_processing_ast_parsing_line_map_building {
                println!("    - Line map building: {}", format_duration(d));
            }
        }

        if let Some(duration) = timings.result_processing_block_extraction {
            println!("  - Block extraction:   {}", format_duration(duration));

            // Print granular block extraction sub-step timings
            if let Some(d) = timings.result_processing_block_extraction_code_structure {
                println!("    - Code structure:    {}", format_duration(d));
            }
            if let Some(d) = timings.result_processing_block_extraction_filtering {
                println!("    - Filtering:         {}", format_duration(d));
            }
            if let Some(d) = timings.result_processing_block_extraction_result_building {
                println!("    - Result building:   {}", format_duration(d));
            }
        }

        if let Some(duration) = timings.result_processing_result_building {
            println!("  - Result building:    {}", format_duration(duration));

            // Print detailed result building timings if available
            if let Some(d) = timings.result_processing_term_matching {
                println!("    - Term matching:      {}", format_duration(d));
            }
            if let Some(d) = timings.result_processing_compound_processing {
                println!("    - Compound processing: {}", format_duration(d));
            }
            if let Some(d) = timings.result_processing_line_matching {
                println!("    - Line matching:      {}", format_duration(d));
            }
            if let Some(d) = timings.result_processing_result_creation {
                println!("    - Result creation:    {}", format_duration(d));
            }
            if let Some(d) = timings.result_processing_synchronization {
                println!("    - Synchronization:    {}", format_duration(d));
            }
            if let Some(d) = timings.result_processing_uncovered_lines {
                println!("    - Uncovered lines:    {}", format_duration(d));
            }
        }
    }

    if let Some(duration) = timings.result_ranking {
        println!("Result ranking:        {}", format_duration(duration));
    }

    if let Some(duration) = timings.limit_application {
        println!("Limit application:     {}", format_duration(duration));
    }

    if let Some(duration) = timings.block_merging {
        println!("Block merging:         {}", format_duration(duration));
    }

    if let Some(duration) = timings.final_caching {
        println!("Final caching:         {}", format_duration(duration));
    }

    if let Some(duration) = timings.total_search_time {
        println!("Total search time:     {}", format_duration(duration));
    }

    println!("===================================\n");
}

// Removed evaluate_ignoring_negatives helper function in favor of direct usage

/// Our main "perform_probe" function remains largely the same. Below we show how you might
/// incorporate "search_with_structured_patterns" to handle the AST logic in a specialized path.
/// For simplicity, we won't fully replace the existing logic. Instead, we'll demonstrate
/// how you'd do it if you wanted to leverage the new approach.
pub fn perform_probe(options: &SearchOptions) -> Result<LimitedSearchResults> {
    // Start timing the entire search process
    let total_start = Instant::now();

    let SearchOptions {
        path,
        queries,
        files_only,
        custom_ignores,
        exclude_filenames,
        reranker,
        frequency_search: _,
        exact,
        language,
        max_results,
        max_bytes,
        max_tokens,
        allow_tests,
        no_merge,
        merge_threshold,
        dry_run: _, // We don't need this in perform_probe, but need to include it in the pattern
        session,
        timeout,
    } = options;
    // Start the timeout thread
    let timeout_handle = timeout::start_timeout_thread(*timeout);

    let include_filenames = !exclude_filenames;
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    // Handle session ID generation if session is provided but empty
    // For test runs, force session to None to disable caching
    let (effective_session, session_was_generated) = if let Some(s) = session {
        if s.is_empty() || *s == "new" {
            // Check if we have a session ID in the environment variable
            if let Ok(env_session_id) = std::env::var("PROBE_SESSION_ID") {
                if !env_session_id.is_empty() {
                    if debug_mode {
                        println!("DEBUG: Using session ID from environment: {env_session_id}");
                    }
                    // Convert to a static string (this leaks memory, but it's a small amount and only happens once per session)
                    let static_id: &'static str = Box::leak(env_session_id.into_boxed_str());
                    (Some(static_id), false)
                } else {
                    // Generate a unique session ID
                    match cache::generate_session_id() {
                        Ok((new_id, _is_new)) => {
                            if debug_mode {
                                println!("DEBUG: Generated new session ID: {new_id}");
                            }
                            (Some(new_id), true)
                        }
                        Err(e) => {
                            eprintln!("Error generating session ID: {e}");
                            (None, false)
                        }
                    }
                }
            } else {
                // Generate a unique session ID
                match cache::generate_session_id() {
                    Ok((new_id, _is_new)) => {
                        if debug_mode {
                            println!("DEBUG: Generated new session ID: {new_id}");
                        }
                        (Some(new_id), true)
                    }
                    Err(e) => {
                        eprintln!("Error generating session ID: {e}");
                        (None, false)
                    }
                }
            }
        } else {
            (Some(*s), false)
        }
    } else {
        // Check if we have a session ID in the environment variable
        if let Ok(env_session_id) = std::env::var("PROBE_SESSION_ID") {
            if !env_session_id.is_empty() {
                if debug_mode {
                    println!("DEBUG: Using session ID from environment: {env_session_id}");
                }
                // Convert to a static string (this leaks memory, but it's a small amount and only happens once per session)
                let static_id: &'static str = Box::leak(env_session_id.into_boxed_str());
                (Some(static_id), false)
            } else {
                (None, false)
            }
        } else {
            (None, false)
        }
    };

    let mut timings = SearchTimings {
        query_preprocessing: None,
        pattern_generation: None,
        file_searching: None,
        filename_matching: None,
        early_filtering: None,
        early_caching: None,
        result_processing: None,
        result_processing_file_io: None,
        result_processing_line_collection: None,
        result_processing_ast_parsing: None,
        result_processing_block_extraction: None,
        result_processing_result_building: None,

        // Initialize granular AST parsing sub-step timings
        result_processing_ast_parsing_language_init: None,
        result_processing_ast_parsing_parser_init: None,
        result_processing_ast_parsing_tree_parsing: None,
        result_processing_ast_parsing_line_map_building: None,

        // Initialize granular block extraction sub-step timings
        result_processing_block_extraction_code_structure: None,
        result_processing_block_extraction_filtering: None,
        result_processing_block_extraction_result_building: None,

        // Initialize detailed result building timings
        result_processing_term_matching: None,
        result_processing_compound_processing: None,
        result_processing_line_matching: None,
        result_processing_result_creation: None,
        result_processing_synchronization: None,
        result_processing_uncovered_lines: None,

        result_ranking: None,
        limit_application: None,
        block_merging: None,
        final_caching: None,
        total_search_time: None,
    };

    // Combine multiple queries with AND or just parse single query
    let qp_start = Instant::now();
    if debug_mode {
        println!("DEBUG: Starting query preprocessing...");
    }

    let parse_res = if queries.len() > 1 {
        // Join multiple queries with AND
        let combined_query = queries.join(" AND ");
        create_query_plan(&combined_query, *exact)
    } else {
        create_query_plan(&queries[0], *exact)
    };

    let qp_duration = qp_start.elapsed();
    timings.query_preprocessing = Some(qp_duration);

    if debug_mode {
        println!(
            "DEBUG: Query preprocessing completed in {}",
            format_duration(qp_duration)
        );
    }

    // If the query fails to parse, return empty results
    if parse_res.is_err() {
        println!("Failed to parse query as AST expression");
        return Ok(LimitedSearchResults {
            results: Vec::new(),
            skipped_files: Vec::new(),
            limits_applied: None,
            cached_blocks_skipped: None,
        });
    }

    // All queries go through the AST path
    let plan = parse_res.unwrap();

    // Pattern generation timing
    let pg_start = Instant::now();
    if debug_mode {
        println!("DEBUG: Starting pattern generation...");
        println!("DEBUG: Using combined pattern approach for more efficient searching");
    }

    // Use combined pattern approach for more efficient searching
    let structured_patterns = create_structured_patterns(&plan);

    let pg_duration = pg_start.elapsed();
    timings.pattern_generation = Some(pg_duration);

    if debug_mode {
        println!(
            "DEBUG: Pattern generation completed in {}",
            format_duration(pg_duration)
        );
        println!(
            "DEBUG: Generated {patterns_len} patterns",
            patterns_len = structured_patterns.len()
        );
        if structured_patterns.len() == 1 {
            println!("DEBUG: Successfully created a single combined pattern for all terms");
        }
    }

    // File searching timing
    let fs_start = Instant::now();
    if debug_mode {
        println!("DEBUG: Starting file searching...");
    }

    /*
      Important Note on Non-Determinism:
      The code in `search_with_structured_patterns` builds a single "combined" regex
      with multiple capturing groups. If more than one subpattern can match the same
      text, the regex engine’s backtracking might fill capture group 1 vs. group 2
      differently from run to run under multithreading, producing inconsistent
      matched lines (and thus inconsistent "required terms"). That can cause files
      to be accepted or removed in “early filtering” unpredictably. If you're
      experiencing random 0-result runs, this combined-regex approach is the most
      likely culprit.
    */

    // Normalize language parameter to handle aliases
    let lang_param = language.as_ref().map(|lang| normalize_language_alias(lang));

    let mut file_term_map = search_with_structured_patterns(
        path,
        &plan,
        &structured_patterns,
        custom_ignores,
        *allow_tests,
        lang_param,
    )?;

    let fs_duration = fs_start.elapsed();
    timings.file_searching = Some(fs_duration);

    // Print debug information about search results
    if debug_mode {
        // Calculate total matches across all files
        let total_matches: usize = file_term_map
            .values()
            .map(|term_map| term_map.values().map(|lines| lines.len()).sum::<usize>())
            .sum();

        // Get number of unique files
        let unique_files = file_term_map.keys().len();

        println!(
            "DEBUG: File searching completed in {} - Found {} matches in {} unique files",
            format_duration(fs_duration),
            total_matches,
            unique_files
        );
    }

    // Build final results
    let mut all_files = file_term_map.keys().cloned().collect::<HashSet<_>>();

    // Add filename matches if enabled
    let fm_start = Instant::now();
    if include_filenames && !exact {
        if debug_mode {
            println!("DEBUG: Starting filename matching...");
        }
        // Find all files that match our patterns by filename, along with the terms that matched
        // Resolve the path if it's a special format (e.g., "go:github.com/user/repo")
        let resolved_path = if let Some(path_str) = path.to_str() {
            match resolve_path(path_str) {
                Ok(resolved_path) => {
                    if debug_mode {
                        println!(
                            "DEBUG: Resolved path '{}' to '{}'",
                            path_str,
                            resolved_path.display()
                        );
                    }
                    resolved_path
                }
                Err(err) => {
                    if debug_mode {
                        println!("DEBUG: Failed to resolve path '{path_str}': {err}");
                    }
                    // Fall back to the original path
                    path.to_path_buf()
                }
            }
        } else {
            // If we can't convert the path to a string, use it as is
            path.to_path_buf()
        };

        let filename_matches: HashMap<PathBuf, HashSet<usize>> =
            file_list_cache::find_matching_filenames(
                &resolved_path,
                queries,
                &all_files,
                custom_ignores,
                *allow_tests,
                &plan.term_indices,
                lang_param,
            )?;

        if debug_mode {
            println!(
                "DEBUG: Found {} files matching by filename",
                filename_matches.len()
            );
        }

        // Process files that matched by filename
        for (pathbuf, matched_terms) in &filename_matches {
            // Define a reasonable maximum file size (e.g., 10MB)
            const MAX_FILE_SIZE: u64 = 1024 * 1024;

            // Check file metadata and resolve symlinks before reading
            let resolved_path = match std::fs::canonicalize(pathbuf.as_path()) {
                Ok(path) => path,
                Err(e) => {
                    if debug_mode {
                        println!("DEBUG: Error resolving path for {pathbuf:?}: {e:?}");
                    }
                    continue;
                }
            };

            // Get file metadata to check size and file type
            let metadata = match std::fs::metadata(&resolved_path) {
                Ok(meta) => meta,
                Err(e) => {
                    if debug_mode {
                        println!("DEBUG: Error getting metadata for {resolved_path:?}: {e:?}");
                    }
                    continue;
                }
            };

            // Check if the file is too large
            if metadata.len() > MAX_FILE_SIZE {
                if debug_mode {
                    println!(
                        "DEBUG: Skipping file {:?} - file too large ({} bytes > {} bytes limit)",
                        resolved_path,
                        metadata.len(),
                        MAX_FILE_SIZE
                    );
                }
                continue;
            }

            // Read the file content to get the total number of lines
            let file_content = match std::fs::read_to_string(&resolved_path) {
                Ok(content) => content,
                Err(e) => {
                    if debug_mode {
                        println!(
                            "DEBUG: Error reading file {:?}: {:?} (size: {} bytes)",
                            resolved_path,
                            e,
                            metadata.len()
                        );
                    }
                    continue;
                }
            };

            // Count the number of lines in the file
            let line_count = file_content.lines().count();
            if line_count == 0 {
                if debug_mode {
                    println!("DEBUG: File {pathbuf:?} is empty, skipping");
                }
                continue;
            }

            // Create a set of all line numbers in the file (1-based indexing)
            let all_line_numbers: HashSet<usize> = (1..=line_count).collect();

            // Check if this file already has term matches from content search
            let mut term_map = if let Some(existing_map) = file_term_map.get(pathbuf) {
                if debug_mode {
                    println!(
                        "DEBUG: File {pathbuf:?} already has term matches from content search, extending"
                    );
                }
                existing_map.clone()
            } else {
                if debug_mode {
                    println!("DEBUG: Creating new term map for file {pathbuf:?}");
                }
                HashMap::new()
            };

            // Add the matched terms to the term map with all lines
            for &term_idx in matched_terms {
                term_map
                    .entry(term_idx)
                    .or_insert_with(HashSet::new)
                    .extend(&all_line_numbers);

                if debug_mode {
                    println!(
                        "DEBUG: Added term index {term_idx} to file {pathbuf:?} with all lines"
                    );
                }
            }

            // Update the file_term_map with the new or extended term map
            file_term_map.insert(pathbuf.clone(), term_map);
            all_files.insert(pathbuf.clone());

            if debug_mode {
                println!("DEBUG: Added file {pathbuf:?} with matching terms to file_term_map");
            }
        }
    }

    if debug_mode {
        println!("DEBUG: all_files after filename matches: {all_files:?}");
    }

    // Early filtering step - filter both all_files and file_term_map using full AST evaluation (including excluded terms?).
    // Actually we pass 'true' to 'evaluate(..., true)', so that ignores excluded terms, contrary to the debug comment.
    let early_filter_start = Instant::now();
    if debug_mode {
        println!("DEBUG: Starting early AST filtering...");
        println!("DEBUG: Before filtering: {} files", all_files.len());
    }

    // Create a new filtered file_term_map
    let mut filtered_file_term_map = HashMap::new();
    let mut filtered_all_files = HashSet::new();

    for pathbuf in &all_files {
        if let Some(term_map) = file_term_map.get(pathbuf) {
            // Extract unique terms found in the file
            let matched_terms: HashSet<usize> = term_map.keys().copied().collect();

            // Evaluate the file against the AST, but we pass 'true' for ignore_negatives
            if plan.ast.evaluate(&matched_terms, &plan.term_indices, true) {
                filtered_file_term_map.insert(pathbuf.clone(), term_map.clone());
                filtered_all_files.insert(pathbuf.clone());
            } else if debug_mode {
                println!("DEBUG: Early filtering removed file: {pathbuf:?}");
            }
        } else if debug_mode {
            println!("DEBUG: File {pathbuf:?} not found in file_term_map during early filtering");
        }
    }

    // Replace the original maps with the filtered ones
    file_term_map = filtered_file_term_map;
    all_files = filtered_all_files;

    if debug_mode {
        println!(
            "DEBUG: After early filtering: {} files remain",
            all_files.len()
        );
        println!("DEBUG: all_files after early filtering: {all_files:?}");
    }

    let early_filter_duration = early_filter_start.elapsed();
    timings.early_filtering = Some(early_filter_duration);

    if debug_mode {
        println!(
            "DEBUG: Early AST filtering completed in {}",
            format_duration(early_filter_duration)
        );
    }

    let fm_duration = fm_start.elapsed();
    timings.filename_matching = Some(fm_duration);

    if debug_mode && include_filenames {
        println!(
            "DEBUG: Filename matching completed in {}",
            format_duration(fm_duration)
        );
    }

    // Handle files-only mode
    if *files_only {
        let mut res = Vec::new();
        for f in all_files {
            res.push(SearchResult {
                file: f.to_string_lossy().to_string(),
                lines: (1, 1),
                node_type: "file".to_string(),
                code: String::new(),
                matched_by_filename: None,
                rank: None,
                score: None,
                tfidf_score: None,
                bm25_score: None,
                tfidf_rank: None,
                bm25_rank: None,
                new_score: None,
                hybrid2_rank: None,
                combined_score_rank: None,
                file_unique_terms: None,
                file_total_matches: None,
                file_match_rank: None,
                block_unique_terms: None,
                block_total_matches: None,
                parent_file_id: None,
                block_id: None,
                matched_keywords: None,
                tokenized_content: None,
            });
        }
        let mut limited = apply_limits(res, *max_results, *max_bytes, *max_tokens);

        // No caching for files-only mode
        limited.cached_blocks_skipped = None;

        // Set total search time
        timings.total_search_time = Some(total_start.elapsed());

        // Print timing information
        print_timings(&timings);

        return Ok(limited);
    }

    // Apply early caching if session is provided - AFTER getting ripgrep results but BEFORE processing
    let ec_start = Instant::now();
    let mut early_skipped_count = 0;
    if let Some(session_id) = effective_session {
        // Get the raw query string for caching
        let raw_query = if queries.len() > 1 {
            queries.join(" AND ")
        } else {
            queries[0].clone()
        };

        if debug_mode {
            println!(
                "DEBUG: Starting early caching for session: {session_id} with query: {raw_query}"
            );
            // Print cache contents before filtering
            if let Err(e) = cache::debug_print_cache(session_id, &raw_query) {
                eprintln!("Error printing cache: {e}");
            }
        }

        // Filter matched lines using the cache
        match cache::filter_matched_lines_with_cache(&mut file_term_map, session_id, &raw_query) {
            Ok(skipped) => {
                if debug_mode {
                    println!("DEBUG: Early caching skipped {skipped} matched lines");
                }
                early_skipped_count = skipped;
            }
            Err(e) => {
                // Log the error but continue without early caching
                eprintln!("Error applying early cache: {e}");
            }
        }

        // Update all_files based on the filtered file_term_map
        // Intersect with existing all_files to preserve filtering
        let cached_files = file_term_map.keys().cloned().collect::<HashSet<_>>();
        all_files = all_files.intersection(&cached_files).cloned().collect();

        if debug_mode {
            println!("DEBUG: all_files after caching: {all_files:?}");
        }
    }

    let ec_duration = ec_start.elapsed();
    timings.early_caching = Some(ec_duration);

    if debug_mode && effective_session.is_some() {
        println!(
            "DEBUG: Early caching completed in {}",
            format_duration(ec_duration)
        );
    }

    // Process the files for detailed results
    let rp_start = Instant::now();
    if debug_mode {
        println!(
            "DEBUG: Starting result processing for {} files after early caching...",
            all_files.len()
        );
    }

    let mut final_results = Vec::new();

    // Track granular timing for result processing stages
    let mut total_file_io_time = Duration::new(0, 0);
    let mut total_line_collection_time = Duration::new(0, 0);
    let mut total_ast_parsing_time = Duration::new(0, 0);
    let mut total_block_extraction_time = Duration::new(0, 0);
    let _total_result_building_time = Duration::new(0, 0);

    // Track granular timing for AST parsing sub-steps
    let mut total_ast_parsing_language_init_time = Duration::new(0, 0);
    let mut total_ast_parsing_parser_init_time = Duration::new(0, 0);
    let mut total_ast_parsing_tree_parsing_time = Duration::new(0, 0);
    let mut total_ast_parsing_line_map_building_time = Duration::new(0, 0);

    // Track granular timing for block extraction sub-steps
    let mut total_block_extraction_code_structure_time = Duration::new(0, 0);
    let mut total_block_extraction_filtering_time = Duration::new(0, 0);
    let mut total_block_extraction_result_building_time = Duration::new(0, 0);

    // Track detailed result building timings
    let mut total_term_matching_time = Duration::new(0, 0);
    let mut total_compound_processing_time = Duration::new(0, 0);
    let mut total_line_matching_time = Duration::new(0, 0);
    let mut total_result_creation_time = Duration::new(0, 0);
    let mut total_synchronization_time = Duration::new(0, 0);
    let mut total_uncovered_lines_time = Duration::new(0, 0);
    for pathbuf in &all_files {
        if debug_mode {
            println!("DEBUG: Processing file: {pathbuf:?}");
        }

        // Get the term map for this file
        if let Some(term_map) = file_term_map.get(pathbuf) {
            if debug_mode {
                println!("DEBUG: Term map for file: {term_map:?}");
            }

            // Gather matched lines - measure line collection time
            let line_collection_start = Instant::now();
            let mut all_lines = HashSet::new();
            for lineset in term_map.values() {
                all_lines.extend(lineset.iter());
            }
            let line_collection_duration = line_collection_start.elapsed();
            total_line_collection_time += line_collection_duration;

            if debug_mode {
                println!(
                    "DEBUG: Found {} matched lines in file in {}",
                    all_lines.len(),
                    format_duration(line_collection_duration)
                );
            }

            // Process file with matched lines
            let filename_matched_queries = HashSet::new();

            // Create a list of term pairs for backward compatibility
            let term_pairs: Vec<(String, String)> = plan
                .term_indices
                .keys()
                .map(|term| (term.clone(), term.clone()))
                .collect();

            let pparams = FileProcessingParams {
                path: pathbuf,
                line_numbers: &all_lines,
                allow_tests: *allow_tests,
                term_matches: term_map,
                num_queries: plan.term_indices.len(),
                filename_matched_queries,
                queries_terms: &[term_pairs],
                preprocessed_queries: None,
                no_merge: *no_merge,
                query_plan: &plan,
            };

            if debug_mode {
                println!(
                    "DEBUG: Processing file with params: {}",
                    pparams.path.display()
                );
            }

            // Process file and track granular timings
            match process_file_with_results(&pparams) {
                Ok((mut file_res, file_timings)) => {
                    // Accumulate granular timings from file processing
                    if let Some(duration) = file_timings.file_io {
                        total_file_io_time += duration;
                    }
                    if let Some(duration) = file_timings.ast_parsing {
                        total_ast_parsing_time += duration;
                    }
                    if let Some(duration) = file_timings.block_extraction {
                        total_block_extraction_time += duration;
                    }

                    // Add the new granular timings for AST parsing sub-steps
                    if let Some(duration) = file_timings.ast_parsing_language_init {
                        total_ast_parsing_language_init_time += duration;
                        if debug_mode {
                            println!("DEBUG:     - Language init: {}", format_duration(duration));
                        }
                    }
                    if let Some(duration) = file_timings.ast_parsing_parser_init {
                        total_ast_parsing_parser_init_time += duration;
                        if debug_mode {
                            println!("DEBUG:     - Parser init: {}", format_duration(duration));
                        }
                    }
                    if let Some(duration) = file_timings.ast_parsing_tree_parsing {
                        total_ast_parsing_tree_parsing_time += duration;
                        if debug_mode {
                            println!("DEBUG:     - Tree parsing: {}", format_duration(duration));
                        }
                    }
                    if let Some(duration) = file_timings.ast_parsing_line_map_building {
                        total_ast_parsing_line_map_building_time += duration;
                        if debug_mode {
                            println!(
                                "DEBUG:     - Line map building: {}",
                                format_duration(duration)
                            );
                        }
                    }

                    // Add the new granular timings for block extraction sub-steps
                    if let Some(duration) = file_timings.block_extraction_code_structure {
                        total_block_extraction_code_structure_time += duration;
                        if debug_mode {
                            println!(
                                "DEBUG:     - Code structure finding: {}",
                                format_duration(duration)
                            );
                        }
                    }
                    if let Some(duration) = file_timings.block_extraction_filtering {
                        total_block_extraction_filtering_time += duration;
                        if debug_mode {
                            println!("DEBUG:     - Filtering: {}", format_duration(duration));
                        }
                    }
                    if let Some(duration) = file_timings.block_extraction_result_building {
                        total_block_extraction_result_building_time += duration;
                        if debug_mode {
                            println!(
                                "DEBUG:     - Result building: {}",
                                format_duration(duration)
                            );
                        }
                    }

                    // Add the detailed result building timings
                    if let Some(duration) = file_timings.result_building_term_matching {
                        total_term_matching_time += duration;
                        if debug_mode {
                            println!("DEBUG:     - Term matching: {}", format_duration(duration));
                        }
                    }
                    if let Some(duration) = file_timings.result_building_compound_processing {
                        total_compound_processing_time += duration;
                        if debug_mode {
                            println!(
                                "DEBUG:     - Compound processing: {}",
                                format_duration(duration)
                            );
                        }
                    }
                    if let Some(duration) = file_timings.result_building_line_matching {
                        total_line_matching_time += duration;
                        if debug_mode {
                            println!("DEBUG:     - Line matching: {}", format_duration(duration));
                        }
                    }
                    if let Some(duration) = file_timings.result_building_result_creation {
                        total_result_creation_time += duration;
                        if debug_mode {
                            println!(
                                "DEBUG:     - Result creation: {}",
                                format_duration(duration)
                            );
                        }
                    }
                    if let Some(duration) = file_timings.result_building_synchronization {
                        total_synchronization_time += duration;
                        if debug_mode {
                            println!(
                                "DEBUG:     - Synchronization: {}",
                                format_duration(duration)
                            );
                        }
                    }
                    if let Some(duration) = file_timings.result_building_uncovered_lines {
                        total_uncovered_lines_time += duration;
                        if debug_mode {
                            println!(
                                "DEBUG:     - Uncovered lines: {}",
                                format_duration(duration)
                            );
                        }
                    }

                    if debug_mode {
                        println!("DEBUG: Got {} results from file processing", file_res.len());
                        if let Some(duration) = file_timings.file_io {
                            println!("DEBUG:   File I/O time: {}", format_duration(duration));
                        }
                        if let Some(duration) = file_timings.ast_parsing {
                            println!("DEBUG:   AST parsing time: {}", format_duration(duration));
                        }
                        if let Some(duration) = file_timings.block_extraction {
                            println!(
                                "DEBUG:   Block extraction time: {}",
                                format_duration(duration)
                            );
                        }
                        if let Some(duration) = file_timings.block_extraction_result_building {
                            println!(
                                "DEBUG:   Result building time: {}",
                                format_duration(duration)
                            );
                        }
                    }
                    final_results.append(&mut file_res);
                }
                Err(e) => {
                    if debug_mode {
                        println!("DEBUG: Error processing file: {e:?}");
                    }
                }
            }
        } else {
            // This should never happen, but keep for safety
            if debug_mode {
                println!("DEBUG: ERROR - File {pathbuf:?} not found in file_term_map but was in all_files");
            }
        }
    }

    let rp_duration = rp_start.elapsed();
    // Calculate the total time spent on detailed result building operations
    let detailed_result_building_time = total_term_matching_time
        + total_compound_processing_time
        + total_line_matching_time
        + total_result_creation_time
        + total_synchronization_time
        + total_uncovered_lines_time;

    // Calculate the result building time as the remaining time after accounting for other operations
    let accounted_time = total_file_io_time
        + total_line_collection_time
        + total_ast_parsing_time
        + total_block_extraction_time;
    let remaining_time = if rp_duration > accounted_time {
        rp_duration - accounted_time
    } else {
        // Use the sum of detailed timings if available, otherwise fallback to block extraction result building time
        if detailed_result_building_time > Duration::new(0, 0) {
            detailed_result_building_time
        } else {
            total_block_extraction_result_building_time
        }
    };

    timings.result_processing = Some(rp_duration);
    timings.result_processing_file_io = Some(total_file_io_time);
    timings.result_processing_line_collection = Some(total_line_collection_time);
    timings.result_processing_ast_parsing = Some(total_ast_parsing_time);
    timings.result_processing_block_extraction = Some(total_block_extraction_time);
    timings.result_processing_result_building = Some(remaining_time);

    // Set the detailed result building timings
    timings.result_processing_term_matching = Some(total_term_matching_time);
    timings.result_processing_compound_processing = Some(total_compound_processing_time);
    timings.result_processing_line_matching = Some(total_line_matching_time);
    timings.result_processing_result_creation = Some(total_result_creation_time);
    timings.result_processing_synchronization = Some(total_synchronization_time);
    timings.result_processing_uncovered_lines = Some(total_uncovered_lines_time);

    // Set the granular AST parsing sub-step timings
    timings.result_processing_ast_parsing_language_init =
        Some(total_ast_parsing_language_init_time);
    timings.result_processing_ast_parsing_parser_init = Some(total_ast_parsing_parser_init_time);
    timings.result_processing_ast_parsing_tree_parsing = Some(total_ast_parsing_tree_parsing_time);
    timings.result_processing_ast_parsing_line_map_building =
        Some(total_ast_parsing_line_map_building_time);

    // Set the granular block extraction sub-step timings
    timings.result_processing_block_extraction_code_structure =
        Some(total_block_extraction_code_structure_time);
    timings.result_processing_block_extraction_filtering =
        Some(total_block_extraction_filtering_time);
    timings.result_processing_block_extraction_result_building =
        Some(total_block_extraction_result_building_time);

    if debug_mode {
        println!(
            "DEBUG: Result processing completed in {} - Generated {} results",
            format_duration(rp_duration),
            final_results.len()
        );
        println!("DEBUG: Granular result processing timings:");
        println!("DEBUG:   File I/O: {}", format_duration(total_file_io_time));
        println!(
            "DEBUG:   Line collection: {}",
            format_duration(total_line_collection_time)
        );
        println!(
            "DEBUG:   AST parsing: {}",
            format_duration(total_ast_parsing_time)
        );
        println!(
            "DEBUG:     - Language init: {}",
            format_duration(total_ast_parsing_language_init_time)
        );
        println!(
            "DEBUG:     - Parser init: {}",
            format_duration(total_ast_parsing_parser_init_time)
        );
        println!(
            "DEBUG:     - Tree parsing: {}",
            format_duration(total_ast_parsing_tree_parsing_time)
        );
        println!(
            "DEBUG:     - Line map building: {}",
            format_duration(total_ast_parsing_line_map_building_time)
        );
        println!(
            "DEBUG:   Block extraction: {}",
            format_duration(total_block_extraction_time)
        );
        println!(
            "DEBUG:     - Code structure finding: {}",
            format_duration(total_block_extraction_code_structure_time)
        );
        println!(
            "DEBUG:     - Filtering: {}",
            format_duration(total_block_extraction_filtering_time)
        );
        println!(
            "DEBUG:     - Result building: {}",
            format_duration(total_block_extraction_result_building_time)
        );
        println!(
            "DEBUG:   Result building: {}",
            format_duration(remaining_time)
        );
    }
    // Rank results (skip if exact flag is set)
    let rr_start = Instant::now();
    if debug_mode {
        if *exact {
            println!("DEBUG: Skipping result ranking due to exact flag being set");
        } else {
            println!("DEBUG: Starting result ranking...");
        }
    }

    if !*exact {
        // Only perform ranking if exact flag is not set
        rank_search_results(&mut final_results, queries, reranker);
    }

    let rr_duration = rr_start.elapsed();
    timings.result_ranking = Some(rr_duration);

    if debug_mode {
        if *exact {
            println!(
                "DEBUG: Result ranking skipped in {}",
                format_duration(rr_duration)
            );
        } else {
            println!(
                "DEBUG: Result ranking completed in {}",
                format_duration(rr_duration)
            );
        }
    }

    // We'll move the caching step AFTER limiting results
    let mut skipped_count = early_skipped_count;
    let filtered_results = final_results;

    // Apply limits
    let la_start = Instant::now();
    if debug_mode {
        println!("DEBUG: Starting limit application...");
    }

    // First apply limits to the results
    let mut limited = apply_limits(filtered_results, *max_results, *max_bytes, *max_tokens);

    // Then apply caching AFTER limiting results
    let fc_start = Instant::now();

    if let Some(session_id) = effective_session {
        // Get the raw query string for caching
        let raw_query = if queries.len() > 1 {
            queries.join(" AND ")
        } else {
            queries[0].clone()
        };

        if debug_mode {
            println!(
                "DEBUG: Starting final caching for session: {session_id} with query: {raw_query}"
            );
            println!("DEBUG: Already skipped {early_skipped_count} lines in early caching");
            // Print cache contents before filtering
            if let Err(e) = cache::debug_print_cache(session_id, &raw_query) {
                eprintln!("Error printing cache: {e}");
            }
        }

        // Filter results using the cache - but only to count skipped blocks, not to filter
        match cache::filter_results_with_cache(&limited.results, session_id, &raw_query) {
            Ok((_, cached_skipped)) => {
                if debug_mode {
                    println!("DEBUG: Final caching found {cached_skipped} cached blocks");
                    println!(
                        "DEBUG: Total skipped (early + final): {}",
                        early_skipped_count + cached_skipped
                    );
                }

                skipped_count += cached_skipped;
            }
            Err(e) => {
                // Log the error but continue without caching
                eprintln!("Error checking cache: {e}");
            }
        }

        // Update the cache with the limited results
        if let Err(e) = cache::add_results_to_cache(&limited.results, session_id, &raw_query) {
            eprintln!("Error adding results to cache: {e}");
        }

        if debug_mode {
            println!("DEBUG: Added limited results to cache before merging");
            // Print cache contents after adding new results
            if let Err(e) = cache::debug_print_cache(session_id, &raw_query) {
                eprintln!("Error printing updated cache: {e}");
            }
        }
    }

    // Set the cached blocks skipped count
    limited.cached_blocks_skipped = if skipped_count > 0 {
        Some(skipped_count)
    } else {
        None
    };

    let fc_duration = fc_start.elapsed();
    timings.final_caching = Some(fc_duration);

    if debug_mode && effective_session.is_some() {
        println!(
            "DEBUG: Final caching completed in {}",
            format_duration(fc_duration)
        );
    }

    let la_duration = la_start.elapsed();
    timings.limit_application = Some(la_duration);

    if debug_mode {
        println!(
            "DEBUG: Limit application completed in {} - Final result count: {}",
            format_duration(la_duration),
            limited.results.len()
        );
    }

    // Optional block merging - AFTER initial caching
    let bm_start = Instant::now();
    if debug_mode && !limited.results.is_empty() && !*no_merge {
        println!("DEBUG: Starting block merging...");
    }

    let final_results = if !limited.results.is_empty() && !*no_merge {
        use crate::search::block_merging::merge_ranked_blocks;
        let merged = merge_ranked_blocks(limited.results.clone(), *merge_threshold);

        let bm_duration = bm_start.elapsed();
        timings.block_merging = Some(bm_duration);

        if debug_mode {
            println!(
                "DEBUG: Block merging completed in {} - Merged result count: {}",
                format_duration(bm_duration),
                merged.len()
            );
        }

        // Create the merged results
        let merged_results = LimitedSearchResults {
            results: merged.clone(),
            skipped_files: limited.skipped_files,
            limits_applied: limited.limits_applied,
            cached_blocks_skipped: limited.cached_blocks_skipped,
        };

        // Update the cache with the merged results (after merging)
        if let Some(session_id) = effective_session {
            // Get the raw query string for caching
            let raw_query = if queries.len() > 1 {
                queries.join(" AND ")
            } else {
                queries[0].clone()
            };

            if let Err(e) = cache::add_results_to_cache(&merged, session_id, &raw_query) {
                eprintln!("Error adding merged results to cache: {e}");
            }

            if debug_mode {
                println!("DEBUG: Added merged results to cache after merging");
                // Print cache contents after adding merged results
                if let Err(e) = cache::debug_print_cache(session_id, &raw_query) {
                    eprintln!("Error printing updated cache: {e}");
                }
            }
        }

        merged_results
    } else {
        let bm_duration = bm_start.elapsed();
        timings.block_merging = Some(bm_duration);

        if debug_mode && !*no_merge {
            println!(
                "DEBUG: Block merging skipped (no results or disabled) - {}",
                format_duration(bm_duration)
            );
        }

        limited
    };

    // Print the session ID to the console if it was generated or provided
    if let Some(session_id) = effective_session {
        if session_was_generated {
            println!("Session ID: {session_id} (generated - ALWAYS USE IT in future sessions for caching)");
        } else {
            println!("Session ID: {session_id}");
        }
    }

    // Set total search time
    timings.total_search_time = Some(total_start.elapsed());

    // Print timing information
    print_timings(&timings);

    // Stop the timeout thread
    timeout_handle.store(true, std::sync::atomic::Ordering::SeqCst);

    Ok(final_results)
}

/// Helper function to search files using structured patterns from a QueryPlan.
/// This function uses a RegexSet approach for deterministic pattern matching
/// and collects matches by term indices. It uses the file_list_cache to get a filtered
/// list of files respecting ignore patterns.
///
/// # Arguments
/// * `root_path` - The base path to search in
/// * `plan` - The parsed query plan
/// * `patterns` - The generated regex patterns with their term indices
/// * `custom_ignores` - Custom ignore patterns
/// * `allow_tests` - Whether to include test files
pub fn search_with_structured_patterns(
    root_path_str: &Path,
    _plan: &QueryPlan,
    patterns: &[(String, HashSet<usize>)],
    custom_ignores: &[String],
    allow_tests: bool,
    language: Option<&str>,
) -> Result<HashMap<PathBuf, HashMap<usize, HashSet<usize>>>> {
    // Resolve the path if it's a special format (e.g., "go:github.com/user/repo")
    let root_path = if let Some(path_str) = root_path_str.to_str() {
        match resolve_path(path_str) {
            Ok(resolved_path) => {
                if std::env::var("DEBUG").unwrap_or_default() == "1" {
                    println!(
                        "DEBUG: Resolved path '{}' to '{}'",
                        path_str,
                        resolved_path.display()
                    );
                }
                resolved_path
            }
            Err(err) => {
                if std::env::var("DEBUG").unwrap_or_default() == "1" {
                    println!("DEBUG: Failed to resolve path '{path_str}': {err}");
                }
                // Fall back to the original path
                root_path_str.to_path_buf()
            }
        }
    } else {
        // If we can't convert the path to a string, use it as is
        root_path_str.to_path_buf()
    };
    use rayon::prelude::*;
    use regex::RegexSet;
    use std::sync::{Arc, Mutex};

    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";
    let search_start = Instant::now();

    // Step 1: Create RegexSet for deterministic pattern matching
    if debug_mode {
        println!("DEBUG: Starting parallel structured pattern search with RegexSet...");
        println!("DEBUG: Creating RegexSet from {} patterns", patterns.len());
    }

    // Extract just the patterns for the RegexSet
    let pattern_strings: Vec<String> = patterns.iter().map(|(p, _)| format!("(?i){p}")).collect();

    // Create a RegexSet for deterministic matching
    let regex_set = RegexSet::new(&pattern_strings)?;

    // Create a mapping from pattern index to term indices
    let pattern_to_terms: Vec<HashSet<usize>> =
        patterns.iter().map(|(_, terms)| terms.clone()).collect();

    if debug_mode {
        println!("DEBUG: RegexSet created successfully");
    }

    // Step 2: Get filtered file list from cache
    if debug_mode {
        println!("DEBUG: Getting filtered file list from cache");
        println!("DEBUG: Custom ignore patterns: {custom_ignores:?}");
    }

    // Use file_list_cache to get a filtered list of files, with language filtering if specified
    let file_list = crate::search::file_list_cache::get_file_list_by_language(
        &root_path,
        allow_tests,
        custom_ignores,
        language,
    )?;

    if debug_mode {
        println!("DEBUG: Got {} files from cache", file_list.files.len());
        println!("DEBUG: Starting parallel file processing with RegexSet");
    }

    // Step 3: Process files in parallel
    // Create thread-safe shared resources
    let regex_set = Arc::new(regex_set);
    let pattern_to_terms = Arc::new(pattern_to_terms);
    let file_term_maps = Arc::new(Mutex::new(HashMap::new()));

    // Also create individual regexes for line number extraction
    let individual_regexes: Vec<regex::Regex> = pattern_strings
        .iter()
        .map(|p| regex::Regex::new(p).unwrap())
        .collect();
    let individual_regexes = Arc::new(individual_regexes);

    file_list.files.par_iter().for_each(|file_path| {
        let regex_set = Arc::clone(&regex_set);
        let pattern_to_terms = Arc::clone(&pattern_to_terms);
        let individual_regexes = Arc::clone(&individual_regexes);

        // Search file with RegexSet for deterministic matching
        match search_file_with_regex_set(
            file_path,
            &regex_set,
            &individual_regexes,
            &pattern_to_terms,
        ) {
            Ok(term_map) => {
                if !term_map.is_empty() {
                    if debug_mode {
                        println!(
                            "DEBUG: File {:?} matched patterns with {} term indices",
                            file_path,
                            term_map.len()
                        );
                    }

                    // Add to results with proper locking
                    let mut maps = file_term_maps.lock().unwrap();
                    maps.insert(file_path.clone(), term_map);
                }
            }
            Err(e) => {
                if debug_mode {
                    println!("DEBUG: Error searching file {file_path:?}: {e:?}");
                }
            }
        }
    });

    let total_duration = search_start.elapsed();

    // Extract the results from the Arc<Mutex<>>
    let result = Arc::try_unwrap(file_term_maps)
        .unwrap_or_else(|_| panic!("Failed to unwrap Arc"))
        .into_inner()
        .unwrap();

    if debug_mode {
        println!(
            "DEBUG: Parallel search completed in {} - Found matches in {} files",
            format_duration(total_duration),
            result.len()
        );
    }

    Ok(result)
}

/// Helper function to search a file with a RegexSet for deterministic pattern matching
/// This function searches a file for matches against a RegexSet and individual regexes
/// to map the matches to their corresponding term indices.
///
/// Using RegexSet ensures deterministic pattern matching across multiple runs,
/// avoiding the non-deterministic behavior of capturing groups in a combined regex.
fn search_file_with_regex_set(
    file_path: &Path,
    regex_set: &regex::RegexSet,
    individual_regexes: &[regex::Regex],
    pattern_to_terms: &[HashSet<usize>],
) -> Result<HashMap<usize, HashSet<usize>>> {
    let mut term_map = HashMap::new();
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    // Define a reasonable maximum file size (e.g., 1MB)
    const MAX_FILE_SIZE: u64 = 1024 * 1024;

    // Check file metadata and resolve symlinks before reading
    let resolved_path = match std::fs::canonicalize(file_path) {
        Ok(path) => path,
        Err(e) => {
            if debug_mode {
                println!("DEBUG: Error resolving path for {file_path:?}: {e:?}");
            }
            return Err(anyhow::anyhow!("Failed to resolve file path: {}", e));
        }
    };

    // Get file metadata to check size and file type
    let metadata = match std::fs::metadata(&resolved_path) {
        Ok(meta) => meta,
        Err(e) => {
            if debug_mode {
                println!("DEBUG: Error getting metadata for {resolved_path:?}: {e:?}");
            }
            return Err(anyhow::anyhow!("Failed to get file metadata: {}", e));
        }
    };

    // Check if the file is too large
    if metadata.len() > MAX_FILE_SIZE {
        if debug_mode {
            println!(
                "DEBUG: Skipping file {:?} - file too large ({} bytes > {} bytes limit)",
                resolved_path,
                metadata.len(),
                MAX_FILE_SIZE
            );
        }
        return Err(anyhow::anyhow!(
            "File too large: {} bytes (limit: {} bytes)",
            metadata.len(),
            MAX_FILE_SIZE
        ));
    }

    // Read the file content with proper error handling
    let content = match std::fs::read_to_string(&resolved_path) {
        Ok(content) => content,
        Err(e) => {
            if debug_mode {
                println!(
                    "DEBUG: Error reading file {:?}: {:?} (size: {} bytes)",
                    resolved_path,
                    e,
                    metadata.len()
                );
            }
            return Err(anyhow::anyhow!("Failed to read file: {}", e));
        }
    };

    // Process each line
    for (line_number, line) in content.lines().enumerate() {
        // Skip lines that are too long
        if line.len() > 2000 {
            if debug_mode {
                println!(
                    "DEBUG: Skipping line {} in file {:?} - line too long ({} characters)",
                    line_number + 1,
                    file_path,
                    line.len()
                );
            }
            continue;
        }

        // First check if any pattern matches using the RegexSet
        let matches = regex_set.matches(line);
        if matches.matched_any() {
            // For each matched pattern, find the specific line numbers using individual regexes
            for pattern_idx in matches.iter() {
                // Use the individual regex to find all matches in the line
                if individual_regexes[pattern_idx].is_match(line) {
                    // Add matches for all terms associated with this pattern
                    for &term_idx in &pattern_to_terms[pattern_idx] {
                        term_map
                            .entry(term_idx)
                            .or_insert_with(HashSet::new)
                            .insert(line_number + 1); // Convert to 1-based line numbers
                    }
                }
            }
        }
    }

    Ok(term_map)
}

/// Normalize language aliases to their canonical names
/// This function maps language aliases like "ts" to their canonical names like "typescript"
fn normalize_language_alias(lang: &str) -> &str {
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
}
