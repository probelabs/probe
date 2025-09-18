use probe_code::models::SearchResult;
use probe_code::search::result_ranking::rank_search_results;

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_results() -> Vec<SearchResult> {
        vec![
            SearchResult {
                file: "file1.rs".to_string(),
                lines: (1, 10),
                node_type: "context".to_string(), // Changed to context for testing context boost
                code: "fn test_function() { println!(\"This is a test function with search terms\"); }".to_string(),
                symbol_signature: None,
                matched_by_filename: None,
                rank: None,
                score: None,
                tfidf_score: None,
                bm25_score: None,
                tfidf_rank: None,
                bm25_rank: None,
                new_score: None,
                file_unique_terms: Some(2), // "search", "terms"
                file_total_matches: Some(2),
                file_match_rank: Some(2),
                block_unique_terms: Some(2),
                block_total_matches: Some(2),
                hybrid2_rank: None,
                combined_score_rank: None,
                parent_file_id: None,
                block_id: None,
                matched_keywords: None,
                matched_lines: None,
                tokenized_content: None,
                parent_context: None,
            },
            SearchResult {
                file: "file2.rs".to_string(),
                lines: (1, 5),
                node_type: "function".to_string(),
                code: "fn another_function() { // This doesn't have the key term }".to_string(),
                symbol_signature: None,
                matched_by_filename: None,
                rank: None,
                score: None,
                tfidf_score: None,
                bm25_score: None,
                tfidf_rank: None,
                bm25_rank: None,
                new_score: None,
                file_unique_terms: Some(1), // No search terms
                file_total_matches: Some(1),
                file_match_rank: Some(3),
                block_unique_terms: Some(0),
                block_total_matches: Some(0),
                hybrid2_rank: None,
                combined_score_rank: None,
                parent_file_id: None,
                block_id: None,
                matched_keywords: None,
                matched_lines: None,
                tokenized_content: None,
                parent_context: None,
            },
            SearchResult {
                file: "file3.rs".to_string(),
                lines: (1, 10),
                node_type: "function".to_string(),
                code: "fn search_function() { // This has search in the function name and multiple search terms search search }".to_string(),
                symbol_signature: None,
                matched_by_filename: None,
                rank: None,
                score: None,
                tfidf_score: None,
                bm25_score: None,
                tfidf_rank: None,
                bm25_rank: None,
                new_score: None,
                file_unique_terms: Some(3), // "search" appears multiple times
                file_total_matches: Some(4),
                file_match_rank: Some(1),
                block_unique_terms: Some(1),
                block_total_matches: Some(3),
                hybrid2_rank: None,
                combined_score_rank: None,
                parent_file_id: None,
                block_id: None,
                matched_keywords: None,
                matched_lines: None,
                tokenized_content: None,
                parent_context: None,
            },
        ]
    }

    #[test]
    fn test_rank_search_results_hybrid() {
        let mut results = create_test_results();
        let queries = vec!["search".to_string()];

        // Enable debug mode for this test to verify logging
        std::env::set_var("DEBUG", "1");
        rank_search_results(&mut results, &queries, "hybrid", None);
        std::env::remove_var("DEBUG");

        // Check that all results have been assigned ranks and scores
        for result in &results {
            assert!(result.rank.is_some());
            assert!(result.score.is_some());
            assert!(result.tfidf_score.is_some());
            assert!(result.bm25_score.is_some());
            assert!(result.new_score.is_some());
            assert!(result.tfidf_rank.is_some());
            assert!(result.bm25_rank.is_some());
        }

        // Check that the results are sorted by rank
        for i in 1..results.len() {
            assert!(results[i-1].rank.unwrap() < results[i].rank.unwrap());
        }

        // File3 should have the highest score (rank 1) because it contains "search" multiple times
        let top_result = &results[0];
        assert!(top_result.file.contains("file3"));
    }

    #[test]
    fn test_rank_search_results_tfidf() {
        let mut results = create_test_results();
        let queries = vec!["search".to_string()];

        rank_search_results(&mut results, &queries, "tfidf", None);

        // Check that all results have been assigned ranks and scores
        for result in &results {
            assert!(result.rank.is_some());
            assert!(result.score.is_some());
            assert!(result.tfidf_score.is_some());
            assert!(result.bm25_score.is_some());
            assert!(result.new_score.is_some());
            assert!(result.tfidf_rank.is_some());
            assert!(result.bm25_rank.is_some());
        }

        // When using tfidf, the rank should match the tfidf_rank
        for result in &results {
            assert_eq!(result.rank, result.tfidf_rank);
        }
    }

    #[test]
    fn test_rank_search_results_bm25() {
        let mut results = create_test_results();
        let queries = vec!["search".to_string()];

        rank_search_results(&mut results, &queries, "bm25", None);

        // Check that all results have been assigned ranks and scores
        for result in &results {
            assert!(result.rank.is_some());
            assert!(result.score.is_some());
            assert!(result.tfidf_score.is_some());
            assert!(result.bm25_score.is_some());
            assert!(result.new_score.is_some());
            assert!(result.tfidf_rank.is_some());
            assert!(result.bm25_rank.is_some());
        }

        // When using bm25, the rank should match the bm25_rank
        for result in &results {
            assert_eq!(result.rank, result.bm25_rank);
        }
    }

    #[test]
    fn test_rank_search_results_multi_term_query() {
        let mut results = create_test_results();
        let queries = vec!["search".to_string(), "function".to_string()];

        rank_search_results(&mut results, &queries, "hybrid", None);

        // Check that all results have been assigned ranks and scores
        for result in &results {
            assert!(result.rank.is_some());
            assert!(result.score.is_some());
            assert!(result.tfidf_score.is_some());
            assert!(result.bm25_score.is_some());
            assert!(result.new_score.is_some());
            assert!(result.tfidf_rank.is_some());
            assert!(result.bm25_rank.is_some());
        }

        // Results with both terms should rank higher
        let top_result = &results[0];
        assert!(top_result.code.contains("search") && top_result.code.contains("function"));
    }

    #[test]
    fn test_rank_search_results_empty() {
        let mut results = Vec::new();
        let queries = vec!["search".to_string()];

        // Should not panic with empty results
        rank_search_results(&mut results, &queries, "hybrid", None);

        assert_eq!(results.len(), 0);
    }
}
