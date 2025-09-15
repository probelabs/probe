use probe_code::search::{perform_probe, SearchOptions};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_symbols_flag_with_rust_code() {
    // Create a temporary directory and file
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let test_file = temp_dir.path().join("test.rs");

    // Write Rust code with various symbols
    let rust_code = r#"
pub struct User {
    pub name: String,
    pub age: u32,
}

impl User {
    pub fn new(name: String, age: u32) -> Self {
        Self { name, age }
    }

    pub fn greet(&self) -> String {
        format!("Hello, I'm {}", self.name)
    }
}

pub fn main() {
    let user = User::new("Alice".to_string(), 30);
    println!("{}", user.greet());
}

pub const MAX_USERS: usize = 1000;
"#;

    fs::write(&test_file, rust_code).expect("Failed to write test file");

    // Test search with symbols flag enabled
    let options = SearchOptions {
        path: temp_dir.path(),
        queries: &["pub fn".to_string()],
        files_only: false,
        custom_ignores: &[],
        exclude_filenames: false,
        reranker: "bm25",
        frequency_search: true,
        exact: false,
        language: None,
        max_results: Some(10),
        max_bytes: None,
        max_tokens: None,
        allow_tests: false,
        no_merge: false,
        merge_threshold: None,
        dry_run: false,
        session: None,
        timeout: 30,
        question: None,
        no_gitignore: true,
    };

    let results = perform_probe(&options).expect("Search should succeed");

    // Verify we got results
    assert!(!results.results.is_empty(), "Should find symbol matches");

    // Verify that symbol signatures are populated
    let has_symbol_signatures = results.results.iter().any(|r| r.symbol_signature.is_some());
    assert!(
        has_symbol_signatures,
        "At least one result should have a symbol signature"
    );

    // Test with symbols flag disabled
    let options_no_symbols = SearchOptions { ..options };

    let results_no_symbols = perform_probe(&options_no_symbols).expect("Search should succeed");

    // Verify that symbol signatures are not populated when flag is disabled
    let has_symbol_signatures = results_no_symbols
        .results
        .iter()
        .any(|r| r.symbol_signature.is_some());
    assert!(
        !has_symbol_signatures,
        "No results should have symbol signatures when flag is disabled"
    );
}

#[test]
fn test_symbols_flag_with_python_code() {
    // Create a temporary directory and file
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let test_file = temp_dir.path().join("test.py");

    // Write Python code with various symbols
    let python_code = r#"
class User:
    def __init__(self, name: str, age: int):
        self.name = name
        self.age = age

    def greet(self) -> str:
        return f"Hello, I'm {self.name}"

def create_user(name: str, age: int) -> User:
    return User(name, age)

async def async_function(data: list) -> dict:
    return {"length": len(data)}

MAX_USERS = 1000
"#;

    fs::write(&test_file, python_code).expect("Failed to write test file");

    // Test search with symbols flag enabled
    let options = SearchOptions {
        path: temp_dir.path(),
        queries: &["def ".to_string()],
        files_only: false,
        custom_ignores: &[],
        exclude_filenames: false,
        reranker: "bm25",
        frequency_search: true,
        exact: false,
        language: None,
        max_results: Some(10),
        max_bytes: None,
        max_tokens: None,
        allow_tests: false,
        no_merge: false,
        merge_threshold: None,
        dry_run: false,
        session: None,
        timeout: 30,
        question: None,
        no_gitignore: true,
    };

    let results = perform_probe(&options).expect("Search should succeed");

    // Verify we got results
    assert!(!results.results.is_empty(), "Should find symbol matches");

    // Verify that symbol signatures are populated and contain Python syntax
    let python_symbols: Vec<_> = results
        .results
        .iter()
        .filter_map(|r| r.symbol_signature.as_ref())
        .collect();

    assert!(
        !python_symbols.is_empty(),
        "Should have Python symbol signatures"
    );

    // Check that we have recognizable Python function signatures
    let has_python_function = python_symbols
        .iter()
        .any(|sig| sig.contains("def ") && sig.contains("(") && sig.contains(")"));
    assert!(
        has_python_function,
        "Should have Python function signatures"
    );
}
