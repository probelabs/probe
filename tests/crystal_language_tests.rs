use probe_code::extract::process_file_for_extraction;
use probe_code::extract::symbols::extract_symbols;
use probe_code::query::{perform_query, QueryOptions};
use probe_code::search::filters::SearchFilters;
use probe_code::search::{perform_probe, SearchOptions};
use probe_code::semantic_context::build_query_source_context;
use std::path::{Path, PathBuf};

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/crystal/project1")
}

fn user_file() -> PathBuf {
    fixture_root().join("src/models/user.cr")
}

#[test]
fn test_crystal_symbols_extract_modules_and_members() {
    let symbols = extract_symbols(&user_file(), false).expect("symbols should parse Crystal");

    let top_names: Vec<_> = symbols
        .symbols
        .iter()
        .map(|symbol| symbol.name.as_str())
        .collect();
    assert!(
        top_names.contains(&"ProbeFixture"),
        "top symbols: {top_names:?}"
    );

    let module = symbols
        .symbols
        .iter()
        .find(|symbol| symbol.name == "ProbeFixture")
        .expect("missing ProbeFixture module");
    let child_names: Vec<_> = module
        .children
        .iter()
        .map(|symbol| symbol.name.as_str())
        .collect();

    assert!(child_names.contains(&"UserId"), "children: {child_names:?}");
    assert!(child_names.contains(&"Route"), "children: {child_names:?}");
    assert!(child_names.contains(&"Role"), "children: {child_names:?}");
    assert!(
        child_names.contains(&"Address"),
        "children: {child_names:?}"
    );
    assert!(
        child_names.contains(&"Serializable"),
        "children: {child_names:?}"
    );
    assert!(child_names.contains(&"User"), "children: {child_names:?}");

    let user = module
        .children
        .iter()
        .find(|symbol| symbol.name == "User")
        .expect("missing User class");
    let user_child_names: Vec<_> = user
        .children
        .iter()
        .map(|symbol| symbol.name.as_str())
        .collect();
    assert!(
        user_child_names.contains(&"initialize"),
        "user children: {user_child_names:?}"
    );
    assert!(
        user_child_names.contains(&"guest"),
        "user children: {user_child_names:?}"
    );
    assert!(
        user_child_names.contains(&"active?"),
        "user children: {user_child_names:?}"
    );
    assert!(
        user_child_names.contains(&"serialize"),
        "user children: {user_child_names:?}"
    );
    assert!(
        user_child_names.contains(&"define_counter"),
        "user children: {user_child_names:?}"
    );
}

#[test]
fn test_crystal_symbol_extraction_by_name() {
    let results = process_file_for_extraction(
        &user_file(),
        None,
        None,
        Some("active?"),
        true,
        0,
        None,
        false,
        false,
    )
    .expect("extract should find Crystal method");

    let code = &results.code;
    assert!(code.contains("def active? : Bool"));
    assert!(code.contains("role != Role::Guest"));
    assert!(
        !code.contains("def serialize"),
        "should extract only active? block"
    );
}

#[test]
fn test_crystal_extraction_by_line_target() {
    let content = std::fs::read_to_string(user_file()).expect("fixture should be readable");
    let target_line = content
        .lines()
        .position(|line| line.contains("role != Role::Guest"))
        .map(|index| index + 1)
        .expect("fixture should contain active? method body");

    let results = process_file_for_extraction(
        &user_file(),
        Some(target_line),
        Some(target_line),
        None,
        true,
        0,
        None,
        false,
        false,
    )
    .expect("extract should find enclosing Crystal method from line target");

    let code = &results.code;
    assert!(code.contains("def active? : Bool"));
    assert!(code.contains("role != Role::Guest"));
    assert!(
        !code.contains("def serialize"),
        "line target should extract only the enclosing active? method"
    );
}

#[test]
fn test_crystal_query_support() {
    let options = QueryOptions {
        path: &fixture_root(),
        pattern: "def active? : Bool",
        language: Some("crystal"),
        ignore: &[],
        allow_tests: true,
        max_results: Some(20),
        with_context: false,
        format: "terminal",
        no_gitignore: true,
        strict: false,
        text_extensions: &[],
    };

    let matches = perform_query(&options).expect("Crystal query should run");
    assert!(
        matches
            .iter()
            .any(|m| m.file_path.ends_with(Path::new("src/models/user.cr"))
                && m.matched_text.contains("def active?")),
        "matches: {:?}",
        matches
            .iter()
            .map(|m| (&m.file_path, &m.matched_text))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_crystal_query_auto_detect_support() {
    let options = QueryOptions {
        path: &fixture_root(),
        pattern: "class User < Serializable",
        language: None,
        ignore: &[],
        allow_tests: true,
        max_results: Some(20),
        with_context: false,
        format: "terminal",
        no_gitignore: true,
        strict: false,
        text_extensions: &[],
    };

    let matches = perform_query(&options).expect("Crystal query should auto-detect .cr files");
    assert!(
        matches
            .iter()
            .any(|m| m.file_path.ends_with(Path::new("src/models/user.cr"))
                && m.matched_text.contains("class User < Serializable")),
        "matches: {:?}",
        matches
            .iter()
            .map(|m| (&m.file_path, &m.matched_text))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_crystal_search_language_filter_and_test_exclusion() {
    let root = fixture_root();
    let query = "MathTools".to_string();
    let options = SearchOptions {
        path: &root,
        queries: &[query],
        files_only: false,
        custom_ignores: &[],
        exclude_filenames: false,
        reranker: "bm25",
        frequency_search: true,
        exact: false,
        language: Some("crystal"),
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

    let results = perform_probe(&options).expect("search should support Crystal language filter");
    assert!(!results.results.is_empty());
    assert!(results
        .results
        .iter()
        .all(|result| result.file.ends_with(".cr")));
    assert!(results
        .results
        .iter()
        .all(|result| !result.file.ends_with("_spec.cr")));
}

#[test]
fn test_crystal_search_excludes_direct_spec_paths() {
    let spec_root = fixture_root().join("spec");
    let query = "describe AND lang:crystal".to_string();
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

    let results = perform_probe(&options).expect("search should handle direct Crystal spec paths");
    assert!(
        results.results.is_empty(),
        "Crystal spec files should be excluded without --allow-tests: {:?}",
        results
            .results
            .iter()
            .map(|result| &result.file)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_crystal_search_language_hint_filter() {
    let root = fixture_root();
    let query = "HTTP::Server AND lang:crystal".to_string();
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

    let results = perform_probe(&options).expect("search should support Crystal lang: hint");
    assert!(
        results
            .results
            .iter()
            .any(|result| result.file.ends_with("src/server.cr")),
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
        .all(|result| result.file.ends_with(".cr")));
}

#[test]
fn test_crystal_source_context_and_language_alias() {
    let content = std::fs::read_to_string(user_file()).expect("fixture should be readable");
    let byte_start = content
        .find("def active?")
        .expect("fixture should contain active? method");
    let byte_end = byte_start + "def active?".len();

    let context = build_query_source_context(&user_file(), byte_start, byte_end, "def active?")
        .expect("Crystal source context should parse");
    assert_eq!(context.language, "crystal");

    let mut filters = SearchFilters::new();
    filters.add_filter("lang", vec!["cr".to_string()]);
    assert_eq!(filters.languages, vec!["crystal"]);
    assert!(filters.matches_file(Path::new("src/models/user.cr")));
    assert!(!filters.matches_file(Path::new("src/models/user.rb")));
}
