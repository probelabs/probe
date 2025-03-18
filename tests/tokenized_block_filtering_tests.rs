use probe::search::file_processing::filter_tokenized_block;
use probe::search::query::create_query_plan;
use std::collections::{HashMap, HashSet};

/// Test direct usage of filter_tokenized_block with various complex queries
#[test]
fn test_filter_tokenized_block_basic() {
    // Create a simple tokenized content
    let tokenized_content = vec![
        "ip".to_string(),
        "whitelist".to_string(),
        "config".to_string(),
    ];

    // Test with a simple AND query
    let query = "ip AND whitelist";
    let plan = create_query_plan(query, false).unwrap();

    // Test filtering
    let result = filter_tokenized_block(&tokenized_content, &plan.term_indices, &plan, true);
    assert!(
        result,
        "Block with 'ip' and 'whitelist' should match the AND query"
    );

    // Test with a simple OR query
    let query = "ip OR port";
    let plan = create_query_plan(query, false).unwrap();

    // Test filtering
    let result = filter_tokenized_block(&tokenized_content, &plan.term_indices, &plan, true);
    assert!(
        result,
        "Block with 'ip' should match the OR query even without 'port'"
    );

    // Test with a more complex query
    let query = "(ip OR port) AND config";
    let plan = create_query_plan(query, false).unwrap();

    // Test filtering
    let result = filter_tokenized_block(&tokenized_content, &plan.term_indices, &plan, true);
    assert!(
        result,
        "Block with 'ip' and 'config' should match the complex query"
    );
}

#[test]
fn test_filter_tokenized_block_with_excluded_terms() {
    // Create a tokenized content with some terms
    let tokenized_content = vec![
        "ip".to_string(),
        "whitelist".to_string(),
        "config".to_string(),
    ];

    // Test with a query that has excluded terms
    let query = "ip -test";
    let plan = create_query_plan(query, false).unwrap();

    // Test filtering
    let result = filter_tokenized_block(&tokenized_content, &plan.term_indices, &plan, true);
    assert!(result, "Block with 'ip' but without 'test' should match");

    // Create a tokenized content with the excluded term
    let tokenized_content_with_excluded = vec![
        "ip".to_string(),
        "whitelist".to_string(),
        "test".to_string(),
    ];

    // Test filtering with the excluded term present
    let result = filter_tokenized_block(
        &tokenized_content_with_excluded,
        &plan.term_indices,
        &plan,
        true,
    );
    assert!(
        !result,
        "Block with 'ip' and 'test' should NOT match because 'test' is excluded"
    );
}

#[test]
fn test_filter_tokenized_block_with_complex_queries() {
    // Create a tokenized content
    let tokenized_content = vec![
        "ip".to_string(),
        "whitelist".to_string(),
        "config".to_string(),
        "server".to_string(),
    ];

    // Test with a complex query with AND, OR, and excluded terms
    let query = "(ip OR port) AND (whitelist OR config) -test";
    let plan = create_query_plan(query, false).unwrap();

    // Test filtering
    let result = filter_tokenized_block(&tokenized_content, &plan.term_indices, &plan, true);
    assert!(
        result,
        "Block with 'ip', 'whitelist', and 'config' but without 'test' should match"
    );

    // Test with a different tokenized content that doesn't match
    let tokenized_content_missing_required = vec!["port".to_string(), "server".to_string()];

    // Test filtering
    let result = filter_tokenized_block(
        &tokenized_content_missing_required,
        &plan.term_indices,
        &plan,
        true,
    );
    assert!(
        !result,
        "Block with only 'port' and 'server' should NOT match because it's missing 'whitelist' or 'config'"
    );
}

#[test]
fn test_filter_tokenized_block_empty_content() {
    // Create an empty tokenized content
    let tokenized_content: Vec<String> = vec![];

    // Test with a simple query
    let query = "ip AND whitelist";
    let plan = create_query_plan(query, false).unwrap();

    // Test filtering
    let result = filter_tokenized_block(&tokenized_content, &plan.term_indices, &plan, true);
    assert!(!result, "Empty block should not match any query");
}

#[test]
fn test_filter_tokenized_block_comparison_with_line_based() {
    // This test compares the behavior of filter_tokenized_block with filter_code_block_with_ast
    // to ensure they produce the same results for equivalent inputs

    // Create a tokenized content
    let tokenized_content = vec![
        "ip".to_string(),
        "whitelist".to_string(),
        "config".to_string(),
    ];

    // Create a query
    let query = "ip AND whitelist";
    let plan = create_query_plan(query, false).unwrap();

    // Test tokenized filtering
    let tokenized_result =
        filter_tokenized_block(&tokenized_content, &plan.term_indices, &plan, true);

    // Import the line-based filtering function for comparison
    use probe::search::file_processing::filter_code_block_with_ast;

    // Create equivalent line-based term matches
    let mut term_matches = HashMap::new();
    term_matches.insert(0, HashSet::from([1])); // ip on line 1
    term_matches.insert(1, HashSet::from([1])); // whitelist on line 1
    term_matches.insert(2, HashSet::from([1])); // config on line 1

    // Test line-based filtering
    let line_based_result = filter_code_block_with_ast((1, 1), &term_matches, &plan, true);

    // Compare results
    assert_eq!(
        tokenized_result, line_based_result,
        "Tokenized filtering and line-based filtering should produce the same result"
    );
}
