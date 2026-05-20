use probe_code::extract::process_file_for_extraction;
use probe_code::extract::symbols::extract_symbols;
use probe_code::search::{perform_probe, SearchOptions};
use std::path::PathBuf;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/solidity/project1")
}

#[test]
fn test_solidity_symbols_extract_contract_members() {
    let file = fixture_root().join("contracts/GovernorExample.sol");
    let symbols = extract_symbols(&file, false).expect("symbols should parse Solidity");

    let top_names: Vec<_> = symbols
        .symbols
        .iter()
        .map(|symbol| symbol.name.as_str())
        .collect();
    assert!(
        top_names.contains(&"IVotesLike"),
        "top symbols: {top_names:?}"
    );
    assert!(
        top_names.contains(&"VoteMath"),
        "top symbols: {top_names:?}"
    );
    assert!(
        top_names.contains(&"GovernorExample"),
        "top symbols: {top_names:?}"
    );

    let governor = symbols
        .symbols
        .iter()
        .find(|symbol| symbol.name == "GovernorExample")
        .expect("missing GovernorExample contract");
    let child_names: Vec<_> = governor
        .children
        .iter()
        .map(|symbol| symbol.name.as_str())
        .collect();

    assert!(
        child_names.contains(&"ProposalState"),
        "children: {child_names:?}"
    );
    assert!(
        child_names.contains(&"ProposalCore"),
        "children: {child_names:?}"
    );
    assert!(
        child_names.contains(&"ProposalCreated"),
        "children: {child_names:?}"
    );
    assert!(
        child_names.contains(&"GovernorUnexpectedProposalState"),
        "children: {child_names:?}"
    );
    assert!(
        child_names.contains(&"onlyActive"),
        "children: {child_names:?}"
    );
    assert!(
        child_names.contains(&"constructor"),
        "children: {child_names:?}"
    );
    assert!(
        child_names.contains(&"propose"),
        "children: {child_names:?}"
    );
    assert!(
        child_names.contains(&"castVote"),
        "children: {child_names:?}"
    );
}

#[test]
fn test_solidity_symbol_extraction_by_name() {
    let file = fixture_root().join("contracts/GovernorExample.sol");
    let results = process_file_for_extraction(
        &file,
        None,
        None,
        Some("castVote"),
        true,
        0,
        None,
        false,
        false,
    )
    .expect("extract should find Solidity function");

    let code = &results.code;
    assert!(code.contains("function castVote"));
    assert!(code.contains("onlyActive(proposalId)"));
    assert!(
        !code.contains("function state("),
        "should extract only castVote block"
    );
}

#[test]
fn test_solidity_search_language_filter_and_test_exclusion() {
    let root = fixture_root();
    let query = "votingDelay".to_string();
    let options = SearchOptions {
        path: &root,
        queries: &[query],
        files_only: false,
        custom_ignores: &[],
        exclude_filenames: false,
        reranker: "bm25",
        frequency_search: true,
        exact: false,
        language: Some("solidity"),
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

    let results = perform_probe(&options).expect("search should support Solidity language filter");
    assert!(!results.results.is_empty());
    assert!(results
        .results
        .iter()
        .all(|result| result.file.ends_with("GovernorExample.sol")));
    assert!(results
        .results
        .iter()
        .all(|result| !result.file.ends_with(".t.sol")));
}
