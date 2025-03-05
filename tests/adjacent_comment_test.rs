use anyhow::Result;
use std::collections::HashSet;
use tree_sitter::Parser as TSParser;
use code_search::language::parser::parse_file_for_code_blocks;

#[test]
fn test_search_comment_with_term() -> Result<()> {
    // Test code with multiple comments before function
    let js_code = r#"
function foo() {
    console.log("foo");
}

// comment line
// comment keyword 
function bar() {
    console.log("bar");
}
"#;
    
    // Create a HashSet of line numbers (line containing "keyword")
    let mut line_numbers = HashSet::new();
    line_numbers.insert(7); // Line with "comment keyword"
    
    // Parse the file for code blocks
    let result = parse_file_for_code_blocks(js_code, "js", &line_numbers, true, None)?;
    
    // We should get two code blocks:
    // 1. The comment line containing "keyword"
    // 2. The function that follows it
    assert_eq!(result.len(), 2, "Should find both comment and related function");
    
    // Check comment block
    assert_eq!(result[0].node_type, "comment", "First block should be the comment");
    assert_eq!(result[0].start_row, 6, "Comment should start at the correct line");
    
    // Check function block
    assert_eq!(result[1].node_type, "function_declaration", "Second block should be the function");
    assert_eq!(result[1].start_row, 7, "Function should start at the correct line");
    
    Ok(())
}

#[test]
fn test_search_comment_with_term_and_line_content() -> Result<()> {
    // Test code with multiple comments and functions
    let js_code = r#"
// First function
function foo() {
    console.log("foo");
}

// comment line
// comment keyword 
function bar() {
    console.log("keyword");  // Same term in function body
}
"#;
    
    // Create a HashSet of line numbers for both keyword occurrences
    let mut line_numbers = HashSet::new();
    line_numbers.insert(7); // Line with "comment keyword"
    line_numbers.insert(9); // Line with "keyword" in function body
    
    // Enable debug output
    std::env::set_var("DEBUG", "1");
    
    // Parse the file for code blocks
    let result = parse_file_for_code_blocks(js_code, "js", &line_numbers, true, None)?;
    
    // Print all blocks for debugging
    println!("\nFound code blocks:");
    for (i, block) in result.iter().enumerate() {
        println!("Block {}: type='{}', lines={}-{}",
            i,
            block.node_type,
            block.start_row + 1,
            block.end_row + 1
        );
    }
    
    // We should get three blocks:
    // 1. The first function (lines 2-4)
    // 2. The comment line (line 6)
    // 3. The function containing the keyword (line 8)
    assert_eq!(result.len(), 3, "Should find all three blocks");
    
    // Verify the blocks
    assert_eq!(result[0].node_type, "function_declaration", "First block should be the first function");
    assert_eq!(result[0].start_row, 2, "First function should start at line 2");
    
    assert_eq!(result[1].node_type, "comment", "Second block should be the comment");
    assert_eq!(result[1].start_row, 6, "Comment should be at line 6");
    
    assert_eq!(result[2].node_type, "function", "Third block should be the second function");
    assert_eq!(result[2].start_row, 8, "Second function should start at line 8");
    
    Ok(())
}
