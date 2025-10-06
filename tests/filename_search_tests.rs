use probe_code::search::elastic_query;
use probe_code::search::filters::SearchFilters;
use std::path::PathBuf;

#[test]
fn test_filename_directive_extraction() {
    // Test explicit filename: directive
    let query = "filename:\"SWE_TASK.txt\"";
    let ast = elastic_query::parse_query(query, false).expect("Failed to parse query");
    let (filters, simplified_ast) = SearchFilters::extract_and_simplify_with_autodetect(ast);

    // Check that filename filter was extracted
    assert_eq!(filters.exact_filenames.len(), 1);
    assert_eq!(filters.exact_filenames[0], "SWE_TASK.txt");

    // Should have no content search since only filename was specified
    assert!(simplified_ast.is_none());

    // Test that the filter matches the file
    assert!(filters.matches_file(&PathBuf::from("SWE_TASK.txt")));
    assert!(filters.matches_file(&PathBuf::from("path/to/SWE_TASK.txt")));
    assert!(!filters.matches_file(&PathBuf::from("OTHER_FILE.txt")));
}

#[test]
fn test_filename_auto_detection() {
    // Test auto-detection of filename-like terms
    let query = "\"config.json\"";
    let ast = elastic_query::parse_query(query, false).expect("Failed to parse query");
    let (filters, simplified_ast) = SearchFilters::extract_and_simplify_with_autodetect(ast);

    // Should auto-detect as filename
    assert_eq!(filters.exact_filenames.len(), 1);
    assert_eq!(filters.exact_filenames[0], "config.json");
    assert!(simplified_ast.is_none());

    // Test that the filter works
    assert!(filters.matches_file(&PathBuf::from("config.json")));
    assert!(filters.matches_file(&PathBuf::from("src/config.json")));
    assert!(!filters.matches_file(&PathBuf::from("data.xml")));
}

#[test]
fn test_filename_or_query() {
    // Test OR query with multiple filenames
    let query = "\"SWE_TASK.txt\" OR \"swebench_problem.json\"";
    let ast = elastic_query::parse_query(query, false).expect("Failed to parse query");
    let (filters, simplified_ast) = SearchFilters::extract_and_simplify_with_autodetect(ast);

    // Both should be detected as filenames
    assert_eq!(filters.exact_filenames.len(), 2);
    assert!(filters
        .exact_filenames
        .contains(&"SWE_TASK.txt".to_string()));
    assert!(filters
        .exact_filenames
        .contains(&"swebench_problem.json".to_string()));
    assert!(simplified_ast.is_none());

    // Test that filters match both files
    assert!(filters.matches_file(&PathBuf::from("SWE_TASK.txt")));
    assert!(filters.matches_file(&PathBuf::from("swebench_problem.json")));
    assert!(!filters.matches_file(&PathBuf::from("other.md")));
}

#[test]
fn test_filename_and_content_query() {
    // Test AND query with filename and content search
    let query = "\"task.txt\" AND error";
    let ast = elastic_query::parse_query(query, false).expect("Failed to parse query");
    let (filters, simplified_ast) = SearchFilters::extract_and_simplify_with_autodetect(ast);

    // Filename should be extracted as filter
    assert_eq!(filters.exact_filenames.len(), 1);
    assert_eq!(filters.exact_filenames[0], "task.txt");

    // "error" should remain in AST for content search
    assert!(simplified_ast.is_some());

    // Test filter matching
    assert!(filters.matches_file(&PathBuf::from("task.txt")));
    assert!(!filters.matches_file(&PathBuf::from("notes.txt")));
}

#[test]
fn test_filename_case_insensitive() {
    // Test case-insensitive filename matching
    let query = "filename:\"readme.md\"";
    let ast = elastic_query::parse_query(query, false).expect("Failed to parse query");
    let (filters, _) = SearchFilters::extract_and_simplify_with_autodetect(ast);

    // Should match regardless of case
    assert!(filters.matches_file(&PathBuf::from("README.md")));
    assert!(filters.matches_file(&PathBuf::from("readme.md")));
    assert!(filters.matches_file(&PathBuf::from("Readme.MD")));
}
