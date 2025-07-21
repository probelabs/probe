use probe_code::language::parser::parse_file_for_code_blocks;
use std::collections::HashSet;

#[test]
fn test_line_map_cache() {
    // Set up test content
    let content = r#"
fn test_function() {
    // This is a comment
    let x = 42;
    println!("Hello, world!");
}

struct TestStruct {
    field1: i32,
    field2: String,
}
"#;

    // Create a set of line numbers to extract
    let mut line_numbers = HashSet::new();
    line_numbers.insert(3); // Comment line
    line_numbers.insert(4); // Code line
    line_numbers.insert(8); // Struct field line

    // First call should be a cache miss
    let result1 = parse_file_for_code_blocks(content, "rs", &line_numbers, true, None).unwrap();

    // Print result1 details
    println!("Cache miss result (result1) - {} blocks:", result1.len());
    for (i, block) in result1.iter().enumerate() {
        println!(
            "  Block {}: type='{}', lines={}-{}",
            i,
            block.node_type,
            block.start_row + 1,
            block.end_row + 1
        );
    }

    // Second call should be a cache hit
    let result2 = parse_file_for_code_blocks(content, "rs", &line_numbers, true, None).unwrap();

    // Print result2 details
    println!("Cache hit result (result2) - {} blocks:", result2.len());
    for (i, block) in result2.iter().enumerate() {
        println!(
            "  Block {}: type='{}', lines={}-{}",
            i,
            block.node_type,
            block.start_row + 1,
            block.end_row + 1
        );
    }

    // Assert that the results from cache miss and cache hit are identical
    assert_eq!(
        result1, result2,
        "Cache miss and cache hit results should be identical"
    );

    // Test with different allow_tests flag
    let result3 = parse_file_for_code_blocks(content, "rs", &line_numbers, false, None).unwrap();

    // Should be a different cache entry, but results should be similar since there are no tests
    // Check that result3 covers all requested lines
    let requested_lines: Vec<usize> = line_numbers.iter().cloned().collect(); // Define requested_lines here
    for &line in &requested_lines {
        let line_covered = result3
            .iter()
            .any(|block| line > block.start_row && line <= block.end_row + 1);
        assert!(
            line_covered,
            "Line {line} not covered by any block in result3"
        );
    }
}

#[test]
fn test_line_map_cache_with_different_content() {
    // Set up test content
    let content1 = r#"
fn test_function() {
    // This is a comment
    let x = 42;
}
"#;

    let content2 = r#"
fn test_function() {
    // This is a different comment
    let x = 100;
}
"#;

    // Create a set of line numbers to extract
    let mut line_numbers = HashSet::new();
    line_numbers.insert(3); // Comment line

    // First call with content1
    let result1 = parse_file_for_code_blocks(content1, "rs", &line_numbers, true, None).unwrap();

    // Second call with content2 (should be a cache miss due to different content)
    let result2 = parse_file_for_code_blocks(content2, "rs", &line_numbers, true, None).unwrap();

    // Results should be different (different comment text)
    assert_eq!(result1.len(), result2.len()); // Same number of blocks
    assert_eq!(result1[0].start_row, result2[0].start_row); // Same start row
    assert_eq!(result1[0].end_row, result2[0].end_row); // Same end row
    assert_eq!(result1[0].node_type, result2[0].node_type); // Same node type

    // But the content is different, which we can't directly test here since we don't have access to the content
    // In a real-world scenario, the extracted code blocks would contain different text
}
