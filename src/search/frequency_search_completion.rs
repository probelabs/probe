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

    // Compute file-level statistics
    let mut file_stats: HashMap<String, (usize, usize, usize)> = HashMap::new();
    let mut file_total_matches: Vec<(String, usize)> = Vec::new();

    // Collect all files with their match counts
    for (file_path, unique_terms, _) in &top_files {
        // Get content matches (line numbers) for this file
        let content_lines = file_matched_lines.get(file_path)
            .map(|lines| lines.len())
            .unwrap_or(0);
        
        // Get filename matches for this file
        let filename = file_path.file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        let filename_terms = get_filename_matched_queries_compat(&filename, &[vec![(query.to_string(), 0)]]);
        
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
        let content_terms = file_matched_terms.get(&file_path)
            .cloned()
            .unwrap_or_default();
        
        // Get filename matches for this file
        let filename = file_path.file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        let filename_terms = get_filename_matched_queries_compat(&filename, &[vec![(query.to_string(), 0)]]);
        
        // Calculate unique terms (union of content and filename matches)
        let unique_terms = content_terms.union(&filename_terms).count();
        
        // Store statistics: (unique_terms, total_matches, rank)
        file_stats.insert(file.clone(), (unique_terms, *total_matches, rank + 1));
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
                file_unique_terms: file_stats.get(&file_path.to_string_lossy().to_string()).map(|s| s.0),
                file_total_matches: file_stats.get(&file_path.to_string_lossy().to_string()).map(|s| s.1),
                file_match_rank: file_stats.get(&file_path.to_string_lossy().to_string()).map(|s| s.2),
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
                    file_unique_terms: file_stats.get(&file_path.to_string_lossy().to_string()).map(|s| s.0),
                    file_total_matches: file_stats.get(&file_path.to_string_lossy().to_string()).map(|s| s.1),
                    file_match_rank: file_stats.get(&file_path.to_string_lossy().to_string()).map(|s| s.2),
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
            match process_file_by_filename(&file_path) {
                Ok(mut result) => {
                    // Ensure the matched_by_filename flag is set
                    result.matched_by_filename = Some(true);
                    
                    // Assign file-level statistics
                    let file_path_str = file_path.to_string_lossy().to_string();
                    if let Some(&(unique_terms, total_matches, rank)) = file_stats.get(&file_path_str) {
                        result.file_unique_terms = Some(unique_terms);
                        result.file_total_matches = Some(total_matches);
                        result.file_match_rank = Some(rank);
                    } else {
                        // If this file wasn't in our original statistics, compute them now
                        let filename = file_path.file_name()
                            .map(|f| f.to_string_lossy().to_string())
                            .unwrap_or_default();
                        let filename_terms = get_filename_matched_queries_compat(&filename, &[vec![(query.to_string(), 0)]]);
                        
                        // For filename-only matches, we set unique terms to the number of matched terms in the filename
                        // and total matches to the same value
                        let unique_terms = filename_terms.len();
                        let total_matches = unique_terms;
                        
                        // For rank, we assign a value after the last ranked file
                        let rank = file_stats.len() + 1;
                        
                        result.file_unique_terms = Some(unique_terms);
                        result.file_total_matches = Some(total_matches);
                        result.file_match_rank = Some(rank);
                        
                        // Add to file_stats for future reference
                        file_stats.insert(file_path_str, (unique_terms, total_matches, rank));
                    }
                    
                    results.push(result);
                },
                Err(err) => eprintln!("Error processing file {:?}: {}", file_path, err),
            }
        }
    }

    // Assign file-level statistics
    for result in results.iter_mut() {
        let file_path_str = result.file.clone();
        if let Some(&(unique_terms, total_matches, rank)) = file_stats.get(&file_path_str) {
            result.file_unique_terms = Some(unique_terms);
            result.file_total_matches = Some(total_matches);
            result.file_match_rank = Some(rank);
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
