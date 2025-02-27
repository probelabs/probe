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
    any_term: bool, // Default is false (all terms must match)
    exact: bool, // Parameter to control exact matching (no stemming/stopwords)
) -> Result<LimitedSearchResults> {
    // Check if debug mode is enabled
    let debug_mode = std::env::var("CODE_SEARCH_DEBUG").unwrap_or_default() == "1";

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
    let term_pairs = preprocess_query(query, exact); // Use exact mode if specified

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
    let term_patterns: Vec<String> = patterns_with_terms.iter().map(|(pattern, _)| pattern.clone()).collect();
    
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

    // 3. Find all files and their matched terms
    let mut file_matched_terms: HashMap<PathBuf, HashSet<usize>> = HashMap::new(); // Term indices matched per file
    let mut file_total_matches: HashMap<PathBuf, usize> = HashMap::new(); // Total matches
    let mut file_matched_lines: HashMap<PathBuf, HashSet<usize>> = HashMap::new();

    // First, find all files matching the first pattern
    let _require_all = !any_term; // If any_term is true, we don't require all patterns to match
    let initial_files = find_files_with_pattern(path, &term_patterns[0], custom_ignores, allow_tests)?;
    if debug_mode {
        println!(
            "Found {} files matching first term pattern",
            initial_files.len()
        );
    }

    // Initialize with empty sets
    for file in &initial_files {
        file_matched_terms.insert(file.clone(), HashSet::new());
        file_matched_lines.insert(file.clone(), HashSet::new());
    }

    // For each pattern, search all files and update matched terms
    for (i, (pattern, term_indices)) in patterns_with_terms.iter().enumerate() {
        let files_to_search = if i == 0 {
            // For first pattern, we already have the files
            initial_files.clone()
        } else {
            // For subsequent patterns, search only in files that matched at least one previous pattern
            file_matched_terms.keys().cloned().collect()
        };

        if debug_mode {
            println!(
                "Searching {} files for pattern {}: {}",
                files_to_search.len(),
                i + 1,
                pattern
            );
        }

        for file in &files_to_search {
            // Search for this pattern in the file
            match search_file_for_pattern(file, pattern, true) { // Use exact mode for frequency search
                Ok((matched, line_numbers)) => {
                    if matched {
                        // Add the term indices this pattern represents to the file's matched terms
                        let terms = file_matched_terms.entry(file.clone()).or_default();
                        terms.extend(term_indices);
                        
                        // Add the number of matches (line numbers) to the total matches count
                        *file_total_matches.entry(file.clone()).or_insert(0) += line_numbers.len();

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

    // 4. Create a combined structure for sorting
    let mut files_by_frequency: Vec<(PathBuf, usize, usize)> = file_matched_terms
        .iter()
        .map(|(path, term_set)| {
            let term_count = term_set.len();
            let total_matches = file_total_matches.get(path).cloned().unwrap_or(0);
            (path.clone(), term_count, total_matches)
        })
        .collect();

    // Sort by term count first, then by total matches if term counts are equal
    files_by_frequency.sort_by(|a, b| {
        b.1.cmp(&a.1).then_with(|| b.2.cmp(&a.2))
    });

    // Take the top files (all files that match all terms, or the top N files that match any term)
    let top_files = if !any_term {
        // For "all terms" mode, only include files that match all terms
        files_by_frequency
            .into_iter()
            .filter(|(path, _, _)| {
                let matched_terms = file_matched_terms.get(path).unwrap();
                matched_terms.len() == term_pairs.len()
            })
            .collect::<Vec<_>>()
    } else {
        // For "any term" mode, include all files that match any term
        files_by_frequency
    };

    if debug_mode {
        println!(
            "Found {} files matching the frequency search criteria",
            top_files.len()
        );
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
            let already_found_files: HashSet<PathBuf> = top_files.iter().map(|(p, _, _)| p.clone()).collect();
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
        // Instead, we'll rank by term count and total matches
        if !results.is_empty() {
            // Assign simple ranks based on term count and total matches
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

    // Process each file and collect results
    let mut results = Vec::new();
    for (file_path, _term_count, _total_matches) in &top_files {
        // Get the matched lines for this file
        let matched_lines = file_matched_lines.get(file_path).cloned().unwrap_or_default();
        
        // Get term-specific matches for this file
        let term_matches = file_matched_terms.get(file_path).map(|terms| {
            let mut map = HashMap::new();
            for &term_idx in terms {
                map.insert(term_idx, matched_lines.clone());
            }
            map
        });
        
        // Get the filename for matching
        let filename = file_path.file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        
        // Determine which terms match in the filename
        let filename_matched_terms = get_filename_matched_queries(&filename, &[term_pairs.clone()]);
        
        // Process the file with both content and filename matches
        let file_results = process_file_with_results(
            file_path, 
            &matched_lines, 
            allow_tests, 
            term_matches.as_ref(), 
            any_term, 
            term_pairs.len(),
            filename_matched_terms
        )?;
        
        results.extend(file_results);
    }

    // If filename matching is enabled, find files whose names match query words
    if include_filenames {
        let already_found_files: HashSet<PathBuf> = top_files.iter().map(|(p, _, _)| p.clone()).collect();
        let matching_files = find_matching_filenames(
            path,
            &[query.to_string()],
            &already_found_files,
            custom_ignores,
            allow_tests,
        )?;

        for file_path in matching_files {
            // For filename matches, we need to determine which terms match the filename
            let filename = file_path.file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_default();
            
            // We don't need to use the filename_matched_queries here since these are already
            // files that matched by filename, but we include it for consistency
            let _filename_matched_queries = get_filename_matched_queries(&filename, &[term_pairs.clone()]);
            
            match process_file_by_filename(&file_path) {
                Ok(mut result) => {
                    // Ensure the matched_by_filename flag is set
                    result.matched_by_filename = Some(true);
                    results.push(result);
                },
                Err(err) => eprintln!("Error processing file {:?}: {}", file_path, err),
            }
        }
    }

    // Rank the results if there are any
    if !results.is_empty() {
        rank_search_results(&mut results, &[query.to_string()], reranker);
    }

    // Apply default token limit of 100k if not specified
    let default_max_tokens = max_tokens.or(Some(100000));
    
    // Apply limits and return
    Ok(apply_limits(results, max_results, max_bytes, default_max_tokens))
}
