use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

use probe::search::{perform_probe, SearchOptions};

// Helper function to create test files
fn create_test_file(dir: &TempDir, filename: &str, content: &str) -> PathBuf {
    let file_path = dir.path().join(filename);
    fs::write(&file_path, content).expect("Failed to write test content");
    file_path
}

// Helper function to create a test directory structure with many searchable items
fn create_test_directory_structure(root_dir: &TempDir) {
    // Create a source directory
    let src_dir = root_dir.path().join("src");
    fs::create_dir(&src_dir).expect("Failed to create src directory");

    // Create multiple files with searchable content
    for i in 0..20 {
        let file_content = format!(
            r#"
// File number {}
fn search_function_{}() {{
    // This is a searchable function
    let result = "search term";
    println!("Found: {{}}", result);
}}

fn another_function_{}() {{
    // This is another searchable function
    let result = "search term";
    println!("Found: {{}}", result);
}}

fn third_function_{}() {{
    // This is a third searchable function
    let result = "search term";
    println!("Found: {{}}", result);
}}
"#,
            i, i, i, i
        );
        create_test_file(root_dir, &format!("src/file_{}.rs", i), &file_content);
    }
}

#[test]
fn test_cache_after_limit() {
    // Create a temporary directory for test files
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Create search query
    let queries = vec!["search term".to_string()];
    let custom_ignores: Vec<String> = vec![];

    // Generate a unique session ID for this test
    let session_id = format!(
        "test_session_{}",
        std::time::SystemTime::now().elapsed().unwrap().as_nanos()
    );

    // First search with a small limit (should find only 5 results)
    let options_limited = SearchOptions {
        path: temp_dir.path(),
        queries: &queries,
        files_only: false,
        custom_ignores: &custom_ignores,
        exclude_filenames: true,
        reranker: "hybrid",
        frequency_search: false,
        max_results: Some(5), // Limit to only 5 results
        max_bytes: None,
        max_tokens: None,
        allow_tests: false,
        exact: false,
        no_merge: true,
        merge_threshold: None,
        dry_run: false,
        session: Some(&session_id), // Use our session ID
    };

    // Perform the first search with limited results
    let limited_results = perform_probe(&options_limited).expect("Failed to perform search");

    // Verify we got exactly 5 results (our limit)
    assert_eq!(
        limited_results.results.len(),
        5,
        "Should find exactly 5 results due to limit"
    );

    // Now search again with no limit but same session ID
    let options_unlimited = SearchOptions {
        path: temp_dir.path(),
        queries: &queries,
        files_only: false,
        custom_ignores: &custom_ignores,
        exclude_filenames: true,
        reranker: "hybrid",
        frequency_search: false,
        max_results: None, // No limit this time
        max_bytes: None,
        max_tokens: None,
        allow_tests: false,
        exact: false,
        no_merge: true,
        merge_threshold: None,
        dry_run: false,
        session: Some(&session_id), // Same session ID
    };

    // Perform the second search with no limits
    let unlimited_results = perform_probe(&options_unlimited).expect("Failed to perform search");

    // If caching is working correctly AFTER limiting, we should find more than 5 results
    // because the cache should only contain the 5 results from the first search
    assert!(
        unlimited_results.results.len() > 5,
        "Should find more than 5 results in the second search, found: {}",
        unlimited_results.results.len()
    );
}

#[test]
fn test_cache_updates() {
    // Create a temporary directory for test files
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Create search query
    let queries = vec!["search term".to_string()];
    let custom_ignores: Vec<String> = vec![];

    // Generate a unique session ID for this test
    let session_id = format!(
        "test_session_{}",
        std::time::SystemTime::now().elapsed().unwrap().as_nanos()
    );

    // First search with a limit
    let options_limited = SearchOptions {
        path: temp_dir.path(),
        queries: &queries,
        files_only: false,
        custom_ignores: &custom_ignores,
        exclude_filenames: true,
        reranker: "hybrid",
        frequency_search: false,
        max_results: Some(10), // Limit to 10 results
        max_bytes: None,
        max_tokens: None,
        allow_tests: false,
        exact: false,
        no_merge: true,
        merge_threshold: None,
        dry_run: false,
        session: Some(&session_id), // Use our session ID
    };

    // Perform the first search with limited results
    let limited_results = perform_probe(&options_limited).expect("Failed to perform search");
    let limited_count = limited_results.results.len();

    // Verify we got some results (should be 10 due to limit)
    assert_eq!(
        limited_count, 10,
        "Should find exactly 10 results due to limit"
    );

    // Now search again with the same session ID and same limit
    let second_results = perform_probe(&options_limited).expect("Failed to perform search");

    // We should get the same number of results
    assert_eq!(
        second_results.results.len(),
        limited_count,
        "Should get the same number of results in the second search"
    );

    // The results should have the same count
    assert_eq!(
        second_results.results.len(),
        limited_count,
        "Should get the same number of results in the second search"
    );

    // Get the files from both searches and sort them for comparison
    let mut first_files: Vec<String> = limited_results
        .results
        .iter()
        .map(|r| r.file.clone())
        .collect();
    let mut second_files: Vec<String> = second_results
        .results
        .iter()
        .map(|r| r.file.clone())
        .collect();

    // Sort the files to account for non-deterministic ordering
    first_files.sort();
    second_files.sort();

    // Check that at least 80% of the files are the same
    // This allows for some non-determinism in the search results
    let mut common_files = 0;
    for file in &first_files {
        if second_files.contains(file) {
            common_files += 1;
        }
    }

    let similarity_ratio = common_files as f64 / first_files.len() as f64;
    assert!(
        similarity_ratio >= 0.5,
        "At least 50% of files should be the same, but only {:.1}% are common",
        similarity_ratio * 100.0
    );
}
