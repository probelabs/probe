use std::fs;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_outline_format_no_cross_file_interference() {
    // Create a temporary directory for test files
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let temp_path = temp_dir.path();

    // Create file1.rs with specific content and line numbers
    let file1_content = r#"// File 1 - Test file with overlapping line numbers
use std::collections::HashMap;

fn main() {
    println!("File 1 main function");
}

fn calculate_score(items: &[i32]) -> i32 {
    let mut total = 0;
    for item in items {
        total += item;
        if total > 100 {
            println!("Score is high: {}", total);
            break;
        }
        println!("Current score: {}", total);
    }
    total
}

fn process_data(data: Vec<String>) -> HashMap<String, usize> {
    let mut result = HashMap::new();
    for entry in data {
        let count = result.entry(entry).or_insert(0);
        *count += 1;
    }
    result
}

fn handle_request() {
    println!("Handling request in file 1");
    let data = vec!["test".to_string(), "example".to_string()];
    let processed = process_data(data);
    println!("Processed: {:?}", processed);
}

impl MyStruct {
    fn new() -> Self {
        MyStruct {
            value: 0,
        }
    }

    fn calculate(&self) -> i32 {
        self.value * 2
    }
}

struct MyStruct {
    value: i32,
}"#;

    // Create file2.rs with overlapping line numbers (same line structure)
    let file2_content = r#"// File 2 - Test file with overlapping line numbers
use std::vec::Vec;

fn main() {
    println!("File 2 main function");
}

fn calculate_average(numbers: &[f64]) -> f64 {
    let mut sum = 0.0;
    for num in numbers {
        sum += num;
        if sum > 1000.0 {
            println!("Sum is very large: {}", sum);
            return sum / numbers.len() as f64;
        }
        println!("Current sum: {}", sum);
    }
    sum / numbers.len() as f64
}

fn process_input(input: String) -> Vec<String> {
    let mut tokens = Vec::new();
    for word in input.split_whitespace() {
        tokens.push(word.to_string());
    }
    tokens
}

fn handle_response() {
    println!("Handling response in file 2");
    let input = "test example data".to_string();
    let tokens = process_input(input);
    println!("Tokens: {:?}", tokens);
}

impl DataProcessor {
    fn new() -> Self {
        DataProcessor {
            buffer: Vec::new(),
        }
    }

    fn calculate(&self) -> usize {
        self.buffer.len()
    }
}

struct DataProcessor {
    buffer: Vec<String>,
}"#;

    let file1_path = temp_path.join("file1.rs");
    let file2_path = temp_path.join("file2.rs");

    fs::write(&file1_path, file1_content).expect("Failed to write file1.rs");
    fs::write(&file2_path, file2_content).expect("Failed to write file2.rs");

    // Run probe with outline format
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "search",
            "calculate",
            temp_path.to_str().unwrap(),
            "--format",
            "outline",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("Failed to run probe");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Debug: print the output for troubleshooting
    if !output.status.success() {
        println!("Command failed with status: {}", output.status);
        println!("STDOUT:\n{}", stdout);
        println!("STDERR:\n{}", String::from_utf8_lossy(&output.stderr));
    }

    assert!(output.status.success(), "Command should succeed");

    // Check that both files show their complete function structures
    // File 1 should show both calculate_score and calculate methods
    assert!(
        stdout.contains("fn calculate_score(items: &[i32]) -> i32"),
        "File 1 should show calculate_score function. Output: {}",
        stdout
    );
    assert!(
        stdout.contains("fn calculate(&self) -> i32"),
        "File 1 should show calculate method. Output: {}",
        stdout
    );

    // File 2 should show both calculate_average and calculate methods (despite overlapping line numbers)
    assert!(
        stdout.contains("fn calculate_average(numbers: &[f64]) -> f64"),
        "File 2 should show calculate_average function. Output: {}",
        stdout
    );
    assert!(
        stdout.contains("fn calculate(&self) -> usize"),
        "File 2 should show calculate method with different signature. Output: {}",
        stdout
    );

    // Both files should appear in the output
    assert!(
        stdout.contains("file1.rs"),
        "Output should contain file1.rs"
    );
    assert!(
        stdout.contains("file2.rs"),
        "Output should contain file2.rs"
    );

    // Verify that both files show their complete implementations
    // File 1: calculate_score should show lines 8-18 (approximate)
    assert!(
        stdout.contains("8    fn calculate_score(items: &[i32]) -> i32"),
        "File 1 should show line 8 with calculate_score"
    );

    // File 2: calculate_average should show lines 8-18 (same line numbers as file1, but different content)
    assert!(
        stdout.contains("8    fn calculate_average(numbers: &[f64]) -> f64"),
        "File 2 should show line 8 with calculate_average"
    );

    // Ensure we have multiple results (at least 4: 2 functions + 2 methods)
    if let Some(found_line) = stdout.lines().find(|line| line.starts_with("Found")) {
        // Extract the number from "Found X search results"
        if let Some(num_str) = found_line.split_whitespace().nth(1) {
            if let Ok(count) = num_str.parse::<usize>() {
                assert!(
                    count >= 4,
                    "Should find at least 4 results. Found: {} results. Output: {}",
                    count,
                    stdout
                );
            }
        }
    } else {
        panic!("Could not find results count line in output: {}", stdout);
    }
}

#[test]
fn test_outline_format_ellipsis_per_file() {
    // Create a temporary directory for test files
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let temp_path = temp_dir.path();

    // Create files with println statements that will trigger ellipsis
    let file1_content = r#"fn main() {
    println!("File 1 main");
}

// Many lines to create gaps
// Line 6
// Line 7
// Line 8
// Line 9
// Line 10
// Line 11
// Line 12
// Line 13
// Line 14
// Line 15
// Line 16
// Line 17
// Line 18
// Line 19
// Line 20

fn another_function() {
    println!("File 1 another");
}"#;

    let file2_content = r#"fn main() {
    println!("File 2 main");
}

// Many lines to create gaps
// Line 6
// Line 7
// Line 8
// Line 9
// Line 10
// Line 11
// Line 12
// Line 13
// Line 14
// Line 15
// Line 16
// Line 17
// Line 18
// Line 19
// Line 20

fn another_function() {
    println!("File 2 another");
}"#;

    let file1_path = temp_path.join("file1.rs");
    let file2_path = temp_path.join("file2.rs");

    fs::write(&file1_path, file1_content).expect("Failed to write file1.rs");
    fs::write(&file2_path, file2_content).expect("Failed to write file2.rs");

    // Run probe with outline format
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "search",
            "println",
            temp_path.to_str().unwrap(),
            "--format",
            "outline",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("Failed to run probe");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "Command should succeed");

    // Each file should show ellipsis independently
    // Count the number of "..." (ellipsis) in the output
    let ellipsis_count = stdout.matches("...").count();

    // With 2 files, each having a gap, we should see at least 2 ellipsis
    assert!(
        ellipsis_count >= 2,
        "Should show ellipsis in both files independently. Found {} ellipsis. Output: {}",
        ellipsis_count,
        stdout
    );

    // Both files should appear
    assert!(
        stdout.contains("file1.rs"),
        "Output should contain file1.rs"
    );
    assert!(
        stdout.contains("file2.rs"),
        "Output should contain file2.rs"
    );

    // Both files should show their println statements
    assert!(
        stdout.contains("println!(\"File 1 main\")"),
        "Should show File 1 main println"
    );
    assert!(
        stdout.contains("println!(\"File 2 main\")"),
        "Should show File 2 main println"
    );
    assert!(
        stdout.contains("println!(\"File 1 another\")"),
        "Should show File 1 another println"
    );
    assert!(
        stdout.contains("println!(\"File 2 another\")"),
        "Should show File 2 another println"
    );
}
