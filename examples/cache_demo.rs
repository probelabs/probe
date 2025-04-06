use probe::language::parser::parse_file_for_code_blocks;
use std::collections::HashSet;

fn main() {
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

    println!("First call (should be a cache miss):");
    let result1 = parse_file_for_code_blocks(content, "rs", &line_numbers, true, None).unwrap();
    println!("Found {} code blocks", result1.len());

    println!("\nSecond call (should be a cache hit):");
    let result2 = parse_file_for_code_blocks(content, "rs", &line_numbers, true, None).unwrap();
    println!("Found {} code blocks", result2.len());

    println!("\nThird call with different allow_tests flag (should be a cache miss):");
    let result3 = parse_file_for_code_blocks(content, "rs", &line_numbers, false, None).unwrap();
    println!("Found {} code blocks", result3.len());

    println!("\nFourth call with different content (should be a cache miss):");
    let content2 = r#"
fn different_function() {
    // This is a different comment
    let y = 100;
}
"#;
    let result4 = parse_file_for_code_blocks(content2, "rs", &line_numbers, true, None).unwrap();
    println!("Found {} code blocks", result4.len());
}
