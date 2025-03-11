use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
// No need for term_exceptions import

use rayon::prelude::*;

use crate::models::{LimitedSearchResults, SearchResult};
use crate::search::{
    elastic_query::Expr,
    file_processing::{process_file_by_filename, process_file_with_results, FileProcessingParams},
    file_search::{find_files_with_pattern, find_matching_filenames, search_file_for_pattern},
    query::{create_query_plan, create_structured_patterns, QueryPlan},
    result_ranking::rank_search_results,
    search_limiter::apply_limits,
    search_options::SearchOptions,
    tokenization,
};

/// Struct to hold timing information for different stages of the search process
pub struct SearchTimings {
    pub query_preprocessing: Option<Duration>,
    pub file_searching: Option<Duration>,
    pub result_processing: Option<Duration>,
    pub result_ranking: Option<Duration>,
    pub limit_application: Option<Duration>,
    pub block_merging: Option<Duration>,
}

/// Our main "perform_probe" function remains largely the same. Below we show how you might
/// incorporate "search_with_structured_patterns" to handle the AST logic in a specialized path.
/// For simplicity, we won't fully replace the existing logic. Instead, we'll demonstrate
/// how you'd do it if you wanted to leverage the new approach.
pub fn perform_probe(options: &SearchOptions) -> Result<LimitedSearchResults> {
    let SearchOptions {
        path,
        queries,
        files_only,
        custom_ignores,
        exclude_filenames,
        reranker,
        frequency_search: _,
        max_results,
        max_bytes,
        max_tokens,
        allow_tests,
        exact,
        no_merge,
        merge_threshold,
        dry_run: _, // We don't need this in perform_probe, but need to include it in the pattern
    } = options;

    let include_filenames = !exclude_filenames;
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    let mut timings = SearchTimings {
        query_preprocessing: None,
        file_searching: None,
        result_processing: None,
        result_ranking: None,
        limit_application: None,
        block_merging: None,
    };

    // Combine multiple queries with AND or just parse single query
    let qp_start = Instant::now();
    let parse_res = if queries.len() > 1 {
        // Join multiple queries with AND
        let combined_query = queries.join(" AND ");
        create_query_plan(&combined_query, *exact)
    } else {
        create_query_plan(&queries[0], *exact)
    };
    timings.query_preprocessing = Some(qp_start.elapsed());

    // If the query fails to parse, return empty results
    if parse_res.is_err() {
        println!("Failed to parse query as AST expression");
        return Ok(LimitedSearchResults {
            results: Vec::new(),
            skipped_files: Vec::new(),
            limits_applied: None,
        });
    }

    // All queries go through the AST path
    let plan = parse_res.unwrap();
    let sp_start = Instant::now();
    let structured_patterns = create_structured_patterns(&plan);
    let file_term_map = search_with_structured_patterns(
        path,
        &plan,
        &structured_patterns,
        custom_ignores,
        *allow_tests,
    )?;
    timings.file_searching = Some(sp_start.elapsed());

    // Build final results
    let mut all_files = file_term_map.keys().cloned().collect::<HashSet<_>>();

    // Add filename matches if enabled
    let filename_matching_files = if include_filenames {
        find_matching_filenames(path, queries, &all_files, custom_ignores, *allow_tests)?
    } else {
        Vec::new()
    };
    all_files.extend(filename_matching_files.iter().cloned());

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
            });
        }
        return Ok(apply_limits(res, *max_results, *max_bytes, *max_tokens));
    }

    // Process the files for detailed results
    let rp_start = Instant::now();
    let mut final_results = Vec::new();

    if debug_mode {
        println!(
            "DEBUG: Processing {} files for detailed results",
            all_files.len()
        );
    }
    for pathbuf in &all_files {
        if debug_mode {
            println!("DEBUG: Processing file: {:?}", pathbuf);
        }

        // We'll handle excluded terms at the block level in filter_code_block_with_ast
        // This allows for more precise filtering based on the AST

        let empty_map = HashMap::new();
        let term_map = file_term_map.get(pathbuf).unwrap_or(&empty_map);

        if debug_mode {
            println!("DEBUG: Term map for file: {:?}", term_map);
        }

        if term_map.is_empty() {
            // File matched by filename only
            if debug_mode {
                println!("DEBUG: File matched by filename only");
            }
            if let Ok(sr) = process_file_by_filename(pathbuf, &[], None) {
                final_results.push(sr);
            }
            continue;
        }

        // Gather matched lines
        let mut all_lines = HashSet::new();
        for lineset in term_map.values() {
            all_lines.extend(lineset.iter());
        }

        if debug_mode {
            println!("DEBUG: Found {} matched lines in file", all_lines.len());
        }

        // Process file with matched lines
        let mut filename_matched_queries = HashSet::new();

        // For compound words like "networkfirewall", we need to ensure that
        // the individual terms "network" and "firewall" are counted as matches
        if queries.len() == 1 {
            let query = &queries[0];

            // Check if this is a compound word query
            let parts: Vec<&str> = query.split_whitespace().collect();
            if parts.len() == 1 {
                // This is a single word query, check if it's a compound word
                let compound_parts = tokenization::split_camel_case(query);
                if compound_parts.len() > 1 {
                    // This is a compound word, add all term indices to filename_matched_queries
                    for idx in 0..plan.term_indices.len() {
                        filename_matched_queries.insert(idx);
                    }
                }
            }
        }

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
            term_matches: Some(term_map),
            num_queries: plan.term_indices.len(),
            filename_matched_queries,
            queries_terms: &[term_pairs],
            preprocessed_queries: None,
            no_merge: *no_merge,
            query_plan: Some(&plan),
        };

        if debug_mode {
            println!("DEBUG: Processing file with params: {:?}", pparams.path);
        }

        match process_file_with_results(&pparams) {
            Ok(mut file_res) => {
                if debug_mode {
                    println!("DEBUG: Got {} results from file processing", file_res.len());
                }
                final_results.append(&mut file_res);
            }
            Err(e) => {
                if debug_mode {
                    println!("DEBUG: Error processing file: {:?}", e);
                }
            }
        }
    }
    if debug_mode {
        println!(
            "DEBUG: Final results before ranking: {}",
            final_results.len()
        );
        for (i, res) in final_results.iter().enumerate() {
            println!("DEBUG: Result {}: file={}", i, res.file);
        }
    }

    timings.result_processing = Some(rp_start.elapsed());

    // Rank results
    let rr_start = Instant::now();
    rank_search_results(&mut final_results, queries, reranker);
    timings.result_ranking = Some(rr_start.elapsed());

    if debug_mode {
        println!(
            "DEBUG: Final results after ranking: {}",
            final_results.len()
        );
        for (i, res) in final_results.iter().enumerate() {
            println!("DEBUG: Result {}: file={}", i, res.file);
        }
    }

    // Apply limits
    let la_start = Instant::now();
    let limited = apply_limits(final_results, *max_results, *max_bytes, *max_tokens);
    timings.limit_application = Some(la_start.elapsed());

    if debug_mode {
        println!(
            "DEBUG: Final results after limits: {}",
            limited.results.len()
        );
        for (i, res) in limited.results.iter().enumerate() {
            println!("DEBUG: Result {}: file={}", i, res.file);
        }
    }

    // Optional block merging
    let bm_start = Instant::now();
    if !limited.results.is_empty() && !*no_merge {
        use crate::search::block_merging::merge_ranked_blocks;
        let merged = merge_ranked_blocks(limited.results, *merge_threshold);
        timings.block_merging = Some(bm_start.elapsed());
        Ok(LimitedSearchResults {
            results: merged,
            skipped_files: limited.skipped_files,
            limits_applied: limited.limits_applied,
        })
    } else {
        timings.block_merging = Some(bm_start.elapsed());
        Ok(limited)
    }
}
/// Helper function to search files using structured patterns from a QueryPlan.
/// This function uses parallel processing to search for patterns and collects matches by term indices.
///
/// # Arguments
/// * `root_path` - The base path to search in
/// * `plan` - The parsed query plan
/// * `patterns` - The generated regex patterns with their term indices
/// * `custom_ignores` - Custom ignore patterns
/// * `allow_tests` - Whether to include test files
pub fn search_with_structured_patterns(
    root_path: &Path,
    plan: &QueryPlan,
    patterns: &[(String, HashSet<usize>)],
    custom_ignores: &[String],
    allow_tests: bool,
) -> Result<HashMap<PathBuf, HashMap<usize, HashSet<usize>>>> {
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";
    // Define a type alias for the complex nested type
    type FileMatchMap = HashMap<PathBuf, HashMap<usize, HashSet<usize>>>;
    let matches_by_file: Arc<Mutex<FileMatchMap>> = Arc::new(Mutex::new(HashMap::new()));

    // Extract required terms from AST
    fn extract_required_terms(expr: &Expr) -> Vec<String> {
        match expr {
            Expr::Term {
                keywords, required, ..
            } if *required => keywords.clone(),
            Expr::And(left, right) => {
                let mut terms = extract_required_terms(left);
                terms.extend(extract_required_terms(right));
                terms
            }
            Expr::Or(left, right) => {
                // For OR, a term is required if it's required in either branch
                let mut terms = extract_required_terms(left);
                terms.extend(extract_required_terms(right));
                terms
            }
            _ => vec![],
        }
    }

    // Get all required terms from the AST
    let required_term_list = extract_required_terms(&plan.ast);
    let required_term_set: HashSet<_> = required_term_list.iter().map(|s| s.as_str()).collect();

    // Debug output
    if debug_mode {
        println!("DEBUG: All patterns: {:?}", patterns);
        println!("DEBUG: Required terms: {:?}", required_term_list);
    }

    // Filter patterns for required terms
    let required_terms: Vec<_> = patterns
        .iter()
        .filter(|(_, _term_idx_set)| {
            _term_idx_set.iter().any(|&idx| {
                // Find the term corresponding to this index
                plan.term_indices
                    .iter()
                    .find(|(_, &term_idx)| term_idx == idx)
                    .map(|(term, _)| required_term_set.contains(term.as_str()))
                    .unwrap_or(false)
            })
        })
        .collect();

    // Special handling for excluded terms in OR queries
    // If we have a query like "(key OR word OR 1) -keyword3", we need to make sure
    // we find files that match the OR part but don't contain the excluded term
    let has_excluded_terms = !plan.excluded_terms.is_empty();
    let has_or_expr = match &plan.ast {
        Expr::Or(_, _) => true,
        Expr::And(left, right) => {
            matches!(**left, Expr::Or(_, _)) || matches!(**right, Expr::Or(_, _))
        }
        _ => false,
    };

    // Debug output
    if debug_mode {
        println!("DEBUG: Required patterns: {:?}", required_terms);
    }

    // If we have required terms, search them first
    if !required_terms.is_empty() || (has_excluded_terms && has_or_expr) {
        let required_files: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new(HashSet::new()));

        // Use all patterns if we have excluded terms with OR expressions
        if has_excluded_terms && has_or_expr {
            patterns.par_iter().for_each(|(pat, _term_idx_set)| {
                if debug_mode {
                    println!("DEBUG: Searching for pattern: {}", pat);
                }
                if let Ok(files) =
                    find_files_with_pattern(root_path, pat, custom_ignores, allow_tests)
                {
                    if debug_mode {
                        println!("DEBUG: Found {} files matching pattern", files.len());
                    }
                    for f in files {
                        if debug_mode {
                            println!("DEBUG: Checking file: {:?}", f);
                        }
                        if let Ok((matched, _)) = search_file_for_pattern(&f, pat, true) {
                            if debug_mode {
                                println!("DEBUG: File {:?} matched: {}", f, matched);
                            }
                            if matched {
                                required_files.lock().unwrap().insert(f);
                            }
                        }
                    }
                }
            });
        } else {
            // Otherwise just use required terms
            required_terms.par_iter().for_each(|(pat, _term_idx_set)| {
                if debug_mode {
                    println!("DEBUG: Searching for pattern: {}", pat);
                }
                if let Ok(files) =
                    find_files_with_pattern(root_path, pat, custom_ignores, allow_tests)
                {
                    if debug_mode {
                        println!("DEBUG: Found {} files matching pattern", files.len());
                    }
                    for f in files {
                        if debug_mode {
                            println!("DEBUG: Checking file: {:?}", f);
                        }
                        if let Ok((matched, _)) = search_file_for_pattern(&f, pat, true) {
                            if debug_mode {
                                println!("DEBUG: File {:?} matched: {}", f, matched);
                            }
                            if matched {
                                required_files.lock().unwrap().insert(f);
                            }
                        }
                    }
                }
            });
        }

        let required_files = required_files.lock().unwrap().clone();

        // Filter patterns for non-excluded terms
        let non_excluded_patterns: Vec<_> = patterns
            .iter()
            .filter(|(_, term_idx_set)| {
                term_idx_set.iter().all(|&idx| {
                    // Find the term corresponding to this index
                    plan.term_indices
                        .iter()
                        .find(|(_, &term_idx)| term_idx == idx)
                        .map(|(term, _)| !plan.excluded_terms.contains(term))
                        .unwrap_or(true)
                })
            })
            .collect();

        // Now search non-excluded patterns but only in files that matched required terms
        non_excluded_patterns
            .par_iter()
            .for_each(|(pat, term_idx_set)| {
                for f in &required_files {
                    if let Ok((matched, lines)) = search_file_for_pattern(f, pat, true) {
                        if matched {
                            let mut guard = matches_by_file.lock().unwrap();
                            let entry = guard.entry(f.clone()).or_default();
                            for &ti in term_idx_set {
                                entry.entry(ti).or_default().extend(lines.iter());
                            }
                        }
                    }
                }
            });

        // Filter out files containing excluded terms
        let files_to_exclude: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new(HashSet::new()));

        // Check each file in our results for excluded terms
        let all_files = matches_by_file
            .lock()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();

        all_files.par_iter().for_each(|file_path| {
            // Check if this file contains any excluded terms
            for excluded_term in &plan.excluded_terms {
                let mut should_exclude = false;
                // First, check if the file contains the exact excluded term
                // Create a pattern for the excluded term with word boundaries
                let base_pattern = crate::search::query::regex_escape(excluded_term);

                // Use word boundaries to ensure we match the exact term
                // This prevents matching parts of compound words
                let word_boundary_pattern = format!("\\b{}\\b", base_pattern);

                // Check if the file contains this exact excluded term
                if let Ok((matched, _)) = search_file_for_pattern(file_path, &word_boundary_pattern, true) {
                    if matched {
                        if debug_mode {
                            println!("DEBUG: File {:?} contains exact excluded term '{}', excluding from results",
                                    file_path, excluded_term);
                        }
                        should_exclude = true;
                    }
                }

                // Also check for the exact term in camelCase or PascalCase
                if !should_exclude {
                    // This handles cases like "NetworkFirewall" when the excluded term is "networkfirewall"
                    let camel_case_pattern = format!("(?i)\\b{}\\b", base_pattern);
                    if let Ok((matched, _)) = search_file_for_pattern(file_path, &camel_case_pattern, true) {
                        if matched {
                            if debug_mode {
                                println!("DEBUG: File {:?} contains excluded term '{}' (case-insensitive), excluding from results",
                                        file_path, excluded_term);
                            }
                            should_exclude = true;
                        }
                    }
                }

                // For compound words, also check if the file contains all components separately
                if !should_exclude {
                    // Split the excluded term into components
                    let components = tokenization::split_camel_case(excluded_term);

                    // Only proceed if it's actually a compound word (has multiple components)
                    if components.len() > 1 {
                        // Check if all components are present in the file
                        let mut all_components_present = true;

                        for component in &components {
                            // Skip very short components (less than 3 chars)
                            if component.len() < 3 {
                                continue;
                            }

                            let component_pattern = format!("\\b{}\\b", crate::search::query::regex_escape(component));

                            if let Ok((matched, _)) = search_file_for_pattern(file_path, &component_pattern, true) {
                                if !matched {
                                    // If any component is not present, the file doesn't contain all components
                                    all_components_present = false;
                                    break;
                                }
                            } else {
                                // If there's an error searching for the component, assume it's not present
                                all_components_present = false;
                                break;
                            }
                        }

                        if all_components_present {
                            if debug_mode {
                                println!("DEBUG: File {:?} contains all components of excluded compound word '{}', excluding from results",
                                        file_path, excluded_term);
                            }
                            should_exclude = true;
                        }
                    }
                }

                if should_exclude {
                    files_to_exclude.lock().unwrap().insert(file_path.clone());
                    break;
                }
            }
        });

        // Remove excluded files from the results
        let excluded_files = files_to_exclude.lock().unwrap();
        if !excluded_files.is_empty() {
            if debug_mode {
                println!(
                    "DEBUG: Removing {} files that match excluded patterns",
                    excluded_files.len()
                );
            }
            let mut guard = matches_by_file.lock().unwrap();
            for file in excluded_files.iter() {
                guard.remove(file);
            }
        }
    } else {
        // No required terms - search all patterns in all files
        if debug_mode {
            println!("DEBUG: No required terms, searching all patterns");
        }

        // First, filter out patterns for excluded terms
        let non_excluded_patterns: Vec<_> = patterns
            .iter()
            .filter(|(_, term_idx_set)| {
                term_idx_set.iter().all(|&idx| {
                    // Find the term corresponding to this index
                    plan.term_indices
                        .iter()
                        .find(|(_, &term_idx)| term_idx == idx)
                        .map(|(term, _)| !plan.excluded_terms.contains(term))
                        .unwrap_or(true)
                })
            })
            .collect();

        // Search for non-excluded patterns
        non_excluded_patterns
            .par_iter()
            .for_each(|(pat, term_idx_set)| {
                if debug_mode {
                    println!("DEBUG: Searching for pattern: {}", pat);
                }
                if let Ok(files) =
                    find_files_with_pattern(root_path, pat, custom_ignores, allow_tests)
                {
                    if debug_mode {
                        println!("DEBUG: Found {} files matching pattern", files.len());
                    }
                    for f in files {
                        if debug_mode {
                            println!("DEBUG: Checking file: {:?}", f);
                        }
                        if let Ok((matched, lines)) = search_file_for_pattern(&f, pat, true) {
                            if debug_mode {
                                println!("DEBUG: File {:?} matched: {}", f, matched);
                            }
                            if matched {
                                let mut guard = matches_by_file.lock().unwrap();
                                let entry = guard.entry(f.clone()).or_default();
                                for &ti in term_idx_set {
                                    entry.entry(ti).or_default().extend(lines.iter());
                                }
                            }
                        }
                    }
                }
            });

        // Filter out files containing excluded terms
        let files_to_exclude: Arc<Mutex<HashSet<PathBuf>>> = Arc::new(Mutex::new(HashSet::new()));

        // Check each file in our results for excluded terms
        let all_files = matches_by_file
            .lock()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();

        all_files.par_iter().for_each(|file_path| {
            // Check if this file contains any excluded terms
            for excluded_term in &plan.excluded_terms {
                let mut should_exclude = false;

                // First, check if the file contains the exact excluded term
                // Create a pattern for the excluded term with word boundaries
                let base_pattern = crate::search::query::regex_escape(excluded_term);

                // Use word boundaries to ensure we match the exact term
                // This prevents matching parts of compound words
                let word_boundary_pattern = format!("\\b{}\\b", base_pattern);

                // Check if the file contains this exact excluded term
                if let Ok((matched, _)) = search_file_for_pattern(file_path, &word_boundary_pattern, true) {
                    if matched {
                        if debug_mode {
                            println!("DEBUG: File {:?} contains exact excluded term '{}', excluding from results",
                                    file_path, excluded_term);
                        }
                        should_exclude = true;
                    }
                }

                // Also check for the exact term in camelCase or PascalCase
                if !should_exclude {
                    // This handles cases like "NetworkFirewall" when the excluded term is "networkfirewall"
                    let camel_case_pattern = format!("(?i)\\b{}\\b", base_pattern);
                    if let Ok((matched, _)) = search_file_for_pattern(file_path, &camel_case_pattern, true) {
                        if matched {
                            if debug_mode {
                                println!("DEBUG: File {:?} contains excluded term '{}' (case-insensitive), excluding from results",
                                        file_path, excluded_term);
                            }
                            should_exclude = true;
                        }
                    }
                }

                // For compound words, also check if the file contains all components separately
                if !should_exclude {
                    // Split the excluded term into components
                    let components = tokenization::split_camel_case(excluded_term);

                    // Only proceed if it's actually a compound word (has multiple components)
                    if components.len() > 1 {
                        // Check if all components are present in the file
                        let mut all_components_present = true;

                        for component in &components {
                            // Skip very short components (less than 3 chars)
                            if component.len() < 3 {
                                continue;
                            }

                            let component_pattern = format!("\\b{}\\b", crate::search::query::regex_escape(component));

                            if let Ok((matched, _)) = search_file_for_pattern(file_path, &component_pattern, true) {
                                if !matched {
                                    // If any component is not present, the file doesn't contain all components
                                    all_components_present = false;
                                    break;
                                }
                            } else {
                                // If there's an error searching for the component, assume it's not present
                                all_components_present = false;
                                break;
                            }
                        }

                        if all_components_present {
                            if debug_mode {
                                println!("DEBUG: File {:?} contains all components of excluded compound word '{}', excluding from results",
                                        file_path, excluded_term);
                            }
                            should_exclude = true;
                        }
                    }
                }

                if should_exclude {
                    files_to_exclude.lock().unwrap().insert(file_path.clone());
                    break;
                }
            }
        });

        // Remove excluded files from the results
        let excluded_files = files_to_exclude.lock().unwrap();
        if !excluded_files.is_empty() {
            if debug_mode {
                println!(
                    "DEBUG: Removing {} files that match excluded patterns",
                    excluded_files.len()
                );
            }
            let mut guard = matches_by_file.lock().unwrap();
            for file in excluded_files.iter() {
                guard.remove(file);
            }
        }
    }

    let final_map = Arc::try_unwrap(matches_by_file)
        .unwrap()
        .into_inner()
        .unwrap();

    // Optionally, we could filter each file with the AST:
    // But we rely on block-level filtering or a final pass. For demonstration, we skip it here.

    Ok(final_map)
}
