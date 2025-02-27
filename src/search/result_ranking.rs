use crate::models::SearchResult;
use crate::ranking;

/// Function to rank search results based on query relevance
pub fn rank_search_results(results: &mut Vec<SearchResult>, queries: &[String], reranker: &str) {
    // Check if debug mode is enabled
    let debug_mode = std::env::var("CODE_SEARCH_DEBUG").unwrap_or_default() == "1";

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
    let ranked_indices = ranking::rank_documents(&documents_refs, &combined_query);

    // Update the search results with rank and score information
    for (rank_index, (original_index, combined_score, tfidf_score, bm25_score)) in
        ranked_indices.iter().enumerate()
    {
        if let Some(result) = results.get_mut(*original_index) {
            result.rank = Some(rank_index + 1); // 1-based rank
            result.score = Some(*combined_score);
            result.tfidf_score = Some(*tfidf_score);
            result.bm25_score = Some(*bm25_score);
        }
    }

    // Create separate rankings for TF-IDF and BM25 scores
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
    tfidf_ranking.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
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

    // Sort the results based on the selected reranker
    match reranker {
        "tfidf" => {
            if debug_mode {
                println!("DEBUG: Using TF-IDF for ranking");
            } else {
                println!("Using TF-IDF for ranking");
            }

            // Sort by TF-IDF score
            results.sort_by(|a, b| {
                let score_a = a.tfidf_score.unwrap_or(0.0);
                let score_b = b.tfidf_score.unwrap_or(0.0);
                // Sort in descending order
                score_b
                    .partial_cmp(&score_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            // Update the rank field to match the TF-IDF rank
            for (i, result) in results.iter_mut().enumerate() {
                result.rank = Some(i + 1); // 1-based rank
            }
        }
        "bm25" => {
            if debug_mode {
                println!("DEBUG: Using BM25 for ranking");
            } else {
                println!("Using BM25 for ranking");
            }

            // Sort by BM25 score
            results.sort_by(|a, b| {
                let score_a = a.bm25_score.unwrap_or(0.0);
                let score_b = b.bm25_score.unwrap_or(0.0);
                // Sort in descending order
                score_b
                    .partial_cmp(&score_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            // Update the rank field to match the BM25 rank
            for (i, result) in results.iter_mut().enumerate() {
                result.rank = Some(i + 1); // 1-based rank
            }
        }
        _ => {
            if debug_mode {
                println!("DEBUG: Using hybrid ranking (default)");
            } else {
                println!("Using hybrid ranking (default)");
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

            // Update the rank field to match the hybrid rank
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
