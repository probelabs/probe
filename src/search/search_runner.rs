use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rayon::prelude::*;

use crate::models::{LimitedSearchResults, SearchResult};
use crate::search::file_processing::{process_file_by_filename, process_file_with_results};
use crate::search::file_search::{find_files_with_pattern, find_matching_filenames, get_filename_matched_queries, get_filename_matched_queries_compat, search_file_for_pattern};
use crate::search::query::{create_term_patterns, preprocess_query};
use crate::search::result_ranking::rank_search_results;
use crate::search::search_limiter::apply_limits;

/// Struct to hold timing information for different stages of the search process
pub struct SearchTimings {
    pub query_preprocessing: Option<Duration>,
    pub file_searching: Option<Duration>,
    pub result_processing: Option<Duration>,
    pub result_ranking: Option<Duration>,
    pub limit_application: Option<Duration>,
    pub block_merging: Option<Duration>,
}

/// Performs a search on code repositories and returns results in a structured format
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
    merge_blocks: bool, // Parameter to control post-ranking block merging
    merge_threshold: Option<usize>, // Parameter to control how many lines between blocks to merge
) -> Result<LimitedSearchResults> {
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    // Initialize timing structure
    let mut timings = SearchTimings {
        query_preprocessing: None,
        file_searching: None,
        result_processing: None,
        result_ranking: None,
        limit_application: None,
        block_merging: None,
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
        println!("DEBUG:   Match mode: {}", if any_term { "any term" } else { "all terms" });
    }

    // If frequency-based search is enabled and we have exactly one query
    if frequency_search && queries.len() == 1 {
        if debug_mode {
            println!("DEBUG: Using frequency-based search for query: {}", queries[0]);
        }
        
        // Print ranking method being used
        match reranker {
            "tfidf" => println!("Using TF-IDF for ranking"),
            "bm25" => println!("Using BM25 for ranking"),
            "hybrid" => println!("Using hybrid ranking (default - simple TF-IDF + BM25 combination)"),
            "hybrid2" => println!("Using hybrid2 ranking (advanced - separate ranking components)"),
            _ => println!("Using {} for ranking", reranker),
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
            merge_blocks,
            merge_threshold,
        );
    }

    // Process each query string into multiple terms and store the term pairs for later use
    let queries_terms: Vec<Vec<(String, String)>> =
        queries.iter().map(|q| preprocess_query(q, exact)).collect();

    // Cache preprocessed query terms for reuse
    let preprocessed_queries: Vec<Vec<String>> = queries_terms
        .iter()
        .map(|terms| terms.iter().map(|(_, stemmed)| stemmed.clone()).collect())
        .collect();

    // We'll gather all patterns for multi-term search, plus a map from patterns to global term indices
    let mut all_patterns = Vec::new();
    let mut pattern_to_terms: Vec<HashSet<usize>> = Vec::new();

    // We also track the mapping of pattern index to query index (for older code usage)
    let mut pattern_to_query: Vec<usize> = Vec::new();

    // Calculate offsets for each query's terms
    let mut term_offset: Vec<usize> = queries_terms
        .iter()
        .scan(0, |state, terms| {
            let offset = *state;
            *state += terms.len();
            Some(offset)
        })
        .collect();
    let total_terms: usize = queries_terms.iter().map(|t| t.len()).sum();
    term_offset.push(total_terms);

    // Build patterns for each query
    for (query_idx, _query) in queries.iter().enumerate() {
        let term_pairs = &queries_terms[query_idx];
        let patterns_with_terms = create_term_patterns(term_pairs);

        // For each pattern, adjust local term indices to be global
        for (pattern, term_indices) in &patterns_with_terms {
            let query_offset = term_offset[query_idx];
            let mut global_indices = HashSet::new();
            for &ti in term_indices {
                global_indices.insert(query_offset + ti);
            }
            all_patterns.push(pattern.clone());
            pattern_to_terms.push(global_indices);
            pattern_to_query.push(query_idx);
        }
    }

    timings.query_preprocessing = Some(query_preprocessing_start.elapsed());

    println!(
        "Searching for {} queries with {} patterns in {:?} (mode: {})...",
        queries.len(),
        all_patterns.len(),
        path,
        if any_term { "any term matches" } else { "all terms must match" }
    );

    // Start timing file searching
    let file_searching_start = Instant::now();
    let matches_by_file_and_term: Arc<Mutex<HashMap<PathBuf, HashMap<usize, HashSet<usize>>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    // Prepare single-term patterns separately from multi-term combos
    let mut single_term_patterns: HashMap<usize, Vec<String>> = HashMap::new();
    let mut combo_patterns: Vec<(String, HashSet<usize>)> = Vec::new();

    for (pattern_idx, pattern) in all_patterns.iter().enumerate() {
        let term_indices = &pattern_to_terms[pattern_idx];
        if term_indices.len() == 1 {
            let sole = *term_indices.iter().next().unwrap();
            single_term_patterns.entry(sole).or_default().push(pattern.clone());
        } else {
            combo_patterns.push((pattern.clone(), term_indices.clone()));
        }
    }

    // Helper for searching a group of patterns
    let search_group = |patterns: &[String], indices: &HashSet<usize>| {
        let local_matches = Arc::new(Mutex::new(HashMap::new()));

        patterns.par_iter().for_each(|pat| {
            if let Ok(matching_files) =
                find_files_with_pattern(path, pat, custom_ignores, allow_tests)
            {
                for file_path in matching_files {
                    if let Ok((did_match, line_numbers)) = search_file_for_pattern(&file_path, pat, true)
                    {
                        if did_match {
                            let mut local = local_matches.lock().unwrap();
                            let file_entry: &mut HashMap<usize, HashSet<usize>> = local.entry(file_path.clone()).or_default();
                            for &ti in indices {
                                file_entry.entry(ti).or_default().extend(line_numbers.clone());
                            }
                        }
                    }
                }
            }
        });

        let merged = Arc::try_unwrap(local_matches).expect("Arc error").into_inner().expect("Mutex error");
        merged
    };

    // Search single-term patterns
    let single_term_results: Vec<_> = single_term_patterns
        .par_iter()
        .map(|(&term_idx, pats)| {
            let res = search_group(pats, &HashSet::from([term_idx]));
            (res, term_idx)
        })
        .collect();

    // Search combo patterns
    let combo_result_map = if !combo_patterns.is_empty() {
        let patterns_only: Vec<String> = combo_patterns.iter().map(|(p, _)| p.clone()).collect();
        let all_indices: HashSet<usize> = combo_patterns
            .iter()
            .flat_map(|(_, set)| set.iter().cloned())
            .collect();
        search_group(&patterns_only, &all_indices)
    } else {
        HashMap::new()
    };

    {
        // Merge single-term results into the main map
        let mut main_map = matches_by_file_and_term.lock().unwrap();
        for (res_map, _term_idx) in single_term_results {
            for (pathbuf, map_of_lines) in res_map {
                let file_map = main_map.entry(pathbuf).or_default();
                for (ti, lineset) in map_of_lines {
                    file_map.entry(ti).or_default().extend(lineset);
                }
            }
        }
        // Merge combo results
        for (pathbuf, map_of_lines) in combo_result_map {
            let file_map = main_map.entry(pathbuf).or_default();
            for (ti, lineset) in map_of_lines {
                file_map.entry(ti).or_default().extend(lineset);
            }
        }
    }

    let matches_by_file_and_term = Arc::try_unwrap(matches_by_file_and_term)
        .expect("Arc unwrap error")
        .into_inner()
        .expect("Mutex unwrap error");

    // Collect all files with content matches
    let content_files: HashSet<PathBuf> = matches_by_file_and_term.keys().cloned().collect();
    // Optionally find files by filename
    let filename_matching_files = if include_filenames {
        find_matching_filenames(path, queries, &content_files, custom_ignores, allow_tests)?
    } else {
        Vec::new()
    };
    let mut all_files = content_files.clone();
    all_files.extend(filename_matching_files.iter().cloned());

    timings.file_searching = Some(file_searching_start.elapsed());

    // Log or filter results
    if queries.len() > 1 {
        println!(
            "Term filtering: {} files matched at least one term, {} files matched the filter criteria ({}).",
            matches_by_file_and_term.len(),
            all_files.len(),
            if any_term { "any term" } else { "all terms must match" }
        );
    }

    // If no matches found and no filename-based matches, we can exit
    if all_files.is_empty() && !include_filenames {
        println!("No matches found.");
        return Ok(LimitedSearchResults {
            results: Vec::new(),
            skipped_files: Vec::new(),
            limits_applied: None,
        });
    }

    println!("Found matches in {} files.", all_files.len());

    // If files_only is set, return file-level results only
    if files_only {
        let mut results = Vec::new();
        for path in all_files.iter() {
            results.push(SearchResult {
                file: path.to_string_lossy().to_string(),
                lines: (1, 1), // Default line count since we don't have the actual count
                node_type: "file".to_string(),
                code: String::new(), // Empty content since we don't have the actual content
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
        if include_filenames {
            let found_files: HashSet<PathBuf> = results.iter().map(|r| PathBuf::from(&r.file)).collect();
            let additional_files = find_matching_filenames(path, queries, &found_files, custom_ignores, allow_tests)?;
            for af in additional_files {
                results.push(SearchResult {
                    file: af.to_string_lossy().to_string(),
                    lines: (1, 1), // Default line count
                    node_type: "file".to_string(),
                    code: String::new(), // Empty content
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
        }

        // Since we have no content, do a simple ranking by filename
        if !results.is_empty() {
            for (i, r) in results.iter_mut().enumerate() {
                r.rank = Some(i + 1);
                let sc = 1.0 / (i as f64 + 1.0);
                r.score = Some(sc);
                r.tfidf_score = Some(sc);
                r.bm25_score = Some(sc);
                r.tfidf_rank = Some(i + 1);
                r.bm25_rank = Some(i + 1);
            }
        }
        return Ok(apply_limits(results, max_results, max_bytes, max_tokens));
    }

    // Otherwise, process each file, retrieving relevant line info
    let result_processing_start = Instant::now();
    let mut results = Vec::new();

    // We'll need stats: unique terms, total matches, rank. We can rank by total matches for a simpler approach
    let mut file_stats = HashMap::new();
    let mut file_total_matches = Vec::new();

    // Build a map from file -> all matched term line sets
    let mut final_matches_by_file = HashMap::new();
    for path in all_files {
        let content_map = matches_by_file_and_term.get(&path).cloned().unwrap_or_default();
        let content_terms: HashSet<usize> = content_map.keys().cloned().collect();

        // Get the filename matches
        let filename = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
        let filename_terms = get_filename_matched_queries_compat(&filename, &queries_terms);

        // If any_term is true, any matched term is enough
        // If all_term is true, we check if all query sets have at least one matched term
        let should_include = if any_term {
            !(content_terms.is_empty() && filename_terms.is_empty())
        } else {
            queries.iter().enumerate().all(|(q_idx, _)| {
                let start = term_offset[q_idx];
                let end = term_offset[q_idx + 1];
                let subset = (start..end).collect::<HashSet<_>>();
                subset.intersection(&content_terms).count() > 0
                    || subset.intersection(&filename_terms).count() > 0
            })
        };
        if !should_include {
            continue;
        }

        let combined_set: HashSet<usize> = content_terms.union(&filename_terms).cloned().collect();
        // Compute total matches
        let line_count = content_map.values().fold(0, |acc, lineset| acc + lineset.len());
        let total_matches_count = line_count + filename_terms.len();

        file_stats.insert(path.clone(), (combined_set.len(), total_matches_count));
        file_total_matches.push((path.clone(), total_matches_count));
        final_matches_by_file.insert(path.clone(), (content_map, filename_terms));
    }

    // Sort by total matches desc
    file_total_matches.sort_by(|a, b| b.1.cmp(&a.1));
    // Assign ranks
    let mut rank_map = HashMap::new();
    for (i, (path, _matches)) in file_total_matches.into_iter().enumerate() {
        rank_map.insert(path, i + 1);
    }

    // Keep track of which files we've processed to avoid duplicates
    let mut processed_files = HashSet::new();
    
    // Process line-based results
    for (path, (content_map, filename_terms)) in final_matches_by_file.clone() {
        processed_files.insert(path.clone());
        let (unique_terms, total_matches) = file_stats.get(&path).cloned().unwrap_or((0, 0));
        let file_match_rank = rank_map.get(&path).cloned().unwrap_or(0);

        // If no line-based matches, treat as full file if matched by filename
        if content_map.is_empty() && !filename_terms.is_empty() {
            match process_file_by_filename(&path, &queries_terms, Some(&preprocessed_queries)) {
                Ok(mut sf) => {
                    sf.matched_by_filename = Some(true);
                    sf.file_unique_terms = Some(unique_terms);
                    sf.file_total_matches = Some(total_matches);
                    sf.file_match_rank = Some(file_match_rank);
                    results.push(sf);
                }
                Err(e) => {
                    eprintln!("Error processing file by filename: {:?}", e);
                }
            }
        } else {
            // We have line-based matches
            // Gather all matched lines into one set
            let mut all_lines = HashSet::new();
            for lineset in content_map.values() {
                all_lines.extend(lineset);
            }
            // Call process_file_with_results
            let rres = process_file_with_results(
                &path,
                &all_lines,
                allow_tests,
                Some(&content_map),
                any_term,
                total_terms,
                filename_terms.clone(),
                &queries_terms,
                Some(&preprocessed_queries),
            );
            if let Ok(mut file_results) = rres {
                // Attach file-level stats
                for fr in &mut file_results {
                    fr.file_unique_terms = Some(unique_terms);
                    fr.file_total_matches = Some(total_matches);
                    fr.file_match_rank = Some(file_match_rank);
                }
                results.extend(file_results);
            }
        }
    }

    // Also handle pure filename matches if we haven't processed them above
    for file_path in filename_matching_files {
        if !processed_files.contains(&file_path) {
            // Strictly matched by filename, no content matches
            match process_file_by_filename(&file_path, &queries_terms, Some(&preprocessed_queries)) {
                Ok(mut sr) => {
                    // If we had a rank or stats from earlier
                    let _file_path_str = file_path.to_string_lossy().to_string();
                    if let Some((uniq, total)) = file_stats.get(&file_path) {
                        sr.file_unique_terms = Some(*uniq);
                        sr.file_total_matches = Some(*total);
                        sr.file_match_rank = Some(*rank_map.get(&file_path).unwrap_or(&0));
                    }
                    sr.matched_by_filename = Some(true);
                    results.push(sr);
                }
                Err(err) => eprintln!("Error processing file by filename: {}", err),
            }
        }
    }

    timings.result_processing = Some(result_processing_start.elapsed());

    // Rank the results
    let result_ranking_start = Instant::now();
    
    // Print ranking method being used
    match reranker {
        "tfidf" => println!("Using TF-IDF for ranking"),
        "bm25" => println!("Using BM25 for ranking"),
        "hybrid" => println!("Using hybrid ranking (default - simple TF-IDF + BM25 combination)"),
        "hybrid2" => println!("Using hybrid2 ranking (advanced - separate ranking components)"),
        _ => println!("Using {} for ranking", reranker),
    }
    
    if !results.is_empty() {
        // For hybrid2, we want to ensure we have two separate ranks
        if reranker == "hybrid2" {
            // First calculate regular combined score ranks
            rank_search_results(&mut results, queries, "combined");
            
            // Keep a copy of the combined score ranks
            for result in &mut results {
                if let Some(rank) = result.rank {
                    result.combined_score_rank = Some(rank);
                }
            }
            
            // Then apply hybrid2 rankings
            rank_search_results(&mut results, queries, reranker);
        } else {
            // For other rerankers, just do the normal ranking
            rank_search_results(&mut results, queries, reranker);
            
            // Set combined_score_rank to be the same as rank for consistency
            for result in &mut results {
                if let Some(rank) = result.rank {
                    result.combined_score_rank = Some(rank);
                }
            }
        }
    }
    timings.result_ranking = Some(result_ranking_start.elapsed());

    // Default token limit if none specified
    let default_max_tokens = max_tokens.or(Some(100000));
    let limit_application_start = Instant::now();
    let limited_results = apply_limits(results, max_results, max_bytes, default_max_tokens);
    timings.limit_application = Some(limit_application_start.elapsed());

    // Apply post-ranking block merging
    let block_merging_start = Instant::now();
    let merged_results = if !limited_results.results.is_empty() && merge_blocks {
        use crate::search::block_merging::merge_ranked_blocks;
        let original_count = limited_results.results.len();
        let merged = merge_ranked_blocks(limited_results.results, merge_threshold);
        
        if debug_mode {
            println!("Post-ranking block merging: {} blocks merged into {} blocks", 
                     original_count, merged.len());
        }
        
        LimitedSearchResults {
            results: merged,
            skipped_files: limited_results.skipped_files,
            limits_applied: limited_results.limits_applied,
        }
    } else {
        limited_results
    };
    timings.block_merging = Some(block_merging_start.elapsed());

    // Debug timing info
    if debug_mode {
        println!("Search Timings:");
        if let Some(d) = timings.query_preprocessing { println!("  Query Preprocessing: {:?}", d); }
        if let Some(d) = timings.file_searching { println!("  File Searching: {:?}", d); }
        if let Some(d) = timings.result_processing { println!("  Result Processing: {:?}", d); }
        if let Some(d) = timings.result_ranking { println!("  Result Ranking: {:?}", d); }
        if let Some(d) = timings.limit_application { println!("  Limit Application: {:?}", d); }
        if let Some(d) = timings.block_merging { println!("  Post-Ranking Block Merging: {:?}", d); }
    }

    Ok(merged_results)
}

/// Performs a frequency-based search for a single query
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
    merge_blocks: bool, // Parameter to control post-ranking block merging
    merge_threshold: Option<usize>, // Parameter to control how many lines between blocks to merge
) -> Result<LimitedSearchResults> {
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    let mut timings = SearchTimings {
        query_preprocessing: None,
        file_searching: None,
        result_processing: None,
        result_ranking: None,
        limit_application: None,
        block_merging: None,
    };

    let qp_start = Instant::now();
    if debug_mode {
        println!("Performing frequency-based search for query: {}", query);
    }

    let term_pairs = preprocess_query(query, exact);
    if term_pairs.is_empty() {
        println!("No valid search terms after preprocessing");
        return Ok(LimitedSearchResults {
            results: Vec::new(),
            skipped_files: Vec::new(),
            limits_applied: None,
        });
    }

    let patterns_with_terms = create_term_patterns(&term_pairs);

    timings.query_preprocessing = Some(qp_start.elapsed());

    println!("Frequency search enabled");
    if debug_mode {
        println!("Original query: {}", query);
        for (i, (orig, stem)) in term_pairs.iter().enumerate() {
            println!("  Term {}: {} (stemmed to {})", i + 1, orig, stem);
        }
        println!("Search patterns: {:?}", patterns_with_terms);
    }

    let fs_start = Instant::now();
    let file_matched_terms: Arc<Mutex<HashMap<PathBuf, HashSet<usize>>>> = Arc::new(Mutex::new(HashMap::new()));
    let file_matched_lines: Arc<Mutex<HashMap<PathBuf, HashSet<usize>>>> = Arc::new(Mutex::new(HashMap::new()));

    let mut all_files = HashSet::new();
    for (pattern, _) in &patterns_with_terms {
        let found = find_files_with_pattern(path, pattern, custom_ignores, allow_tests)?;
        all_files.extend(found);
    }

    for f in &all_files {
        file_matched_terms.lock().unwrap().insert(f.clone(), HashSet::new());
        file_matched_lines.lock().unwrap().insert(f.clone(), HashSet::new());
    }

    // Track filename matches
    let mut file_filename_matches = HashMap::new();
    {
        let indexed_terms: Vec<(String, usize)> = term_pairs
            .iter()
            .enumerate()
            .map(|(i, (orig, _))| (orig.clone(), i))
            .collect();

        for fp in &all_files {
            let matched = get_filename_matched_queries(fp, path, &[indexed_terms.clone()]);
            if !matched.is_empty() {
                file_filename_matches.insert(fp.clone(), matched);
            }
        }
    }

    patterns_with_terms.par_iter().for_each(|(pat, tset)| {
        if let Ok(matching_files) = find_files_with_pattern(path, pat, custom_ignores, allow_tests) {
            for mfile in matching_files {
                if let Ok((matched, lines)) = search_file_for_pattern(&mfile, pat, true) {
                    if matched {
                        // Use a mutex lock to safely modify the shared data structures
                        let mut file_matched_terms_lock = file_matched_terms.lock().unwrap();
                        if let Some(fmt) = file_matched_terms_lock.get_mut(&mfile) {
                            fmt.extend(tset.clone());
                        }
                        drop(file_matched_terms_lock); // Release the lock before acquiring another

                        let mut file_matched_lines_lock = file_matched_lines.lock().unwrap();
                        if let Some(fml) = file_matched_lines_lock.get_mut(&mfile) {
                            fml.extend(lines);
                        }
                    }
                }
            }
        }
    });

    timings.file_searching = Some(fs_start.elapsed());

    let mut freq_files: Vec<(PathBuf, usize, usize)> = file_matched_terms
        .lock()
        .unwrap()
        .iter()
        .map(|(path, tset)| {
            let fmatches = file_filename_matches.get(path).cloned().unwrap_or_default();
            let unioned: HashSet<usize> = tset.union(&fmatches).cloned().collect();
            let line_count = file_matched_lines.lock().unwrap().get(path).map(|ls| ls.len()).unwrap_or(0);
            (path.clone(), unioned.len(), line_count + fmatches.len())
        })
        .collect();

    // If any_term is false, only include files that matched all terms
    // There's only one query, but multiple terms if the user typed e.g. "ip address"
    // For frequency search, usually it's "any term" anyway, but let's keep consistent
    if !any_term {
        freq_files.retain(|(p, _unique_count, _)| {
            let file_terms = file_matched_terms.lock().unwrap().get(p).cloned().unwrap_or_default();
            let file_name_terms = file_filename_matches.get(p).cloned().unwrap_or_default();
            let unioned: HashSet<usize> = file_terms.union(&file_name_terms).cloned().collect();
            // must match all terms in `term_pairs`
            (0..term_pairs.len()).all(|i| unioned.contains(&i))
        });
    } else {
        // only keep files that matched something
        freq_files.retain(|(p, c, _)| {
            *c > 0 || file_filename_matches.contains_key(p)
        });
    }

    if freq_files.is_empty() && !include_filenames {
        println!("No matches found.");
        return Ok(LimitedSearchResults {
            results: Vec::new(),
            skipped_files: Vec::new(),
            limits_applied: None,
        });
    }

    // If files_only, return them directly
    if files_only {
        let mut out_results = Vec::new();
        for (path, _, _) in &freq_files {
            out_results.push(SearchResult {
                file: path.to_string_lossy().to_string(),
                lines: (0, 0), // We'll compute this later when we process the file
                node_type: "file".to_string(),
                code: "".to_string(), // Will be populated during file processing
                rank: None,
                score: None,
                matched_by_filename: Some(true),
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

        if include_filenames {
            let existing: HashSet<PathBuf> = freq_files.iter().map(|(p, _, _)| p.clone()).collect();
            let found_filenames =
                find_matching_filenames(path, &[query.to_string()], &existing, custom_ignores, allow_tests)?;
            for ff in found_filenames {
                out_results.push(SearchResult {
                    file: ff.to_string_lossy().to_string(),
                    lines: (0, 0), // We'll compute this later when we process the file
                    node_type: "file".to_string(),
                    code: "".to_string(), // Will be populated during file processing
                    rank: None,
                    score: None,
                    matched_by_filename: Some(true),
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
        }

        // Simple ranking
        for (i, r) in out_results.iter_mut().enumerate() {
            r.rank = Some(i + 1);
            let sc = 1.0 / (i as f64 + 1.0);
            r.score = Some(sc);
            r.tfidf_score = Some(sc);
            r.bm25_score = Some(sc);
            r.tfidf_rank = Some(i + 1);
            r.bm25_rank = Some(i + 1);
        }

        return Ok(apply_limits(out_results, max_results, max_bytes, max_tokens));
    }

    // else, gather final results with line-based context
    let rp_start = Instant::now();
    let mut results = Vec::new();
    let mut stats_map = HashMap::new();
    let mut rank_vec = Vec::new();

    for (path, unique_count, total_matches) in &freq_files {
        rank_vec.push((path.clone(), *total_matches));
        stats_map.insert(path.clone(), (*unique_count, *total_matches));
    }

    rank_vec.sort_by(|a, b| b.1.cmp(&a.1));
    // Assign ranks
    let mut rank_map = HashMap::new();
    for (i, (p, _m)) in rank_vec.into_iter().enumerate() {
        rank_map.insert(p, i + 1);
    }

    for (path, _uc, _tm) in freq_files {
        let (unique_terms, total_matches) = stats_map.get(&path).cloned().unwrap_or((0, 0));
        let fmrank = rank_map.get(&path).cloned().unwrap_or(0);

        let content_lines = file_matched_lines.lock().unwrap().get(&path).cloned().unwrap_or_default();
        if content_lines.is_empty() {
            // purely filename matched
            match process_file_by_filename(&path, &[term_pairs.clone()], Some(&[term_pairs.iter().map(|(_, s)| s.clone()).collect()])) {
                Ok(mut sr) => {
                    sr.matched_by_filename = Some(true);
                    sr.file_unique_terms = Some(unique_terms);
                    sr.file_total_matches = Some(total_matches);
                    sr.file_match_rank = Some(fmrank);
                    results.push(sr);
                }
                Err(e) => {
                    eprintln!("Error reading file for freq search: {}", e);
                }
            }
        } else {
            // line-based content
            let mut tmp_map = HashMap::new();
            let file_terms = file_matched_terms.lock().unwrap().get(&path).cloned().unwrap_or_default();
            for ti in file_terms {
                tmp_map.insert(ti, content_lines.clone());
            }
            // filename terms
            let filename = path.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
            let fmatch = get_filename_matched_queries_compat(&filename, &[term_pairs.clone()]);

            let res = process_file_with_results(
                &path,
                &content_lines,
                allow_tests,
                Some(&tmp_map),
                any_term,
                term_pairs.len(),
                fmatch.clone(),
                &[term_pairs.clone()],
                Some(&[term_pairs.iter().map(|(_, s)| s.clone()).collect()]),
            );
            if let Ok(mut file_results) = res {
                // Attach file-level stats
                for fr in &mut file_results {
                    fr.file_unique_terms = Some(unique_terms);
                    fr.file_total_matches = Some(total_matches);
                    fr.file_match_rank = Some(fmrank);
                }
                results.extend(file_results);
            }
        }
    }

    if include_filenames {
        let mut found_set: HashSet<PathBuf> = HashSet::new();
        found_set.extend(results.iter().map(|r| PathBuf::from(&r.file)));
        let extra = find_matching_filenames(path, &[query.to_string()], &found_set, custom_ignores, allow_tests)?;
        for xf in extra {
            match process_file_by_filename(&xf, &[term_pairs.clone()], Some(&[term_pairs.iter().map(|(_, s)| s.clone()).collect()])) {
                Ok(mut sr) => {
                    if let Some((u, t)) = stats_map.get(&xf) {
                        sr.file_unique_terms = Some(*u);
                        sr.file_total_matches = Some(*t);
                        sr.file_match_rank = Some(*rank_map.get(&xf).unwrap_or(&0));
                    }
                    sr.matched_by_filename = Some(true);
                    results.push(sr);
                }
                Err(err) => eprintln!("Error reading extra freq file: {}", err),
            }
        }
    }

    timings.result_processing = Some(rp_start.elapsed());

    let rr_start = Instant::now();
    if !results.is_empty() {
        // For hybrid2, we want to ensure we have two separate ranks
        if reranker == "hybrid2" {
            // First calculate regular combined score ranks
            rank_search_results(&mut results, &[query.to_string()], "combined");
            
            // Keep a copy of the combined score ranks
            for result in &mut results {
                if let Some(rank) = result.rank {
                    result.combined_score_rank = Some(rank);
                }
            }
            
            // Then apply hybrid2 rankings
            rank_search_results(&mut results, &[query.to_string()], reranker);
        } else {
            // For other rerankers, just do the normal ranking
            rank_search_results(&mut results, &[query.to_string()], reranker);
            
            // Set combined_score_rank to be the same as rank for consistency
            for result in &mut results {
                if let Some(rank) = result.rank {
                    result.combined_score_rank = Some(rank);
                }
            }
        }
    }
    timings.result_ranking = Some(rr_start.elapsed());

    // apply limits
    let lam_start = Instant::now();
    let default_max_tokens = max_tokens.or(Some(100000));
    let limited = apply_limits(results, max_results, max_bytes, default_max_tokens);
    timings.limit_application = Some(lam_start.elapsed());

    // Apply post-ranking block merging
    let block_merging_start = Instant::now();
    let merged_results = if !limited.results.is_empty() && merge_blocks {
        use crate::search::block_merging::merge_ranked_blocks;
        let original_count = limited.results.len();
        let merged = merge_ranked_blocks(limited.results, merge_threshold);
        
        if debug_mode {
            println!("Post-ranking block merging: {} blocks merged into {} blocks", 
                     original_count, merged.len());
        }
        
        LimitedSearchResults {
            results: merged,
            skipped_files: limited.skipped_files,
            limits_applied: limited.limits_applied,
        }
    } else {
        limited
    };
    timings.block_merging = Some(block_merging_start.elapsed());

    if debug_mode {
        println!("Search Timings:");
        if let Some(d) = timings.query_preprocessing { println!("  Query Preprocessing: {:?}", d); }
        if let Some(d) = timings.file_searching { println!("  File Searching: {:?}", d); }
        if let Some(d) = timings.result_processing { println!("  Result Processing: {:?}", d); }
        if let Some(d) = timings.result_ranking { println!("  Result Ranking: {:?}", d); }
        if let Some(d) = timings.limit_application { println!("  Limit Application: {:?}", d); }
        if let Some(d) = timings.block_merging { println!("  Post-Ranking Block Merging: {:?}", d); }
    }

    Ok(merged_results)
}
