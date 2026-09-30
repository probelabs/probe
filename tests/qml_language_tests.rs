use probe_code::extract::process_file_for_extraction;
use probe_code::extract::symbols::extract_symbols;
use probe_code::query::{perform_query, QueryOptions};
use probe_code::search::filters::SearchFilters;
use probe_code::search::{perform_probe, SearchOptions};
use std::path::{Path, PathBuf};

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/qml/project1")
}

fn card_file() -> PathBuf {
    fixture_root().join("src/components/RequirementCard.qml")
}

#[test]
fn test_qml_symbols_extract_objects_properties_signals() {
    let symbols = extract_symbols(&card_file(), false).expect("symbols should parse QML");

    let dump: Vec<String> = symbols
        .symbols
        .iter()
        .map(|symbol| format!("{}:{}", symbol.name, symbol.kind))
        .collect();

    assert!(
        symbols
            .symbols
            .iter()
            .any(|symbol| symbol.name == "Item" && symbol.kind == "class"),
        "top symbols: {dump:?}"
    );

    let item = symbols
        .symbols
        .iter()
        .find(|symbol| symbol.name == "Item")
        .expect("missing Item object");
    let child_names: Vec<_> = item
        .children
        .iter()
        .map(|symbol| symbol.name.as_str())
        .collect();

    assert!(
        child_names.contains(&"reqId"),
        "children: {child_names:?}"
    );
    assert!(
        child_names.contains(&"requirementActivated"),
        "children: {child_names:?}"
    );
    assert!(
        child_names.contains(&"activate"),
        "children: {child_names:?}"
    );
}

#[test]
fn test_qml_symbol_extraction_by_name() {
    let results = process_file_for_extraction(
        &card_file(),
        None,
        None,
        Some("activate"),
        true,
        0,
        None,
        false,
        false,
    )
    .expect("extract should find QML function");

    let code = &results.code;
    assert!(code.contains("function activate()"));
    assert!(code.contains("requirementActivated(reqId)"));
}

#[test]
fn test_qml_extraction_by_line_target() {
    let content = std::fs::read_to_string(card_file()).expect("fixture should be readable");
    let target_line = content
        .lines()
        .position(|line| line.contains("requirementActivated(reqId)"))
        .map(|index| index + 1)
        .expect("fixture should contain activate body");

    let results = process_file_for_extraction(
        &card_file(),
        Some(target_line),
        Some(target_line),
        None,
        true,
        0,
        None,
        false,
        false,
    )
    .expect("extract should find enclosing QML function from line target");

    let code = &results.code;
    assert!(code.contains("function activate()"));
}

#[test]
fn test_qml_query_support() {
    // NOTE: tree-sitter-qmljs only forms standalone ast-grep pattern nodes for
    // object definitions, so query with an object pattern here.
    let options = QueryOptions {
        path: &fixture_root(),
        pattern: "MouseArea {\n  $$$\n}",
        language: Some("qml"),
        ignore: &[],
        allow_tests: true,
        max_results: Some(20),
        format: "terminal",
        no_gitignore: true,
        with_context: false,
        strict: false,
        text_extensions: &[],
    };

    let matches = perform_query(&options).expect("QML query should run");
    assert!(
        matches
            .iter()
            .any(|m| m
                .file_path
                .ends_with(Path::new("src/components/RequirementCard.qml"))
                && m.matched_text.contains("onClicked: card.activate()")),
        "matches: {:?}",
        matches
            .iter()
            .map(|m| (&m.file_path, &m.matched_text))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_qml_query_auto_detect_support() {
    let options = QueryOptions {
        path: &fixture_root(),
        pattern: "RequirementCard {\n  $$$\n}",
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

    let matches = perform_query(&options).expect("QML query should auto-detect .qml files");
    assert!(
        matches
            .iter()
            .any(|m| m.file_path.ends_with(Path::new("src/Main.qml"))
                && m.matched_text.contains("reqId")),
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
fn test_qml_search_language_filter_and_test_exclusion() {
    let root = fixture_root();
    let query = "requirementActivated".to_string();
    let options = search_options(&root, &query, Some("qml"), false);

    let results = perform_probe(&options).expect("search should support QML language filter");
    assert!(!results.results.is_empty());
    assert!(results
        .results
        .iter()
        .all(|result| result.file.ends_with(".qml")));
    assert!(results
        .results
        .iter()
        .all(|result| !result.file.contains("tst_")));
}

#[test]
fn test_qml_search_excludes_direct_test_paths() {
    let tests_root = fixture_root().join("tests");
    let query = "TestCase AND lang:qml".to_string();
    let options = search_options(&tests_root, &query, None, false);

    let results = perform_probe(&options).expect("search should handle direct QML test paths");
    assert!(
        results.results.is_empty(),
        "QML tst_* files should be excluded without --allow-tests: {:?}",
        results
            .results
            .iter()
            .map(|result| &result.file)
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_qml_search_language_hint_filter() {
    let root = fixture_root();
    let query = "incrementCount AND lang:qml".to_string();
    let options = search_options(&root, &query, None, false);

    let results = perform_probe(&options).expect("search should support QML lang: hint");
    assert!(
        results
            .results
            .iter()
            .any(|result| result.file.ends_with("src/Main.qml")),
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
        .all(|result| result.file.ends_with(".qml")));
}

#[test]
fn test_qml_language_alias_filter() {
    let mut filters = SearchFilters::new();
    filters.add_filter("lang", vec!["qml".to_string()]);
    assert_eq!(filters.languages, vec!["qml"]);
    assert!(filters.matches_file(Path::new("src/components/RequirementCard.qml")));
    assert!(!filters.matches_file(Path::new("src/components/RequirementCard.js")));
}
