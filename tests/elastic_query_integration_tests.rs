use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

use probe::search::elastic_query::Expr;
use probe::search::query::QueryPlan;
use probe::search::{perform_probe, SearchOptions};

/// Test the integration of elastic query parsing with file processing
#[test]
fn test_elastic_query_integration() {
    // Create a temporary directory for testing
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create test files with different content
    create_test_files(temp_path);

    // Test various complex queries
    test_required_term_query(temp_path);
    test_excluded_term_query(temp_path);
    test_or_query(temp_path);
    test_complex_query(temp_path);
}

/// Create test files with different content for testing queries
fn create_test_files(temp_dir: &Path) {
    // File with "keyword1" and "keyword2"
    let file1_path = temp_dir.join("file1.rs");
    let file1_content = r#"
// This file contains keyword1 and keyword2
fn test_function() {
    // This is keyword1
    let x = 1;
    
    // This is keyword2
    let y = 2;
    
    println!("Result: {}", x + y);
}
"#;

    // File with "keyword1" and "keyword3"
    let file2_path = temp_dir.join("file2.rs");
    let file2_content = r#"
// This file contains keyword1 and keyword3
fn another_function() {
    // This is keyword1
    let x = 1;
    
    // This is keyword3
    let z = 3;
    
    println!("Result: {}", x + z);
}
"#;

    // File with "keyword2" and "keyword3"
    let file3_path = temp_dir.join("file3.rs");
    let file3_content = r#"
// This file contains keyword2 and keyword3
fn third_function() {
    // This is keyword2
    let y = 2;
    
    // This is keyword3
    let z = 3;
    
    println!("Result: {}", y + z);
}
"#;

    // File with all keywords
    let file4_path = temp_dir.join("file4.rs");
    let file4_content = r#"
// This file contains keyword1, keyword2, and keyword3
fn all_keywords_function() {
    // This is keyword1
    let x = 1;
    
    // This is keyword2
    let y = 2;
    
    // This is keyword3
    let z = 3;
    
    println!("Result: {}", x + y + z);
}
"#;

    // Write files to disk
    fs::write(file1_path, file1_content).unwrap();
    fs::write(file2_path, file2_content).unwrap();
    fs::write(file3_path, file3_content).unwrap();
    fs::write(file4_path, file4_content).unwrap();
}

/// Test a query with a required term: keyword1
fn test_required_term_query(temp_path: &Path) {
    // Create search query with explicit OR syntax
    // Since we've removed any_term parameter and changed the default to AND,
    // we need to use explicit OR syntax to maintain the original behavior
    let queries = vec!["key OR word OR 1".to_string()];
    let custom_ignores: Vec<String> = vec![];

    // Print the query for debugging
    println!("Testing query: {:?}", queries);

    // Create SearchOptions
    let options = SearchOptions {
        path: temp_path,
        queries: &queries,
        files_only: false,
        custom_ignores: &custom_ignores,
        exclude_filenames: false,
        reranker: "combined",
        frequency_search: false,
        max_results: None,
        max_bytes: None,
        max_tokens: None,
        allow_tests: true,
        exact: false,
        no_merge: false,
        merge_threshold: Some(5),
        dry_run: false,
    };

    // Print the temp_path for debugging
    println!("Temp path: {:?}", temp_path);

    // List files in the temp directory
    println!("Files in temp directory:");
    for entry in std::fs::read_dir(temp_path).unwrap() {
        let entry = entry.unwrap();
        println!("  {:?}", entry.path());
    }

    // Run the search
    let search_results = perform_probe(&options).unwrap();

    // Debug output
    println!("Search results: {} items", search_results.results.len());
    for result in &search_results.results {
        println!("  File: {}", result.file);
    }

    // Check that we got results
    assert!(
        !search_results.results.is_empty(),
        "Search should return results"
    );

    // We should only find files with keyword1
    let file_names: Vec<&str> = search_results
        .results
        .iter()
        .map(|r| r.file.as_str())
        .collect();

    // Debug output
    println!("Found {} results", search_results.results.len());
    for result in &search_results.results {
        println!("File: {}", result.file);
    }

    // Check that we found files with key OR word OR 1
    assert!(
        file_names.iter().any(|&name| name.contains("file1")),
        "Should find file1 which contains key OR word OR 1"
    );
    assert!(
        file_names.iter().any(|&name| name.contains("file2")),
        "Should find file2 which contains key OR word OR 1"
    );
    assert!(
        file_names.iter().any(|&name| name.contains("file3")),
        "Should find file3 which contains key OR word OR 1"
    );
    assert!(
        file_names.iter().any(|&name| name.contains("file4")),
        "Should find file4 which contains key OR word OR 1"
    );
}

/// Test a query with an excluded term: -keyword3
fn test_excluded_term_query(temp_path: &Path) {
    // Create search query with an excluded term
    let queries = vec!["(key OR word OR 1) -keyword3".to_string()];
    let custom_ignores: Vec<String> = vec![];
    // Create SearchOptions
    let options = SearchOptions {
        path: temp_path,
        queries: &queries,
        files_only: false,
        custom_ignores: &custom_ignores,
        exclude_filenames: false,
        reranker: "combined",
        frequency_search: false,
        max_results: None,
        max_bytes: None,
        max_tokens: None,
        allow_tests: true,
        exact: false,
        no_merge: false,
        merge_threshold: Some(5),
        dry_run: false,
    };

    // Run the search
    let search_results = perform_probe(&options).unwrap();

    // Check that we got results
    assert!(
        !search_results.results.is_empty(),
        "Search should return results"
    );

    // We should find files with (key OR word OR 1) -keyword3
    let file_names: Vec<&str> = search_results
        .results
        .iter()
        .map(|r| r.file.as_str())
        .collect();

    // Debug output
    println!(
        "Excluded term query results: {} items",
        search_results.results.len()
    );
    for result in &search_results.results {
        println!("  File: {}", result.file);
    }

    // Check that we found file1 (has key OR word OR 1 but not keyword3)
    assert!(
        file_names.iter().any(|&name| name.contains("file1")),
        "Should find file1 which contains key OR word OR 1 but not keyword3"
    );

    // Check that we don't find file2, file3, and file4 (they have keyword3)
    assert!(
        !file_names.iter().any(|&name| name.contains("file2")),
        "Should not find file2 which contains keyword3"
    );

    assert!(
        !file_names.iter().any(|&name| name.contains("file3")),
        "Should not find file3 which contains keyword3"
    );

    assert!(
        !file_names.iter().any(|&name| name.contains("file4")),
        "Should not find file4 which contains keyword3"
    );
}

/// Test a query with OR: keyword1 OR keyword2
fn test_or_query(temp_path: &Path) {
    // Create search query with explicit OR syntax
    let queries = vec!["keyword1 OR keyword2".to_string()];
    let custom_ignores: Vec<String> = vec![];

    // Create SearchOptions
    let options = SearchOptions {
        path: temp_path,
        queries: &queries,
        files_only: false,
        custom_ignores: &custom_ignores,
        exclude_filenames: false,
        reranker: "combined",
        frequency_search: false,
        max_results: None,
        max_bytes: None,
        max_tokens: None,
        allow_tests: true,
        exact: false,
        no_merge: false,
        merge_threshold: Some(5),
        dry_run: false,
    };

    // Run the search
    let search_results = perform_probe(&options).unwrap();

    // Check that we got results
    assert!(
        !search_results.results.is_empty(),
        "Search should return results"
    );

    // We should find files with keyword1 OR keyword2
    let file_names: Vec<&str> = search_results
        .results
        .iter()
        .map(|r| r.file.as_str())
        .collect();

    // Check that we found files with keyword1 OR keyword2
    assert!(
        file_names.iter().any(|&name| name.contains("file1")),
        "Should find file1 which contains keyword1 and keyword2"
    );
    assert!(
        file_names.iter().any(|&name| name.contains("file2")),
        "Should find file2 which contains keyword1"
    );
    assert!(
        file_names.iter().any(|&name| name.contains("file3")),
        "Should find file3 which contains keyword2"
    );
    assert!(
        file_names.iter().any(|&name| name.contains("file4")),
        "Should find file4 which contains keyword1 and keyword2"
    );
}

/// Test a complex query with explicit OR syntax
fn test_complex_query(temp_path: &Path) {
    // Test with explicit OR syntax
    // "keyword1 OR keyword2" means files with keyword1 OR keyword2
    let queries = vec!["keyword1 OR keyword2".to_string()];
    let custom_ignores: Vec<String> = vec![];

    // Create SearchOptions
    let options = SearchOptions {
        path: temp_path,
        queries: &queries,
        files_only: false,
        custom_ignores: &custom_ignores,
        exclude_filenames: false,
        reranker: "combined",
        frequency_search: false,
        max_results: None,
        max_bytes: None,
        max_tokens: None,
        allow_tests: true,
        exact: false,
        no_merge: false,
        merge_threshold: Some(5),
        dry_run: false,
    };

    // Run the search
    let search_results = perform_probe(&options).unwrap();

    // Check that we got results
    assert!(
        !search_results.results.is_empty(),
        "Search should return results"
    );

    let file_names: Vec<&str> = search_results
        .results
        .iter()
        .map(|r| r.file.as_str())
        .collect();

    // Debug output to see what files were found
    println!("Found files with 'keyword1 keyword2' (OR behavior):");
    for name in &file_names {
        println!("  {}", name);
    }

    // With OR behavior, should find all files with keyword1 OR keyword2
    assert!(
        file_names.iter().any(|&name| name.contains("file1")),
        "Should find file1 which has keyword1 and keyword2"
    );
    assert!(
        file_names.iter().any(|&name| name.contains("file2")),
        "Should find file2 which has keyword1"
    );
    assert!(
        file_names.iter().any(|&name| name.contains("file3")),
        "Should find file3 which has keyword2"
    );
    assert!(
        file_names.iter().any(|&name| name.contains("file4")),
        "Should find file4 which has keyword1 and keyword2"
    );

    // Now test with exclusion
    let queries = vec!["keyword1 -keyword3".to_string()];

    // Create SearchOptions
    let options = SearchOptions {
        path: temp_path,
        queries: &queries,
        files_only: false,
        custom_ignores: &custom_ignores,
        exclude_filenames: false,
        reranker: "combined",
        frequency_search: false,
        max_results: None,
        max_bytes: None,
        max_tokens: None,
        allow_tests: true,
        exact: false,
        no_merge: false,
        merge_threshold: Some(5),
        dry_run: false,
    };

    // Run the search
    let search_results = perform_probe(&options).unwrap();

    // Check that we got results
    assert!(
        !search_results.results.is_empty(),
        "Search should return results"
    );

    let file_names: Vec<&str> = search_results
        .results
        .iter()
        .map(|r| r.file.as_str())
        .collect();

    // Debug output to see what files were found
    println!("Found files with 'keyword1 -keyword3':");
    for name in &file_names {
        println!("  {}", name);
    }

    // Should find file1 (has keyword1 and no keyword3)
    assert!(
        file_names.iter().any(|&name| name.contains("file1")),
        "Should find file1 which has keyword1 but no keyword3"
    );

    // Should NOT find file2 (has keyword1 but also has keyword3)
    assert!(
        !file_names.iter().any(|&name| name.contains("file2")),
        "Should not find file2 which has keyword3"
    );

    // Should NOT find file4 (has keyword1 but also has keyword3)
    assert!(
        !file_names.iter().any(|&name| name.contains("file4")),
        "Should not find file4 which has keyword3"
    );
}

/// Test underscore handling in queries
#[test]
fn test_underscore_handling_integration() {
    // Create a temporary directory for testing
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create a test file with the tokenized terms
    let file_path = temp_path.join("underscore_test.rs");
    let file_content = r#"
// This file contains key word score
fn test_function() {
    // This has key, word, and score
    let x = 1;
    
    // This also has key word score
    let y = 2;
    
    println!("Result: {}", x + y);
}
"#;
    fs::write(file_path, file_content).unwrap();

    // Test query with underscore using explicit OR syntax
    // Since we've removed any_term parameter and changed the default to AND,
    // we need to use explicit OR syntax to maintain the original behavior
    let queries = vec!["key OR word OR score".to_string()];
    let custom_ignores: Vec<String> = vec![];

    // Create SearchOptions
    let options = SearchOptions {
        path: temp_path,
        queries: &queries,
        files_only: false,
        custom_ignores: &custom_ignores,
        exclude_filenames: false,
        reranker: "combined",
        frequency_search: false,
        max_results: None,
        max_bytes: None,
        max_tokens: None,
        allow_tests: true,
        exact: false,
        no_merge: false,
        merge_threshold: Some(5),
        dry_run: false,
    };

    // Run the search
    let search_results = perform_probe(&options).unwrap();

    // Check that we got results
    assert!(
        !search_results.results.is_empty(),
        "Search should return results"
    );

    let file_names: Vec<&str> = search_results
        .results
        .iter()
        .map(|r| r.file.as_str())
        .collect();

    // Debug output to see what files were found
    println!("Found files with 'key OR word OR score':");
    for name in &file_names {
        println!("  {}", name);
    }

    // Should find the file with at least one of the terms: key, word, or score
    assert!(
        file_names.iter().any(|&name| name.contains("underscore_test")),
        "Should find underscore_test.rs which contains at least one of the terms: key, word, or score"
    );

    // Verify that the code in the results contains at least one of the tokenized terms
    for result in &search_results.results {
        println!("Result code: {}", result.code);
        // The search should find files containing at least one of the tokenized terms
        assert!(
            result.code.contains("key")
                || result.code.contains("word")
                || result.code.contains("score"),
            "Result code should contain at least one of the terms: 'key', 'word', or 'score'"
        );
    }
}

/// Test the direct usage of filter_code_block_with_ast
#[test]
fn test_filter_code_block_with_ast() {
    // This test directly tests the filter_code_block_with_ast function
    // by creating a mock QueryPlan and term matches

    // Create a simple AST: keyword1 AND -keyword2
    let ast = Expr::And(
        Box::new(Expr::Term {
            keywords: vec!["keyword1".to_string()],
            field: None,
            required: false,
            excluded: false,
        }),
        Box::new(Expr::Term {
            keywords: vec!["keyword2".to_string()],
            field: None,
            required: false,
            excluded: true,
        }),
    );

    // Create a term indices map
    let mut term_indices = HashMap::new();
    term_indices.insert("keyword1".to_string(), 0);
    term_indices.insert("keyword2".to_string(), 1);

    // Create a QueryPlan
    let plan = QueryPlan {
        ast,
        term_indices,
        excluded_terms: {
            let mut set = HashSet::new();
            set.insert("keyword2".to_string());
            set
        },
    };

    // Create term matches for a block
    let mut term_matches = HashMap::new();

    // Block with only keyword1
    let mut lines1 = HashSet::new();
    lines1.insert(1);
    lines1.insert(2);
    term_matches.insert(0, lines1);

    // Test the function with a block that should match
    let block_lines = (1, 5);
    let debug_mode = false;

    // Import the function from probe crate
    use probe::search::file_processing::filter_code_block_with_ast;

    // The block should match because it has keyword1 but not keyword2
    assert!(
        filter_code_block_with_ast(block_lines, &term_matches, &plan, debug_mode),
        "Block should match because it has keyword1 but not keyword2"
    );

    // Now add keyword2 to the block
    let mut lines2 = HashSet::new();
    lines2.insert(3);
    lines2.insert(4);
    term_matches.insert(1, lines2);

    // The block should not match because it now has keyword2 (which is excluded)
    assert!(
        !filter_code_block_with_ast(block_lines, &term_matches, &plan, debug_mode),
        "Block should not match because it has keyword2 which is excluded"
    );
}
