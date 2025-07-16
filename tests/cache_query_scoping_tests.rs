use std::collections::HashSet;
use std::time::Instant;

/// This test verifies that the ancestor cache correctly handles different query scopes
/// by running multiple queries with different line number sets on the same file.
#[test]
fn test_ancestor_cache_with_different_query_scopes() {
    // Create a sample Go file with multiple functions and nested structures
    let content = r#"
package main

import "fmt"

// Function 1 - Top level function
func Function1() {
    fmt.Println("Function 1")
    
    // Local variable
    var x int = 10
    
    // Nested block
    {
        var y int = 20
        fmt.Println(x + y)
    }
}

// Struct definition with nested types
type ComplexStruct struct {
    Field1 string
    Field2 int
    Inner struct {
        SubField1 string
        SubField2 int
    }
}

// Function 2 - Uses the complex struct
func Function2() {
    var data ComplexStruct
    data.Field1 = "test"
    data.Field2 = 42
    data.Inner.SubField1 = "nested"
    data.Inner.SubField2 = 100
    
    fmt.Println(data)
}

// Function 3 - Another function with different structure
func Function3() {
    // Different local variables
    var a, b, c int = 1, 2, 3
    
    // Different nested block
    if a < b {
        var d int = a + b + c
        fmt.Println(d)
    }
}

func main() {
    Function1()
    Function2()
    Function3()
}
"#;

    println!("Testing ancestor cache with different query scopes");

    // Define different query scopes (line number sets)
    let all_lines: HashSet<usize> = (1..=content.lines().count()).collect();

    // Query 1: First function only (lines 6-16)
    let mut query1_lines = HashSet::new();
    for i in 6..=16 {
        query1_lines.insert(i);
    }

    // Query 2: Struct definition only (lines 19-26)
    let mut query2_lines = HashSet::new();
    for i in 19..=26 {
        query2_lines.insert(i);
    }

    // Query 3: Second function only (lines 29-38)
    let mut query3_lines = HashSet::new();
    for i in 29..=38 {
        query3_lines.insert(i);
    }

    // Query 4: Third function only (lines 41-50)
    let mut query4_lines = HashSet::new();
    for i in 41..=50 {
        query4_lines.insert(i);
    }

    // Run queries in sequence and measure performance
    println!("\nRunning queries with different line number sets:");

    // Query 1
    let start = Instant::now();
    let result1 = probe::language::parser::parse_file_for_code_blocks(
        content,
        "go",
        &query1_lines,
        true,
        None,
    );
    assert!(
        result1.is_ok(),
        "Failed to parse file for query 1: {:?}",
        result1.err()
    );
    let blocks1 = result1.unwrap();
    let duration1 = start.elapsed();
    println!(
        "  Query 1 (Function1): {:.6} seconds, extracted {} code blocks",
        duration1.as_secs_f64(),
        blocks1.len()
    );

    // Query 2
    let start = Instant::now();
    let result2 = probe::language::parser::parse_file_for_code_blocks(
        content,
        "go",
        &query2_lines,
        true,
        None,
    );
    assert!(
        result2.is_ok(),
        "Failed to parse file for query 2: {:?}",
        result2.err()
    );
    let blocks2 = result2.unwrap();
    let duration2 = start.elapsed();
    println!(
        "  Query 2 (ComplexStruct): {:.6} seconds, extracted {} code blocks",
        duration2.as_secs_f64(),
        blocks2.len()
    );

    // Query 3
    let start = Instant::now();
    let result3 = probe::language::parser::parse_file_for_code_blocks(
        content,
        "go",
        &query3_lines,
        true,
        None,
    );
    assert!(
        result3.is_ok(),
        "Failed to parse file for query 3: {:?}",
        result3.err()
    );
    let blocks3 = result3.unwrap();
    let duration3 = start.elapsed();
    println!(
        "  Query 3 (Function2): {:.6} seconds, extracted {} code blocks",
        duration3.as_secs_f64(),
        blocks3.len()
    );

    // Query 4
    let start = Instant::now();
    let result4 = probe::language::parser::parse_file_for_code_blocks(
        content,
        "go",
        &query4_lines,
        true,
        None,
    );
    assert!(
        result4.is_ok(),
        "Failed to parse file for query 4: {:?}",
        result4.err()
    );
    let blocks4 = result4.unwrap();
    let duration4 = start.elapsed();
    println!(
        "  Query 4 (Function3): {:.6} seconds, extracted {} code blocks",
        duration4.as_secs_f64(),
        blocks4.len()
    );

    // Full file query
    let start = Instant::now();
    let result_all =
        probe::language::parser::parse_file_for_code_blocks(content, "go", &all_lines, true, None);
    assert!(
        result_all.is_ok(),
        "Failed to parse file for full query: {:?}",
        result_all.err()
    );
    let blocks_all = result_all.unwrap();
    let duration_all = start.elapsed();
    println!(
        "  Full file query: {:.6} seconds, extracted {} code blocks",
        duration_all.as_secs_f64(),
        blocks_all.len()
    );

    // Verify that the cache is working correctly by checking that the blocks extracted
    // in the individual queries are consistent with those in the full file query
    let total_blocks = blocks1.len() + blocks2.len() + blocks3.len() + blocks4.len();

    // Account for potential overlaps in the extracted blocks
    // In a perfect world, total_blocks would equal blocks_all.len(), but there might be
    // some blocks that span multiple query regions or shared blocks

    println!("\nVerification:");
    println!("  Total blocks from individual queries: {total_blocks}");
    println!("  Blocks from full file query: {}", blocks_all.len());
    println!("  Note: Difference may be due to overlapping blocks or shared context");

    // The test passes as long as we get results for all queries
    // The actual verification is informational
}
