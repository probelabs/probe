#![cfg(feature = "legacy-tests")]
//! Integration test for empty LSP response handling
//!
//! This test verifies that when LSP returns empty results ([]),
//! the system correctly creates and stores "none" edges to cache
//! the empty state and avoid repeated LSP calls.

use anyhow::Result;
use lsp_daemon::database::sqlite_backend::SQLiteBackend;
use lsp_daemon::database::{create_none_call_hierarchy_edges, DatabaseBackend, DatabaseConfig};
use lsp_daemon::lsp_database_adapter::LspDatabaseAdapter;
use lsp_daemon::protocol::{CallHierarchyItem, CallHierarchyResult, Position, Range};
use std::path::Path;
use tempfile::TempDir;
use tracing::info;

async fn create_test_database() -> Result<(SQLiteBackend, TempDir)> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("empty_lsp_test.db");

    let config = DatabaseConfig {
        path: Some(db_path),
        temporary: false,
        cache_capacity: 1024 * 1024,
        ..Default::default()
    };

    let db = SQLiteBackend::new(config).await?;
    Ok((db, temp_dir))
}

/// Create a CallHierarchyResult that simulates what we get when LSP returns []
fn create_empty_lsp_result() -> CallHierarchyResult {
    CallHierarchyResult {
        // This is what parse_call_hierarchy_from_lsp returns for []
        item: CallHierarchyItem {
            name: "unknown".to_string(),
            kind: "unknown".to_string(),
            uri: "".to_string(),
            range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 0,
                },
            },
            selection_range: Range {
                start: Position {
                    line: 0,
                    character: 0,
                },
                end: Position {
                    line: 0,
                    character: 0,
                },
            },
        },
        incoming: vec![],
        outgoing: vec![],
    }
}

#[tokio::test]
async fn test_empty_lsp_response_creates_none_edges() -> Result<()> {
    let (database, _temp_dir) = create_test_database().await?;
    let adapter = LspDatabaseAdapter::new();

    // Simulate empty LSP response
    let empty_result = create_empty_lsp_result();
    let test_file = Path::new("/test/src/empty.rs");
    let workspace_root = Path::new("/test");
    let symbol_name = "EmptyFunction";
    let symbol_uid = format!("{}:{}:10:5", test_file.display(), symbol_name);

    // Convert to database format - should produce empty symbols and edges
    let (symbols, edges) = adapter.convert_call_hierarchy_to_database(
        &empty_result,
        test_file,
        "rust",
        1,
        workspace_root,
    )?;

    // Verify the conversion produces empty results (because item.name is "unknown")
    assert!(
        symbols.is_empty(),
        "Should not create symbols for unknown item"
    );
    assert!(
        edges.is_empty(),
        "Should not create edges for empty incoming/outgoing"
    );

    info!("✅ Empty LSP response correctly produces empty symbols/edges");

    // Now test the logic that should create "none" edges
    // This simulates what store_call_hierarchy_in_database_enhanced should do
    let edges_to_store =
        if edges.is_empty() && empty_result.incoming.is_empty() && empty_result.outgoing.is_empty()
        {
            info!("LSP returned empty call hierarchy, creating 'none' edges");
            let none_edges = create_none_call_hierarchy_edges(&symbol_uid, 1);
            assert_eq!(
                none_edges.len(),
                2,
                "Should create 2 none edges (incoming and outgoing)"
            );
            assert_eq!(none_edges[0].target_symbol_uid, "none");
            assert_eq!(none_edges[1].target_symbol_uid, "none");
            none_edges
        } else {
            edges
        };

    // Store the "none" edges
    database.store_edges(&edges_to_store).await?;
    info!(
        "✅ Successfully stored {} 'none' edges",
        edges_to_store.len()
    );

    // Verify we can retrieve them and they work for caching
    let workspace_id = 1i64;
    let result = database
        .get_call_hierarchy_for_symbol(workspace_id, &symbol_uid)
        .await?;

    assert!(result.is_some(), "Should return Some (cached empty result)");
    let hierarchy = result.unwrap();
    assert!(hierarchy.incoming.is_empty(), "Incoming should be empty");
    assert!(hierarchy.outgoing.is_empty(), "Outgoing should be empty");

    info!("✅ Cache correctly returns empty hierarchy (not None)");

    Ok(())
}

#[tokio::test]
async fn test_daemon_integration_with_empty_lsp() -> Result<()> {
    // This test would require a full daemon setup with mocked LSP server
    // For now, we test the core logic above

    let (database, _temp_dir) = create_test_database().await?;

    // Test the complete flow:
    // 1. First query returns None (cache miss)
    // 2. LSP returns []
    // 3. System creates "none" edges
    // 4. Second query returns Some([]) (cache hit)

    let symbol_uid = "src/test.rs:TestSymbol:20:10";
    let workspace_id = 1i64;

    // Step 1: Cache miss
    let first_result = database
        .get_call_hierarchy_for_symbol(workspace_id, symbol_uid)
        .await?;
    assert!(first_result.is_none(), "First query should be cache miss");
    info!("✅ Step 1: Cache miss returns None");

    // Step 2 & 3: Simulate LSP returning [] and creating "none" edges
    info!("Simulating LSP returning empty result []");
    let none_edges = create_none_call_hierarchy_edges(symbol_uid, 1);
    database.store_edges(&none_edges).await?;
    info!("✅ Step 2-3: Created and stored 'none' edges for empty LSP response");

    // Step 4: Cache hit
    let second_result = database
        .get_call_hierarchy_for_symbol(workspace_id, symbol_uid)
        .await?;
    assert!(second_result.is_some(), "Second query should be cache hit");
    let hierarchy = second_result.unwrap();
    assert!(
        hierarchy.incoming.is_empty() && hierarchy.outgoing.is_empty(),
        "Should return empty hierarchy"
    );
    info!("✅ Step 4: Cache hit returns empty hierarchy");

    // Verify no more LSP calls would be made
    for i in 0..3 {
        let result = database
            .get_call_hierarchy_for_symbol(workspace_id, symbol_uid)
            .await?;
        assert!(result.is_some(), "Query {} should hit cache", i + 3);
        info!("✅ Query {}: Cache hit (no LSP call needed)", i + 3);
    }

    Ok(())
}

#[tokio::test]
async fn test_none_edge_detection_logic() -> Result<()> {
    let (database, _temp_dir) = create_test_database().await?;

    // Test different scenarios
    let test_cases = vec![
        ("empty_function", vec![], vec![], true, "Empty LSP response"),
        (
            "has_incoming",
            vec!["caller1"],
            vec![],
            false,
            "Has incoming calls",
        ),
        (
            "has_outgoing",
            vec![],
            vec!["callee1"],
            false,
            "Has outgoing calls",
        ),
        (
            "has_both",
            vec!["caller1"],
            vec!["callee1"],
            false,
            "Has both calls",
        ),
    ];

    for (symbol_name, incoming, outgoing, should_create_none, description) in test_cases {
        info!("Testing: {}", description);

        let symbol_uid = format!("test.rs:{}:1:1", symbol_name);

        // Simulate different LSP responses
        let edges_count = incoming.len() + outgoing.len();
        let should_create_none_edges =
            edges_count == 0 && incoming.is_empty() && outgoing.is_empty();

        assert_eq!(
            should_create_none_edges, should_create_none,
            "None edge detection failed for {}",
            description
        );

        if should_create_none_edges {
            let none_edges = create_none_call_hierarchy_edges(&symbol_uid, 1);
            assert_eq!(
                none_edges.len(),
                2,
                "Should create 2 none edges for {}",
                description
            );
            database.store_edges(&none_edges).await?;
            info!("✅ Created none edges for {}", description);
        } else {
            info!("✅ No none edges needed for {}", description);
        }
    }

    Ok(())
}
