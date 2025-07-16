use crate::models::SearchResult;
use crate::ranking;
use std::time::Instant;

/// Helper function to format duration in a human-readable way
fn format_duration(duration: std::time::Duration) -> String {
    if duration.as_millis() < 1000 {
        let millis = duration.as_millis();
        format!("{millis}ms")
    } else {
        let secs = duration.as_secs_f64();
        format!("{secs:.2}s")
    }
}

/// Function to rank search results based on query relevance using BM25 algorithm
pub fn rank_search_results(results: &mut [SearchResult], queries: &[String], reranker: &str) {
    let start_time = Instant::now();

    // Check if debug mode is enabled
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    if debug_mode {
        println!(
            "DEBUG: Starting result ranking with {} results",
            results.len()
        );
        println!("DEBUG: Using reranker: {reranker}");
        println!("DEBUG: Queries: {queries:?}");
    }

    // Combine all queries into a single string for ranking
    let query_combine_start = Instant::now();
    let combined_query = queries.join(" ");
    let query_combine_duration = query_combine_start.elapsed();

    if debug_mode {
        println!(
            "DEBUG: Query combination completed in {} - Combined query: '{}'",
            format_duration(query_combine_duration),
            combined_query
        );
    }

    // Extract document texts for ranking, including filename in each document
    let document_extraction_start = Instant::now();
    // This ensures filename terms are considered in the ranking algorithms
    let documents: Vec<String> = results
        .iter()
        .map(|r| {
            let mut doc = String::with_capacity(r.file.len() + r.code.len() + 15);
            doc.push_str("// Filename: ");
            doc.push_str(&r.file);
            doc.push('\n');
            doc.push_str(&r.code);
            doc
        })
        .collect();
    let documents_refs: Vec<&str> = documents.iter().map(|s| s.as_str()).collect();
    let document_extraction_duration = document_extraction_start.elapsed();

    if debug_mode {
        println!(
            "DEBUG: Document extraction completed in {} - Extracted {} documents",
            format_duration(document_extraction_duration),
            documents.len()
        );
    }

    // Rank the documents
    let metrics_extraction_start = Instant::now();
    // Get metrics from the first result (assuming they're the same for all results in this set)
    let file_unique_terms = results.first().and_then(|r| r.file_unique_terms);
    let file_total_matches = results.first().and_then(|r| r.file_total_matches);
    let block_unique_terms = results.first().and_then(|r| r.block_unique_terms);
    let block_total_matches = results.first().and_then(|r| r.block_total_matches);
    let metrics_extraction_duration = metrics_extraction_start.elapsed();

    if debug_mode {
        println!(
            "DEBUG: Metrics extraction completed in {}",
            format_duration(metrics_extraction_duration)
        );
        println!(
            "DEBUG: Extracted metrics - file_unique_terms: {:?}, file_total_matches: {:?}, block_unique_terms: {:?}, block_total_matches: {:?}",
            file_unique_terms, file_total_matches, block_unique_terms, block_total_matches
        );
    }

    // Extract pre-tokenized content if available
    let tokenized_extraction_start = Instant::now();
    let pre_tokenized: Vec<Vec<String>> = results
        .iter()
        .filter_map(|r| r.tokenized_content.clone())
        .collect();

    let has_tokenized = !pre_tokenized.is_empty() && pre_tokenized.len() == results.len();

    if debug_mode {
        if has_tokenized {
            println!(
                "DEBUG: Using pre-tokenized content from {} results",
                pre_tokenized.len()
            );
        } else {
            println!(
                "DEBUG: Pre-tokenized content not available for all results (found {}/{}), falling back to tokenization",
                pre_tokenized.len(),
                results.len()
            );
        }
    }

    let tokenized_extraction_duration = tokenized_extraction_start.elapsed();

    if debug_mode {
        println!(
            "DEBUG: Tokenized content extraction completed in {}",
            format_duration(tokenized_extraction_duration)
        );
    }

    let ranking_params = ranking::RankingParams {
        documents: &documents_refs,
        query: &combined_query,
        pre_tokenized: if has_tokenized {
            Some(&pre_tokenized)
        } else {
            None
        },
    };

    let document_ranking_start = Instant::now();
    if debug_mode {
        println!("DEBUG: Starting document ranking...");
    }

    // Get ranked indices from the ranking module (BM25 scores)
    let ranked_indices = ranking::rank_documents(&ranking_params);

    let document_ranking_duration = document_ranking_start.elapsed();

    if debug_mode {
        println!(
            "DEBUG: Document ranking completed in {} - Ranked {} documents",
            format_duration(document_ranking_duration),
            ranked_indices.len()
        );
    }

    // Update scores for all results returned by the ranking module
    // We don't filter by BM25 score here because the ranking module already does some filtering
    // based on the query, and we want to preserve OR query behavior
    let filtering_start = Instant::now();
    let mut updated_results = Vec::new();

    // Update scores for all results
    for (rank_index, (original_index, bm25_score)) in ranked_indices.iter().enumerate() {
        if let Some(result) = results.get(*original_index) {
            let mut result_clone = result.clone();
            result_clone.rank = Some(rank_index + 1); // 1-based rank
            result_clone.score = Some(*bm25_score);
            result_clone.bm25_score = Some(*bm25_score);
            updated_results.push(result_clone);
        }
    }

    let updated_len = updated_results.len();

    if debug_mode {
        println!(
            "DEBUG: Score update completed - Updated {} results",
            updated_len
        );
    }

    // Sort updated results by BM25 score in descending order
    let reranker_sort_start = Instant::now();

    if debug_mode {
        println!("DEBUG: Using BM25 ranking (Okapi BM25 algorithm)");
    } else {
        println!("Using BM25 ranking (Okapi BM25 algorithm)");
    }

    // Sort by BM25 score in descending order
    updated_results.sort_by(|a, b| {
        let score_a = a.bm25_score.unwrap_or(0.0);
        let score_b = b.bm25_score.unwrap_or(0.0);
        // Sort in descending order (higher score is better)
        score_b
            .partial_cmp(&score_a)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Reassign ranks based on the sorted order
    for (rank, result) in updated_results.iter_mut().enumerate() {
        result.bm25_rank = Some(rank + 1); // 1-based rank
        result.rank = Some(rank + 1);
    }

    let reranker_sort_duration = reranker_sort_start.elapsed();

    if debug_mode {
        println!(
            "DEBUG: Reranker-specific sorting completed in {}",
            format_duration(reranker_sort_duration)
        );
    }

    // Replace original results with updated results
    if updated_len < results.len() {
        // If we have fewer results than the original array, copy only what we have
        for (i, result) in updated_results.into_iter().enumerate() {
            results[i] = result;
        }

        // Instead of truncating, we'll keep the original file paths
        // but mark these results with a special flag
        for result in results.iter_mut().skip(updated_len) {
            // Set a special flag to indicate this result should be skipped
            // but preserve the file path
            result.matched_by_filename = Some(false);
            result.score = Some(0.0);
            result.rank = Some(usize::MAX);
        }
    } else {
        // If we have the same number of results, just replace them all
        for (i, result) in updated_results.into_iter().enumerate() {
            results[i] = result;
        }
    }

    let filtering_duration = filtering_start.elapsed();

    if debug_mode {
        println!(
            "DEBUG: Result processing completed in {} - Processed {} results",
            format_duration(filtering_duration),
            updated_len
        );
    }

    let total_duration = start_time.elapsed();

    if debug_mode {
        println!(
            "DEBUG: Total result ranking completed in {}",
            format_duration(total_duration)
        );
    }
}
