use crate::models::SearchResult;
use crate::search::result_ranking::rank_search_results;

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_results() -> Vec<SearchResult> {
        vec![
            SearchResult {
                file: "file1.rs".to_string(),
                lines: (1, 10),
                node_type: "function".to_string(),
                code: "fn test_function() { println!(\"This is a test function with search terms\"); }".to_string(),
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
            },
            SearchResult {
                file: "file2.rs".to_string(),
                lines: (1, 5),
                node_type: "function".to_string(),
                code: "fn another_function() { // This doesn't have the key term }".to_string(),
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
            },
            SearchResult {
                file: "file3.rs".to_string(),
                lines: (1, 10),
                node_type: "function".to_string(),
                code: "fn search_function() { // This has search in the function name and multiple search terms search search }".to_string(),
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
            },
        ]
    }

    #[test]
    fn test_rank_search_results_hybrid() {
        let mut results = create_test_results();
        let queries = vec!["search".to_string()];
        
        rank_search_results(&mut results, &queries, "hybrid");
        
        // Check that all results have been assigned ranks and scores
        for result in &results {
            assert!(result.rank.is_some());
            assert!(result.score.is_some());
            assert!(result.tfidf_score.is_some());
            assert!(result.bm25_score.is_some());
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
        
        rank_search_results(&mut results, &queries, "tfidf");
        
        // Check that all results have been assigned ranks and scores
        for result in &results {
            assert!(result.rank.is_some());
            assert!(result.score.is_some());
            assert!(result.tfidf_score.is_some());
            assert!(result.bm25_score.is_some());
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
        
        rank_search_results(&mut results, &queries, "bm25");
        
        // Check that all results have been assigned ranks and scores
        for result in &results {
            assert!(result.rank.is_some());
            assert!(result.score.is_some());
            assert!(result.tfidf_score.is_some());
            assert!(result.bm25_score.is_some());
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
        
        rank_search_results(&mut results, &queries, "hybrid");
        
        // Check that all results have been assigned ranks and scores
        for result in &results {
            assert!(result.rank.is_some());
            assert!(result.score.is_some());
            assert!(result.tfidf_score.is_some());
            assert!(result.bm25_score.is_some());
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
        rank_search_results(&mut results, &queries, "hybrid");
        
        assert_eq!(results.len(), 0);
    }
}