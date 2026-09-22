#[cfg(test)]
mod tests {
    use probe_code::search::{perform_probe, SearchOptions};
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_search_functionality() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("sample.rs");
        fs::write(
            &file_path,
            r#"
fn lib_usage_search_sample() {
    // unique_library_search_marker
}
"#,
        )
        .unwrap();

        // Create search options
        let options = SearchOptions {
            path: temp_dir.path(),
            queries: &["unique_library_search_marker".to_string()],
            files_only: false,
            custom_ignores: &[],
            exclude_filenames: false,
            reranker: "bm25",
            frequency_search: true,
            exact: false,
            language: None,
            max_results: Some(5),
            max_bytes: None,
            max_tokens: None,
            allow_tests: true,
            no_merge: false,
            merge_threshold: None,
            dry_run: false,
            session: None,
            timeout: 30,
            question: None,
            no_gitignore: false,
            lsp: false,
        };

        let results = perform_probe(&options).unwrap();

        // Just check that we get some results
        assert!(!results.results.is_empty());
        println!("Found {} results", results.results.len());
    }

    #[test]
    fn test_query_functionality() {
        use probe_code::query::{perform_query, QueryOptions};

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("sample.rs");
        fs::write(
            &file_path,
            r#"
fn lib_usage_query_sample(value: i32) -> i32 {
    value + 1
}
"#,
        )
        .unwrap();

        let options = QueryOptions {
            path: temp_dir.path(),
            pattern: "fn $NAME($$$PARAMS) $$$BODY",
            language: Some("rust"),
            ignore: &[],
            allow_tests: true,
            max_results: Some(5),
            with_context: false,
            format: "text",
            no_gitignore: false,
            strict: false,
            text_extensions: &[],
        };

        let matches = perform_query(&options).unwrap();

        // Just check that we get some results
        assert!(!matches.is_empty());
        println!("Found {} matches", matches.len());
    }
}
