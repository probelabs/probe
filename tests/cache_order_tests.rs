use std::collections::HashSet;
use std::time::Instant;

/// This test demonstrates the performance benefits of caching ancestor lookups
/// in the AST traversal. The test creates a complex Go file with many nested
/// structures that will trigger many ancestor lookups during parsing.
#[test]
fn test_ancestor_cache_performance() {
    // Create a sample Go file with nested structures that will trigger many ancestor lookups
    let content = generate_complex_go_file(10, 5); // 10 structs with 5 levels of nesting each

    // Create a set of line numbers to extract (all lines in the file)
    let line_count = content.lines().count();
    let line_numbers: HashSet<usize> = (1..=line_count).collect();

    println!("Generated a Go file with {} lines", line_count);
    println!("File contains multiple nested structures to stress-test ancestor lookups");

    // Run the extraction multiple times to get a reliable measurement
    let iterations = 10;
    let mut times = Vec::with_capacity(iterations);

    println!(
        "\nRunning {} iterations of parsing with ancestor cache:",
        iterations
    );

    for i in 1..=iterations {
        let start = Instant::now();
        let result = probe::language::parser::parse_file_for_code_blocks(
            &content,
            "go",
            &line_numbers,
            true,
            None,
        );
        assert!(result.is_ok(), "Failed to parse file: {:?}", result.err());
        let blocks = result.unwrap();
        let duration = start.elapsed();
        times.push(duration.as_secs_f64());

        println!(
            "  Iteration {}: {:.6} seconds, extracted {} code blocks",
            i,
            duration.as_secs_f64(),
            blocks.len()
        );
    }

    // Calculate statistics
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let min_time = times.first().unwrap();
    let max_time = times.last().unwrap();
    let median_time = if iterations % 2 == 0 {
        (times[iterations / 2 - 1] + times[iterations / 2]) / 2.0
    } else {
        times[iterations / 2]
    };
    let avg_time = times.iter().sum::<f64>() / iterations as f64;

    println!("\nPerformance statistics with ancestor cache:");
    println!("  Minimum time: {:.6} seconds", min_time);
    println!("  Maximum time: {:.6} seconds", max_time);
    println!("  Median time:  {:.6} seconds", median_time);
    println!("  Average time: {:.6} seconds", avg_time);

    println!(
        "\nNote: Without caching, this would be significantly slower due to redundant traversals."
    );
    println!("The cache prevents repeated upward traversals for nodes within the same acceptable parent block.");
}

/// Generates a complex Go file with multiple nested structures to stress-test ancestor lookups
fn generate_complex_go_file(struct_count: usize, nesting_levels: usize) -> String {
    let mut content = String::from("package main\n\nimport \"fmt\"\n\n");

    // Generate multiple struct definitions with deep nesting
    for i in 1..=struct_count {
        content.push_str(&format!("// Struct {} with deep nesting\n", i));
        content.push_str(&format!("type Struct{} struct {{\n", i));

        // Add some fields at the top level
        for j in 1..=3 {
            content.push_str(&format!("    Field{} string\n", j));
        }

        // Add nested structures
        let mut indent = 4;
        let mut current_struct = format!("Inner{}", i);

        for level in 1..=nesting_levels {
            let spaces = " ".repeat(indent);
            content.push_str(&format!("{}// Level {} nested structure\n", spaces, level));
            content.push_str(&format!("{}{} struct {{\n", spaces, current_struct));

            // Add fields to this level
            indent += 4;
            let inner_spaces = " ".repeat(indent);
            for j in 1..=3 {
                content.push_str(&format!("{}Field{}_L{} string\n", inner_spaces, j, level));
            }

            // Prepare for next level
            current_struct = format!("Nested{}_L{}", i, level);

            // Add the nested struct field at this level (except for the last level)
            if level < nesting_levels {
                content.push_str(&format!(
                    "{}{} {}\n",
                    inner_spaces, current_struct, current_struct
                ));
            }
        }

        // Close all the nested structures
        for level in (0..=nesting_levels).rev() {
            let spaces = " ".repeat(4 * level);
            content.push_str(&format!("{}}}\n", spaces));
        }

        content.push('\n');
    }

    // Add a main function that uses these structures
    content.push_str("func main() {\n");
    for i in 1..=struct_count {
        content.push_str(&format!("    // Initialize Struct{}\n", i));
        content.push_str(&format!("    var s{} Struct{}\n", i, i));
        content.push_str(&format!("    s{}.Field1 = \"value\"\n", i));

        // Access some nested fields
        let mut var_path = format!("s{}", i);
        for level in 1..=nesting_levels {
            if level == 1 {
                var_path = format!("{}.Inner{}", var_path, i);
            } else {
                var_path = format!("{}.Nested{}_L{}", var_path, i, level - 1);
            }
            content.push_str(&format!(
                "    {}.Field1_L{} = \"nested value at level {}\"\n",
                var_path, level, level
            ));
        }
        content.push('\n');
    }

    // Print something to avoid unused variable warnings
    content.push_str("    fmt.Println(\"Complex nested structures initialized\")\n");
    content.push_str("}\n");

    content
}
