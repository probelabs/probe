#![cfg(feature = "legacy-tests")]
//! Cache behavior tests for the null edge system
//!
//! Tests that validate the complete cycle:
//! 1. First query (cache miss) -> LSP call -> empty result -> store "none" edges
//! 2. Second query (cache hit) -> find "none" edges -> return empty without LSP call

use anyhow::Result;
use lsp_daemon::database::sqlite_backend::SQLiteBackend;
use lsp_daemon::database::{create_none_call_hierarchy_edges, DatabaseBackend, DatabaseConfig};
use std::sync::Arc;
use std::time::Instant;
use tempfile::TempDir;

async fn create_test_database() -> Result<SQLiteBackend> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("cache_test.db");

    let config = DatabaseConfig {
        path: Some(db_path),
        temporary: false,
        cache_capacity: 1024 * 1024,
        ..Default::default()
    };

    SQLiteBackend::new(config).await.map_err(Into::into)
}

#[tokio::test]
async fn test_complete_cache_cycle_with_empty_call_hierarchy() -> Result<()> {
    let database = create_test_database().await?;
    let symbol_uid = "src/empty_struct.rs:EmptyStruct:10";
    let workspace_id = 1i64;

    // Phase 1: Cache miss - should return None (triggering LSP call)
    let start_time = Instant::now();
    let first_result = database
        .get_call_hierarchy_for_symbol(workspace_id, symbol_uid)
        .await?;
    let first_duration = start_time.elapsed();

    assert!(
        first_result.is_none(),
        "First query should be cache miss (return None)"
    );
    println!("✅ First query (cache miss): {:?}", first_duration);

    // Simulate LSP returning empty call hierarchy and storing "none" edges
    let none_edges = create_none_call_hierarchy_edges(symbol_uid, 1);
    database.store_edges(&none_edges).await?;

    // Phase 2: Cache hit - should return empty call hierarchy (not None)
    let start_time = Instant::now();
    let second_result = database
        .get_call_hierarchy_for_symbol(workspace_id, symbol_uid)
        .await?;
    let second_duration = start_time.elapsed();

    assert!(
        second_result.is_some(),
        "Second query should be cache hit (return Some)"
    );
    let hierarchy = second_result.unwrap();
    assert!(
        hierarchy.incoming.is_empty(),
        "Incoming calls should be empty"
    );
    assert!(
        hierarchy.outgoing.is_empty(),
        "Outgoing calls should be empty"
    );

    // Cache hit should be much faster than cache miss
    println!("✅ Second query (cache hit): {:?}", second_duration);
    if first_duration.as_nanos() > 0 && second_duration.as_nanos() > 0 {
        println!(
            "✅ Cache performance improvement: {}x faster",
            first_duration.as_nanos() / second_duration.as_nanos().max(1)
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_cache_miss_vs_cache_hit_performance() -> Result<()> {
    let database = create_test_database().await?;
    let workspace_id = 1i64;

    // Test multiple symbols
    let test_symbols = vec![
        "src/test1.rs:Symbol1:10",
        "src/test2.rs:Symbol2:20",
        "src/test3.rs:Symbol3:30",
    ];

    for symbol_uid in &test_symbols {
        // First query - cache miss
        let miss_start = Instant::now();
        let miss_result = database
            .get_call_hierarchy_for_symbol(workspace_id, symbol_uid)
            .await?;
        let miss_duration = miss_start.elapsed();

        assert!(
            miss_result.is_none(),
            "Should be cache miss for {}",
            symbol_uid
        );

        // Store "none" edges
        let none_edges = create_none_call_hierarchy_edges(symbol_uid, 1);
        database.store_edges(&none_edges).await?;

        // Second query - cache hit
        let hit_start = Instant::now();
        let hit_result = database
            .get_call_hierarchy_for_symbol(workspace_id, symbol_uid)
            .await?;
        let hit_duration = hit_start.elapsed();

        assert!(
            hit_result.is_some(),
            "Should be cache hit for {}",
            symbol_uid
        );

        println!(
            "Symbol {}: miss={:?}, hit={:?}, speedup={}x",
            symbol_uid,
            miss_duration,
            hit_duration,
            if hit_duration.as_nanos() > 0 {
                miss_duration.as_nanos() / hit_duration.as_nanos().max(1)
            } else {
                1
            }
        );
    }

    println!("✅ Cache performance test completed");
    Ok(())
}

#[tokio::test]
async fn test_references_cache_behavior() -> Result<()> {
    let database = create_test_database().await?;
    let workspace_id = 1i64;
    let symbol_uid = "src/unused.rs:unused_function:42";

    // First query - cache miss (returns empty vec, not None for references)
    let first_result = database
        .get_references_for_symbol(workspace_id, symbol_uid, true)
        .await?;
    assert!(
        first_result.is_empty(),
        "First references query should return empty vec"
    );

    // Simulate storing none edges for empty references
    let none_edges = lsp_daemon::database::create_none_reference_edges(symbol_uid, 1);
    database.store_edges(&none_edges).await?;

    // Second query - should still return empty vec but from cache
    let second_result = database
        .get_references_for_symbol(workspace_id, symbol_uid, true)
        .await?;
    assert!(
        second_result.is_empty(),
        "Second references query should still return empty vec"
    );

    // Verify the edges can be retrieved directly
    let edges = database
        .get_symbol_references(workspace_id, symbol_uid)
        .await?;
    assert_eq!(edges.len(), 1, "Should have one none edge");
    assert_eq!(
        edges[0].target_symbol_uid, "none",
        "Edge should be a none edge"
    );

    println!("✅ References cache behavior test passed");
    Ok(())
}

#[tokio::test]
async fn test_definitions_cache_behavior() -> Result<()> {
    let database = create_test_database().await?;
    let workspace_id = 1i64;
    let symbol_uid = "src/external.rs:external_symbol:100";

    // First query - cache miss (returns empty vec)
    let first_result = database
        .get_definitions_for_symbol(workspace_id, symbol_uid)
        .await?;
    assert!(
        first_result.is_empty(),
        "First definitions query should return empty vec"
    );

    // Simulate storing none edges for empty definitions
    let none_edges = lsp_daemon::database::create_none_definition_edges(symbol_uid, 1);
    database.store_edges(&none_edges).await?;

    // Second query - should return empty vec from cache
    let second_result = database
        .get_definitions_for_symbol(workspace_id, symbol_uid)
        .await?;
    assert!(
        second_result.is_empty(),
        "Second definitions query should still return empty vec"
    );

    println!("✅ Definitions cache behavior test passed");
    Ok(())
}

#[tokio::test]
async fn test_implementations_cache_behavior() -> Result<()> {
    let database = create_test_database().await?;
    let workspace_id = 1i64;
    let symbol_uid = "src/trait.rs:unimplemented_trait:200";

    // First query - cache miss (returns empty vec)
    let first_result = database
        .get_implementations_for_symbol(workspace_id, symbol_uid)
        .await?;
    assert!(
        first_result.is_empty(),
        "First implementations query should return empty vec"
    );

    // Simulate storing none edges for empty implementations
    let none_edges = lsp_daemon::database::create_none_implementation_edges(symbol_uid, 1);
    database.store_edges(&none_edges).await?;

    // Second query - should return empty vec from cache
    let second_result = database
        .get_implementations_for_symbol(workspace_id, symbol_uid)
        .await?;
    assert!(
        second_result.is_empty(),
        "Second implementations query should still return empty vec"
    );

    println!("✅ Implementations cache behavior test passed");
    Ok(())
}

#[tokio::test]
async fn test_concurrent_cache_access() -> Result<()> {
    let database = Arc::new(create_test_database().await?);
    let workspace_id = 1i64;
    let symbol_uid = "src/concurrent.rs:ConcurrentSymbol:500";

    // Store none edges first
    let none_edges = create_none_call_hierarchy_edges(symbol_uid, 1);
    database.store_edges(&none_edges).await?;

    // Simulate multiple concurrent requests
    let handles = (0..5)
        .map(|i| {
            let db = Arc::clone(&database);
            let uid = symbol_uid;
            tokio::spawn(async move {
                let result = db
                    .get_call_hierarchy_for_symbol(workspace_id, uid)
                    .await
                    .expect(&format!("Request {} should succeed", i));
                assert!(result.is_some(), "Request {} should get cached result", i);
                let hierarchy = result.unwrap();
                assert!(
                    hierarchy.incoming.is_empty(),
                    "Request {} should get empty incoming",
                    i
                );
                assert!(
                    hierarchy.outgoing.is_empty(),
                    "Request {} should get empty outgoing",
                    i
                );
                i
            })
        })
        .collect::<Vec<_>>();

    // Wait for all requests to complete
    for handle in handles {
        handle.await?;
    }

    println!("✅ Concurrent cache access test passed");
    Ok(())
}

#[tokio::test]
async fn test_cache_invalidation_scenarios() -> Result<()> {
    let database = create_test_database().await?;
    let workspace_id = 1i64;
    let symbol_uid = "src/changing.rs:ChangingSymbol:600";

    // Initially no cache - cache miss
    let initial_result = database
        .get_call_hierarchy_for_symbol(workspace_id, symbol_uid)
        .await?;
    assert!(initial_result.is_none(), "Should be cache miss initially");

    // Store none edges (empty result)
    let none_edges = create_none_call_hierarchy_edges(symbol_uid, 1);
    database.store_edges(&none_edges).await?;

    // Should now return cached empty result
    let cached_result = database
        .get_call_hierarchy_for_symbol(workspace_id, symbol_uid)
        .await?;
    assert!(cached_result.is_some(), "Should return cached result");
    let hierarchy = cached_result.unwrap();
    assert!(
        hierarchy.incoming.is_empty(),
        "Cached result should be empty"
    );

    // Simulate code change - new file version with real call relationships
    let new_file_version_id = 2i64;
    let real_edge = lsp_daemon::database::Edge {
        relation: lsp_daemon::database::EdgeRelation::IncomingCall,
        source_symbol_uid: symbol_uid.to_string(),
        target_symbol_uid: "src/caller.rs:new_caller:10".to_string(),
        file_path: Some("src/caller.rs".to_string()),
        start_line: Some(10),
        start_char: Some(5),
        confidence: 0.95,
        language: "rust".to_string(),
        metadata: Some("real_edge".to_string()),
    };

    database.store_edges(&[real_edge]).await?;

    // The cache should now reflect the new edges
    // Note: In a real system, cache invalidation would happen based on file version changes
    let updated_edges = database
        .get_symbol_calls(
            workspace_id,
            symbol_uid,
            lsp_daemon::database::CallDirection::Incoming,
        )
        .await?;
    assert!(updated_edges.len() > 0, "Should have edges after update");

    // Find the real edge (not the none edge)
    let real_edges: Vec<_> = updated_edges
        .into_iter()
        .filter(|e| e.target_symbol_uid != "none")
        .collect();
    assert_eq!(real_edges.len(), 1, "Should have one real edge");
    assert_eq!(
        real_edges[0].target_symbol_uid,
        "src/caller.rs:new_caller:10"
    );

    println!("✅ Cache invalidation test passed");
    Ok(())
}

#[tokio::test]
async fn test_batch_cache_operations() -> Result<()> {
    let database = create_test_database().await?;
    let workspace_id = 1i64;

    // Create multiple symbols for batch testing
    let symbol_uids = (1..=10)
        .map(|i| format!("src/batch_{}.rs:BatchSymbol{}:{}", i, i, i * 10))
        .collect::<Vec<_>>();

    // First pass - all cache misses
    let mut miss_durations = Vec::new();
    for symbol_uid in &symbol_uids {
        let start = Instant::now();
        let result = database
            .get_call_hierarchy_for_symbol(workspace_id, symbol_uid)
            .await?;
        miss_durations.push(start.elapsed());
        assert!(result.is_none(), "Should be cache miss for {}", symbol_uid);
    }

    // Store none edges for all symbols
    for (i, symbol_uid) in symbol_uids.iter().enumerate() {
        let none_edges = create_none_call_hierarchy_edges(symbol_uid, (i + 1) as i64);
        database.store_edges(&none_edges).await?;
    }

    // Second pass - all cache hits
    let mut hit_durations = Vec::new();
    for symbol_uid in &symbol_uids {
        let start = Instant::now();
        let result = database
            .get_call_hierarchy_for_symbol(workspace_id, symbol_uid)
            .await?;
        hit_durations.push(start.elapsed());
        assert!(result.is_some(), "Should be cache hit for {}", symbol_uid);
        let hierarchy = result.unwrap();
        assert!(
            hierarchy.incoming.is_empty() && hierarchy.outgoing.is_empty(),
            "Cache hit should return empty hierarchy for {}",
            symbol_uid
        );
    }

    // Calculate performance statistics
    let avg_miss_duration =
        miss_durations.iter().sum::<std::time::Duration>() / miss_durations.len() as u32;
    let avg_hit_duration =
        hit_durations.iter().sum::<std::time::Duration>() / hit_durations.len() as u32;

    println!("✅ Batch cache operations test:");
    println!("  Average cache miss duration: {:?}", avg_miss_duration);
    println!("  Average cache hit duration: {:?}", avg_hit_duration);
    if avg_hit_duration.as_nanos() > 0 {
        println!(
            "  Average speedup: {}x",
            avg_miss_duration.as_nanos() / avg_hit_duration.as_nanos().max(1)
        );
    }

    Ok(())
}
