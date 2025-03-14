use std::fs;
use std::process::Command;

// Import the necessary functions from the extract module
use probe::extract::{format_and_print_extraction_results, process_file_for_extraction};

#[test]
fn test_process_file_for_extraction_full_file() {
    // Create a temporary file for testing
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test_file.txt");
    let content = "Line 1\nLine 2\nLine 3\n";
    fs::write(&file_path, content).unwrap();

    // Test processing the full file
    let result = process_file_for_extraction(&file_path, None, None, None, false, 0).unwrap();

    assert_eq!(result.file, file_path.to_string_lossy().to_string());
    assert_eq!(result.lines, (1, 3)); // 3 lines in the content
    assert_eq!(result.node_type, "file");
    assert_eq!(result.code, content);

    // Test with non-existent file
    let non_existent = temp_dir.path().join("non_existent.txt");
    let err = process_file_for_extraction(&non_existent, None, None, None, false, 0).unwrap_err();
    assert!(err.to_string().contains("does not exist"));
}

#[test]
fn test_process_file_for_extraction_with_line() {
    // Create a temporary file with Rust code for testing
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test_file.rs");
    let content = r#"
fn main() {
    println!("Hello, world!");
    
    let x = 42;
    if x > 0 {
        println!("Positive");
    } else {
        println!("Non-positive");
    }
}

struct Point {
    x: i32,
    y: i32,
}

impl Point {
    fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}
"#;
    fs::write(&file_path, content).unwrap();

    // Test extracting a function
    let result = process_file_for_extraction(&file_path, Some(3), None, None, false, 0).unwrap();
    assert_eq!(result.file, file_path.to_string_lossy().to_string());
    assert!(result.lines.0 <= 3 && result.lines.1 >= 3);
    assert!(result.code.contains("fn main()"));
    assert!(result.code.contains("Hello, world!"));

    // Test extracting a struct
    let result = process_file_for_extraction(&file_path, Some(13), None, None, false, 0).unwrap();
    assert_eq!(result.file, file_path.to_string_lossy().to_string());
    assert!(result.lines.0 <= 13 && result.lines.1 >= 13);
    assert!(result.code.contains("struct Point"));
    assert!(result.code.contains("x: i32"));
    assert!(result.code.contains("y: i32"));

    // Test with out-of-bounds line number
    let err =
        process_file_for_extraction(&file_path, Some(1000), None, None, false, 0).unwrap_err();
    assert!(err.to_string().contains("out of bounds"));
}

#[test]
fn test_process_file_for_extraction_fallback() {
    // Create a temporary file with a non-supported extension
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test_file.xyz");
    let mut content = String::from(
        "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\nLine 6\nLine 7\nLine 8\nLine 9\nLine 10\n",
    );
    content.push_str("Line 11\nLine 12\nLine 13\nLine 14\nLine 15\nLine 16\nLine 17\nLine 18\nLine 19\nLine 20\n");
    content.push_str("Line 21\nLine 22\nLine 23\nLine 24\nLine 25\n");
    fs::write(&file_path, content).unwrap();

    // Test fallback to line-based context with default context lines (10)
    let result = process_file_for_extraction(&file_path, Some(15), None, None, false, 10).unwrap();
    assert_eq!(result.file, file_path.to_string_lossy().to_string());
    assert_eq!(result.node_type, "context");

    // Should include 10 lines before and after line 15
    let start_line = result.lines.0;
    let end_line = result.lines.1;

    // Check that the range includes line 15 and has appropriate context
    assert!(start_line <= 15 && end_line >= 15);
    assert!(end_line - start_line >= 10); // At least 10 lines of context

    // Test with a line at the beginning of the file
    let result = process_file_for_extraction(&file_path, Some(2), None, None, false, 10).unwrap();
    assert!(result.lines.0 <= 2); // Should start at or before line 2
    assert!(result.lines.1 >= 2); // Should include line 2

    // Test with a line at the end of the file
    let result = process_file_for_extraction(&file_path, Some(25), None, None, false, 10).unwrap();
    assert!(result.lines.0 <= 25); // Should include some lines before line 25
    assert_eq!(result.lines.1, 25); // Can't go beyond the last line

    // Test with custom context lines
    let result = process_file_for_extraction(&file_path, Some(15), None, None, false, 5).unwrap();
    assert_eq!(result.file, file_path.to_string_lossy().to_string());
    assert_eq!(result.node_type, "context");

    // Should include 5 lines before and after line 15
    let start_line = result.lines.0;
    let end_line = result.lines.1;

    // Check that the range includes line 15 and has appropriate context
    assert!(start_line <= 15 && end_line >= 15);
    assert!(end_line - start_line >= 5); // At least 5 lines of context
    assert!(end_line - start_line <= 11); // At most 11 lines (5 before, 5 after, plus the line itself)
}

#[test]
fn test_format_and_print_extraction_results() {
    // Create a simple search result
    let result = probe::models::SearchResult {
        file: "test_file.rs".to_string(),
        lines: (1, 5),
        node_type: "function".to_string(),
        code: "fn test() {\n    println!(\"Hello\");\n}".to_string(),
        matched_by_filename: None,
        rank: None,
        score: None,
        tfidf_score: None,
        bm25_score: None,
        tfidf_rank: None,
        bm25_rank: None,
        new_score: None,
        hybrid2_rank: None,
        combined_score_rank: None,
        file_unique_terms: None,
        file_total_matches: None,
        file_match_rank: None,
        block_unique_terms: None,
        block_total_matches: None,
        parent_file_id: None,
        block_id: None,
        matched_keywords: None,
    };

    // Test different formats
    let results = vec![result];

    // We can't easily test the output directly, but we can at least ensure the function doesn't panic
    format_and_print_extraction_results(&results, "terminal").unwrap();
    format_and_print_extraction_results(&results, "markdown").unwrap();
    format_and_print_extraction_results(&results, "plain").unwrap();
    format_and_print_extraction_results(&results, "json").unwrap();
    format_and_print_extraction_results(&results, "xml").unwrap();
}

#[test]
fn test_json_format_extraction_results() {
    use serde_json::Value;
    use std::process::Command;
    use tempfile::TempDir;

    // Create a temporary file for testing
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let file_path = temp_dir.path().join("test_file.rs");
    let content = r#"
fn test() {
    println!("Hello");
}
"#;
    fs::write(&file_path, content).unwrap();

    // Run the extract command with JSON format
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "extract",
            file_path.to_string_lossy().as_ref(),
            "--format",
            "json",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command executed successfully
    assert!(output.status.success());

    // Get the output as a string
    let stdout = String::from_utf8_lossy(&output.stdout);

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

    // Extract and parse the JSON
    let json_str = extract_json_from_output(&stdout);
    let json_value: Value = serde_json::from_str(json_str).expect("Failed to parse JSON output");

    // Validate the structure of the JSON output
    assert!(json_value.is_object(), "JSON output should be an object");
    assert!(
        json_value.get("results").is_some(),
        "JSON output should have a 'results' field"
    );
    assert!(
        json_value.get("summary").is_some(),
        "JSON output should have a 'summary' field"
    );

    // Validate the results array
    let results_array = json_value.get("results").unwrap().as_array().unwrap();
    assert_eq!(
        results_array.len(),
        1,
        "Results array should have 1 element"
    );

    // Validate the structure of the result
    let result_obj = &results_array[0];
    assert!(
        result_obj.get("file").is_some(),
        "Result should have a 'file' field"
    );
    assert!(
        result_obj.get("lines").is_some(),
        "Result should have a 'lines' field"
    );
    assert!(
        result_obj.get("node_type").is_some(),
        "Result should have a 'node_type' field"
    );
    assert!(
        result_obj.get("code").is_some(),
        "Result should have a 'code' field"
    );

    // Validate the values
    assert!(result_obj
        .get("file")
        .unwrap()
        .as_str()
        .unwrap()
        .contains("test_file.rs"));
    assert!(
        result_obj.get("node_type").is_some(),
        "Result should have a node_type field"
    );
    assert!(
        result_obj
            .get("code")
            .unwrap()
            .as_str()
            .unwrap()
            .contains("fn test()")
            && result_obj
                .get("code")
                .unwrap()
                .as_str()
                .unwrap()
                .contains("println!(\"Hello\")")
    );

    // Validate the lines field is an array with two elements
    let lines = result_obj.get("lines").unwrap().as_array().unwrap();
    assert_eq!(lines.len(), 2, "Lines should be an array with 2 elements");
    assert!(
        lines[0].as_u64().unwrap() >= 1,
        "Start line should be at least 1"
    );
    assert!(
        lines[1].as_u64().unwrap() >= lines[0].as_u64().unwrap(),
        "End line should be at least start line"
    );

    // Validate the summary object
    let summary = json_value.get("summary").unwrap();
    assert!(summary.is_object(), "Summary should be an object");
    assert!(
        summary.get("count").is_some(),
        "Summary should have a 'count' field"
    );
    assert!(
        summary.get("total_bytes").is_some(),
        "Summary should have a 'total_bytes' field"
    );
    assert!(
        summary.get("total_tokens").is_some(),
        "Summary should have a 'total_tokens' field"
    );

    // Validate the count matches the number of results
    assert_eq!(summary.get("count").unwrap().as_u64().unwrap(), 1);
}

#[test]
fn test_xml_format_extraction_results() {
    use roxmltree::{Document, Node};
    use std::process::Command;
    use tempfile::TempDir;

    // Create a temporary file for testing
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let file_path = temp_dir.path().join("test_file.rs");
    let content = r#"
fn test() {
    println!("Hello");
}
"#;
    fs::write(&file_path, content).unwrap();

    // Run the extract command with XML format
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "extract",
            file_path.to_string_lossy().as_ref(),
            "--format",
            "xml",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command executed successfully
    assert!(output.status.success());

    // Get the output as a string
    let stdout = String::from_utf8_lossy(&output.stdout);

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

    // Extract and parse the XML
    let xml_str = extract_xml_from_output(&stdout);
    let doc = Document::parse(xml_str).expect("Failed to parse XML output");
    let root = doc.root_element();

    // Validate the structure of the XML output
    assert_eq!(
        root.tag_name().name(),
        "probe_results",
        "Root element should be 'probe_results'"
    );

    // Validate that there are result elements
    let results_nodes: Vec<Node> = root
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "result")
        .collect();
    assert_eq!(
        results_nodes.len(),
        1,
        "Should have exactly one result element"
    );

    // Validate the structure of the result
    let result_node = &results_nodes[0];

    // Check for required elements
    let file = result_node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "file");
    assert!(file.is_some(), "Result should have a file element");
    assert!(
        file.unwrap().text().unwrap().contains("test_file.rs"),
        "File element should contain the correct file path"
    );

    // For a full file extraction, the lines element might not be present
    let node_type = result_node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "node_type");
    if let Some(node_type) = node_type {
        if node_type.text().unwrap() != "file" {
            let lines = result_node
                .children()
                .find(|n| n.is_element() && n.tag_name().name() == "lines");
            assert!(lines.is_some(), "Result should have a lines element");
        }
    }

    // The node_type element might not be present for all extraction types
    let _node_type = result_node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "node_type");

    let code = result_node
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "code");
    assert!(code.is_some(), "Result should have a code element");
    assert!(
        code.unwrap().text().unwrap().contains("fn test()")
            && code
                .unwrap()
                .text()
                .unwrap()
                .contains("println!(\"Hello\")"),
        "Code element should contain the correct code"
    );

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
        assert_eq!(count.unwrap().text().unwrap(), "1", "Count should be 1");

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
    }
}

#[test]
fn test_process_file_for_extraction_with_range() {
    // Create a temporary file for testing
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test_file.txt");
    let mut content = String::new();
    for i in 1..=20 {
        content.push_str(&format!("Line {}\n", i));
    }
    fs::write(&file_path, &content).unwrap();

    // Test extracting a range of lines
    let result =
        process_file_for_extraction(&file_path, Some(1), Some(10), None, false, 0).unwrap();
    assert_eq!(result.file, file_path.to_string_lossy().to_string());
    assert_eq!(result.lines, (1, 10));
    assert_eq!(result.node_type, "range");

    // Check that the extracted content contains exactly lines 1-10
    let expected_content = content.lines().take(10).collect::<Vec<_>>().join("\n");
    assert_eq!(result.code, expected_content);

    // Test with a different range
    let result =
        process_file_for_extraction(&file_path, Some(5), Some(15), None, false, 0).unwrap();
    assert_eq!(result.lines, (5, 15));

    // Check that the extracted content contains exactly lines 5-15
    let expected_content = content
        .lines()
        .skip(4)
        .take(11)
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(result.code, expected_content);

    // Test with invalid range (start > end)
    let err =
        process_file_for_extraction(&file_path, Some(10), Some(5), None, false, 0).unwrap_err();
    assert!(err.to_string().contains("invalid"));

    // Test with out-of-bounds range
    let err =
        process_file_for_extraction(&file_path, Some(15), Some(25), None, false, 0).unwrap_err();
    assert!(err.to_string().contains("invalid"));
}

#[test]
fn test_integration_extract_command() {
    // Create a temporary file for testing
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test_file.rs");
    let content = r#"
fn main() {
    println!("Hello, world!");
}

struct Point {
    x: i32,
    y: i32,
}
"#;
    fs::write(&file_path, content).unwrap();

    // Run the extract command
    let output = Command::new("cargo")
        .args(["run", "--", "extract", file_path.to_string_lossy().as_ref()])
        .output()
        .expect("Failed to execute command");

    // Check that the command executed successfully
    assert!(output.status.success());

    // Check that the output contains the file content
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("File:"));
    assert!(stdout.contains("fn main()"));

    // Run with a line number
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "extract",
            &format!("{}:3", file_path.to_string_lossy()),
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command executed successfully
    assert!(output.status.success());

    // Check that the output contains the function
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("File:"));
    assert!(stdout.contains("fn main()"));
    assert!(stdout.contains("Hello, world!"));

    // Run with a different format
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "extract",
            file_path.to_string_lossy().as_ref(),
            "--format",
            "markdown",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command executed successfully
    assert!(output.status.success());

    // Check that the output is in markdown format
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("## File:"));

    // Run with a line range
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "extract",
            &format!("{}:2-7", file_path.to_string_lossy()),
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command executed successfully
    assert!(output.status.success());

    // Check that the output contains the specified range
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("File:"));
    assert!(stdout.contains("Lines: 2-7"));
    assert!(stdout.contains("fn main()"));
    assert!(stdout.contains("println!(\"Hello, world!\")"));
    assert!(stdout.contains("struct Point"));
}

#[test]
fn test_integration_extract_command_json_format() {
    use serde_json::Value;

    // Create a temporary file for testing with special characters
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test_file.rs");
    let content = r#"
fn main() {
    // This contains special characters: "quotes", 'apostrophes', <tags>, &ampersands
    println!("Hello, \"world\"!");
    let message = 'A';
    let html = "<div>Content & More</div>";
}
"#;
    fs::write(&file_path, content).unwrap();

    // Run the extract command with JSON format
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "extract",
            file_path.to_string_lossy().as_ref(),
            "--format",
            "json",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command executed successfully
    assert!(output.status.success());

    // Get the output as a string
    let stdout = String::from_utf8_lossy(&output.stdout);

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

    // Extract and parse the JSON
    let json_str = extract_json_from_output(&stdout);
    let json_value: Value = serde_json::from_str(json_str).expect("Failed to parse JSON output");

    // Validate the structure of the JSON output
    assert!(json_value.is_object(), "JSON output should be an object");
    assert!(
        json_value.get("results").is_some(),
        "JSON output should have a 'results' field"
    );
    assert!(
        json_value.get("summary").is_some(),
        "JSON output should have a 'summary' field"
    );

    // Validate the results array
    let results = json_value.get("results").unwrap().as_array().unwrap();
    assert!(!results.is_empty(), "Results array should not be empty");

    // Validate the first result
    let first_result = &results[0];
    assert!(
        first_result.get("file").is_some(),
        "Result should have a 'file' field"
    );
    assert!(
        first_result.get("code").is_some(),
        "Result should have a 'code' field"
    );

    // Verify that special characters are properly escaped in the JSON
    let code = first_result.get("code").unwrap().as_str().unwrap();
    assert!(
        code.contains("\"quotes\""),
        "Double quotes should be properly escaped in JSON"
    );
    assert!(
        code.contains("'apostrophes'"),
        "Apostrophes should be properly escaped in JSON"
    );
    assert!(
        code.contains("<tags>"),
        "Tags should be properly escaped in JSON"
    );
    assert!(
        code.contains("&ampersands"),
        "Ampersands should be properly escaped in JSON"
    );

    // Run with a line number and JSON format
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "extract",
            &format!("{}:3", file_path.to_string_lossy()),
            "--format",
            "json",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command executed successfully
    assert!(output.status.success());

    // Parse the JSON output
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json_str = extract_json_from_output(&stdout);
    let json_value: Value = serde_json::from_str(json_str).expect("Failed to parse JSON output");

    // Validate the structure
    let results = json_value.get("results").unwrap().as_array().unwrap();
    assert!(!results.is_empty(), "Results array should not be empty");

    // The result should contain the function with line 3
    let first_result = &results[0];
    let code = first_result.get("code").unwrap().as_str().unwrap();
    assert!(
        code.contains("fn main()"),
        "Code should contain the main function"
    );
    assert!(
        code.contains("Hello, \\\"world\\\"!"),
        "Code should contain the println statement with escaped quotes"
    );
}

#[test]
fn test_integration_extract_command_xml_format() {
    use roxmltree::Document;

    // Create a temporary file for testing with special characters
    let temp_dir = tempfile::tempdir().unwrap();
    let file_path = temp_dir.path().join("test_file.rs");
    let content = r#"
fn main() {
    // This contains special characters: "quotes", 'apostrophes', <tags>, &ampersands
    println!("Hello, \"world\"!");
    let message = 'A';
    let html = "<div>Content & More</div>";
}
"#;
    fs::write(&file_path, content).unwrap();

    // Run the extract command with XML format
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "extract",
            file_path.to_string_lossy().as_ref(),
            "--format",
            "xml",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command executed successfully
    assert!(output.status.success());

    // Get the output as a string
    let stdout = String::from_utf8_lossy(&output.stdout);

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

    // Extract and parse the XML
    let xml_str = extract_xml_from_output(&stdout);
    let doc = Document::parse(xml_str).expect("Failed to parse XML output");
    let root = doc.root_element();

    // Validate the structure of the XML output
    assert_eq!(
        root.tag_name().name(),
        "probe_results",
        "Root element should be 'probe_results'"
    );

    // Validate that there are result elements
    let results: Vec<_> = root
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "result")
        .collect();
    assert!(
        !results.is_empty(),
        "Should have at least one result element"
    );

    // Validate the first result
    let first_result = &results[0];

    // Check for required elements
    let file = first_result
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "file");
    assert!(file.is_some(), "Result should have a file element");

    let code = first_result
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "code");
    assert!(code.is_some(), "Result should have a code element");

    // Verify that special characters are properly handled in the XML (should be in CDATA)
    if let Some(code_elem) = code {
        let code_text = code_elem.text().unwrap();
        assert!(
            code_text.contains("\"quotes\""),
            "Double quotes should be preserved in CDATA"
        );
        assert!(
            code_text.contains("'apostrophes'"),
            "Apostrophes should be preserved in CDATA"
        );
        assert!(
            code_text.contains("<tags>"),
            "Tags should be preserved in CDATA"
        );
        assert!(
            code_text.contains("&ampersands"),
            "Ampersands should be preserved in CDATA"
        );
    }

    // Run with a line number and XML format
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "extract",
            &format!("{}:3", file_path.to_string_lossy()),
            "--format",
            "xml",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command executed successfully
    assert!(output.status.success());

    // Parse the XML output
    let stdout = String::from_utf8_lossy(&output.stdout);
    let xml_str = extract_xml_from_output(&stdout);
    let doc = Document::parse(xml_str).expect("Failed to parse XML output");
    let root = doc.root_element();

    // Validate the structure
    let results: Vec<_> = root
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "result")
        .collect();
    assert!(
        !results.is_empty(),
        "Should have at least one result element"
    );

    // The result should contain the function with line 3
    let first_result = &results[0];
    if let Some(code_elem) = first_result
        .children()
        .find(|n| n.is_element() && n.tag_name().name() == "code")
    {
        let code_text = code_elem.text().unwrap();
        assert!(
            code_text.contains("fn main()"),
            "Code should contain the main function"
        );
        assert!(
            code_text.contains("Hello") && code_text.contains("world"),
            "Code should contain the println statement"
        );
    }
}
