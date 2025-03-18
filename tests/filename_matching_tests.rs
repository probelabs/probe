use std::collections::{HashMap, HashSet};
use std::path::Path;

use probe::search::file_search::find_matching_filenames;

#[test]
fn test_filename_matching() {
    // Create a test directory path
    let test_dir = Path::new("tests/mocks");

    // Test with a simple query that should match test_object.js
    let queries = vec!["object".to_string()];
    let already_found = HashSet::new();
    let custom_ignores = Vec::new();

    // Create term indices map
    let mut term_indices = HashMap::new();
    term_indices.insert("object".to_string(), 0);

    let result = find_matching_filenames(
        test_dir,
        &queries,
        &already_found,
        &custom_ignores,
        true,
        &term_indices,
    )
    .unwrap();

    // Check if test_object.js is in the results
    let has_test_object = result.iter().any(|(path, _)| {
        path.file_name()
            .map(|name| name.to_string_lossy().contains("test_object.js"))
            .unwrap_or(false)
    });

    assert!(
        has_test_object,
        "test_object.js should be matched by filename"
    );

    // Test with a query that should match test_struct.go
    // Note: "struct" is a programming keyword in PROGRAMMING_STOP_WORDS and is filtered out during tokenization
    // Instead, we'll use "test" which is part of the filename
    let queries = vec!["test".to_string()];

    // Create term indices map
    let mut term_indices = HashMap::new();
    term_indices.insert("test".to_string(), 0);

    let result = find_matching_filenames(
        test_dir,
        &queries,
        &already_found,
        &custom_ignores,
        true,
        &term_indices,
    )
    .unwrap();

    // Check if test_struct.go is in the results
    let has_test_struct = result.iter().any(|(path, _)| {
        path.file_name()
            .map(|name| name.to_string_lossy().contains("test_struct.go"))
            .unwrap_or(false)
    });

    assert!(
        has_test_struct,
        "test_struct.go should be matched by 'test' in filename"
    );

    // Test with a query that shouldn't match any files
    let queries = vec!["nonexistent".to_string()];

    // Create term indices map
    let mut term_indices = HashMap::new();
    term_indices.insert("nonexistent".to_string(), 0);

    let result = find_matching_filenames(
        test_dir,
        &queries,
        &already_found,
        &custom_ignores,
        true,
        &term_indices,
    )
    .unwrap();

    assert!(result.is_empty(), "No files should match 'nonexistent'");

    // Test with multiple queries
    let queries = vec!["object".to_string(), "test".to_string()];

    // Create term indices map
    let mut term_indices = HashMap::new();
    term_indices.insert("object".to_string(), 0);
    term_indices.insert("test".to_string(), 1);

    let result = find_matching_filenames(
        test_dir,
        &queries,
        &already_found,
        &custom_ignores,
        true,
        &term_indices,
    )
    .unwrap();

    // Both files should be matched
    let has_test_object = result.iter().any(|(path, _)| {
        path.file_name()
            .map(|name| name.to_string_lossy().contains("test_object.js"))
            .unwrap_or(false)
    });

    let has_test_struct = result.iter().any(|(path, _)| {
        path.file_name()
            .map(|name| name.to_string_lossy().contains("test_struct.go"))
            .unwrap_or(false)
    });

    assert!(
        has_test_object,
        "test_object.js should be matched by filename with multiple queries"
    );
    assert!(
        has_test_struct,
        "test_struct.go should be matched by 'test' with multiple queries"
    );
}

#[test]
fn test_filename_matching_with_already_found() {
    // Create a test directory path
    let test_dir = Path::new("tests/mocks");

    // Create a set of already found files including test_object.js
    let mut already_found = HashSet::new();
    already_found.insert(test_dir.join("test_object.js"));

    // Test with a query that should match test_object.js and test_struct.go
    let queries = vec!["test".to_string()];
    let custom_ignores = Vec::new();

    // Create term indices map
    let mut term_indices = HashMap::new();
    term_indices.insert("test".to_string(), 0);

    let result = find_matching_filenames(
        test_dir,
        &queries,
        &already_found,
        &custom_ignores,
        true,
        &term_indices,
    )
    .unwrap();

    // Check if test_object.js is NOT in the results (because it's already found)
    let has_test_object = result.iter().any(|(path, _)| {
        path.file_name()
            .map(|name| name.to_string_lossy().contains("test_object.js"))
            .unwrap_or(false)
    });

    // Check if test_struct.go IS in the results
    let has_test_struct = result.iter().any(|(path, _)| {
        path.file_name()
            .map(|name| name.to_string_lossy().contains("test_struct.go"))
            .unwrap_or(false)
    });

    assert!(
        !has_test_object,
        "test_object.js should not be in results because it's already found"
    );
    assert!(
        has_test_struct,
        "test_struct.go should be matched by filename"
    );
}

#[test]
fn test_filename_matching_with_test_files() {
    // Create a test directory path
    let test_dir = Path::new("tests/mocks");

    // Test with allow_tests = false
    let queries = vec!["test".to_string()];
    let already_found = HashSet::new();
    let custom_ignores = Vec::new();

    // Create term indices map
    let mut term_indices = HashMap::new();
    term_indices.insert("test".to_string(), 0);

    // First with allow_tests = true
    let result_with_tests = find_matching_filenames(
        test_dir,
        &queries,
        &already_found,
        &custom_ignores,
        true,
        &term_indices,
    )
    .unwrap();

    // Then with allow_tests = false
    let result_without_tests = find_matching_filenames(
        test_dir,
        &queries,
        &already_found,
        &custom_ignores,
        false,
        &term_indices,
    )
    .unwrap();

    // With allow_tests = true, we should find all test files
    assert!(
        !result_with_tests.is_empty(),
        "Should find test files with allow_tests = true"
    );

    // With allow_tests = false, we might find fewer files
    // This test is more of a sanity check since our mocks directory might not have explicit test files
    // that would be filtered out by the allow_tests flag
    assert!(
        result_without_tests.len() <= result_with_tests.len(),
        "Should find fewer or equal number of files with allow_tests = false"
    );
}
