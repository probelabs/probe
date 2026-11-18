use probe_code::extract::process_file_for_extraction;
use probe_code::extract::symbols::extract_symbols;
use probe_code::search::{perform_probe, SearchOptions};
use std::path::Path;

const CONFIG: &str = r#"title = "Workspace tools"

[[tools]]
name = "list_tasks"
source = "local"
statement = "SELECT title FROM tasks WHERE done = ?"
readonly = true

[[tools]]
name = "delete_tasks"
statement = "DELETE FROM tasks"
readonly = false

[tool."lint.rules"]
enabled = [
  "imports",
  "format",
]
"#;

fn write_config(dir: &Path) -> std::path::PathBuf {
    let path = dir.join("dbhub.toml");
    std::fs::write(&path, CONFIG).unwrap();
    path
}

#[test]
fn extract_toml_statement_keeps_its_tool_table() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_config(dir.path());
    let result =
        process_file_for_extraction(&path, Some(6), None, None, true, 0, None, false, false)
            .unwrap();
    assert!(result.code.contains("[[tools]]"));
    assert!(result.code.contains("name = \"list_tasks\""));
    assert!(result.code.contains("readonly = true"));
    assert!(!result.code.contains("delete_tasks"));
}

#[test]
fn extract_toml_multiline_value_keeps_quoted_table_header() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_config(dir.path());
    let result =
        process_file_for_extraction(&path, Some(17), None, None, true, 0, None, false, false)
            .unwrap();
    assert!(result.code.contains("[tool.\"lint.rules\"]"));
    assert!(result.code.contains("\"imports\""));
    assert!(result.code.contains("\"format\""));
    assert!(!result.code.contains("delete_tasks"));
}

#[test]
fn toml_symbols_show_tables_and_their_keys() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_config(dir.path());
    let result = extract_symbols(&path, false).unwrap();
    let names: Vec<_> = result.symbols.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["title", "tools", "tools", "tool.\"lint.rules\""]);
    let tool = &result.symbols[1];
    assert_eq!(tool.signature, "[[tools]]");
    assert_eq!(tool.line, 3);
    assert!(tool.children.iter().any(|s| s.name == "readonly"));
}

#[test]
fn extract_toml_dotted_table_by_name() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_config(dir.path());
    let result = process_file_for_extraction(
        &path,
        None,
        None,
        Some("tool.\"lint.rules\""),
        true,
        0,
        None,
        false,
        false,
    )
    .unwrap();
    assert_eq!(result.node_type, "table");
    assert!(result.code.contains("\"format\""));
    assert!(!result.code.contains("delete_tasks"));
}

#[test]
fn toml_search_language_filter_returns_complete_tool_table() {
    let dir = tempfile::tempdir().unwrap();
    write_config(dir.path());
    std::fs::write(dir.path().join("notes.txt"), "list_tasks unrelated").unwrap();
    let queries = ["list_tasks".to_string()];
    let result = perform_probe(&SearchOptions {
        path: dir.path(),
        queries: &queries,
        files_only: false,
        custom_ignores: &[],
        exclude_filenames: false,
        reranker: "bm25",
        frequency_search: true,
        exact: true,
        language: Some("toml"),
        max_results: None,
        max_bytes: None,
        max_tokens: None,
        allow_tests: true,
        no_merge: true,
        merge_threshold: None,
        lsp: false,
        dry_run: false,
        session: None,
        timeout: 30,
        question: None,
        no_gitignore: true,
    })
    .unwrap();
    assert!(!result.results.is_empty());
    assert!(result.results.iter().all(|r| r.file.ends_with(".toml")));
    assert!(result
        .results
        .iter()
        .any(|r| { r.code.contains("[[tools]]") && r.code.contains("readonly = true") }));
}

#[test]
fn extract_toml_root_multiline_pair() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.toml");
    std::fs::write(
        &path,
        "include = [\n  \"src\",\n  \"docs\",\n]\n[agent]\nname = \"reader\"\n",
    )
    .unwrap();
    let result =
        process_file_for_extraction(&path, Some(2), None, None, true, 0, None, false, false)
            .unwrap();
    assert!(result.code.contains("include = ["));
    assert!(result.code.contains("\"docs\""));
    assert!(!result.code.contains("[agent]"));
}

#[test]
fn cli_accepts_toml_search_and_extracts_table_context() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_config(dir.path());
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_probe"))
        .args([
            "search",
            "list_tasks",
            "--language",
            "toml",
            "--exact",
            "--format",
            "plain",
        ])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("readonly = true"), "{text}");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_probe"))
        .args([
            "extract",
            &format!("{}:6", path.display()),
            "--format",
            "plain",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("[[tools]]"), "{text}");
    assert!(text.contains("source = \"local\""), "{text}");
    assert!(!text.contains("delete_tasks"), "{text}");
}

#[test]
fn adjacent_toml_tables_do_not_leak_the_next_header() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dbhub.toml");
    std::fs::write(
        &path,
        "[[tools]]\nname = 'reader'\nreadonly = true\n[[tools.parameters]]\nname = 'done'\n",
    )
    .unwrap();
    let result =
        process_file_for_extraction(&path, Some(2), None, None, true, 0, None, false, false)
            .unwrap();
    assert_eq!(result.lines, (1, 3));
    assert!(result.code.contains("readonly = true"));
    assert!(!result.code.contains("tools.parameters"));
    let symbols = extract_symbols(&path, false).unwrap();
    assert_eq!(symbols.symbols[0].end_line, 3);
    assert_eq!(symbols.symbols[1].line, 4);
}

#[test]
fn repeated_toml_extraction_keeps_context_for_previously_unvisited_lines() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("workspace.toml");
    std::fs::write(&path, format!("{CONFIG}\n# Sequential extraction\n")).unwrap();
    for (line, expected) in [
        (17, "[tool.\"lint.rules\"]"),
        (6, "[[tools]]"),
        (17, "[tool.\"lint.rules\"]"),
    ] {
        let result =
            process_file_for_extraction(&path, Some(line), None, None, true, 0, None, false, false)
                .unwrap();
        assert!(result.code.contains(expected), "{}", result.code);
    }
}

#[test]
fn toml_keys_resolve_under_simple_and_dotted_tables() {
    let dir = tempfile::tempdir().unwrap();
    let path = write_config(dir.path());
    let result = process_file_for_extraction(
        &path,
        None,
        None,
        Some("tools.readonly"),
        true,
        0,
        None,
        false,
        false,
    )
    .unwrap();
    assert_eq!(result.node_type, "pair");
    assert_eq!(result.code.trim(), "readonly = true");

    let result = process_file_for_extraction(
        &path,
        None,
        None,
        Some("tool.\"lint.rules\".enabled"),
        true,
        0,
        None,
        false,
        false,
    )
    .unwrap();
    assert_eq!(result.node_type, "pair");
    assert!(result.code.starts_with("enabled = ["));
    assert!(result.code.contains("\"format\""));
    assert!(!result.code.contains("[[tools]]"));
}
