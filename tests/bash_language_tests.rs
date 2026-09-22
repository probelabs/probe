use probe_code::extract::process_file_for_extraction;
use probe_code::extract::symbols::extract_symbols;
use probe_code::query::{perform_query, QueryOptions};
use probe_code::search::filters::SearchFilters;
use probe_code::search::{perform_probe, SearchOptions};
use std::path::{Path, PathBuf};

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bash/project1")
}

fn utils_file() -> PathBuf {
    fixture_root().join("src/lib/utils.sh")
}

#[test]
fn test_bash_symbols_extract_functions_and_variables() {
    let symbols = extract_symbols(&utils_file(), false).expect("symbols should parse Bash");

    let names: Vec<_> = symbols
        .symbols
        .iter()
        .map(|symbol| (symbol.name.as_str(), symbol.kind.as_str()))
        .collect();

    assert!(
        names.iter().any(|(name, _)| *name == "trim"),
        "symbols: {names:?}"
    );
    assert!(
        names.iter().any(|(name, _)| *name == "to_upper"),
        "symbols: {names:?}"
    );
    assert!(
        names.iter().any(|(name, _)| *name == "join_by"),
        "symbols: {names:?}"
    );
}

#[test]
fn test_bash_symbol_extraction_by_name() {
    let results = process_file_for_extraction(
        &utils_file(),
        None,
        None,
        Some("to_upper"),
        true,
        0,
        None,
        false,
        false,
    )
    .expect("extract should find Bash function");

    let code = &results.code;
    assert!(code.contains("to_upper()"));
    assert!(
        !code.contains("join_by()"),
        "should extract only the to_upper block"
    );
}

#[test]
fn test_bash_extraction_by_line_target() {
    let content = std::fs::read_to_string(utils_file()).expect("fixture should be readable");
    let target_line = content
        .lines()
        .position(|line| line.contains("tr '[:lower:]'"))
        .map(|index| index + 1)
        .expect("fixture should contain to_upper body");

    let results = process_file_for_extraction(
        &utils_file(),
        Some(target_line),
        Some(target_line),
        None,
        true,
        0,
        None,
        false,
        false,
    )
    .expect("extract should find enclosing Bash function from line target");

    let code = &results.code;
    assert!(code.contains("to_upper()"));
    assert!(
        !code.contains("join_by()"),
        "line target should extract only the enclosing to_upper function"
    );
}

#[test]
fn test_bash_query_support() {
    let options = QueryOptions {
        path: &fixture_root(),
        pattern: "join_by() {\n  $$$\n}",
        language: Some("bash"),
        ignore: &[],
        allow_tests: true,
        max_results: Some(20),
        format: "terminal",
        no_gitignore: true,
        with_context: false,
        strict: false,
        text_extensions: &[],
    };

    let matches = perform_query(&options).expect("Bash query should run");
    assert!(
        matches
            .iter()
            .any(|m| m.file_path.ends_with(Path::new("src/lib/utils.sh"))
                && m.matched_text.contains("join_by")),
        "matches: {:?}",
        matches
            .iter()
            .map(|m| (&m.file_path, &m.matched_text))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_bash_query_auto_detect_support() {
    let options = QueryOptions {
        path: &fixture_root(),
        pattern: "deploy_app() {\n  $$$\n}",
        language: None,
        ignore: &[],
        allow_tests: true,
        max_results: Some(20),
        format: "terminal",
        no_gitignore: true,
        with_context: false,
        strict: false,
        text_extensions: &[],
    };

    let matches = perform_query(&options).expect("Bash query should auto-detect .sh files");
    assert!(
        matches
            .iter()
            .any(|m| m.file_path.ends_with(Path::new("src/deploy.sh"))
                && m.matched_text.contains("deploy_app")),
        "matches: {:?}",
        matches
            .iter()
            .map(|m| (&m.file_path, &m.matched_text))
            .collect::<Vec<_>>()
    );
}

fn search_options<'a>(
    root: &'a Path,
    query: &'a String,
    language: Option<&'a str>,
    allow_tests: bool,
) -> SearchOptions<'a> {
    SearchOptions {
        path: root,
        queries: std::slice::from_ref(query),
        files_only: false,
        custom_ignores: &[],
        exclude_filenames: false,
        reranker: "bm25",
        frequency_search: true,
        exact: false,
        language,
        max_results: Some(20),
        max_bytes: None,
        max_tokens: None,
        allow_tests,
        no_merge: false,
        merge_threshold: None,
        dry_run: false,
        session: None,
        timeout: 30,
        question: None,
        no_gitignore: true,
        lsp: false,
    }
}

#[test]
fn test_bash_search_language_filter_and_test_exclusion() {
    let root = fixture_root();
    let query = "log_info".to_string();
    let options = search_options(&root, &query, Some("bash"), false);

    let results = perform_probe(&options).expect("search should support Bash language filter");
    assert!(!results.results.is_empty());
    assert!(results
        .results
        .iter()
        .all(|result| result.file.ends_with(".sh") || result.file.ends_with(".bash")));
    assert!(results
        .results
        .iter()
        .all(|result| !result.file.ends_with(".bats")));
}

#[test]
fn test_bash_search_excludes_direct_test_paths() {
    let tests_root = fixture_root().join("tests");
    let query = "trim AND lang:bash".to_string();
    let options = search_options(&tests_root, &query, None, false);

    let results = perform_probe(&options).expect("search should handle direct Bash test paths");
    assert!(
        results.results.is_empty(),
        "Bash test files should be excluded without --allow-tests: {:?}",
        results
            .results
            .iter()
            .map(|result| &result.file)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_bash_search_language_hint_filter() {
    let root = fixture_root();
    let query = "deploy_app AND lang:bash".to_string();
    let options = search_options(&root, &query, None, false);

    let results = perform_probe(&options).expect("search should support Bash lang: hint");
    assert!(
        results
            .results
            .iter()
            .any(|result| result.file.ends_with("src/deploy.sh")),
        "results: {:?}",
        results
            .results
            .iter()
            .map(|result| &result.file)
            .collect::<Vec<_>>()
    );
    assert!(results
        .results
        .iter()
        .all(|result| result.file.ends_with(".sh") || result.file.ends_with(".bash")));
}

#[test]
fn test_bash_language_alias_filter() {
    let mut filters = SearchFilters::new();
    filters.add_filter("lang", vec!["sh".to_string()]);
    assert_eq!(filters.languages, vec!["bash"]);
    assert!(filters.matches_file(Path::new("src/lib/utils.sh")));
    assert!(!filters.matches_file(Path::new("src/lib/utils.py")));
}

fn extensionless_tool() -> PathBuf {
    fixture_root().join("src/bin/extensionless-tool")
}

#[test]
fn test_extensionless_bash_symbols_via_shebang() {
    let symbols =
        extract_symbols(&extensionless_tool(), false).expect("shebang should select Bash grammar");

    let names: Vec<_> = symbols
        .symbols
        .iter()
        .map(|symbol| symbol.name.as_str())
        .collect();

    assert!(names.contains(&"tool_greet"), "symbols: {names:?}");
    assert!(names.contains(&"tool_dispatch"), "symbols: {names:?}");
}

#[test]
fn test_extensionless_bash_extract_by_symbol_name() {
    let results = process_file_for_extraction(
        &extensionless_tool(),
        None,
        None,
        Some("tool_greet"),
        true,
        0,
        None,
        false,
        false,
    )
    .expect("extract should work on extensionless bash via shebang");

    assert!(results.code.contains("tool_greet()"));
    assert!(
        !results.code.contains("tool_dispatch()"),
        "should extract only the tool_greet block"
    );
}

#[test]
fn test_extensionless_bash_search_with_language_filter() {
    let root = fixture_root();
    let query = "tool_dispatch".to_string();
    let options = search_options(&root, &query, Some("bash"), false);

    let results = perform_probe(&options).expect("search -l bash should see extensionless scripts");
    assert!(
        results
            .results
            .iter()
            .any(|result| result.file.ends_with("src/bin/extensionless-tool")),
        "results: {:?}",
        results
            .results
            .iter()
            .map(|result| &result.file)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_extensionless_non_shebang_file_unaffected() {
    // A text file without a shebang must not be treated as bash. Since the
    // plain-text fallback (#581), such files extract as raw text lines; the
    // key invariant is that no bash AST symbols are produced.
    let plain = fixture_root().join("src/bin/plain-data");
    let extracted =
        extract_symbols(&plain, false).expect("plain-text fallback extraction should succeed");
    assert!(
        extracted.symbols.iter().all(|s| s.kind == "text"),
        "non-shebang extensionless file must not produce bash symbols: {:?}",
        extracted.symbols
    );

    // Nor should it pass a bash language filter (it contains a decoy
    // `tool_greet` mention that must never surface in bash-filtered search).
    let root = fixture_root();
    let query = "tool_greet".to_string();
    let options = search_options(&root, &query, Some("bash"), false);
    let results = perform_probe(&options).expect("filtered search should run");
    assert!(
        !results
            .results
            .iter()
            .any(|result| result.file.ends_with("src/bin/plain-data")),
        "results: {:?}",
        results
            .results
            .iter()
            .map(|result| &result.file)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_language_filter_matches_extensionless_bash() {
    let mut filters = SearchFilters::new();
    filters.add_filter("lang", vec!["bash".to_string()]);
    assert!(filters.matches_file(&extensionless_tool()));
    assert!(!filters.matches_file(&fixture_root().join("src/bin/plain-data")));
}
