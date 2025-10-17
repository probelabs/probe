use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

// Helper function to create test files
fn create_test_file(dir: &TempDir, filename: &str, content: &str) -> PathBuf {
    let file_path = dir.path().join(filename);
    let mut file = File::create(&file_path).expect("Failed to create test file");
    file.write_all(content.as_bytes())
        .expect("Failed to write test content");
    file_path
}

// Helper function to create a test directory with code samples
fn create_test_code_structure(root_dir: &TempDir) {
    // Create a source directory
    let src_dir = root_dir.path().join("src");
    fs::create_dir(&src_dir).expect("Failed to create src directory");

    // Create Rust file with snake_case and camelCase identifiers
    let rust_content = r#"
fn get_user_id(user: &User) -> u32 {
    user.id
}

fn getUserName(user: &User) -> String {
    user.name.clone()
}

fn handle_error(err: Error) {
    eprintln!("Error: {:?}", err);
}

fn error_handler(err: Error) {
    handle_error(err);
}
"#;
    create_test_file(root_dir, "src/lib.rs", rust_content);
}

#[test]
fn test_strict_syntax_rejects_vague_queries() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_code_structure(&temp_dir);

    // Test with multiple words without operators (should fail)
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "search",
            "error handler", // Vague query - multiple words without AND/OR
            temp_dir.path().to_str().unwrap(),
            "--strict-elastic-syntax",
            "--max-results",
            "1",
        ])
        .output()
        .expect("Failed to execute command");

    // Command should fail with validation error
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Check for validation error message
    assert!(
        stderr.contains("Vague query format detected"),
        "Should detect vague query format. stderr: {}",
        stderr
    );
    assert!(
        stderr.contains("Use explicit AND/OR operators"),
        "Should suggest using explicit operators. stderr: {}",
        stderr
    );
}

#[test]
fn test_strict_syntax_rejects_unquoted_snake_case() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_code_structure(&temp_dir);

    // Test with unquoted snake_case (should fail)
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "search",
            "get_user_id", // Unquoted snake_case
            temp_dir.path().to_str().unwrap(),
            "--strict-elastic-syntax",
            "--max-results",
            "1",
        ])
        .output()
        .expect("Failed to execute command");

    // Command should fail with validation error
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Check for validation error message
    assert!(
        stderr.contains("contains special characters"),
        "Should detect special characters in snake_case. stderr: {}",
        stderr
    );
    assert!(
        stderr.contains("should be wrapped in quotes"),
        "Should suggest wrapping in quotes. stderr: {}",
        stderr
    );
}

#[test]
fn test_strict_syntax_rejects_unquoted_camel_case() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_code_structure(&temp_dir);

    // Test with unquoted camelCase (should fail)
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "search",
            "getUserName", // Unquoted camelCase
            temp_dir.path().to_str().unwrap(),
            "--strict-elastic-syntax",
            "--max-results",
            "1",
        ])
        .output()
        .expect("Failed to execute command");

    // Command should fail with validation error
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Check for validation error message
    assert!(
        stderr.contains("contains special characters"),
        "Should detect special characters in camelCase. stderr: {}",
        stderr
    );
}

#[test]
fn test_strict_syntax_accepts_explicit_operators() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_code_structure(&temp_dir);

    // Test with explicit AND operator (should succeed)
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "search",
            "(error AND handler)", // Valid query with explicit operator
            temp_dir.path().to_str().unwrap(),
            "--strict-elastic-syntax",
            "--max-results",
            "1",
        ])
        .output()
        .expect("Failed to execute command");

    // Command should succeed
    assert!(
        output.status.success(),
        "Command should succeed with explicit operators. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should execute search (may or may not find results, but shouldn't error on syntax)
    assert!(
        stdout.contains("Probe version") || stdout.contains("Search completed"),
        "Should execute search successfully. stdout: {}",
        stdout
    );
}

#[test]
fn test_strict_syntax_accepts_quoted_snake_case() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_code_structure(&temp_dir);

    // Test with quoted snake_case (should succeed)
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "search",
            "\"get_user_id\"", // Quoted snake_case
            temp_dir.path().to_str().unwrap(),
            "--strict-elastic-syntax",
            "--max-results",
            "1",
        ])
        .output()
        .expect("Failed to execute command");

    // Command should succeed
    assert!(
        output.status.success(),
        "Command should succeed with quoted snake_case. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should execute search
    assert!(
        stdout.contains("Probe version") || stdout.contains("Search completed"),
        "Should execute search successfully. stdout: {}",
        stdout
    );
}

#[test]
fn test_strict_syntax_accepts_quoted_camel_case() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_code_structure(&temp_dir);

    // Test with quoted camelCase (should succeed)
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "search",
            "\"getUserName\"", // Quoted camelCase
            temp_dir.path().to_str().unwrap(),
            "--strict-elastic-syntax",
            "--max-results",
            "1",
        ])
        .output()
        .expect("Failed to execute command");

    // Command should succeed
    assert!(
        output.status.success(),
        "Command should succeed with quoted camelCase. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should execute search
    assert!(
        stdout.contains("Probe version") || stdout.contains("Search completed"),
        "Should execute search successfully. stdout: {}",
        stdout
    );
}

#[test]
fn test_strict_syntax_accepts_single_word() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_code_structure(&temp_dir);

    // Test with single lowercase word (should succeed)
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "search",
            "error", // Single word
            temp_dir.path().to_str().unwrap(),
            "--strict-elastic-syntax",
            "--max-results",
            "1",
        ])
        .output()
        .expect("Failed to execute command");

    // Command should succeed
    assert!(
        output.status.success(),
        "Command should succeed with single word. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should execute search
    assert!(
        stdout.contains("Probe version") || stdout.contains("Search completed"),
        "Should execute search successfully. stdout: {}",
        stdout
    );
}

#[test]
fn test_strict_syntax_accepts_complex_query() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_code_structure(&temp_dir);

    // Test with complex query using AND, OR, NOT (should succeed)
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "search",
            "(\"get_user_id\" AND NOT test)", // Complex query
            temp_dir.path().to_str().unwrap(),
            "--strict-elastic-syntax",
            "--max-results",
            "1",
        ])
        .output()
        .expect("Failed to execute command");

    // Command should succeed
    assert!(
        output.status.success(),
        "Command should succeed with complex query. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should execute search
    assert!(
        stdout.contains("Probe version") || stdout.contains("Search completed"),
        "Should execute search successfully. stdout: {}",
        stdout
    );
}

#[test]
fn test_without_strict_syntax_flag_allows_vague_queries() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_code_structure(&temp_dir);

    // Test with vague query but WITHOUT --strict-elastic-syntax flag (should succeed)
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "search",
            "error handler", // Vague query but flag not enabled
            temp_dir.path().to_str().unwrap(),
            "--max-results",
            "1",
        ])
        .output()
        .expect("Failed to execute command");

    // Command should succeed when flag is not set
    assert!(
        output.status.success(),
        "Command should succeed without strict flag. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should execute search normally
    assert!(
        stdout.contains("Probe version") || stdout.contains("Search completed"),
        "Should execute search successfully. stdout: {}",
        stdout
    );
}

#[test]
fn test_strict_syntax_error_provides_helpful_examples() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_code_structure(&temp_dir);

    // Test that error messages include helpful examples
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "search",
            "error handler",
            temp_dir.path().to_str().unwrap(),
            "--strict-elastic-syntax",
        ])
        .output()
        .expect("Failed to execute command");

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Check that error includes examples
    assert!(
        stderr.contains("Examples:"),
        "Error should include examples section. stderr: {}",
        stderr
    );
    assert!(
        stderr.contains("(error AND handler)") || stderr.contains("getUserId"),
        "Error should include concrete examples. stderr: {}",
        stderr
    );
}
