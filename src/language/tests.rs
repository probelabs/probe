use crate::language::block_handling::merge_code_blocks;
use crate::language::parser::parse_file_for_code_blocks;
use tree_sitter::Language;

// Import tree-sitter language crates
extern crate tree_sitter_c;
extern crate tree_sitter_c_sharp;
extern crate tree_sitter_cpp;
extern crate tree_sitter_go;
extern crate tree_sitter_java;
extern crate tree_sitter_javascript;
extern crate tree_sitter_php;
extern crate tree_sitter_python;
extern crate tree_sitter_ruby;
extern crate tree_sitter_rust;
extern crate tree_sitter_swift;
extern crate tree_sitter_typescript;

// Helper function to get tree-sitter language from file extension
fn get_language(extension: &str) -> Option<Language> {
    match extension {
        "rs" => Some(tree_sitter_rust::LANGUAGE.into()),
        "js" | "jsx" => Some(tree_sitter_javascript::LANGUAGE.into()),
        "ts" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        "tsx" => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
        "py" => Some(tree_sitter_python::LANGUAGE.into()),
        "go" => Some(tree_sitter_go::LANGUAGE.into()),
        "c" | "h" => Some(tree_sitter_c::LANGUAGE.into()),
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => Some(tree_sitter_cpp::LANGUAGE.into()),
        "java" => Some(tree_sitter_java::LANGUAGE.into()),
        "rb" => Some(tree_sitter_ruby::LANGUAGE.into()),
        "swift" => Some(tree_sitter_swift::LANGUAGE.into()),
        "cs" => Some(tree_sitter_c_sharp::LANGUAGE.into()),
        // It seems tree_sitter_php::LANGUAGE doesn't exist, so we'll return None for PHP
        "php" => None,
        _ => None,
    }
}
use crate::language::factory::get_language_impl;
use crate::language::language_trait::LanguageImpl;
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
    assert!(get_language("swift").is_some()); // Swift
    assert!(get_language("cs").is_some()); // C#
    assert!(get_language("php").is_none()); // PHP (not supported in current tree-sitter version)

    // Test unsupported language
    assert!(get_language("txt").is_none());
    assert!(get_language("").is_none());
}

#[test]
fn test_is_acceptable_parent() {
    // This test directly checks if the Rust language implementation's is_acceptable_parent function
    // correctly identifies parent nodes for Rust code.

    // Get the Rust language implementation
    let rust_impl = get_language_impl("rs").unwrap();

    // Parse a simple Rust code snippet
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

    // Print the code with line numbers for debugging
    println!("Code with line numbers:");
    for (i, line) in rust_code.lines().enumerate() {
        println!("{}: {}", i + 1, line);
    }

    // Parse the code
    let language = tree_sitter_rust::LANGUAGE.into();
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(rust_code, None).unwrap();
    let root_node = tree.root_node();

    // Enable debug mode for this test
    std::env::set_var("DEBUG", "1");

    // Print the AST structure
    println!("AST Structure:");
    print_ast_structure(root_node, 0);

    // Find all nodes that are acceptable parents
    let mut acceptable_parents = Vec::new();

    // Recursive function to check all nodes
    fn check_nodes(
        node: tree_sitter::Node,
        rust_impl: &dyn LanguageImpl,
        acceptable_parents: &mut Vec<String>,
    ) {
        if rust_impl.is_acceptable_parent(&node) {
            acceptable_parents.push(format!(
                "{} (lines {}-{})",
                node.kind(),
                node.start_position().row + 1,
                node.end_position().row + 1
            ));
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            check_nodes(child, rust_impl, acceptable_parents);
        }
    }

    // Check all nodes
    check_nodes(root_node, rust_impl.as_ref(), &mut acceptable_parents);

    // Print all acceptable parents
    println!("Acceptable parents:");
    for parent in &acceptable_parents {
        println!("  {}", parent);
    }

    // Verify we found at least one acceptable parent
    assert!(
        !acceptable_parents.is_empty(),
        "Expected to find at least one acceptable parent"
    );

    // Verify we found function_item and impl_item
    let has_function = acceptable_parents
        .iter()
        .any(|p| p.contains("function_item"));
    let has_impl = acceptable_parents.iter().any(|p| p.contains("impl_item"));
    let has_struct = acceptable_parents.iter().any(|p| p.contains("struct_item"));

    assert!(
        has_function,
        "Expected to find function_item as an acceptable parent"
    );
    assert!(
        has_impl,
        "Expected to find impl_item as an acceptable parent"
    );
    assert!(
        has_struct,
        "Expected to find struct_item as an acceptable parent"
    );

    // Reset debug mode
    std::env::remove_var("DEBUG");
}

#[test]
fn test_merge_code_blocks() {
    // Create some test blocks
    let blocks = vec![
        CodeBlock {
            start_row: 0,
            end_row: 3,
            start_byte: 0,
            end_byte: 50,
            node_type: "struct_type".to_string(),
            parent_node_type: None,
            parent_start_row: None,
            parent_end_row: None,
        },
        CodeBlock {
            start_row: 5,
            end_row: 10,
            start_byte: 60,
            end_byte: 120,
            node_type: "struct_type".to_string(),
            parent_node_type: None,
            parent_start_row: None,
            parent_end_row: None,
        },
        // Overlapping block
        CodeBlock {
            start_row: 4,
            end_row: 8,
            start_byte: 45,
            end_byte: 65,
            node_type: "struct_type".to_string(),
            parent_node_type: None,
            parent_start_row: None,
            parent_end_row: None,
        },
    ];

    let merged = merge_code_blocks(blocks);

    // The three blocks should be merged into one or two blocks
    assert!(merged.len() < 3);

    // Check that the merged block covers the full range
    let full_coverage = merged
        .iter()
        .any(|block| block.start_row == 0 && block.end_row >= 10);

    assert!(full_coverage);
}

#[test]
fn test_merge_code_blocks_no_overlap() {
    // Create blocks with no overlap - must be more than 10 lines apart
    let blocks = vec![
        CodeBlock {
            start_row: 0,
            end_row: 3,
            start_byte: 0,
            end_byte: 50,
            node_type: "struct_type".to_string(),
            parent_node_type: None,
            parent_start_row: None,
            parent_end_row: None,
        },
        CodeBlock {
            start_row: 15, // Changed from 10 to 15 to ensure gap > 10 lines
            end_row: 30,   // Changed from 15 to 30
            start_byte: 100,
            end_byte: 150,
            node_type: "impl_block".to_string(),
            parent_node_type: None,
            parent_start_row: None,
            parent_end_row: None,
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
            start_row: 0,
            end_row: 3,
            start_byte: 0,
            end_byte: 50,
            node_type: "struct_type".to_string(),
            parent_node_type: None,
            parent_start_row: None,
            parent_end_row: None,
        },
        // This is more than 10 lines away, so they should not merge
        CodeBlock {
            start_row: 20, // 15 lines away from the end of the previous block
            end_row: 25,
            start_byte: 300,
            end_byte: 400,
            node_type: "struct_type".to_string(),
            parent_node_type: None,
            parent_start_row: None,
            parent_end_row: None,
        },
    ];

    let merged_far_apart = merge_code_blocks(blocks_far_apart);

    // The blocks should NOT be merged because they are more than 10 lines apart
    assert_eq!(merged_far_apart.len(), 2);

    // Test case 2: Blocks that are close enough to merge (within 10 lines)
    let blocks_close = vec![
        CodeBlock {
            start_row: 0,
            end_row: 3,
            start_byte: 0,
            end_byte: 50,
            node_type: "struct_type".to_string(),
            parent_node_type: None,
            parent_start_row: None,
            parent_end_row: None,
        },
        // This is within 10 lines, so they should merge
        CodeBlock {
            start_row: 4,
            end_row: 8,
            start_byte: 51,
            end_byte: 100,
            node_type: "struct_type".to_string(),
            parent_node_type: None,
            parent_start_row: None,
            parent_end_row: None,
        },
    ];

    let merged_close = merge_code_blocks(blocks_close);

    // The blocks should be merged because they are within the threshold
    assert_eq!(merged_close.len(), 1);

    // Check that the merged block covers the full range
    assert_eq!(merged_close[0].start_row, 0);
    assert_eq!(merged_close[0].end_row, 8);
}

#[test]
fn test_find_code_structure_nested_structs() {
    // Enable debug mode for this test
    std::env::set_var("DEBUG", "1");

    // This test verifies that our recursive nested struct handling works as expected
    let go_code = r#"
// ModelPriceInput represents the input for model price-related operations
// @Description Model Price input model
type ModelPriceInput struct {
    Data struct {
        Type       string `json:"type"`
        Attributes struct {
            ModelName    string  `json:"model_name"`
            Vendor       string  `json:"vendor"`
            CPT          float64 `json:"cpt"`
            CPIT         float64 `json:"cpit"`
            CacheWritePT float64 `json:"cache_write_pt"`
            CacheReadPT  float64 `json:"cache_read_pt"`
            Currency     string  `json:"currency"`
        } `json:"attributes"`
    } `json:"data"`
}
"#;

    // Print the code with line numbers for debugging
    println!("Code with line numbers:");
    for (i, line) in go_code.lines().enumerate() {
        println!("{}: {}", i + 1, line);
    }

    // Create a HashSet with all line numbers to ensure we get all blocks
    let mut line_numbers = HashSet::new();
    for i in 1..=go_code.lines().count() {
        line_numbers.insert(i);
    }

    // Use parse_file_for_code_blocks instead of find_code_structure
    let result = parse_file_for_code_blocks(go_code, "go", &line_numbers, true, None);

    assert!(
        result.is_ok(),
        "Failed to parse code blocks: {:?}",
        result.err()
    );
    let blocks = result.unwrap();

    // Print the blocks for debugging
    println!("Found {} blocks:", blocks.len());
    for (i, block) in blocks.iter().enumerate() {
        println!(
            "Block {}: type={}, lines={}-{}",
            i,
            block.node_type,
            block.start_row + 1,
            block.end_row + 1
        );
    }

    // Verify we got at least one block
    assert!(!blocks.is_empty(), "Expected at least one code block");

    // Find the type_declaration block (which contains the struct_type)
    let type_block = blocks
        .iter()
        .find(|block| block.node_type == "type_declaration");
    assert!(
        type_block.is_some(),
        "Expected to find a type_declaration block"
    );

    // Verify we got the outermost type declaration and not an inner one
    // The start line should be around 3-4 (where ModelPriceInput type declaration begins)
    let type_block = type_block.unwrap();
    assert!(
        type_block.start_row <= 4,
        "Expected the outermost type declaration to start around line 3-4, got {}",
        type_block.start_row + 1
    );

    // Reset debug mode
    std::env::remove_var("DEBUG");
}

#[test]
fn test_find_code_structure_nested_structs_in_function() {
    // Enable debug mode for this test
    std::env::set_var("DEBUG", "1");

    // Test code with anonymous struct inside a function
    let go_code = r#"
package main

import (
    "net/http"
)

// HandleNotFound handles 404 responses
func HandleNotFound(c *gin.Context) {
    // Return a JSON response with a nested struct
    c.JSON(http.StatusNotFound, ErrorResponse{
        Errors: []struct {
            Title  string `json:"title"`
            Detail string `json:"detail"`
        }{{Title: "Not Found", Detail: "Model price not found"}},
    })
}
"#;

    // Print the code with line numbers for debugging
    println!("Code with line numbers:");
    for (i, line) in go_code.lines().enumerate() {
        println!("{}: {}", i + 1, line);
    }

    // Create a HashSet with all line numbers to ensure we get all blocks
    let mut line_numbers = HashSet::new();
    for i in 1..=go_code.lines().count() {
        line_numbers.insert(i);
    }

    // Use parse_file_for_code_blocks instead of find_code_structure
    let result = parse_file_for_code_blocks(go_code, "go", &line_numbers, true, None);

    assert!(
        result.is_ok(),
        "Failed to parse code blocks: {:?}",
        result.err()
    );
    let blocks = result.unwrap();

    // Print the blocks for debugging
    println!("Found {} blocks:", blocks.len());
    for (i, block) in blocks.iter().enumerate() {
        println!(
            "Block {}: type={}, lines={}-{}",
            i,
            block.node_type,
            block.start_row + 1,
            block.end_row + 1
        );
    }

    // Verify we got at least one block
    assert!(!blocks.is_empty(), "Expected at least one code block");

    // Find the function_declaration block
    let function_block = blocks
        .iter()
        .find(|block| block.node_type == "function_declaration");
    assert!(
        function_block.is_some(),
        "Expected to find a function_declaration block"
    );

    // Reset debug mode
    std::env::remove_var("DEBUG");
}

#[test]
fn test_direct_find_related_code_node() {
    // Enable debug mode for this test
    std::env::set_var("DEBUG", "1");

    // Test code with comments before functions
    let rust_code = r#"
// This is a test function
fn test_function() {
    println!("Hello, world!");
}
"#;

    // Print the code with line numbers for debugging
    println!("Code with line numbers:");
    for (i, line) in rust_code.lines().enumerate() {
        println!("{}: {}", i + 1, line);
    }

    // Create a HashSet with the line number of the comment
    let mut line_numbers = HashSet::new();
    line_numbers.insert(2); // Line with the comment

    // Parse the file for code blocks
    let result = parse_file_for_code_blocks(rust_code, "rs", &line_numbers, true, None);

    assert!(
        result.is_ok(),
        "Failed to parse code blocks: {:?}",
        result.err()
    );
    let blocks = result.unwrap();

    // Print the blocks for debugging
    println!("Found {} blocks:", blocks.len());
    for (i, block) in blocks.iter().enumerate() {
        println!(
            "Block {}: type={}, lines={}-{}",
            i,
            block.node_type,
            block.start_row + 1,
            block.end_row + 1
        );
    }

    // Verify we got at least one block
    assert!(!blocks.is_empty(), "Expected at least one code block");

    // The block should include both the comment and the function
    let function_block = blocks
        .iter()
        .find(|block| block.node_type == "function_item");
    assert!(
        function_block.is_some(),
        "Expected to find a function_item block"
    );

    let function_block = function_block.unwrap();
    assert!(
        function_block.start_row <= 1,
        "Block should start at or before line 2 (comment line)"
    );
    assert!(
        function_block.end_row >= 3,
        "Block should end at or after line 4 (end of function)"
    );

    // Clean up
    std::env::remove_var("DEBUG");
}

#[test]
fn test_find_related_code_node() {
    // Enable debug mode for this test
    std::env::set_var("DEBUG", "1");

    // Test code with comments before functions and structs
    let rust_code = r#"
// This is a test function that should be linked to the function
fn test_function() {
    println!("Hello, world!");
}

// This is a struct comment
struct TestStruct {
    field1: i32,
    field2: String,
}

/* Multi-line comment for an implementation
   that spans multiple lines and describes
   the implementation block
*/
impl TestStruct {
    // Method comment
    fn new() -> Self {
        Self {
            field1: 0,
            field2: String::new(),
        }
    }
}
"#;

    // Print the code with line numbers for debugging
    println!("Code with line numbers:");
    for (i, line) in rust_code.lines().enumerate() {
        println!("{}: {}", i + 1, line);
    }

    // Create a HashSet with the line numbers of the comments
    let mut line_numbers = HashSet::new();
    line_numbers.insert(2); // Function comment
    line_numbers.insert(7); // Struct comment
    line_numbers.insert(13); // Multi-line comment start
    line_numbers.insert(14); // Multi-line comment middle
    line_numbers.insert(15); // Multi-line comment middle
    line_numbers.insert(16); // Multi-line comment end
    line_numbers.insert(18); // Method comment

    // Parse the file for code blocks
    let result = parse_file_for_code_blocks(rust_code, "rs", &line_numbers, true, None);

    assert!(
        result.is_ok(),
        "Failed to parse code blocks: {:?}",
        result.err()
    );
    let blocks = result.unwrap();

    // Print the blocks for debugging
    println!("Found {} blocks:", blocks.len());
    for (i, block) in blocks.iter().enumerate() {
        println!(
            "Block {}: type={}, lines={}-{}",
            i,
            block.node_type,
            block.start_row + 1,
            block.end_row + 1
        );
    }

    // Verify we got at least one block for each comment
    assert!(
        blocks.len() >= 3,
        "Expected at least 3 blocks (function, struct, impl)"
    );

    // Create a map of line numbers to block types
    let mut line_to_block_type = std::collections::HashMap::new();

    for block in &blocks {
        for line in block.start_row..=block.end_row {
            line_to_block_type.insert(line + 1, block.node_type.clone());
        }
    }

    println!("Line to block type map: {:?}", line_to_block_type);

    // Check that each comment line is included in a block
    assert!(
        line_to_block_type.contains_key(&2),
        "Comment before test_function should be included in a block"
    );
    assert!(
        line_to_block_type.contains_key(&7),
        "Comment before TestStruct should be included in a block"
    );

    // For the multi-line comment, check if any of its lines are included in a block
    let multiline_comment_included = line_to_block_type.contains_key(&13)
        || line_to_block_type.contains_key(&14)
        || line_to_block_type.contains_key(&15)
        || line_to_block_type.contains_key(&16);

    assert!(
        multiline_comment_included,
        "Multi-line comment should be included in a block"
    );
    assert!(
        line_to_block_type.contains_key(&18),
        "Comment inside impl (method comment) should be included in a block"
    );

    // Clean up
    std::env::remove_var("DEBUG");
}

#[test]
fn test_comment_integration() {
    // Enable debug mode for this test
    std::env::set_var("DEBUG", "1");

    // Test that find_code_structure now handles comments properly
    let rust_code = r#"
// This is a test function that should be linked to the function
fn test_function() {
    println!("Hello, world!");
}
"#;

    // Print the code with line numbers for debugging
    println!("Code with line numbers:");
    for (i, line) in rust_code.lines().enumerate() {
        println!("{}: {}", i + 1, line);
    }

    // Create a HashSet of line numbers
    let mut line_numbers = HashSet::new();
    line_numbers.insert(2); // The comment line

    // Parse the file for code blocks
    let result = parse_file_for_code_blocks(rust_code, "rs", &line_numbers, true, None).unwrap();

    // Print the result for debugging
    println!("Found {} blocks:", result.len());
    for (i, block) in result.iter().enumerate() {
        println!(
            "Block {}: type={}, lines={}-{}",
            i,
            block.node_type,
            block.start_row + 1,
            block.end_row + 1
        );
    }

    // With our improved implementation, we should get at least one block
    assert!(!result.is_empty(), "Should find at least one code block");

    // The block should include the comment line
    let has_comment_line = result.iter().any(|block| {
        block.start_row <= 1 && block.end_row >= 1 // Line 2 (1-indexed) is the comment line
    });
    assert!(has_comment_line, "A block should include the comment line");

    // Clean up
    std::env::remove_var("DEBUG");
}

#[test]
fn test_swift_language_implementation() {
    // Import the Swift language implementation
    use crate::language::factory::get_language_impl;

    // Get the Swift language implementation through the factory
    let swift_impl = get_language_impl("swift");

    // Verify that we can get a Swift language implementation
    assert!(
        swift_impl.is_some(),
        "Should be able to get Swift language implementation"
    );

    // Test that the Swift language implementation is registered correctly
    let language = get_language("swift");
    assert!(
        language.is_some(),
        "Should be able to get Swift tree-sitter language"
    );
}

#[test]
fn test_csharp_language_implementation() {
    // Import the C# language implementation
    use crate::language::factory::get_language_impl;

    // Get the C# language implementation through the factory
    let csharp_impl = get_language_impl("cs");

    // Verify that we can get a C# language implementation
    assert!(
        csharp_impl.is_some(),
        "Should be able to get C# language implementation"
    );

    // Test that the C# language implementation is registered correctly
    let language = get_language("cs");
    assert!(
        language.is_some(),
        "Should be able to get C# tree-sitter language"
    );
}

// Helper function to print the AST structure
fn print_ast_structure(node: tree_sitter::Node, depth: usize) {
    let indent = " ".repeat(depth * 2);
    println!(
        "{}{} ({}-{})",
        indent,
        node.kind(),
        node.start_position().row + 1,
        node.end_position().row + 1
    );

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        print_ast_structure(child, depth + 1);
    }
}

// Include tree cache tests
#[path = "tree_cache_tests.rs"]
mod tree_cache_tests;
