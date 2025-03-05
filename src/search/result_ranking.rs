use crate::models::SearchResult;
use crate::ranking;

/// Function to rank search results based on query relevance
pub fn rank_search_results(results: &mut Vec<SearchResult>, queries: &[String], reranker: &str) {
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

    let ranked_indices = ranking::rank_documents(
        &documents_refs, 
        &combined_query,
        file_unique_terms,
        file_total_matches,
        results.first().and_then(|r| r.file_match_rank),
        block_unique_terms,
        block_total_matches,
        node_type
    );

    // Store original document indices and their various scores for later use
    let mut doc_scores: Vec<(usize, f64, f64, f64, f64)> = Vec::new();

    // Update the search results with rank and score information
    for (rank_index, (original_index, combined_score, tfidf_score, bm25_score, new_score)) in
        ranked_indices.iter().enumerate()
    {
        doc_scores.push((*original_index, *combined_score, *tfidf_score, *bm25_score, *new_score));
        
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
            hybrid2_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            
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
            } else {
                println!("Using hybrid ranking (simple TF-IDF + BM25 combination)");
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
                println!("DEBUG: Using hybrid ranking (default - simple TF-IDF + BM25 combination)");
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
}
