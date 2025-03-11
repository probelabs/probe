use crate::models::SearchResult;
use crate::ranking;
// No need for term_exceptions import

/// Function to rank search results based on query relevance
pub fn rank_search_results(results: &mut [SearchResult], queries: &[String], reranker: &str) {
    // Check if debug mode is enabled
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    // Combine all queries into a single string for ranking
    let combined_query = queries.join(" ");

    // Extract document texts for ranking, including filename in each document
    // This ensures filename terms are considered in the ranking algorithms
    let documents: Vec<String> = results
        .iter()
        .map(|r| format!("// Filename: {}\n{}", r.file, r.code))
        .collect();
    let documents_refs: Vec<&str> = documents.iter().map(|s| s.as_str()).collect();

    // Rank the documents
    // Get metrics from the first result (assuming they're the same for all results in this set)
    let file_unique_terms = results.first().and_then(|r| r.file_unique_terms);
    let file_total_matches = results.first().and_then(|r| r.file_total_matches);
    let block_unique_terms = results.first().and_then(|r| r.block_unique_terms);
    let block_total_matches = results.first().and_then(|r| r.block_total_matches);
    let node_type = results.first().map(|r| r.node_type.as_str());

    let ranking_params = ranking::RankingParams {
        documents: &documents_refs,
        query: &combined_query,
        file_unique_terms,
        file_total_matches,
        file_match_rank: results.first().and_then(|r| r.file_match_rank),
        block_unique_terms,
        block_total_matches,
        node_type,
    };

    let ranked_indices = ranking::rank_documents(&ranking_params);

    // Store original document indices and their various scores for later use
    let mut doc_scores: Vec<(usize, f64, f64, f64, f64)> = Vec::new();

    // Update the search results with rank and score information
    for (rank_index, (original_index, combined_score, tfidf_score, bm25_score, new_score)) in
        ranked_indices.iter().enumerate()
    {
        doc_scores.push((
            *original_index,
            *combined_score,
            *tfidf_score,
            *bm25_score,
            *new_score,
        ));

        if let Some(result) = results.get_mut(*original_index) {
            result.rank = Some(rank_index + 1); // 1-based rank
            result.score = Some(*combined_score); // Keep original combined score
            result.tfidf_score = Some(*tfidf_score);
            result.bm25_score = Some(*bm25_score);
            result.new_score = Some(*new_score); // Store new score separately
        }
    }

    // Create separate rankings for TF-IDF and BM25 scores
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

    // Create hybrid2 ranking separately
    let mut hybrid2_ranking: Vec<(usize, f64)> = results
        .iter()
        .enumerate()
        .filter_map(|(idx, r)| r.new_score.map(|score| (idx, score)))
        .collect();

    // Sort by scores in descending order
    tfidf_ranking.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    bm25_ranking.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    hybrid2_ranking.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Assign ranks for each metric
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

    // Assign hybrid2 ranks - this is crucial for our fix
    for (rank, (idx, _)) in hybrid2_ranking.iter().enumerate() {
        if let Some(result) = results.get_mut(*idx) {
            result.hybrid2_rank = Some(rank + 1); // 1-based rank
        }
    }

    // Sort the results based on the selected reranker
    match reranker {
        "tfidf" => {
            if debug_mode {
                println!("DEBUG: Using TF-IDF ranking (term frequency-inverse document frequency)");
            } else {
                println!("Using TF-IDF ranking (term frequency-inverse document frequency)");
            }

            // First collect TF-IDF scores into a vector with their indices
            let mut tfidf_scores: Vec<(usize, f64)> = Vec::new();
            for (idx, result) in results.iter().enumerate() {
                let tfidf = result.tfidf_score.unwrap_or(0.0);
                tfidf_scores.push((idx, tfidf));
            }

            // Sort by TF-IDF score in descending order
            tfidf_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            // Assign TF-IDF ranks and update result scores
            for (rank, (idx, tfidf_score)) in tfidf_scores.iter().enumerate() {
                if let Some(result) = results.get_mut(*idx) {
                    result.tfidf_rank = Some(rank + 1); // 1-based rank
                    result.score = Some(*tfidf_score); // Dereference to get the f64 value
                    result.rank = Some(rank + 1);
                }
            }

            // Sort by TF-IDF rank for display
            results.sort_by(|a, b| {
                let tfidf_rank_a = a.tfidf_rank.unwrap_or(usize::MAX);
                let tfidf_rank_b = b.tfidf_rank.unwrap_or(usize::MAX);
                // Sort in ascending order (1 is best)
                tfidf_rank_a.cmp(&tfidf_rank_b)
            });
        }
        "bm25" => {
            if debug_mode {
                println!("DEBUG: Using BM25 ranking (Okapi BM25 algorithm)");
            } else {
                println!("Using BM25 ranking (Okapi BM25 algorithm)");
            }

            // First collect BM25 scores into a vector with their indices
            let mut bm25_scores: Vec<(usize, f64)> = Vec::new();
            for (idx, result) in results.iter().enumerate() {
                let bm25 = result.bm25_score.unwrap_or(0.0);
                bm25_scores.push((idx, bm25));
            }

            // Sort by BM25 score in descending order
            bm25_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            // Assign BM25 ranks and update result scores
            for (rank, (idx, bm25_score)) in bm25_scores.iter().enumerate() {
                if let Some(result) = results.get_mut(*idx) {
                    result.bm25_rank = Some(rank + 1); // 1-based rank
                    result.score = Some(*bm25_score); // Dereference to get the f64 value
                    result.rank = Some(rank + 1);
                }
            }

            // Sort by BM25 rank for display
            results.sort_by(|a, b| {
                let bm25_rank_a = a.bm25_rank.unwrap_or(usize::MAX);
                let bm25_rank_b = b.bm25_rank.unwrap_or(usize::MAX);
                // Sort in ascending order (1 is best)
                bm25_rank_a.cmp(&bm25_rank_b)
            });
        }
        "hybrid2" => {
            if debug_mode {
                println!("DEBUG: Using hybrid2 ranking (comprehensive multi-metric score with emphasis on block-level matches)");
            } else {
                println!("Using hybrid2 ranking (comprehensive multi-metric score with emphasis on block-level matches)");
            }

            // First combine the results
            let mut hybrid2_scores: Vec<(usize, f64)> = results
                .iter()
                .enumerate()
                .filter_map(|(idx, r)| r.new_score.map(|score| (idx, score)))
                .collect();

            // Sort by score in descending order
            hybrid2_scores
                .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            // Assign ranks based on sorted scores
            for (rank, (idx, _)) in hybrid2_scores.iter().enumerate() {
                if let Some(result) = results.get_mut(*idx) {
                    result.hybrid2_rank = Some(rank + 1); // 1-based rank
                }
            }

            // Sort by hybrid2_rank for display
            results.sort_by(|a, b| {
                let hybrid2_rank_a = a.hybrid2_rank.unwrap_or(usize::MAX);
                let hybrid2_rank_b = b.hybrid2_rank.unwrap_or(usize::MAX);
                // Sort in ascending order (1 is best)
                hybrid2_rank_a.cmp(&hybrid2_rank_b)
            });

            // Update the main rank field to match the hybrid2 rank to ensure results are displayed in hybrid2 order
            for (i, result) in results.iter_mut().enumerate() {
                // Keep the combined_score_rank as is, but update the main rank to reflect display order
                result.rank = Some(i + 1);
            }
        }
        "hybrid" => {
            if debug_mode {
                println!("DEBUG: Using hybrid ranking (simple TF-IDF + BM25 combination)");
            }

            // Sort by combined score (original hybrid method)
            results.sort_by(|a, b| {
                let score_a = a.score.unwrap_or(0.0);
                let score_b = b.score.unwrap_or(0.0);
                // Sort in descending order
                score_b
                    .partial_cmp(&score_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            // Update only the main rank field
            for (i, result) in results.iter_mut().enumerate() {
                result.rank = Some(i + 1); // 1-based rank
            }
        }
        _ => {
            if debug_mode {
                println!(
                    "DEBUG: Using hybrid ranking (default - simple TF-IDF + BM25 combination)"
                );
            } else {
                println!("Using hybrid ranking (default - simple TF-IDF + BM25 combination)");
            }

            // Sort by combined score (default)
            results.sort_by(|a, b| {
                let score_a = a.score.unwrap_or(0.0);
                let score_b = b.score.unwrap_or(0.0);
                // Sort in descending order
                score_b
                    .partial_cmp(&score_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            // Update only the main rank field
            for (i, result) in results.iter_mut().enumerate() {
                result.rank = Some(i + 1); // 1-based rank
            }
        }
    }

    // Log ranking information in debug mode
    if debug_mode {
        println!("DEBUG: Ranked {} search results", results.len());
    }

    // Now that all scores have been calculated, filter out results with zero scores and matches
    // This ensures we only filter after scores have been properly calculated
    let mut filtered_results = Vec::new();

    // First, determine the total number of unique terms across all queries
    // We'll use the maximum file_unique_terms value as an approximation
    let mut total_unique_terms = results
        .iter()
        .filter_map(|r| r.file_unique_terms)
        .max()
        .unwrap_or(1);

    // If the query is a single term, ensure total_unique_terms is at least 1
    if queries.len() == 1 && total_unique_terms == 0 {
        total_unique_terms = 1;
    }

    if debug_mode {
        println!(
            "DEBUG: Total unique terms across all queries: {}",
            total_unique_terms
        );
    }

    for result in results.iter() {
        let has_matches = result.block_total_matches.unwrap_or(0) > 0;
        let has_tfidf = result.tfidf_score.unwrap_or(0.0) > 0.0;
        let has_bm25 = result.bm25_score.unwrap_or(0.0) > 0.0;

        // Get the number of unique terms in the block
        let mut block_unique_terms = result.block_unique_terms.unwrap_or(0);

        // Calculate the minimum required terms based on the total number of unique terms
        // Using the corrected formula:
        // 1 term: require 1 term
        // 2 terms: require 1 term
        // 3 terms: require 2 terms
        // 4 terms: require 2 terms
        // 5 terms: require 3 terms
        // 6 terms: require 3 terms
        // 7 terms: require 4 terms
        let min_required_terms = match total_unique_terms {
            0 => 0,
            1 | 2 => 1,
            3 | 4 => 2,
            5 | 6 => 3,
            7 | 8 => 4,
            9 | 10 => 5,
            11 | 12 => 6,
            n => (n + 1) / 2, // General formula: ceil(n/2)
        };

        // Special case for compound word queries like "networkfirewall"
        // If the query is a single term that can be split into multiple terms,
        // and the file has a good score, we should consider it as having enough unique terms
        if block_unique_terms == 0 && has_tfidf && has_bm25 {
            // Check if this is likely a compound word match
            if result.tfidf_score.unwrap_or(0.0) > 0.1 && result.bm25_score.unwrap_or(0.0) > 0.1 {
                // This is likely a compound word match, so set block_unique_terms to match the required terms
                block_unique_terms = min_required_terms.max(1);
            }
        }

        // Check if the block has enough unique terms
        let has_enough_unique_terms = block_unique_terms >= min_required_terms;

        if debug_mode {
            println!(
                "DEBUG: Post-ranking filtering - file: {}, matches: {}, tfidf: {}, bm25: {}, unique_terms: {}/{}, retained: {}",
                result.file,
                result.block_total_matches.unwrap_or(0),
                result.tfidf_score.unwrap_or(0.0),
                result.bm25_score.unwrap_or(0.0),
                block_unique_terms,
                min_required_terms,
                has_matches && has_tfidf && has_bm25 && has_enough_unique_terms
            );
        }

        if has_matches && has_tfidf && has_bm25 && has_enough_unique_terms {
            filtered_results.push(result.clone());
        }
    }

    // Replace original results with filtered results
    let filtered_len = filtered_results.len();
    if filtered_len < results.len() {
        // If we filtered out some results, copy only what we have
        for (i, result) in filtered_results.into_iter().enumerate() {
            results[i] = result;
        }

        // Instead of truncating, we'll keep the original file paths
        // but mark these results with a special flag
        for result in results.iter_mut().skip(filtered_len) {
            // Set a special flag to indicate this result should be skipped
            // but preserve the file path
            result.matched_by_filename = Some(false);
            result.score = Some(0.0);
            result.rank = Some(usize::MAX);
        }
    }
}
