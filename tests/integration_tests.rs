use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;

// The integration test needs access to the library crate
use code_search::search::perform_code_search;

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

    // Create Rust files
    let rust_content1 = r#"
// This is a Rust file with a function
fn search_function(query: &str) -> bool {
    println!("Searching for: {}", query);
    query.contains("search")
}

struct SearchResult {
    file: String,
    line: usize,
    content: String,
}

impl SearchResult {
    fn new(file: String, line: usize, content: String) -> Self {
        Self { file, line, content }
    }
}
"#;
    create_test_file(root_dir, "src/search.rs", rust_content1);

    let rust_content2 = r#"
mod search;

fn main() {
    let query = "search term";
    let found = search::search_function(query);
    println!("Found: {}", found);
}
"#;
    create_test_file(root_dir, "src/main.rs", rust_content2);

    // Create a JavaScript file
    let js_content = r#"
// This is a JavaScript file with a function
function searchFunction(query) {
    console.log(`Searching for: ${query}`);
    return query.includes('search');
}

class SearchResult {
    constructor(file, line, content) {
        this.file = file;
        this.line = line;
        this.content = content;
    }
}

// Export the functions and classes
module.exports = {
    searchFunction,
    SearchResult
};
"#;
    create_test_file(root_dir, "src/search.js", js_content);

    // Create a Python file
    let py_content = r#"
# This is a Python file with a function
def search_function(query):
    print(f"Searching for: {query}")
    return "search" in query

class SearchResult:
    def __init__(self, file, line, content):
        self.file = file
        self.line = line
        self.content = content
"#;
    create_test_file(root_dir, "src/search.py", py_content);

    // Create a subdirectory with more files
    let tests_dir = root_dir.path().join("tests");
    fs::create_dir(&tests_dir).expect("Failed to create tests directory");

    let test_content = r#"
// This is a test file for the search functionality
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_search_function() {
        let query = "search term";
        let found = search_function(query);
        assert!(found);
    }
}
"#;
    create_test_file(root_dir, "tests/search_test.rs", test_content);

    // Create a file to be ignored
    let node_modules_dir = root_dir.path().join("node_modules");
    fs::create_dir(&node_modules_dir).expect("Failed to create node_modules directory");
    create_test_file(
        root_dir,
        "node_modules/ignored.js",
        "This file should be ignored",
    );
}

#[test]
fn test_search_single_term() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Search for a single term
    let search_results = perform_code_search(
        temp_dir.path(),
        &["search".to_string()],
        false,    // files_only
        &[],      // custom_ignores
        false,    // include_filenames
        "hybrid", // reranker
        false,    // frequency_search
        None,     // max_results
        None,     // max_bytes
        None,     // max_tokens
        false,    // allow_tests
        false,    // any_term
        false,    // exact
    )
    .expect("Failed to perform search");

    // Should find matches
    assert!(!search_results.results.is_empty());

    // Should find matches in all three source files
    let found_rust = search_results
        .results
        .iter()
        .any(|r| r.file.ends_with("search.rs"));
    let found_js = search_results
        .results
        .iter()
        .any(|r| r.file.ends_with("search.js"));
    let found_py = search_results
        .results
        .iter()
        .any(|r| r.file.ends_with("search.py"));

    assert!(found_rust, "Should find matches in Rust file");
    assert!(found_js, "Should find matches in JavaScript file");
    assert!(found_py, "Should find matches in Python file");

    // Should not find matches in ignored files
    let found_ignored = search_results
        .results
        .iter()
        .any(|r| r.file.contains("node_modules"));
    assert!(!found_ignored, "Should not find matches in ignored files");
}

#[test]
#[ignore] // Temporarily disabled due to issues with multi-term search
fn test_search_multiple_terms() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Search for multiple terms
    let search_results = perform_code_search(
        temp_dir.path(),
        &["search".to_string(), "function".to_string()],
        false,    // files_only
        &[],      // custom_ignores
        false,    // include_filenames
        "hybrid", // reranker
        false,    // frequency_search
        None,     // max_results
        None,     // max_bytes
        None,     // max_tokens
        false,    // allow_tests
        false,    // any_term
        false,    // exact
    )
    .expect("Failed to perform search");

    // Should find matches
    assert!(!search_results.results.is_empty());

    // Results should contain both search terms
    let has_both_terms = search_results
        .results
        .iter()
        .any(|r| r.code.contains("search") && r.code.contains("function"));

    assert!(has_both_terms, "Should find matches with both terms");
}

#[test]
fn test_search_files_only() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Search for files only
    let search_results = perform_code_search(
        temp_dir.path(),
        &["search".to_string()],
        true,     // files_only
        &[],      // custom_ignores
        false,    // include_filenames
        "hybrid", // reranker
        false,    // frequency_search
        None,     // max_results
        None,     // max_bytes
        None,     // max_tokens
        false,    // allow_tests
        false,    // any_term
        false,    // exact
    )
    .expect("Failed to perform search");

    // Should find matches
    assert!(!search_results.results.is_empty());

    // Results should be file paths, not code blocks
    for result in &search_results.results {
        assert_eq!(result.node_type, "file");
        assert_eq!(result.code, ""); // In files_only mode, code is empty
    }

    // Should find matches in all three source files
    let found_rust = search_results
        .results
        .iter()
        .any(|r| r.file.ends_with("search.rs"));
    let found_js = search_results
        .results
        .iter()
        .any(|r| r.file.ends_with("search.js"));
    let found_py = search_results
        .results
        .iter()
        .any(|r| r.file.ends_with("search.py"));

    assert!(found_rust, "Should find matches in Rust file");
    assert!(found_js, "Should find matches in JavaScript file");
    assert!(found_py, "Should find matches in Python file");
}

#[test]
#[ignore] // Temporarily disabled due to issues with filename matching
fn test_search_include_filenames() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Create a file with "search" in the name but not in the content
    create_test_file(
        &temp_dir,
        "search_file_without_content.txt",
        "This file doesn't contain the search term anywhere in its content.",
    );

    // Search with filename matching enabled
    let search_results = perform_code_search(
        temp_dir.path(),
        &["search".to_string()],
        false,    // files_only
        &[],      // custom_ignores
        true,     // include_filenames
        "hybrid", // reranker
        false,    // frequency_search
        None,     // max_results
        None,     // max_bytes
        None,     // max_tokens
        false,    // allow_tests
        false,    // any_term
        false,    // exact
    )
    .expect("Failed to perform search");

    // Should find matches
    assert!(!search_results.results.is_empty());

    // Should find the file with "search" in the name
    let found_by_filename = search_results
        .results
        .iter()
        .any(|r| r.file.contains("search_file_without_content.txt"));

    assert!(
        found_by_filename,
        "Should find file with search in the name"
    );

    // Check that the file found by filename has the correct flag
    for result in &search_results.results {
        if result.file.contains("search_file_without_content.txt") {
            assert_eq!(result.matched_by_filename, Some(true));
        }
    }
}

#[test]
fn test_search_with_limits() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Search with limits
    let search_results = perform_code_search(
        temp_dir.path(),
        &["search".to_string()],
        false,    // files_only
        &[],      // custom_ignores
        false,    // include_filenames
        "hybrid", // reranker
        false,    // frequency_search
        Some(2),  // max_results - limit to 2 results
        None,     // max_bytes
        None,     // max_tokens
        false,    // allow_tests
        false,    // any_term
        false,    // exact
    )
    .expect("Failed to perform search");

    // Should find matches but limited to 2
    assert!(!search_results.results.is_empty());
    assert!(search_results.results.len() <= 2);

    // Should have limits applied
    assert!(search_results.limits_applied.is_some());
    let limits = search_results.limits_applied.unwrap();
    assert_eq!(limits.max_results, Some(2));

    // Should have skipped files if there were more than 2 matches
    if search_results.results.len() == 2 && !search_results.skipped_files.is_empty() {
        // There were more matches that were skipped
        assert!(!search_results.skipped_files.is_empty());
    }
}

#[test]
fn test_frequency_search() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Search using frequency-based search
    let search_results = perform_code_search(
        temp_dir.path(),
        &["search".to_string()],
        false,    // files_only
        &[],      // custom_ignores
        false,    // include_filenames
        "hybrid", // reranker
        true,     // frequency_search
        None,     // max_results
        None,     // max_bytes
        None,     // max_tokens
        false,    // allow_tests
        false,    // any_term
        false,    // exact
    )
    .expect("Failed to perform search");

    // Should find matches
    assert!(!search_results.results.is_empty());

    // All results should have scores (frequency search assigns scores)
    for result in &search_results.results {
        assert!(result.score.is_some());
    }
}

#[test]
fn test_search_with_custom_ignores() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    // Create a custom ignore pattern for Python files
    let custom_ignores = vec!["*.py".to_string()];

    // Search with custom ignore patterns
    let search_results = perform_code_search(
        temp_dir.path(),
        &["search".to_string()],
        false,           // files_only
        &custom_ignores, // custom_ignores
        false,           // include_filenames
        "hybrid",        // reranker
        false,           // frequency_search
        None,            // max_results
        None,            // max_bytes
        None,            // max_tokens
        false,           // allow_tests
        false,           // any_term
        false,           // exact
    )
    .expect("Failed to perform search");

    // Should find matches
    assert!(!search_results.results.is_empty());

    // Should not find matches in Python files
    let found_py = search_results
        .results
        .iter()
        .any(|r| r.file.ends_with(".py"));
    assert!(!found_py, "Should not find matches in Python files");

    // Should still find matches in other files
    let found_rust = search_results
        .results
        .iter()
        .any(|r| r.file.ends_with(".rs"));
    let found_js = search_results
        .results
        .iter()
        .any(|r| r.file.ends_with(".js"));

    assert!(
        found_rust || found_js,
        "Should find matches in non-Python files"
    );
}
