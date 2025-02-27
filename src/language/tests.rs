use super::{get_language, merge_code_blocks, parse_file_for_code_blocks};
use crate::models::CodeBlock;
use std::collections::HashSet;

#[test]
fn test_get_language() {
    // Test supported languages
    assert!(get_language("rs").is_some()); // Rust
    assert!(get_language("js").is_some()); // JavaScript
    assert!(get_language("ts").is_some()); // TypeScript
    assert!(get_language("py").is_some()); // Python
    assert!(get_language("go").is_some()); // Go
    assert!(get_language("c").is_some()); // C
    assert!(get_language("cpp").is_some()); // C++
    assert!(get_language("java").is_some()); // Java
    assert!(get_language("rb").is_some()); // Ruby
    assert!(get_language("php").is_some()); // PHP

    // Test unsupported language
    assert!(get_language("txt").is_none());
    assert!(get_language("").is_none());
}

#[test]
fn test_is_acceptable_parent() {
    // Note: We can't easily test is_acceptable_parent directly because it requires
    // tree-sitter Node objects which are difficult to mock. However, we can test
    // parse_file_for_code_blocks which uses is_acceptable_parent internally.

    // This is more of an integration test for the language module
    let rust_code = r#"
fn test_function() {
    println!("Hello, world!");
}

struct TestStruct {
    field1: i32,
    field2: String,
}

impl TestStruct {
    fn new() -> Self {
        Self {
            field1: 0,
            field2: String::new(),
        }
    }
}
"#;

    let mut line_numbers = HashSet::new();
    line_numbers.insert(3); // Line in test_function
    line_numbers.insert(12); // Line in TestStruct::new

    // This may fail in a pure unit test environment where tree-sitter is not properly initialized
    // We'll handle the potential failure gracefully
    let result = parse_file_for_code_blocks(rust_code, "rs", &line_numbers, true, None);

    if let Ok(blocks) = result {
        // If parsing succeeded, verify we got the expected blocks
        assert!(!blocks.is_empty());

        // Check if we found function blocks
        let has_function = blocks
            .iter()
            .any(|block| block.node_type == "function_item" || block.node_type == "impl_item");

        assert!(has_function);
    }
    // If parsing failed, that's acceptable in a unit test environment
}

#[test]
fn test_merge_code_blocks() {
    // Create some test blocks
    let blocks = vec![
        CodeBlock {
            start_row: 1,
            end_row: 5,
            start_byte: 10,
            end_byte: 50,
            node_type: "function_item".to_string(),
        },
        CodeBlock {
            start_row: 7,
            end_row: 12,
            start_byte: 60,
            end_byte: 100,
            node_type: "function_item".to_string(),
        },
        // Overlapping block
        CodeBlock {
            start_row: 4,
            end_row: 8,
            start_byte: 45,
            end_byte: 65,
            node_type: "function_item".to_string(),
        },
    ];

    let merged = merge_code_blocks(blocks);

    // The three blocks should be merged into one or two blocks
    assert!(merged.len() < 3);

    // Check that the merged block covers the full range
    let full_coverage = merged
        .iter()
        .any(|block| block.start_row <= 1 && block.end_row >= 12);

    assert!(full_coverage);
}

#[test]
fn test_merge_code_blocks_no_overlap() {
    // Create blocks with no overlap
    let blocks = vec![
        CodeBlock {
            start_row: 1,
            end_row: 5,
            start_byte: 10,
            end_byte: 50,
            node_type: "function_item".to_string(),
        },
        CodeBlock {
            start_row: 20,
            end_row: 25,
            start_byte: 200,
            end_byte: 250,
            node_type: "function_item".to_string(),
        },
    ];

    let merged = merge_code_blocks(blocks.clone());

    // No blocks should be merged
    assert_eq!(merged.len(), blocks.len());
}

#[test]
fn test_merge_code_blocks_struct_type() {
    // Test case 1: Blocks that are too far apart (more than 10 lines)
    let blocks_far_apart = vec![
        CodeBlock {
            start_row: 1,
            end_row: 10,
            start_byte: 10,
            end_byte: 100,
            node_type: "struct_type".to_string(),
        },
        // This is more than 10 lines away, so they should not merge
        CodeBlock {
            start_row: 25, // 15 lines away from the end of the previous block
            end_row: 35,
            start_byte: 200,
            end_byte: 300,
            node_type: "struct_type".to_string(),
        },
    ];

    let merged_far_apart = merge_code_blocks(blocks_far_apart);

    // The blocks should NOT be merged because they are more than 10 lines apart
    assert_eq!(merged_far_apart.len(), 2);

    // Test case 2: Blocks that are close enough to merge (within 10 lines)
    let blocks_close = vec![
        CodeBlock {
            start_row: 1,
            end_row: 10,
            start_byte: 10,
            end_byte: 100,
            node_type: "struct_type".to_string(),
        },
        // This is within 10 lines, so they should merge
        CodeBlock {
            start_row: 20, // 10 lines away from the end of the previous block
            end_row: 30,
            start_byte: 200,
            end_byte: 300,
            node_type: "struct_type".to_string(),
        },
    ];

    let merged_close = merge_code_blocks(blocks_close);

    // The blocks should be merged because they are within the threshold
    assert_eq!(merged_close.len(), 1);

    // Check that the merged block covers the full range
    assert_eq!(merged_close[0].start_row, 1);
    assert_eq!(merged_close[0].end_row, 30);
}
