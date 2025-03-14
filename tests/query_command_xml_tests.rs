use roxmltree::{Document, Node};
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

// Helper function to extract XML from command output
fn extract_xml_from_output(output: &str) -> &str {
    // Find the first occurrence of '<?xml'
    if let Some(start_index) = output.find("<?xml") {
        // Return the substring from the first '<?xml' to the end
        &output[start_index..]
    } else {
        // If no '<?xml' is found, return the original string
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
fn test_query_xml_output_rust_functions() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Run the CLI with XML output format
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
            "xml",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    assert!(output.status.success());

    // Convert stdout to string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Extract the XML part from the output
    let xml_str = extract_xml_from_output(&stdout);

    // Parse the XML output
    let doc = Document::parse(xml_str).expect("Failed to parse XML output");
    let root = doc.root_element();

    // Validate the structure of the XML output
    assert_eq!(
        root.tag_name().name(),
        "probe_results",
        "Root element should be 'probe_results'"
    );

    // Validate that there are result elements
    let matches: Vec<Node> = root
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "result")
        .collect();
    assert!(
        !matches.is_empty(),
        "Should have at least one match element"
    );
    assert_eq!(matches.len(), 2, "Should find 2 Rust functions");

    // Validate the summary element
    let summary = root
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "summary");
    assert!(summary.is_some(), "Should have a summary element");

    if let Some(summary) = summary {
        // Validate the summary contains count, total_bytes, and total_tokens
        let count = summary
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "count");
        assert!(count.is_some(), "Summary should have a count element");

        let total_bytes = summary
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "total_bytes");
        assert!(
            total_bytes.is_some(),
            "Summary should have a total_bytes element"
        );

        let total_tokens = summary
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "total_tokens");
        assert!(
            total_tokens.is_some(),
            "Summary should have a total_tokens element"
        );

        // Validate the count matches the number of matches
        if let Some(count) = count {
            let count_value = count.text().unwrap_or("0").parse::<usize>().unwrap_or(0);
            assert_eq!(
                count_value,
                matches.len(),
                "Count should match the number of matches"
            );
        }
    }

    // Validate the structure of each match
    for match_node in matches {
        // Check for required elements
        let file = match_node
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "file");
        assert!(file.is_some(), "Each match should have a file element");

        let lines = match_node
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "lines");
        assert!(lines.is_some(), "Each result should have a lines element");

        let node_type = match_node
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "node_type");
        assert!(
            node_type.is_some(),
            "Each result should have a node_type element"
        );

        let column_start = match_node
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "column_start");
        assert!(
            column_start.is_some(),
            "Each result should have a column_start element"
        );

        let column_end = match_node
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "column_end");
        assert!(
            column_end.is_some(),
            "Each result should have a column_end element"
        );

        let code = match_node
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "code");
        assert!(code.is_some(), "Each result should have a code element");

        // Validate that code element contains function code
        if let Some(code) = code {
            let code_content = code.text();
            assert!(
                code_content.is_some(),
                "Code element should have text content"
            );
            assert!(
                code_content.unwrap().starts_with("fn "),
                "Function code should start with 'fn '"
            );
        }
    }
}

#[test]
fn test_query_xml_output_javascript_functions() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Run the CLI with XML output format
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
            "xml",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    assert!(output.status.success());

    // Convert stdout to string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Extract the XML part from the output
    let xml_str = extract_xml_from_output(&stdout);

    // Parse the XML output
    let doc = Document::parse(xml_str).expect("Failed to parse XML output");
    let root = doc.root_element();

    // Validate that there are result elements
    let matches: Vec<Node> = root
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "result")
        .collect();
    assert!(
        !matches.is_empty(),
        "Should have at least one result element"
    );

    // Check that we found the JavaScript functions
    let has_greet = matches.iter().any(|m| {
        if let Some(code) = m
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "code")
        {
            if let Some(content) = code.text() {
                return content.contains("greet");
            }
        }
        false
    });

    assert!(has_greet, "Should find the 'greet' function");
}

#[test]
fn test_query_xml_output_with_special_characters() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Run the CLI with XML output format, searching for the escape function
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
            "xml",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    assert!(output.status.success());

    // Convert stdout to string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Extract the XML part from the output
    let xml_str = extract_xml_from_output(&stdout);

    // Parse the XML output
    let doc = Document::parse(xml_str).expect("Failed to parse XML output");
    let root = doc.root_element();

    // Validate that there are result elements
    let matches: Vec<Node> = root
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "result")
        .collect();
    assert!(
        !matches.is_empty(),
        "Should have at least one result element"
    );

    // Find the result with special characters
    let escape_match = matches.iter().find(|&m| {
        if let Some(code) = m
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "code")
        {
            if let Some(content) = code.text() {
                return content.contains("escapeTest");
            }
        }
        false
    });

    assert!(
        escape_match.is_some(),
        "Should find the 'escapeTest' function"
    );

    // Verify that special characters are properly handled in the XML
    if let Some(match_node) = escape_match {
        if let Some(code) = match_node
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "code")
        {
            if let Some(content) = code.text() {
                // Check for special characters in the function body
                // The function contains special characters in the string literals
                assert!(content.contains("&lt;"), "Should contain '&lt;'");
                assert!(content.contains("&gt;"), "Should contain '&gt;'");
                assert!(content.contains("&amp;"), "Should contain '&amp;'");
                assert!(content.contains("&quot;"), "Should contain '&quot;'");
                assert!(content.contains("&#39;"), "Should contain '&#39;'");
            } else {
                panic!("Code element should have content");
            }
        } else {
            panic!("Result should have a code element");
        }

        // Check that file path is properly escaped
        if let Some(file) = match_node
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "file")
        {
            if let Some(text) = file.text() {
                // The XML should parse correctly, which means special characters in attributes are escaped
                assert!(
                    text.contains("special_chars.js"),
                    "File path should be correct"
                );
            } else {
                panic!("File element should have text content");
            }
        } else {
            panic!("Result should have a file element");
        }
    }
}

#[test]
fn test_query_xml_output_with_multiple_languages() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Run the CLI with XML output format, searching for Python functions
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
            "xml",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    assert!(output.status.success());

    // Convert stdout to string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Extract the XML part from the output
    let xml_str = extract_xml_from_output(&stdout);

    // Parse the XML output
    let doc = Document::parse(xml_str).expect("Failed to parse XML output");
    let root = doc.root_element();

    // Validate that there are result elements
    let matches: Vec<Node> = root
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "result")
        .collect();
    assert!(
        !matches.is_empty(),
        "Should have at least one result element"
    );

    // Check that we found Python functions
    let has_calculate_sum = matches.iter().any(|m| {
        if let Some(code) = m
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "code")
        {
            if let Some(content) = code.text() {
                return content.contains("calculate_sum");
            }
        }
        false
    });

    let has_process_data = matches.iter().any(|m| {
        if let Some(code) = m
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "code")
        {
            if let Some(content) = code.text() {
                return content.contains("process_data");
            }
        }
        false
    });

    assert!(
        has_calculate_sum,
        "Should find the 'calculate_sum' function"
    );
    assert!(has_process_data, "Should find the 'process_data' function");
}

#[test]
fn test_query_xml_output_with_no_results() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Run the CLI with XML output format, searching for a pattern that doesn't exist
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "query",
            "class $NAME { $$$METHODS }", // Pattern that doesn't match any file
            temp_dir.path().to_str().unwrap(),
            "--format",
            "xml",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    assert!(output.status.success());

    // Convert stdout to string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Extract the XML part from the output
    let xml_str = extract_xml_from_output(&stdout);

    // Parse the XML output
    let doc = Document::parse(xml_str).expect("Failed to parse XML output");
    let root = doc.root_element();

    // Validate the structure of the XML output
    assert_eq!(
        root.tag_name().name(),
        "probe_results",
        "Root element should be 'probe_results'"
    );

    // Validate that there are no result elements
    let matches: Vec<Node> = root
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "result")
        .collect();
    assert!(matches.is_empty(), "Should have no result elements");

    // Validate the summary element
    let summary = root
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "summary");
    assert!(summary.is_some(), "Should have a summary element");

    if let Some(summary) = summary {
        // Validate the summary contains count, total_bytes, and total_tokens with zero values
        if let Some(count) = summary
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "count")
        {
            let count_value = count.text().unwrap_or("0").parse::<usize>().unwrap_or(0);
            assert_eq!(count_value, 0, "Count should be 0");
        } else {
            panic!("Summary should have a count element");
        }

        if let Some(total_bytes) = summary
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "total_bytes")
        {
            let total_bytes_value = total_bytes
                .text()
                .unwrap_or("0")
                .parse::<usize>()
                .unwrap_or(0);
            assert_eq!(total_bytes_value, 0, "Total bytes should be 0");
        } else {
            panic!("Summary should have a total_bytes element");
        }

        if let Some(total_tokens) = summary
            .children()
            .find(|n| n.is_element() && n.tag_name().name() == "total_tokens")
        {
            let total_tokens_value = total_tokens
                .text()
                .unwrap_or("0")
                .parse::<usize>()
                .unwrap_or(0);
            assert_eq!(total_tokens_value, 0, "Total tokens should be 0");
        } else {
            panic!("Summary should have a total_tokens element");
        }
    }
}
