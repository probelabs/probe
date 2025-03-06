use std::collections::HashSet;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use tempfile::TempDir;

use crate::search::file_processing::{process_file_by_filename, process_file_with_results};

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_file(dir: &TempDir, filename: &str, content: &str) -> std::path::PathBuf {
        let file_path = dir.path().join(filename);
        let mut file = File::create(&file_path).expect("Failed to create test file");
        file.write_all(content.as_bytes()).expect("Failed to write test content");
        file_path
    }

    #[test]
    fn test_process_file_by_filename() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let content = "function test() {\n  console.log('Hello, world!');\n}\n";
        let file_path = create_test_file(&temp_dir, "test.js", content);

        let result = process_file_by_filename(&file_path, &[], None).expect("Failed to process file");

        assert_eq!(result.file, file_path.to_string_lossy());
        assert_eq!(result.lines, (1, 3));  // 3 lines in the file
        assert_eq!(result.node_type, "file");
        assert_eq!(result.code, content);
        assert_eq!(result.matched_by_filename, Some(true));
    }

    #[test]
    fn test_process_file_with_results_single_line() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let content = "line 1\nline 2\nline 3\nline 4\nline 5\n";
        let file_path = create_test_file(&temp_dir, "test.txt", content);

        let mut line_numbers = HashSet::new();
        line_numbers.insert(3);  // Match on "line 3"

        let results = process_file_with_results(
            &file_path, 
            &line_numbers, 
            false,
            None,
            false,
            0,
            HashSet::new(),
            &[],
            None
        )
            .expect("Failed to process file with results");

        assert!(!results.is_empty());
        // Should get context around line 3
        let result = &results[0];
        assert_eq!(result.file, file_path.to_string_lossy());
        assert!(result.lines.0 <= 3);  // Start line should be at or before line 3
        assert!(result.lines.1 >= 3);  // End line should be at or after line 3
    }

    #[test]
    fn test_process_file_with_results_multiple_lines() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        // Create a file with function-like content to test AST parsing
        let content = r#"
function test1() {
  console.log('Test 1');
}

function test2() {
  console.log('Test 2');
}
"#;
        let file_path = create_test_file(&temp_dir, "test.js", content);

        let mut line_numbers = HashSet::new();
        line_numbers.insert(3);  // Line in test1 function
        line_numbers.insert(7);  // Line in test2 function

        let results = process_file_with_results(
            &file_path, 
            &line_numbers, 
            false,
            None,
            false,
            0,
            HashSet::new(),
            &[],
            None
        )
            .expect("Failed to process file with results");

        assert!(!results.is_empty());
        // With AST parsing disabled in tests (since tree-sitter is hard to mock),
        // we should still get context blocks
        assert!(results.len() >= 1);
    }

    #[test]
    fn test_process_file_with_results_high_coverage() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let content = "line 1\nline 2\nline 3\nline 4\nline 5\n";
        let file_path = create_test_file(&temp_dir, "test.txt", content);

        // Match on most lines to trigger high coverage behavior
        let mut line_numbers = HashSet::new();
        line_numbers.insert(1);
        line_numbers.insert(2);
        line_numbers.insert(3);
        line_numbers.insert(4);

        let results = process_file_with_results(
            &file_path, 
            &line_numbers, 
            false,
            None,
            false,
            0,
            HashSet::new(),
            &[],
            None
        )
            .expect("Failed to process file with results");

        assert_eq!(results.len(), 1);
        // When coverage is high, we should just get the whole file
        let result = &results[0];
        assert_eq!(result.file, file_path.to_string_lossy());
        assert_eq!(result.lines, (1, 5));  // All 5 lines
        assert_eq!(result.node_type, "file");
        assert_eq!(result.code, content);
    }
    
    #[test]
    fn test_process_empty_file() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let content = "";
        let file_path = create_test_file(&temp_dir, "empty.txt", content);
        
        let result = process_file_by_filename(&file_path, &[], None).expect("Failed to process empty file");
        
        assert_eq!(result.file, file_path.to_string_lossy());
        assert_eq!(result.lines, (1, 0));  // 0 lines in the file
        assert_eq!(result.node_type, "file");
        assert_eq!(result.code, content);
    }

    #[test]
    fn test_blocks_remain_separate() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        // Create a file with multiple adjacent functions
        let content = r#"
function test1() {
  console.log('Test 1');
}

function test2() {
  console.log('Test 2');
}

function test3() {
  console.log('Test 3');
}
"#;
        let file_path = create_test_file(&temp_dir, "test.js", content);

        let mut line_numbers = HashSet::new();
        // Add line numbers from all three functions
        line_numbers.insert(2); // Line in test1 function
        line_numbers.insert(6); // Line in test2 function
        line_numbers.insert(10); // Line in test3 function

        let results = process_file_with_results(
            &file_path,
            &line_numbers,
            true, // Allow tests
            None, // No term matches
            false, // All terms mode
            0, // No queries
            HashSet::new(), // No filename matches
            &[], // No query terms
            None // No preprocessed queries
        ).expect("Failed to process file with results");

        // With tree-sitter, each function should be a separate block
        // Even though tree-sitter might not be available in tests, we can
        // still check that we're not explicitly merging blocks anymore
        
        // Check if blocks have parent_file_id and block_id set
        for result in &results {
            // Each result should have a parent_file_id that matches the file path
            if let Some(parent_id) = &result.parent_file_id {
                assert!(parent_id.contains(&file_path.to_string_lossy()));
            }
            
            // Each result should have a unique block_id
            assert!(result.block_id.is_some());
        }
        
        // Check if file paths are set correctly
        for result in &results {
            assert_eq!(result.file, file_path.to_string_lossy());
        }
        
        // Check if there are no duplicate block_ids within the same file
        let mut seen_block_ids = HashSet::new();
        for result in &results {
            if let Some(block_id) = result.block_id {
                // We should not have seen this block_id before
                assert!(!seen_block_ids.contains(&block_id));
                seen_block_ids.insert(block_id);
            }
        }
    }

    #[test]
    fn test_block_unique_terms_with_stemming() {
        use crate::search::query::preprocess_query;
        use std::collections::HashMap;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        // Create a file with different forms of the same words
        let content = r#"
function processData() {
  // This function handles data processing
  const data = fetchData();
  return processResults(data);
}

function fetchData() {
  // Fetching the data from API
  return api.fetch('/data');
}

function processResults(results) {
  // Processing the results
  return results.map(r => r.processed);
}
"#;
        let file_path = create_test_file(&temp_dir, "data_processing.js", content);

        // Create query with different forms of the same words
        // "processing" and "process" should stem to the same root
        let query = "processing data";
        let term_pairs = preprocess_query(query, false);
        
        // Verify that stemming is working in the query preprocessing
        assert!(term_pairs.iter().any(|(orig, stemmed)| orig == "processing" && stemmed == "process"));
        
        // Create preprocessed queries for the test
        let preprocessed_queries = vec![term_pairs.iter().map(|(_, s)| s.clone()).collect()];
        
        let mut line_numbers = HashSet::new();
        // Add line numbers from the file
        line_numbers.insert(3); // Line with "data processing"
        
        // Create term matches map
        let mut term_matches = HashMap::new();
        let mut matches_for_term = HashSet::new();
        matches_for_term.insert(3);
        term_matches.insert(0, matches_for_term); // Term index 0 matches line 3
        
        // Process the file
        let params = crate::search::file_processing::FileProcessingParams {
            path: &file_path,
            line_numbers: &line_numbers,
            allow_tests: true,
            term_matches: Some(&term_matches),
            any_term: true,
            num_queries: 2, // "processing" and "data"
            filename_matched_queries: HashSet::new(),
            queries_terms: &[term_pairs.clone()],
            preprocessed_queries: Some(&preprocessed_queries),
        };
        
        let results = process_file_with_results(&params).expect("Failed to process file with results");
        
        // Verify that we got results
        assert!(!results.is_empty());
        
        // Check that block_unique_terms is correctly counting stemmed terms
        for result in &results {
            if let Some(block_unique_terms) = result.block_unique_terms {
                // We should have at least 2 unique terms ("process" and "data")
                // due to stemming, "processing" and "process" count as the same term
                assert!(block_unique_terms >= 2,
                    "Expected at least 2 unique terms, got {}", block_unique_terms);
                
                // Check that block_total_matches is also set
                assert!(result.block_total_matches.is_some());
            }
        }
    }
}
