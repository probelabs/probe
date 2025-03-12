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
    let result = process_file_for_extraction(&file_path, None, None, false, 0).unwrap();

    assert_eq!(result.file, file_path.to_string_lossy().to_string());
    assert_eq!(result.lines, (1, 3)); // 3 lines in the content
    assert_eq!(result.node_type, "file");
    assert_eq!(result.code, content);

    // Test with non-existent file
    let non_existent = temp_dir.path().join("non_existent.txt");
    let err = process_file_for_extraction(&non_existent, None, None, false, 0).unwrap_err();
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
    let result = process_file_for_extraction(&file_path, Some(3), None, false, 0).unwrap();
    assert_eq!(result.file, file_path.to_string_lossy().to_string());
    assert!(result.lines.0 <= 3 && result.lines.1 >= 3);
    assert!(result.code.contains("fn main()"));
    assert!(result.code.contains("Hello, world!"));

    // Test extracting a struct
    let result = process_file_for_extraction(&file_path, Some(13), None, false, 0).unwrap();
    assert_eq!(result.file, file_path.to_string_lossy().to_string());
    assert!(result.lines.0 <= 13 && result.lines.1 >= 13);
    assert!(result.code.contains("struct Point"));
    assert!(result.code.contains("x: i32"));
    assert!(result.code.contains("y: i32"));

    // Test with out-of-bounds line number
    let err = process_file_for_extraction(&file_path, Some(1000), None, false, 0).unwrap_err();
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
    let result = process_file_for_extraction(&file_path, Some(15), None, false, 10).unwrap();
    assert_eq!(result.file, file_path.to_string_lossy().to_string());
    assert_eq!(result.node_type, "context");

    // Should include 10 lines before and after line 15
    let start_line = result.lines.0;
    let end_line = result.lines.1;

    // Check that the range includes line 15 and has appropriate context
    assert!(start_line <= 15 && end_line >= 15);
    assert!(end_line - start_line >= 10); // At least 10 lines of context

    // Test with a line at the beginning of the file
    let result = process_file_for_extraction(&file_path, Some(2), None, false, 10).unwrap();
    assert!(result.lines.0 <= 2); // Should start at or before line 2
    assert!(result.lines.1 >= 2); // Should include line 2

    // Test with a line at the end of the file
    let result = process_file_for_extraction(&file_path, Some(25), None, false, 10).unwrap();
    assert!(result.lines.0 <= 25); // Should include some lines before line 25
    assert_eq!(result.lines.1, 25); // Can't go beyond the last line

    // Test with custom context lines
    let result = process_file_for_extraction(&file_path, Some(15), None, false, 5).unwrap();
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
    let result = process_file_for_extraction(&file_path, Some(1), Some(10), false, 0).unwrap();
    assert_eq!(result.file, file_path.to_string_lossy().to_string());
    assert_eq!(result.lines, (1, 10));
    assert_eq!(result.node_type, "range");
    
    // Check that the extracted content contains exactly lines 1-10
    let expected_content = content.lines().take(10).collect::<Vec<_>>().join("\n");
    assert_eq!(result.code, expected_content);

    // Test with a different range
    let result = process_file_for_extraction(&file_path, Some(5), Some(15), false, 0).unwrap();
    assert_eq!(result.lines, (5, 15));
    
    // Check that the extracted content contains exactly lines 5-15
    let expected_content = content.lines().skip(4).take(11).collect::<Vec<_>>().join("\n");
    assert_eq!(result.code, expected_content);

    // Test with invalid range (start > end)
    let err = process_file_for_extraction(&file_path, Some(10), Some(5), false, 0).unwrap_err();
    assert!(err.to_string().contains("invalid"));

    // Test with out-of-bounds range
    let err = process_file_for_extraction(&file_path, Some(15), Some(25), false, 0).unwrap_err();
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
