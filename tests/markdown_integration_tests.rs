use std::fs;
use tempfile::tempdir;

use probe_code::search::{perform_probe, SearchOptions};

#[test]
fn test_markdown_basic_search() {
    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("test.md");

    let markdown_content = r#"# Getting Started

This is a guide for getting started with our project.

## Installation

To install the project, run:

```bash
npm install my-project
```

## Usage

Here are some usage examples:

```python
def hello_world():
    print("Hello, world!")
```
"#;

    fs::write(&file_path, markdown_content).unwrap();

    // Test search for "installation"
    let queries = vec!["installation".to_string()];
    let custom_ignores: Vec<String> = vec![];
    let options = SearchOptions {
        path: temp_dir.path(),
        queries: &queries,
        files_only: false,
        custom_ignores: &custom_ignores,
        exclude_filenames: true,
        language: Some("md"),
        reranker: "hybrid",
        frequency_search: true,
        exact: false,
        max_results: None,
        max_bytes: None,
        max_tokens: None,
        allow_tests: true,
        no_merge: true,
        merge_threshold: None,
        dry_run: false,
        session: None,
        timeout: 30,
        question: None,
        no_gitignore: false,
    };
    let results = perform_probe(&options).unwrap();
    assert!(!results.results.is_empty());
    assert!(results.results[0]
        .code
        .to_lowercase()
        .contains("installation"));
}

#[test]
fn test_markdown_code_block_search() {
    let temp_dir = tempdir().unwrap();
    let file_path = temp_dir.path().join("test.md");

    let markdown_content = r#"# Code Examples

```python
def hello_world():
    print("Hello, world!")
```

```javascript
console.log("Hello from JS");
```
"#;

    fs::write(&file_path, markdown_content).unwrap();

    // Test search in code blocks
    let queries = vec!["hello_world".to_string()];
    let custom_ignores: Vec<String> = vec![];
    let options = SearchOptions {
        path: temp_dir.path(),
        queries: &queries,
        files_only: false,
        custom_ignores: &custom_ignores,
        exclude_filenames: true,
        language: Some("md"),
        reranker: "hybrid",
        frequency_search: true,
        exact: false,
        max_results: None,
        max_bytes: None,
        max_tokens: None,
        allow_tests: true,
        no_merge: true,
        merge_threshold: None,
        dry_run: false,
        session: None,
        timeout: 30,
        question: None,
        no_gitignore: false,
    };
    let results = perform_probe(&options).unwrap();
    assert!(!results.results.is_empty());
    assert!(results.results[0].code.contains("def hello_world"));
}

#[test]
fn test_markdown_language_detection() {
    use probe_code::language::factory::get_language_impl;

    // Test .md extension
    let lang_impl = get_language_impl("md");
    assert!(lang_impl.is_some());

    // Test .markdown extension
    let lang_impl = get_language_impl("markdown");
    assert!(lang_impl.is_some());
}

#[test]
fn test_markdown_with_various_extensions() {
    let temp_dir = tempdir().unwrap();

    // Test both .md and .markdown extensions
    let extensions = vec!["md", "markdown"];

    for ext in extensions {
        let file_path = temp_dir.path().join(format!("test.{}", ext));
        let content = r#"# Test Document

This is a test document with a simple header.

## Features

- Feature 1
- Feature 2

```rust
fn main() {
    println!("Hello from Rust!");
}
```
"#;

        fs::write(&file_path, content).unwrap();

        // Test search functionality
        let queries = vec!["Feature".to_string()];
        let custom_ignores: Vec<String> = vec![];
        let options = SearchOptions {
            path: temp_dir.path(),
            queries: &queries,
            files_only: false,
            custom_ignores: &custom_ignores,
            exclude_filenames: true,
            language: Some(ext),
            reranker: "hybrid",
            frequency_search: true,
            exact: false,
            max_results: None,
            max_bytes: None,
            max_tokens: None,
            allow_tests: true,
            no_merge: true,
            merge_threshold: None,
            dry_run: false,
            session: None,
            timeout: 30,
            question: None,
            no_gitignore: false,
        };
        let results = perform_probe(&options).unwrap();
        assert!(
            !results.results.is_empty(),
            "No results found for .{} file",
            ext
        );
        assert!(results.results[0].code.contains("Feature"));
    }
}
