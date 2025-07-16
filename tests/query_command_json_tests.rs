use serde_json::Value;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

// Helper function to extract JSON from command output
fn extract_json_from_output(output: &str) -> &str {
    // Find the first occurrence of '{'
    if let Some(start_index) = output.find('{') {
        // Return the substring from the first '{' to the end
        &output[start_index..]
    } else {
        // If no '{' is found, return the original string
        output
    }
}

// Helper function to create test files
fn create_test_file(dir: &TempDir, filename: &str, content: &str) -> PathBuf {
    let file_path = dir.path().join(filename);
    let parent_dir = file_path.parent().unwrap();
    fs::create_dir_all(parent_dir).expect("Failed to create parent directories");
    let mut file = File::create(&file_path).expect("Failed to create test file");
    file.write_all(content.as_bytes())
        .expect("Failed to write test content");
    file_path
}

// Helper function to create a test directory structure with various test files
fn create_test_directory_structure(root_dir: &TempDir) {
    // Create a source directory
    let src_dir = root_dir.path().join("src");
    fs::create_dir(&src_dir).expect("Failed to create src directory");

    // Create Rust files with functions
    let rust_content = r#"
fn hello_world() {
    println!("Hello, world!");
}

fn add(a: i32, b: i32) -> i32 {
    a + b
}
"#;
    create_test_file(root_dir, "src/functions.rs", rust_content);

    // Create a JavaScript file with functions
    let js_content = r#"
function greet(name) {
    return `Hello, ${name}!`;
}

const multiply = (a, b) => a * b;
"#;
    create_test_file(root_dir, "src/functions.js", js_content);

    // Create a file with special characters
    let special_chars_content = r#"
// This file contains special characters: "quotes", 'apostrophes', <tags>, &ampersands
function escapeTest(input) {
    return input.replace(/[<>&"']/g, function(c) {
        return {
            '<': '&lt;',
            '>': '&gt;',
            '&': '&amp;',
            '"': '&quot;',
            "'": '&#39;'
        }[c];
    });
}
"#;
    create_test_file(root_dir, "src/special_chars.js", special_chars_content);

    // Create a Python file with functions
    let python_content = r#"
def calculate_sum(numbers):
    """Calculate the sum of a list of numbers."""
    return sum(numbers)

def process_data(data, callback):
    """Process data using the provided callback function."""
    return callback(data)
"#;
    create_test_file(root_dir, "src/functions.py", python_content);
}

#[test]
fn test_query_json_output_rust_functions() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Run the CLI with JSON output format
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "query",
            "fn $NAME($$$PARAMS) $$$BODY", // Pattern to search for Rust functions
            temp_dir.path().to_str().unwrap(),
            "--language",
            "rust",
            "--format",
            "json",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    assert!(output.status.success());

    // Convert stdout to string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Extract the JSON part from the output
    let json_str = extract_json_from_output(&stdout);

    // Parse the JSON output
    let json_result: Value = serde_json::from_str(json_str).expect("Failed to parse JSON output");

    // Validate the structure of the JSON output
    assert!(json_result.is_object(), "JSON output should be an object");
    assert!(
        json_result.get("results").is_some(),
        "JSON output should have a 'results' field"
    );
    assert!(
        json_result.get("summary").is_some(),
        "JSON output should have a 'summary' field"
    );

    // Validate the results array
    let results = json_result.get("results").unwrap().as_array().unwrap();
    assert!(!results.is_empty(), "'results' array should not be empty");
    assert_eq!(results.len(), 2, "Should find 2 Rust functions");

    // Validate the structure of each result
    for result in results {
        assert!(
            result.get("file").is_some(),
            "Each result should have a 'file' field"
        );
        assert!(
            result.get("lines").is_some(),
            "Each result should have a 'lines' field"
        );
        assert!(
            result.get("node_type").is_some(),
            "Each result should have a 'node_type' field"
        );
        assert!(
            result.get("column_start").is_some(),
            "Each result should have a 'column_start' field"
        );
        assert!(
            result.get("column_end").is_some(),
            "Each result should have a 'column_end' field"
        );
        assert!(
            result.get("code").is_some(),
            "Each result should have a 'code' field"
        );

        // Check that the code contains function code
        let code = result.get("code").unwrap().as_str().unwrap();
        assert!(
            code.starts_with("fn "),
            "Function code should start with 'fn '"
        );
    }

    // Validate the summary object
    let summary = json_result.get("summary").unwrap();
    assert!(summary.is_object(), "'summary' should be an object");
    assert!(
        summary.get("count").is_some(),
        "'summary' should have a 'count' field"
    );
    assert!(
        summary.get("total_bytes").is_some(),
        "'summary' should have a 'total_bytes' field"
    );
    assert!(
        summary.get("total_tokens").is_some(),
        "'summary' should have a 'total_tokens' field"
    );

    // Validate the count matches the number of results
    let count = summary.get("count").unwrap().as_u64().unwrap();
    let results_count = results.len() as u64;
    assert_eq!(
        count, results_count,
        "The 'count' in summary should match the number of results"
    );
}

#[test]
fn test_query_json_output_javascript_functions() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Run the CLI with JSON output format
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "query",
            "function $NAME($$$PARAMS) $$$BODY", // Pattern to search for JavaScript functions
            temp_dir.path().to_str().unwrap(),
            "--language",
            "javascript",
            "--format",
            "json",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    assert!(output.status.success());

    // Convert stdout to string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Extract the JSON part from the output
    let json_str = extract_json_from_output(&stdout);

    // Parse the JSON output
    let json_result: Value = serde_json::from_str(json_str).expect("Failed to parse JSON output");

    // Validate the results array
    let results = json_result.get("results").unwrap().as_array().unwrap();
    assert!(!results.is_empty(), "'results' array should not be empty");

    // Check that we found the JavaScript functions
    let has_greet = results
        .iter()
        .any(|r| r.get("code").unwrap().as_str().unwrap().contains("greet"));

    assert!(has_greet, "Should find the 'greet' function");
}

#[test]
fn test_query_json_output_with_special_characters() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Run the CLI with JSON output format, searching for the escape function
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "query",
            "function escapeTest", // Pattern to search for the escape function
            temp_dir.path().to_str().unwrap(),
            "--language",
            "javascript",
            "--format",
            "json",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    assert!(output.status.success());

    // Convert stdout to string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Extract the JSON part from the output
    let json_str = extract_json_from_output(&stdout);

    // Parse the JSON output
    let json_result: Value = serde_json::from_str(json_str).expect("Failed to parse JSON output");

    // Validate the results array
    let results = json_result.get("results").unwrap().as_array().unwrap();
    assert!(!results.is_empty(), "'results' array should not be empty");

    // Check that we found the escape function
    let escape_result = results.iter().find(|r| {
        r.get("code")
            .unwrap()
            .as_str()
            .unwrap()
            .contains("escapeTest")
    });

    assert!(
        escape_result.is_some(),
        "Should find the 'escapeTest' function"
    );

    // Verify that special characters are properly escaped in the JSON
    let code = escape_result
        .unwrap()
        .get("code")
        .unwrap()
        .as_str()
        .unwrap();

    // Print the code content for debugging
    println!("Code content: {code}");

    // Check for special characters in the function body
    // The function contains special characters in the string literals
    assert!(code.contains("&lt;"), "Should contain '&lt;'");
    assert!(code.contains("&gt;"), "Should contain '&gt;'");
    assert!(code.contains("&amp;"), "Should contain '&amp;'");
    assert!(code.contains("&quot;"), "Should contain '&quot;'");
    assert!(code.contains("&#39;"), "Should contain '&#39;'");
}

#[test]
fn test_query_json_output_with_multiple_languages() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Run the CLI with JSON output format, without specifying a language
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "query",
            "def $NAME($$$PARAMS): $$$BODY", // Pattern to search for Python functions
            temp_dir.path().to_str().unwrap(),
            "--language",
            "python",
            "--format",
            "json",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    assert!(output.status.success());

    // Convert stdout to string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Extract the JSON part from the output
    let json_str = extract_json_from_output(&stdout);

    // Parse the JSON output
    let json_result: Value = serde_json::from_str(json_str).expect("Failed to parse JSON output");

    // Validate the results array
    let results = json_result.get("results").unwrap().as_array().unwrap();
    assert!(!results.is_empty(), "'results' array should not be empty");

    // Check that we found Python functions
    let has_calculate_sum = results.iter().any(|r| {
        r.get("code")
            .unwrap()
            .as_str()
            .unwrap()
            .contains("calculate_sum")
    });

    let has_process_data = results.iter().any(|r| {
        r.get("code")
            .unwrap()
            .as_str()
            .unwrap()
            .contains("process_data")
    });

    assert!(
        has_calculate_sum,
        "Should find the 'calculate_sum' function"
    );
    assert!(has_process_data, "Should find the 'process_data' function");
}

#[test]
fn test_query_json_output_with_no_results() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Run the CLI with JSON output format, searching for a pattern that doesn't exist
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "query",
            "class $NAME { $$$METHODS }", // Pattern that doesn't match any file
            temp_dir.path().to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    assert!(output.status.success());

    // Convert stdout to string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Extract the JSON part from the output
    let json_str = extract_json_from_output(&stdout);

    // Parse the JSON output
    let json_result: Value = serde_json::from_str(json_str).expect("Failed to parse JSON output");

    // Validate the structure of the JSON output
    assert!(json_result.is_object(), "JSON output should be an object");
    assert!(
        json_result.get("results").is_some(),
        "JSON output should have a 'results' field"
    );
    assert!(
        json_result.get("summary").is_some(),
        "JSON output should have a 'summary' field"
    );

    // Validate the results array is empty
    let results = json_result.get("results").unwrap().as_array().unwrap();
    assert!(results.is_empty(), "'results' array should be empty");

    // Validate the summary object
    let summary = json_result.get("summary").unwrap();
    assert_eq!(
        summary.get("count").unwrap().as_u64().unwrap(),
        0,
        "'count' should be 0"
    );
    assert_eq!(
        summary.get("total_bytes").unwrap().as_u64().unwrap(),
        0,
        "'total_bytes' should be 0"
    );
    assert_eq!(
        summary.get("total_tokens").unwrap().as_u64().unwrap(),
        0,
        "'total_tokens' should be 0"
    );
}
