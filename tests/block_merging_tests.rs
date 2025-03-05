use std::path::Path;
use std::fs;
use tempfile::TempDir;

use probe::models::SearchResult;
use probe::search::block_merging::merge_ranked_blocks;
use probe::search::search_runner::perform_probe;

#[test]
fn test_merge_ranked_blocks() {
    // Create test blocks that should be merged
    let block1 = SearchResult {
        file: "test_file.rs".to_string(),
        lines: (1, 5),
        node_type: "function".to_string(),
        code: "fn test_function() {\n    let x = 1;\n    let y = 2;\n    println!(\"{}\", x + y);\n}".to_string(),
        matched_by_filename: None,
        rank: Some(1),
        score: Some(0.9),
        tfidf_score: Some(0.8),
        bm25_score: Some(0.85),
        tfidf_rank: Some(1),
        bm25_rank: Some(1),
        new_score: Some(0.87),
        hybrid2_rank: Some(1),
        combined_score_rank: Some(1),
        file_unique_terms: Some(3),
        file_total_matches: Some(5),
        file_match_rank: Some(1),
        block_unique_terms: Some(2),
        block_total_matches: Some(3),
        parent_file_id: None,
        block_id: None,
    };

    let block2 = SearchResult {
        file: "test_file.rs".to_string(),
        lines: (6, 10),
        node_type: "function".to_string(),
        code: "fn another_function() {\n    let z = 3;\n    let result = z * 2;\n    println!(\"{}\", result);\n}".to_string(),
        matched_by_filename: None,
        rank: Some(2),
        score: Some(0.8),
        tfidf_score: Some(0.7),
        bm25_score: Some(0.75),
        tfidf_rank: Some(2),
        bm25_rank: Some(2),
        new_score: Some(0.77),
        hybrid2_rank: Some(2),
        combined_score_rank: Some(2),
        file_unique_terms: Some(3),
        file_total_matches: Some(5),
        file_match_rank: Some(1),
        block_unique_terms: Some(2),
        block_total_matches: Some(2),
        parent_file_id: None,
        block_id: None,
    };

    // Create block from a different file that should not be merged
    let block3 = SearchResult {
        file: "other_file.rs".to_string(),
        lines: (1, 5),
        node_type: "function".to_string(),
        code: "fn other_function() {\n    let a = 10;\n    let b = 20;\n    println!(\"{}\", a + b);\n}".to_string(),
        matched_by_filename: None,
        rank: Some(3),
        score: Some(0.7),
        tfidf_score: Some(0.6),
        bm25_score: Some(0.65),
        tfidf_rank: Some(3),
        bm25_rank: Some(3),
        new_score: Some(0.67),
        hybrid2_rank: Some(3),
        combined_score_rank: Some(3),
        file_unique_terms: Some(2),
        file_total_matches: Some(4),
        file_match_rank: Some(2),
        block_unique_terms: Some(1),
        block_total_matches: Some(3),
        parent_file_id: None,
        block_id: None,
    };

    // Create a vector with all blocks
    let blocks = vec![block1, block2, block3];
    
    // Call the merge_ranked_blocks function
    let merged_blocks = merge_ranked_blocks(blocks, Some(5));
    
    // Assert that we now have 2 blocks (the first two merged, the third separate)
    assert_eq!(merged_blocks.len(), 2, "Blocks should be merged from 3 to 2");
    
    // Find the merged block from test_file.rs and the standalone block from other_file.rs
    let test_file_blocks: Vec<&SearchResult> = merged_blocks.iter()
        .filter(|b| b.file == "test_file.rs")
        .collect();
    
    let other_file_blocks: Vec<&SearchResult> = merged_blocks.iter()
        .filter(|b| b.file == "other_file.rs")
        .collect();
    
    // Check that we have one block for each file
    assert_eq!(test_file_blocks.len(), 1, "Should have 1 merged block for test_file.rs");
    assert_eq!(other_file_blocks.len(), 1, "Should have 1 block for other_file.rs");
    
    // Check that the first block is merged correctly
    let merged_block = test_file_blocks[0];
    assert_eq!(merged_block.lines, (1, 10), "Lines should be merged from (1, 5) and (6, 10) to (1, 10)");
    
    // Check that the score is the maximum of the two blocks
    assert_eq!(merged_block.score, Some(0.9), "Merged score should be the maximum of the two blocks");
    
    // Check that the term statistics are combined correctly
    assert!(merged_block.block_unique_terms.unwrap() >= 2, "Merged block should have at least 2 unique terms");
    assert!(merged_block.block_total_matches.unwrap() >= 3, "Merged block should have at least 3 total matches");
    
    // Check that the third block is preserved as is
    let preserved_block = other_file_blocks[0];
    assert_eq!(preserved_block.lines, (1, 5), "Unmerged block should preserve its line range");
}

#[test]
fn test_integration_with_search_flow() {
    // Create a temporary directory for testing
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();
    
    // Create test files with overlapping code blocks
    create_test_files(temp_path);
    
    // Run a search that should produce multiple overlapping blocks
    let search_results = perform_probe(
        temp_path,
        &["function test".to_string()],
        false,  // files_only
        &[],    // custom_ignores
        true,   // include_filenames
        "combined", // reranker
        false,  // frequency_search
        None,   // max_results
        None,   // max_bytes
        None,   // max_tokens
        true,   // allow_tests
        true,   // any_term
        false,  // exact
        true,   // merge_blocks
        Some(5), // merge_threshold
    ).unwrap();
    
    // Check that we got merged results
    assert!(!search_results.results.is_empty(), "Search should return results");
    
    // Verify that blocks from the same file are merged
    let mut file_count = std::collections::HashMap::new();
    for result in &search_results.results {
        *file_count.entry(result.file.clone()).or_insert(0) += 1;
    }
    
    // Each file should have at most one result after merging
    for (_file, count) in file_count {
        assert!(count <= 1, "Each file should have at most one result after merging");
    }
}

/// Helper function to create test files with functions that should trigger merging
fn create_test_files(temp_dir: &Path) {
    // Create a file with multiple adjacent functions
    let file1_path = temp_dir.join("test_functions.rs");
    let file1_content = r#"
// Test file with multiple functions
fn test_function1() {
    // This function does testing
    let x = 1;
    let y = 2;
    println!("Test result: {}", x + y);
}

fn test_function2() {
    // This function also does testing
    let a = 10;
    let b = 20;
    println!("Test result: {}", a + b);
}

fn another_function() {
    // This function does something else
    let z = 100;
    println!("Not a test: {}", z);
}
"#;
    
    // Create a file with non-adjacent blocks
    let file2_path = temp_dir.join("non_adjacent.rs");
    let file2_content = r#"
// Another test file
fn test_function() {
    // This function does testing
    let x = 1;
    println!("Test result: {}", x);
}

// A lot of unrelated code in between
// ...
// ...
// ...

fn another_test_function() {
    // This function also does testing but it's far from the first one
    let y = 2;
    println!("Test result: {}", y);
}
"#;
    
    // Write files to disk
    fs::write(file1_path, file1_content).unwrap();
    fs::write(file2_path, file2_content).unwrap();
}
