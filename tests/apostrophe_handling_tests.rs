use probe_code::extract;
use std::collections::HashSet;
use std::path::PathBuf;

#[test]
fn test_extract_with_apostrophes() {
    // Test that the extract command can handle text with apostrophes
    let text = r#"
Here's the detailed breakdown of performance bottlenecks with full relative path names:

1. Two-Phase Search Process (~600-800ms):
   - `src/search/file_search.rs#find_files_with_pattern`: Initial file discovery
   - `src/search/file_search.rs#search_file_for_pattern`: Secondary content search
   - `src/search/search_runner.rs#search_with_structured_patterns`: Coordinates the multi-phase search
"#;

    // Extract file paths from the text
    let file_paths = extract::extract_file_paths_from_text(text, true);

    // Check that we found the expected file paths
    assert_eq!(
        file_paths.len(),
        3,
        "Expected 3 file paths, found {}",
        file_paths.len()
    );

    // Create a HashSet of the expected file paths
    let mut expected_paths = HashSet::new();
    expected_paths.insert("src/search/file_search.rs#find_files_with_pattern".to_string());
    expected_paths.insert("src/search/file_search.rs#search_file_for_pattern".to_string());
    expected_paths
        .insert("src/search/search_runner.rs#search_with_structured_patterns".to_string());

    // Check that each extracted path is in our expected set
    for (path, _, _, symbol, _) in &file_paths {
        let path_str = path.to_string_lossy().to_string();
        let symbol_str = symbol.as_ref().unwrap();
        let full_path = format!("{path_str}#{symbol_str}");

        assert!(
            expected_paths.contains(&full_path),
            "Unexpected path: {full_path}"
        );
    }
}

#[test]
fn test_parse_file_with_apostrophes() {
    // The parse_file_with_line function is designed to parse a single file path,
    // not extract paths from text. Let's test it with a direct file path instead.
    let input = "src/file.rs:10";

    // Parse the file path
    let file_paths = extract::parse_file_with_line(input, true);

    // We should find the file path
    assert_eq!(
        file_paths.len(),
        1,
        "Expected 1 file path, found {}",
        file_paths.len()
    );

    let (path, line, _, _, _) = &file_paths[0];
    assert_eq!(
        path,
        &PathBuf::from("src/file.rs"),
        "Unexpected path: {path:?}"
    );
    assert_eq!(line, &Some(10), "Unexpected line number: {line:?}");

    // Now let's test that our fix for apostrophes works by creating a test
    // that verifies apostrophes in text don't break the extract_file_paths_from_text function
    let text_with_apostrophe = "Here's a file path: src/file.rs:10";
    let extracted_paths = extract::extract_file_paths_from_text(text_with_apostrophe, true);

    // We should find the file path despite the apostrophe
    assert_eq!(
        extracted_paths.len(),
        1,
        "Expected 1 file path, found {}",
        extracted_paths.len()
    );

    let (path, line, _, _, _) = &extracted_paths[0];
    assert_eq!(
        path,
        &PathBuf::from("src/file.rs"),
        "Unexpected path: {path:?}"
    );
    assert_eq!(line, &Some(10), "Unexpected line number: {line:?}");
}

#[test]
fn test_apostrophes_in_quoted_content() {
    // Test handling of apostrophes within quoted content
    let text = r#"
Here are some paths:
- "src/file_with_apostrophe.rs:10"
- 'src/another_file.rs:20'
- `src/third_file.rs:30`
"#;

    // Extract file paths from the text
    let file_paths = extract::extract_file_paths_from_text(text, true);

    // Check that we found the paths
    // Note: We expect at least 2 paths to be found
    assert!(
        file_paths.len() >= 2,
        "Expected at least 2 file paths, found {}",
        file_paths.len()
    );

    // Create a HashSet of the expected paths
    let mut expected_paths = HashSet::new();
    expected_paths.insert(("src/file_with_apostrophe.rs".to_string(), 10));
    expected_paths.insert(("src/another_file.rs".to_string(), 20));
    expected_paths.insert(("src/third_file.rs".to_string(), 30));

    // Check that each extracted path is in our expected set
    for (path, line, _, _, _) in &file_paths {
        let path_str = path.to_string_lossy().to_string();
        if let Some(line_num) = line {
            assert!(
                expected_paths.contains(&(path_str.clone(), *line_num)),
                "Unexpected path: {path_str} line: {line_num}"
            );
        }
    }
}
