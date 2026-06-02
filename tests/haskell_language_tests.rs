use probe_code::extract::process_file_for_extraction;
use probe_code::extract::symbols::extract_symbols;
use probe_code::query::{perform_query, QueryOptions};
use probe_code::search::filters::SearchFilters;
use probe_code::search::{perform_probe, SearchOptions};
use probe_code::semantic_context::build_query_source_context;
use std::path::{Path, PathBuf};

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/haskell/project1")
}

fn sample_file() -> PathBuf {
    fixture_root().join("src/Demo/Sample.hs")
}

#[test]
fn test_haskell_symbols_extract_types_classes_and_functions() {
    let symbols = extract_symbols(&sample_file(), false).expect("symbols should parse Haskell");

    let names: Vec<_> = symbols
        .symbols
        .iter()
        .map(|symbol| symbol.name.as_str())
        .collect();

    assert!(names.contains(&"UserId"), "symbols: {names:?}");
    assert!(names.contains(&"Role"), "symbols: {names:?}");
    assert!(names.contains(&"Email"), "symbols: {names:?}");
    assert!(names.contains(&"User"), "symbols: {names:?}");
    assert!(names.contains(&"Serializable"), "symbols: {names:?}");
    assert!(names.contains(&"active"), "symbols: {names:?}");
    assert!(names.contains(&"loadUser"), "symbols: {names:?}");
    assert!(
        symbols
            .symbols
            .iter()
            .flat_map(|symbol| symbol.children.iter())
            .all(|child| child.name != "class" && child.name != "instance"),
        "keyword tokens should not be emitted as child symbols: {:?}",
        symbols.symbols
    );
}

#[test]
fn test_haskell_symbol_extraction_by_name() {
    let results = process_file_for_extraction(
        &sample_file(),
        None,
        None,
        Some("active"),
        true,
        0,
        None,
        false,
        false,
    )
    .expect("extract should find Haskell function");

    let code = &results.code;
    assert!(code.contains("active user = userRole user /= Guest"));
    assert!(!code.contains("loadUser uid"));
}

#[test]
fn test_haskell_extraction_by_line_target() {
    let content = std::fs::read_to_string(sample_file()).expect("fixture should be readable");
    let target_line = content
        .lines()
        .position(|line| line.contains("userRole user /= Guest"))
        .map(|index| index + 1)
        .expect("fixture should contain active body");

    let results = process_file_for_extraction(
        &sample_file(),
        Some(target_line),
        Some(target_line),
        None,
        true,
        0,
        None,
        false,
        false,
    )
    .expect("extract should find enclosing Haskell function");

    let code = &results.code;
    assert!(code.contains("active user = userRole user /= Guest"));
    assert!(!code.contains("loadUser uid"));
}

#[test]
fn test_haskell_operator_symbol_extraction_uses_tree_sitter() {
    let plus = process_file_for_extraction(
        &sample_file(),
        None,
        None,
        Some("(<+>)"),
        true,
        0,
        None,
        false,
        false,
    )
    .expect("extract should find Haskell prefix operator definition");

    assert_eq!(plus.node_type, "bind");
    assert!(plus.code.contains("(<+>) = (++)"), "code: {}", plus.code);

    let arrow = process_file_for_extraction(
        &sample_file(),
        None,
        None,
        Some("(-->)"),
        true,
        0,
        None,
        false,
        false,
    )
    .expect("extract should find Haskell infix operator signature");

    assert_eq!(arrow.node_type, "signature");
    assert!(
        arrow.code.contains("(-->) :: Bool -> a -> Maybe a"),
        "code: {}",
        arrow.code
    );
}

#[test]
fn test_haskell_query_support() {
    let options = QueryOptions {
        path: &fixture_root(),
        pattern: "active user = userRole user /= Guest",
        language: Some("haskell"),
        ignore: &[],
        allow_tests: true,
        max_results: Some(20),
        with_context: false,
        format: "terminal",
        no_gitignore: true,
    };

    let matches = perform_query(&options).expect("Haskell query should run");
    assert!(
        matches
            .iter()
            .any(|m| m.file_path.ends_with(Path::new("src/Demo/Sample.hs"))
                && m.matched_text.contains("active user")),
        "matches: {:?}",
        matches
            .iter()
            .map(|m| (&m.file_path, &m.matched_text))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_haskell_query_auto_detect_support() {
    let options = QueryOptions {
        path: &fixture_root(),
        pattern: "data Role = Admin | Guest",
        language: None,
        ignore: &[],
        allow_tests: true,
        max_results: Some(20),
        with_context: false,
        format: "terminal",
        no_gitignore: true,
    };

    let matches = perform_query(&options).expect("Haskell query should auto-detect .hs files");
    assert!(
        matches
            .iter()
            .any(|m| m.file_path.ends_with(Path::new("src/Demo/Sample.hs"))
                && m.matched_text.contains("data Role = Admin | Guest")),
        "matches: {:?}",
        matches
            .iter()
            .map(|m| (&m.file_path, &m.matched_text))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_haskell_search_language_filter_and_test_exclusion() {
    let root = fixture_root();
    let query = "Serializable".to_string();
    let options = SearchOptions {
        path: &root,
        queries: &[query],
        files_only: false,
        custom_ignores: &[],
        exclude_filenames: false,
        reranker: "bm25",
        frequency_search: true,
        exact: false,
        language: Some("haskell"),
        max_results: Some(20),
        max_bytes: None,
        max_tokens: None,
        allow_tests: false,
        no_merge: false,
        merge_threshold: None,
        lsp: false,
        dry_run: false,
        session: None,
        timeout: 30,
        question: None,
        no_gitignore: true,
    };

    let results = perform_probe(&options).expect("search should support Haskell language filter");
    assert!(!results.results.is_empty());
    assert!(results
        .results
        .iter()
        .all(|result| result.file.ends_with(".hs")));
    assert!(results
        .results
        .iter()
        .all(|result| !result.file.ends_with("Spec.hs")));
}

#[test]
fn test_haskell_search_excludes_direct_spec_paths() {
    let spec_root = fixture_root().join("test");
    let query = "describe AND lang:haskell".to_string();
    let options = SearchOptions {
        path: &spec_root,
        queries: &[query],
        files_only: false,
        custom_ignores: &[],
        exclude_filenames: false,
        reranker: "bm25",
        frequency_search: true,
        exact: false,
        language: None,
        max_results: Some(20),
        max_bytes: None,
        max_tokens: None,
        allow_tests: false,
        no_merge: false,
        merge_threshold: None,
        lsp: false,
        dry_run: false,
        session: None,
        timeout: 30,
        question: None,
        no_gitignore: true,
    };

    let results = perform_probe(&options).expect("search should handle direct Haskell spec paths");
    assert!(
        results.results.is_empty(),
        "Haskell spec files should be excluded without --allow-tests: {:?}",
        results
            .results
            .iter()
            .map(|result| &result.file)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_haskell_search_language_hint_filter() {
    let root = fixture_root();
    let query = "loadUser AND lang:hs".to_string();
    let options = SearchOptions {
        path: &root,
        queries: &[query],
        files_only: false,
        custom_ignores: &[],
        exclude_filenames: false,
        reranker: "bm25",
        frequency_search: true,
        exact: false,
        language: None,
        max_results: Some(20),
        max_bytes: None,
        max_tokens: None,
        allow_tests: false,
        no_merge: false,
        merge_threshold: None,
        lsp: false,
        dry_run: false,
        session: None,
        timeout: 30,
        question: None,
        no_gitignore: true,
    };

    let results = perform_probe(&options).expect("search should support Haskell lang: hint");
    assert!(
        results
            .results
            .iter()
            .any(|result| result.file.ends_with("src/Demo/Sample.hs")),
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
        .all(|result| result.file.ends_with(".hs")));
}

#[test]
fn test_haskell_source_context_and_language_alias() {
    let content = std::fs::read_to_string(sample_file()).expect("fixture should be readable");
    let byte_start = content
        .find("active user")
        .expect("fixture should contain active function");
    let byte_end = byte_start + "active user".len();

    let context = build_query_source_context(&sample_file(), byte_start, byte_end, "active")
        .expect("Haskell source context should parse");
    assert_eq!(context.language, "haskell");

    let mut filters = SearchFilters::new();
    filters.add_filter("lang", vec!["lhs".to_string()]);
    assert_eq!(filters.languages, vec!["haskell"]);
    assert!(filters.matches_file(Path::new("src/Demo/Sample.hs")));
    assert!(filters.matches_file(Path::new("src/Demo/Tutorial.lhs")));
    assert!(!filters.matches_file(Path::new("src/Demo/Sample.cr")));
}
