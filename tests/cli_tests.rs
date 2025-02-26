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

// Helper function to create a test directory structure
fn create_test_directory_structure(root_dir: &TempDir) {
    // Create a source directory
    let src_dir = root_dir.path().join("src");
    fs::create_dir(&src_dir).expect("Failed to create src directory");

    // Create Rust files with search terms
    let rust_content = r#"
// This is a Rust file with a search term
fn search_function(query: &str) -> bool {
    println!("Searching for: {}", query);
    query.contains("search")
}
"#;
    create_test_file(root_dir, "src/search.rs", rust_content);

    // Create a JavaScript file with search terms
    let js_content = r#"
// This is a JavaScript file with a search term
function searchFunction(query) {
    console.log(`Searching for: ${query}`);
    return query.includes('search');
}
"#;
    create_test_file(root_dir, "src/search.js", js_content);
}

#[test]
fn test_cli_basic_search() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Run the CLI with basic search
    let output = Command::new("cargo")
        .args(&[
            "run",
            "--",
            "--path",
            temp_dir.path().to_str().unwrap(),
            "--query",
            "search",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    assert!(output.status.success());

    // Convert stdout to string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Check that it found matches
    assert!(
        stdout.contains("Found"),
        "Output should indicate matches were found"
    );

    // Check that it found both Rust and JavaScript files
    assert!(
        stdout.contains("search.rs"),
        "Should find matches in Rust file"
    );
    assert!(
        stdout.contains("search.js"),
        "Should find matches in JavaScript file"
    );
}

#[test]
fn test_cli_files_only() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Run the CLI with files-only option
    let output = Command::new("cargo")
        .args(&[
            "run",
            "--",
            "--path",
            temp_dir.path().to_str().unwrap(),
            "--query",
            "search",
            "--files-only",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    assert!(output.status.success());

    // Convert stdout to string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Check that it found matches
    assert!(
        stdout.contains("Found"),
        "Output should indicate matches were found"
    );

    // Check that it found both Rust and JavaScript files
    assert!(
        stdout.contains("search.rs"),
        "Should find matches in Rust file"
    );
    assert!(
        stdout.contains("search.js"),
        "Should find matches in JavaScript file"
    );

    // In files-only mode, it should not show code
    assert!(
        !stdout.contains("fn search_function"),
        "Should not include code in files-only mode"
    );
    assert!(
        !stdout.contains("function searchFunction"),
        "Should not include code in files-only mode"
    );
}

#[test]
fn test_cli_include_filenames() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Create a file with "search" in the name but not in the content
    create_test_file(
        &temp_dir,
        "search_file_without_content.txt",
        "This file doesn't contain the search term anywhere in its content.",
    );

    // Run the CLI with include-filenames option
    let output = Command::new("cargo")
        .args(&[
            "run",
            "--",
            "--path",
            temp_dir.path().to_str().unwrap(),
            "--query",
            "search",
            "--include-filenames",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    assert!(output.status.success());

    // Convert stdout to string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Check that it found matches
    assert!(
        stdout.contains("Found"),
        "Output should indicate matches were found"
    );

    // Check that it found the file with "search" in the name
    assert!(
        stdout.contains("search_file_without_content.txt"),
        "Should find file with search in the name"
    );
}

#[test]
fn test_cli_reranker() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Run the CLI with tfidf reranker
    let output = Command::new("cargo")
        .args(&[
            "run",
            "--",
            "--path",
            temp_dir.path().to_str().unwrap(),
            "--query",
            "search",
            "--reranker",
            "tfidf",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    assert!(output.status.success());

    // Convert stdout to string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Check that it found matches
    assert!(
        stdout.contains("Found"),
        "Output should indicate matches were found"
    );

    // Check that it used TF-IDF ranking
    assert!(
        stdout.contains("Using TF-IDF for ranking"),
        "Should use TF-IDF reranker"
    );
}

#[test]
fn test_cli_default_frequency_search() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Run the CLI with default settings (frequency search should be enabled by default)
    let output = Command::new("cargo")
        .args(&[
            "run",
            "--",
            "--path",
            temp_dir.path().to_str().unwrap(),
            "--query",
            "search",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    assert!(output.status.success());

    // Convert stdout to string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Check that it found matches
    assert!(
        stdout.contains("Found"),
        "Output should indicate matches were found"
    );

    // Check that it used frequency-based search (which is now the default)
    assert!(
        stdout.contains("Frequency search enabled"),
        "Should use frequency-based search by default"
    );
}

#[test]
fn test_cli_exact_search() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Run the CLI with exact search option
    let output = Command::new("cargo")
        .args(&[
            "run",
            "--",
            "--path",
            temp_dir.path().to_str().unwrap(),
            "--query",
            "search",
            "--exact",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    assert!(output.status.success());

    // Convert stdout to string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Check that it found matches
    assert!(
        stdout.contains("Found"),
        "Output should indicate matches were found"
    );

    // Check that it did NOT use frequency-based search
    assert!(
        !stdout.contains("Frequency search enabled"),
        "Should not use frequency-based search with --exact option"
    );
}

#[test]
fn test_cli_custom_ignores() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Run the CLI with custom ignore pattern
    let output = Command::new("cargo")
        .args(&[
            "run",
            "--",
            "--path",
            temp_dir.path().to_str().unwrap(),
            "--query",
            "search",
            "--ignore",
            "*.js",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    assert!(output.status.success());

    // Convert stdout to string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Check that it found matches
    assert!(
        stdout.contains("Found"),
        "Output should indicate matches were found"
    );

    // Check that it found the Rust file but not the JavaScript file
    assert!(
        stdout.contains("search.rs"),
        "Should find matches in Rust file"
    );
    assert!(
        !stdout.contains("search.js"),
        "Should not find matches in JavaScript file"
    );
}

#[test]
fn test_cli_max_results() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Add more files with search terms to ensure we have more than 1 result
    for i in 1..5 {
        let content = format!("// File {} with search term\n", i);
        create_test_file(&temp_dir, &format!("src/extra{}.rs", i), &content);
    }

    // Run the CLI with max results limit
    let output = Command::new("cargo")
        .args(&[
            "run",
            "--",
            "--path",
            temp_dir.path().to_str().unwrap(),
            "--query",
            "search",
            "--max-results",
            "1",
        ])
        .output()
        .expect("Failed to execute command");

    // Check that the command succeeded
    assert!(output.status.success());

    // Convert stdout to string
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Check that it found matches
    assert!(
        stdout.contains("Found"),
        "Output should indicate matches were found"
    );

    // Check that it limited the results
    assert!(
        stdout.contains("Limits applied"),
        "Should indicate limits were applied"
    );
    assert!(
        stdout.contains("Max results: 1"),
        "Should show max results limit"
    );

    // Should only report 1 result in the summary
    assert!(
        stdout.contains("Found 1 search results"),
        "Should find only 1 result"
    );
}
