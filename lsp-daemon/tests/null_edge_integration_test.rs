//! Integration tests for null edge creation and storage
//!
//! These tests validate that when LSP returns empty results,
//! the system creates "none" edges to cache the empty state
//! and prevent repeated LSP calls.

use anyhow::Result;
use lsp_daemon::database::sqlite_backend::SQLiteBackend;
use lsp_daemon::database::{
    create_none_call_hierarchy_edges, create_none_definition_edges,
    create_none_implementation_edges, create_none_reference_edges, DatabaseBackend, DatabaseConfig,
    EdgeRelation,
};
use lsp_daemon::protocol::{CallHierarchyItem, Position, Range};
use tempfile::TempDir;

async fn create_test_database() -> Result<SQLiteBackend> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test.db");

    let config = DatabaseConfig {
        path: Some(db_path),
        temporary: false,
        cache_capacity: 1024 * 1024, // 1MB
        ..Default::default()
    };

    SQLiteBackend::new(config).await.map_err(Into::into)
}

fn create_test_call_hierarchy_item(name: &str, kind: &str) -> CallHierarchyItem {
    CallHierarchyItem {
        name: name.to_string(),
        kind: kind.to_string(),
        uri: "file:///test/file.rs".to_string(),
        range: Range {
            start: Position {
                line: 10,
                character: 5,
            },
            end: Position {
                line: 10,
                character: 15,
            },
        },
        selection_range: Range {
            start: Position {
                line: 10,
                character: 5,
            },
            end: Position {
                line: 10,
                character: 15,
            },
        },
    }
}

#[tokio::test]
async fn test_none_edge_creation_for_empty_call_hierarchy() -> Result<()> {
    // 1. Set up test database
    let database = create_test_database().await?;

    // 2. Create "none" edges for empty call hierarchy
    let symbol_uid = "src/test.rs:TestStruct:10";
    let file_version_id = 1i64;
    let none_edges = create_none_call_hierarchy_edges(symbol_uid, file_version_id);

    // 3. Verify "none" edges are created correctly
    assert_eq!(
        none_edges.len(),
        2,
        "Should create 2 none edges (incoming + outgoing)"
    );

    let incoming_edge = &none_edges[0];
    assert_eq!(incoming_edge.source_symbol_uid, symbol_uid);
    assert_eq!(incoming_edge.target_symbol_uid, "none");
    assert_eq!(incoming_edge.relation, EdgeRelation::IncomingCall);
    assert_eq!(incoming_edge.anchor_file_version_id, file_version_id);

    let outgoing_edge = &none_edges[1];
    assert_eq!(outgoing_edge.source_symbol_uid, symbol_uid);
    assert_eq!(outgoing_edge.target_symbol_uid, "none");
    assert_eq!(outgoing_edge.relation, EdgeRelation::OutgoingCall);

    // 4. Store "none" edges in database
    database.store_edges(&none_edges).await?;

    // 5. Verify edges can be retrieved
    let incoming_edges = database
        .get_symbol_calls(1, symbol_uid, lsp_daemon::database::CallDirection::Incoming)
        .await?;
    assert_eq!(incoming_edges.len(), 1);
    assert_eq!(incoming_edges[0].target_symbol_uid, "none");

    let outgoing_edges = database
        .get_symbol_calls(1, symbol_uid, lsp_daemon::database::CallDirection::Outgoing)
        .await?;
    assert_eq!(outgoing_edges.len(), 1);
    assert_eq!(outgoing_edges[0].target_symbol_uid, "none");

    // 6. Test edge interpretation through call hierarchy query
    // The presence of "none" edges should result in empty call hierarchy (not None)
    let call_hierarchy = database
        .get_call_hierarchy_for_symbol(1, symbol_uid)
        .await?;
    match call_hierarchy {
        Some(hierarchy) => {
            // ✅ Expected: Should return Some with empty arrays (not None)
            assert!(
                hierarchy.incoming.is_empty(),
                "Incoming calls should be empty"
            );
            assert!(
                hierarchy.outgoing.is_empty(),
                "Outgoing calls should be empty"
            );
        }
        None => {
            panic!("Expected Some(empty hierarchy), got None - this indicates cache miss");
        }
    }

    println!("✅ test_none_edge_creation_for_empty_call_hierarchy passed");
    Ok(())
}

#[tokio::test]
async fn test_none_edge_creation_for_empty_references() -> Result<()> {
    let database = create_test_database().await?;

    let symbol_uid = "src/test.rs:unused_function:20";
    let file_version_id = 1i64;
    let none_edges = create_none_reference_edges(symbol_uid, file_version_id);

    assert_eq!(
        none_edges.len(),
        1,
        "Should create 1 none edge for references"
    );
    assert_eq!(none_edges[0].target_symbol_uid, "none");
    assert_eq!(none_edges[0].relation, EdgeRelation::References);

    database.store_edges(&none_edges).await?;

    // Test retrieval
    let edges = database.get_symbol_references(1, symbol_uid).await?;
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].target_symbol_uid, "none");

    println!("✅ test_none_edge_creation_for_empty_references passed");
    Ok(())
}

#[tokio::test]
async fn test_none_edge_creation_for_empty_definitions() -> Result<()> {
    let database = create_test_database().await?;

    let symbol_uid = "src/test.rs:external_symbol:30";
    let file_version_id = 1i64;
    let none_edges = create_none_definition_edges(symbol_uid, file_version_id);

    assert_eq!(none_edges.len(), 1);
    assert_eq!(none_edges[0].target_symbol_uid, "none");
    assert_eq!(none_edges[0].relation, EdgeRelation::Definition);

    database.store_edges(&none_edges).await?;

    println!("✅ test_none_edge_creation_for_empty_definitions passed");
    Ok(())
}

#[tokio::test]
async fn test_none_edge_creation_for_empty_implementations() -> Result<()> {
    let database = create_test_database().await?;

    let symbol_uid = "src/test.rs:trait_method:40";
    let file_version_id = 1i64;
    let none_edges = create_none_implementation_edges(symbol_uid, file_version_id);

    assert_eq!(none_edges.len(), 1);
    assert_eq!(none_edges[0].target_symbol_uid, "none");
    assert_eq!(none_edges[0].relation, EdgeRelation::Implementation);

    database.store_edges(&none_edges).await?;

    println!("✅ test_none_edge_creation_for_empty_implementations passed");
    Ok(())
}

#[tokio::test]
async fn test_edge_interpretation_logic() -> Result<()> {
    let database = create_test_database().await?;

    // Test 1: No edges (never analyzed) -> should return None (cache miss)
    let symbol_uid_unanalyzed = "src/never_analyzed.rs:UnanalyzedSymbol:999";
    let result_unanalyzed = database
        .get_call_hierarchy_for_symbol(1, symbol_uid_unanalyzed)
        .await?;
    assert!(
        result_unanalyzed.is_none(),
        "Never analyzed symbol should return None (cache miss)"
    );

    // Test 2: Store "none" edges (analyzed but empty) -> should return Some(empty)
    let symbol_uid_analyzed = "src/analyzed_empty.rs:AnalyzedEmptySymbol:888";
    let none_edges = create_none_call_hierarchy_edges(symbol_uid_analyzed, 1);
    database.store_edges(&none_edges).await?;

    let result_analyzed = database
        .get_call_hierarchy_for_symbol(1, symbol_uid_analyzed)
        .await?;
    match result_analyzed {
        Some(hierarchy) => {
            assert!(
                hierarchy.incoming.is_empty(),
                "Analyzed empty should have empty incoming"
            );
            assert!(
                hierarchy.outgoing.is_empty(),
                "Analyzed empty should have empty outgoing"
            );
        }
        None => panic!("Analyzed symbol with none edges should return Some(empty), got None"),
    }

    println!("✅ test_edge_interpretation_logic passed");
    Ok(())
}

#[tokio::test]
async fn test_store_edges_handles_empty_arrays() -> Result<()> {
    let database = create_test_database().await?;

    // Test storing empty array (should succeed, not fail)
    let result = database.store_edges(&[]).await;
    assert!(
        result.is_ok(),
        "store_edges([]) should succeed: {:?}",
        result
    );

    // Test storing "none" edges
    let none_edges = create_none_call_hierarchy_edges("test", 1);
    let result = database.store_edges(&none_edges).await;
    assert!(
        result.is_ok(),
        "store_edges(none_edges) should succeed: {:?}",
        result
    );

    println!("✅ test_store_edges_handles_empty_arrays passed");
    Ok(())
}

#[tokio::test]
async fn test_mixed_none_and_real_edges() -> Result<()> {
    let database = create_test_database().await?;

    // Create a mix of real and none edges
    let symbol_uid = "src/test.rs:mixed_symbol:50";
    let file_version_id = 1i64;

    // Create a real edge (non-none target)
    let real_edge = lsp_daemon::database::Edge {
        relation: EdgeRelation::IncomingCall,
        source_symbol_uid: symbol_uid.to_string(),
        target_symbol_uid: "src/caller.rs:caller_function:15".to_string(),
        file_path: Some("src/caller.rs".to_string()),
        start_line: Some(15),
        start_char: Some(10),
        confidence: 0.9,
        language: "rust".to_string(),
        metadata: Some("real_call".to_string()),
    };

    // Create a none edge
    let none_edge = lsp_daemon::database::create_none_edge(
        symbol_uid,
        EdgeRelation::OutgoingCall,
        file_version_id,
    );

    let mixed_edges = vec![real_edge.clone(), none_edge];
    database.store_edges(&mixed_edges).await?;

    // Test call hierarchy - should show incoming calls but empty outgoing
    let hierarchy = database
        .get_call_hierarchy_for_symbol(1, symbol_uid)
        .await?;
    match hierarchy {
        Some(hierarchy) => {
            assert!(hierarchy.incoming.len() > 0, "Should have incoming calls");
            assert!(
                hierarchy.outgoing.is_empty(),
                "Should have empty outgoing calls"
            );
        }
        None => panic!("Expected Some(hierarchy) for symbol with edges"),
    }

    // Direct edge retrieval should also work
    let incoming_edges = database
        .get_symbol_calls(1, symbol_uid, lsp_daemon::database::CallDirection::Incoming)
        .await?;
    let real_incoming: Vec<_> = incoming_edges
        .into_iter()
        .filter(|e| e.target_symbol_uid != "none")
        .collect();
    assert_eq!(real_incoming.len(), 1);
    assert_eq!(
        real_incoming[0].target_symbol_uid,
        "src/caller.rs:caller_function:15"
    );

    let outgoing_edges = database
        .get_symbol_calls(1, symbol_uid, lsp_daemon::database::CallDirection::Outgoing)
        .await?;
    assert!(
        outgoing_edges.iter().all(|e| e.target_symbol_uid == "none"),
        "Outgoing edges should only contain none edges"
    );

    println!("✅ test_mixed_none_and_real_edges passed");
    Ok(())
}

#[tokio::test]
async fn test_workspace_isolation_with_none_edges() -> Result<()> {
    let database = create_test_database().await?;

    // Create two workspaces
    let workspace_id1 = database
        .create_workspace("workspace1", 1, Some("main"))
        .await?;
    let workspace_id2 = database
        .create_workspace("workspace2", 2, Some("dev"))
        .await?;

    let symbol_uid = "src/shared.rs:shared_symbol:100";
    let file_version_id = 1i64;

    // Store none edges for workspace1
    let none_edges = create_none_call_hierarchy_edges(symbol_uid, file_version_id);
    database.store_edges(&none_edges).await?;

    // Check workspace1 has the none edges (should return Some(empty))
    let workspace1_hierarchy = database
        .get_call_hierarchy_for_symbol(workspace_id1, symbol_uid)
        .await?;
    match workspace1_hierarchy {
        Some(hierarchy) => {
            assert!(
                hierarchy.incoming.is_empty(),
                "Workspace1 should have empty incoming"
            );
            assert!(
                hierarchy.outgoing.is_empty(),
                "Workspace1 should have empty outgoing"
            );
        }
        None => panic!("Workspace1 should return Some(empty), got None"),
    }

    // Check workspace2 has no edges (should return None - cache miss)
    let workspace2_hierarchy = database
        .get_call_hierarchy_for_symbol(workspace_id2, symbol_uid)
        .await?;
    match workspace2_hierarchy {
        None => {} // ✅ Expected - no analysis done for this workspace
        Some(_) => panic!("Workspace2 should return None (cache miss), got Some"),
    }

    println!("✅ test_workspace_isolation_with_none_edges passed");
    Ok(())
}
