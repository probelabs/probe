use anyhow::Result;
use probe_code::search::search_runner::perform_probe;
use probe_code::search::SearchOptions;
use std::fs;
use tempfile::TempDir;

fn create_test_file(root_dir: &TempDir, relative_path: &str, content: &str) {
    let file_path = root_dir.path().join(relative_path);
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).expect("Failed to create parent directories");
    }
    fs::write(&file_path, content).expect("Failed to write test file");
}

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
}

fn main() -> Result<()> {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    create_test_directory_structure(&temp_dir);

    println!("DEBUG: Created test directory structure at: {:?}", temp_dir.path());

    // Create search query
    let queries = vec!["search".to_string(), "function".to_string()];
    let custom_ignores: Vec<String> = vec![];

    // First try: with filename matching disabled (like the original test)
    println!("\n=== TEST 1: With exclude_filenames=true, frequency_search=false ===");
    let options1 = SearchOptions {
        path: temp_dir.path(),
        queries: &queries,
        files_only: false,
        custom_ignores: &custom_ignores,
        exclude_filenames: true,
        language: None,
        reranker: "hybrid",
        frequency_search: false,
        max_results: None,
        max_bytes: None,
        max_tokens: None,
        allow_tests: false,
        no_merge: true,
        merge_threshold: None,
        dry_run: false,
        session: None,
        timeout: 30,
        question: None,
        exact: false,
        no_gitignore: false,
        lsp: false,
    };

    let search_results1 = perform_probe(&options1)?;
    println!("Results count: {}", search_results1.results.len());
    for result in &search_results1.results {
        println!("  - {}: {} (line {})", result.file, result.code.trim(), result.line);
    }

    // Second try: with filename matching enabled and frequency search enabled
    println!("\n=== TEST 2: With exclude_filenames=false, frequency_search=true ===");
    let options2 = SearchOptions {
        path: temp_dir.path(),
        queries: &queries,
        files_only: false,
        custom_ignores: &custom_ignores,
        exclude_filenames: false, // Enable filename matching
        language: None,
        reranker: "hybrid",
        frequency_search: true, // Enable frequency search
        max_results: None,
        max_bytes: None,
        max_tokens: None,
        allow_tests: false,
        no_merge: true,
        merge_threshold: None,
        dry_run: false,
        session: None,
        timeout: 30,
        question: None,
        exact: false,
        no_gitignore: false,
        lsp: false,
    };

    let search_results2 = perform_probe(&options2)?;
    println!("Results count: {}", search_results2.results.len());
    for result in &search_results2.results {
        println!("  - {}: {} (line {})", result.file, result.code.trim(), result.line);
    }

    // Third try: search for individual terms
    println!("\n=== TEST 3: Individual term searches ===");
    for query in &queries {
        println!("\nSearching for: '{}'", query);
        let single_query = vec![query.clone()];
        let options3 = SearchOptions {
            path: temp_dir.path(),
            queries: &single_query,
            files_only: false,
            custom_ignores: &custom_ignores,
            exclude_filenames: false,
            language: None,
            reranker: "hybrid",
            frequency_search: true,
            max_results: None,
            max_bytes: None,
            max_tokens: None,
            allow_tests: false,
            no_merge: true,
            merge_threshold: None,
            dry_run: false,
            session: None,
            timeout: 30,
            question: None,
            exact: false,
            no_gitignore: false,
            lsp: false,
        };

        let search_results3 = perform_probe(&options3)?;
        println!("Results count: {}", search_results3.results.len());
        for result in &search_results3.results {
            println!("  - {}: {} (line {})", result.file, result.code.trim(), result.line);
        }
    }

    Ok(())
}