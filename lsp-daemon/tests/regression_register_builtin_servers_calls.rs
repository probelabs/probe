//! Focused regression for UID mapping and Calls edges in lsp_registry.rs.
//!
//! This test constructs a minimal CallHierarchyResult for two concrete symbols
//! inside `lsp-daemon/src/lsp_registry.rs`:
//! - `LspRegistry::new` (line ~73) – the caller
//! - `register_builtin_servers` (line ~89) – the callee
//!
//! It then verifies that LspDatabaseAdapter converts the hierarchy into:
//! - a Calls edge from `new` -> `register_builtin_servers`
//! - (in a second case) a Calls edge from `register_builtin_servers` -> `register`

use anyhow::Result;
use lsp_daemon::lsp_database_adapter::LspDatabaseAdapter;
use lsp_daemon::protocol::{CallHierarchyCall, CallHierarchyItem, CallHierarchyResult, Position, Range};
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    // Tests run from the workspace root; current_dir should be the repo.
    std::env::current_dir().expect("cwd")
}

fn lsp_registry_path() -> PathBuf {
    repo_root().join("lsp-daemon/src/lsp_registry.rs")
}

fn file_uri(path: &PathBuf) -> String {
    format!("file://{}", path.display())
}

fn mk_item(name: &str, uri: &str, line: u32, character: u32) -> CallHierarchyItem {
    CallHierarchyItem {
        name: name.to_string(),
        kind: "function".to_string(),
        uri: uri.to_string(),
        range: Range {
            start: Position { line, character },
            end: Position { line: line + 1, character: 0 },
        },
        selection_range: Range {
            start: Position { line, character },
            end: Position { line, character: character + 1 },
        },
    }
}

#[test]
fn regression_calls_new_to_register_builtin_servers() -> Result<()> {
    let adapter = LspDatabaseAdapter::new();
    let file = lsp_registry_path();
    let uri = file_uri(&file);
    let workspace_root = repo_root();

    // Concrete positions observed in the source (line numbers are 0-based here):
    //   new() is declared at line 73
    //   register_builtin_servers() starts at line 89
    let item_new = mk_item("new", &uri, 73, 3);
    let item_register_builtin = mk_item("register_builtin_servers", &uri, 89, 5);

    let result = CallHierarchyResult {
        item: item_new.clone(),
        incoming: vec![],
        outgoing: vec![CallHierarchyCall {
            from: item_register_builtin.clone(),
            from_ranges: vec![Range {
                start: Position { line: 79, character: 8 }, // call site within new()
                end: Position { line: 79, character: 34 },
            }],
        }],
    };

    let (_symbols, edges) = adapter.convert_call_hierarchy_to_database(
        &result,
        &file,
        "rust",
        1,
        &workspace_root,
    )?;

    assert!(
        edges.iter().any(|e| {
            matches!(e.relation, lsp_daemon::database::EdgeRelation::Calls)
                && e.source_symbol_uid.contains(":new:")
                && e.target_symbol_uid.contains(":register_builtin_servers:")
        }),
        "Expected a Calls edge new -> register_builtin_servers, got: {:?}",
        edges
    );

    Ok(())
}

#[test]
fn regression_calls_register_builtin_servers_to_register() -> Result<()> {
    let adapter = LspDatabaseAdapter::new();
    let file = lsp_registry_path();
    let uri = file_uri(&file);
    let workspace_root = repo_root();

    // register_builtin_servers at line 89 calls self.register (first call appears around ~91)
    let item_register_builtin = mk_item("register_builtin_servers", &uri, 89, 5);
    let item_register = mk_item("register", &uri, 409, 8);

    let result = CallHierarchyResult {
        item: item_register_builtin.clone(),
        incoming: vec![],
        outgoing: vec![CallHierarchyCall {
            from: item_register.clone(),
            from_ranges: vec![Range {
                start: Position { line: 91, character: 12 },
                end: Position { line: 91, character: 20 },
            }],
        }],
    };

    let (_symbols, edges) = adapter.convert_call_hierarchy_to_database(
        &result,
        &file,
        "rust",
        1,
        &workspace_root,
    )?;

    assert!(
        edges.iter().any(|e| {
            matches!(e.relation, lsp_daemon::database::EdgeRelation::Calls)
                && e.source_symbol_uid.contains(":register_builtin_servers:")
                && e.target_symbol_uid.contains(":register:")
        }),
        "Expected a Calls edge register_builtin_servers -> register, got: {:?}",
        edges
    );

    Ok(())
}

