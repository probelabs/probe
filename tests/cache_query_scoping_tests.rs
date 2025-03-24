use anyhow::Result;
use probe::models::SearchResult;
use probe::search::cache;
use std::collections::HashSet;

#[test]
fn test_query_scoped_caching() -> Result<()> {
    // Create a test session ID
    let session_id = "test_session";

    // Create two different queries
    let query1 = "function implementation";
    let query2 = "error handling";

    // Create some test search results
    let result1 = SearchResult {
        file: "file1.rs".to_string(),
        lines: (10, 20),
        node_type: "function".to_string(),
        code: "fn test() {}".to_string(),
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
        matched_keywords: None,
        tokenized_content: None,
    };

    let result2 = SearchResult {
        file: "file2.rs".to_string(),
        lines: (30, 40),
        node_type: "function".to_string(),
        code: "fn handle_error() {}".to_string(),
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
        matched_keywords: None,
        tokenized_content: None,
    };

    // Add result1 to cache for query1
    cache::add_results_to_cache(&[result1.clone()], session_id, query1)?;

    // Verify result1 is cached for query1
    let (filtered_results, skipped_count) =
        cache::filter_results_with_cache(&[result1.clone()], session_id, query1)?;

    assert_eq!(
        filtered_results.len(),
        0,
        "Result1 should be filtered out for query1"
    );
    assert_eq!(skipped_count, 1, "One result should be skipped for query1");

    // Verify result1 is NOT cached for query2
    let (filtered_results, skipped_count) =
        cache::filter_results_with_cache(&[result1.clone()], session_id, query2)?;

    assert_eq!(
        filtered_results.len(),
        1,
        "Result1 should not be filtered out for query2"
    );
    assert_eq!(skipped_count, 0, "No results should be skipped for query2");

    // Add result2 to cache for query2
    cache::add_results_to_cache(&[result2.clone()], session_id, query2)?;

    // Verify result2 is cached for query2
    let (filtered_results, skipped_count) =
        cache::filter_results_with_cache(&[result2.clone()], session_id, query2)?;

    assert_eq!(
        filtered_results.len(),
        0,
        "Result2 should be filtered out for query2"
    );
    assert_eq!(skipped_count, 1, "One result should be skipped for query2");

    // Verify result2 is NOT cached for query1
    let (filtered_results, skipped_count) =
        cache::filter_results_with_cache(&[result2.clone()], session_id, query1)?;

    assert_eq!(
        filtered_results.len(),
        1,
        "Result2 should not be filtered out for query1"
    );
    assert_eq!(skipped_count, 0, "No results should be skipped for query1");

    // Test with both results for both queries
    let both_results = vec![result1.clone(), result2.clone()];

    // For query1, only result1 should be filtered
    let (filtered_results, skipped_count) =
        cache::filter_results_with_cache(&both_results, session_id, query1)?;

    assert_eq!(
        filtered_results.len(),
        1,
        "Only result2 should remain for query1"
    );
    assert_eq!(skipped_count, 1, "One result should be skipped for query1");
    assert_eq!(
        filtered_results[0].file, "file2.rs",
        "Result2 should remain for query1"
    );

    // For query2, only result2 should be filtered
    let (filtered_results, skipped_count) =
        cache::filter_results_with_cache(&both_results, session_id, query2)?;

    assert_eq!(
        filtered_results.len(),
        1,
        "Only result1 should remain for query2"
    );
    assert_eq!(skipped_count, 1, "One result should be skipped for query2");
    assert_eq!(
        filtered_results[0].file, "file1.rs",
        "Result1 should remain for query2"
    );

    Ok(())
}

#[test]
fn test_early_line_filtering_with_query_scoping() -> Result<()> {
    use std::collections::HashMap;
    use std::path::PathBuf;

    // Create a test session ID (different from the first test)
    let session_id = "test_session_2";

    // Create two different queries
    let query1 = "function implementation";
    let query2 = "error handling";

    // Create a file_term_map for testing
    let mut file_term_map = HashMap::new();
    let file_path = PathBuf::from("test_file.rs");

    // Add some term matches
    let mut term_map = HashMap::new();
    term_map.insert(0, HashSet::from([10, 11, 12])); // Lines 10-12 for term 0
    term_map.insert(1, HashSet::from([20, 21, 22])); // Lines 20-22 for term 1

    file_term_map.insert(file_path.clone(), term_map);

    // Create a search result for the same file
    let result = SearchResult {
        file: "test_file.rs".to_string(),
        lines: (10, 12),
        node_type: "function".to_string(),
        code: "fn test() {}".to_string(),
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
        matched_keywords: None,
        tokenized_content: None,
    };

    // Add the result to cache for query1
    cache::add_results_to_cache(&[result.clone()], session_id, query1)?;

    // Verify the result is in the cache
    let query_hash = cache::hash_query(query1);
    let cache_key = cache::generate_cache_key(&result);
    println!("Cache key for result: {}", cache_key);

    // Manually check if the cache contains the block
    let cache = cache::SessionCache::load(session_id, &query_hash)?;
    println!("Cache contains {} entries", cache.block_identifiers.len());
    for entry in &cache.block_identifiers {
        println!("Cache entry: {}", entry);
    }
    assert!(
        cache.block_identifiers.contains(&cache_key),
        "Cache should contain the result"
    );

    // Clone the file_term_map for each query test
    let mut file_term_map1 = file_term_map.clone();
    let mut file_term_map2 = file_term_map.clone();

    // Filter lines for query1
    let skipped_count1 =
        cache::filter_matched_lines_with_cache(&mut file_term_map1, session_id, query1)?;

    // Lines 10-12 should be filtered for query1
    assert!(
        skipped_count1 > 0,
        "Some lines should be skipped for query1"
    );

    // After filtering, the term 0 should be completely removed since all its lines were in the cache
    // This is because our filter_matched_lines_with_cache function removes terms with empty line sets
    assert!(
        !file_term_map1.contains_key(&file_path)
            || !file_term_map1.get(&file_path).unwrap().contains_key(&0),
        "Term 0 should be removed for query1 as all its lines were cached"
    );

    // Filter lines for query2
    let skipped_count2 =
        cache::filter_matched_lines_with_cache(&mut file_term_map2, session_id, query2)?;

    // No lines should be filtered for query2
    assert_eq!(skipped_count2, 0, "No lines should be skipped for query2");

    // Check if lines 10-12 are still present for term 0 in query2
    let term_map2 = file_term_map2.get(&file_path).unwrap();
    if let Some(term0_lines2) = term_map2.get(&0) {
        assert!(
            term0_lines2.contains(&10),
            "Line 10 should not be filtered for query2"
        );
        assert!(
            term0_lines2.contains(&11),
            "Line 11 should not be filtered for query2"
        );
        assert!(
            term0_lines2.contains(&12),
            "Line 12 should not be filtered for query2"
        );
    } else {
        panic!("Term 0 should still exist for query2");
    }

    Ok(())
}
