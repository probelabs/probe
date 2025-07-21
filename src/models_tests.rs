use probe_code::models::{SearchResult, CodeBlock, LimitedSearchResults, SearchLimits};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_result_creation() {
        let result = SearchResult {
            file: "test.rs".to_string(),
            lines: (1, 10),
            node_type: "function".to_string(),
            code: "fn test() {}".to_string(),
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
        };
        
        assert_eq!(result.file, "test.rs");
        assert_eq!(result.lines, (1, 10));
        assert_eq!(result.node_type, "function");
        assert_eq!(result.code, "fn test() {}");
        assert_eq!(result.matched_by_filename, None);
        assert_eq!(result.rank, None);
        assert_eq!(result.score, None);
        assert_eq!(result.tfidf_score, None);
        assert_eq!(result.bm25_score, None);
        assert_eq!(result.tfidf_rank, None);
        assert_eq!(result.bm25_rank, None);
        assert_eq!(result.file_unique_terms, None);
        assert_eq!(result.file_total_matches, None);
        assert_eq!(result.file_match_rank, None);
    }

    #[test]
    fn test_code_block_creation() {
        let block = CodeBlock {
            start_row: 1,
            end_row: 10,
            start_byte: 0,
            end_byte: 100,
            node_type: "function".to_string(),
        };
        
        assert_eq!(block.start_row, 1);
        assert_eq!(block.end_row, 10);
        assert_eq!(block.start_byte, 0);
        assert_eq!(block.end_byte, 100);
        assert_eq!(block.node_type, "function");
    }

    #[test]
    fn test_limited_search_results() {
        // Create some search results
        let results = vec![
            SearchResult {
                file: "test1.rs".to_string(),
                lines: (1, 10),
                node_type: "function".to_string(),
                code: "fn test1() {}".to_string(),
                matched_by_filename: None,
                rank: Some(1),
                score: Some(0.9),
                tfidf_score: Some(0.9),
                bm25_score: Some(0.9),
                tfidf_rank: Some(1),
                bm25_rank: Some(1),
                file_unique_terms: Some(2),
                file_total_matches: Some(5),
                file_match_rank: Some(1),
            },
            SearchResult {
                file: "test2.rs".to_string(),
                lines: (1, 10),
                node_type: "function".to_string(),
                code: "fn test2() {}".to_string(),
                matched_by_filename: None,
                rank: Some(2),
                score: Some(0.8),
                tfidf_score: Some(0.8),
                bm25_score: Some(0.8),
                tfidf_rank: Some(2),
                bm25_rank: Some(2),
                file_unique_terms: Some(1),
                file_total_matches: Some(3),
                file_match_rank: Some(2),
            },
        ];
        
        // Create some skipped files
        let skipped_files = vec![
            SearchResult {
                file: "test3.rs".to_string(),
                lines: (1, 10),
                node_type: "function".to_string(),
                code: "fn test3() {}".to_string(),
                matched_by_filename: None,
                rank: Some(3),
                score: Some(0.7),
                tfidf_score: Some(0.7),
                bm25_score: Some(0.7),
                tfidf_rank: Some(3),
                bm25_rank: Some(3),
                file_unique_terms: Some(1),
                file_total_matches: Some(2),
                file_match_rank: Some(3),
            },
        ];
        
        // Create limits
        let limits = SearchLimits {
            max_results: Some(2),
            max_bytes: Some(1000),
            max_tokens: Some(200),
            total_bytes: 24,
            total_tokens: 6,
        };
        
        // Create the limited search results
        let limited_results = LimitedSearchResults {
            results: results.clone(),
            skipped_files: skipped_files.clone(),
            limits_applied: Some(limits),
        };
        
        // Check the contents
        assert_eq!(limited_results.results.len(), 2);
        assert_eq!(limited_results.skipped_files.len(), 1);
        
        // Check that the limits are correctly stored
        let limits = limited_results.limits_applied.unwrap();
        assert_eq!(limits.max_results, Some(2));
        assert_eq!(limits.max_bytes, Some(1000));
        assert_eq!(limits.max_tokens, Some(200));
        assert_eq!(limits.total_bytes, 24);
        assert_eq!(limits.total_tokens, 6);
    }

    #[test]
    fn test_search_limits() {
        let limits = SearchLimits {
            max_results: Some(10),
            max_bytes: Some(1000),
            max_tokens: Some(200),
            total_bytes: 500,
            total_tokens: 100,
        };
        
        assert_eq!(limits.max_results, Some(10));
        assert_eq!(limits.max_bytes, Some(1000));
        assert_eq!(limits.max_tokens, Some(200));
        assert_eq!(limits.total_bytes, 500);
        assert_eq!(limits.total_tokens, 100);
    }
}