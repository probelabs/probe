use anyhow::Result;
use tempfile::tempdir;

use lsp_daemon::database::{
    create_none_call_hierarchy_edges, create_none_implementation_edges,
    create_none_reference_edges, DatabaseBackend, DatabaseConfig, SQLiteBackend,
    SymbolEnrichmentPlan, SymbolState,
};

fn test_database_config(path: std::path::PathBuf) -> DatabaseConfig {
    DatabaseConfig {
        path: Some(path),
        temporary: false,
        compression: false,
        cache_capacity: 16 * 1024 * 1024,
        compression_factor: 5,
        flush_every_ms: Some(1000),
    }
}

fn make_symbol(symbol_uid: &str) -> SymbolState {
    SymbolState {
        symbol_uid: symbol_uid.to_string(),
        file_path: "src/lib.rs".to_string(),
        language: "rust".to_string(),
        name: "demo_symbol".to_string(),
        fqn: Some("demo::demo_symbol".to_string()),
        kind: "function".to_string(),
        signature: None,
        visibility: Some("public".to_string()),
        def_start_line: 1,
        def_start_char: 0,
        def_end_line: 2,
        def_end_char: 1,
        is_definition: true,
        documentation: None,
        metadata: None,
    }
}

#[tokio::test]
#[ignore = "Simplified backend returns no pending enrichment; modern scheduler not exercised in legacy tests"]
async fn test_find_symbols_pending_enrichment_internal_tracks_per_operation_state() -> Result<()> {
    let temp_dir = tempdir()?;
    let db_path = temp_dir.path().join("enrichment.db");
    let backend = SQLiteBackend::new(test_database_config(db_path)).await?;

    let symbol_uid = "test::symbol";
    let symbol = make_symbol(symbol_uid);
    backend.store_symbols(&[symbol.clone()]).await?;

    let mut plans = backend.find_symbols_pending_enrichment_internal(10).await?;
    assert!(
        !plans.is_empty(),
        "expected at least one symbol pending enrichment"
    );
    let first_plan = plans.remove(0);
    assert!(first_plan.needs_references);
    assert!(first_plan.needs_implementations);
    assert!(first_plan.needs_call_hierarchy);
    let stored_uid = first_plan.symbol.symbol_uid.clone();

    backend
        .store_edges(&create_none_reference_edges(&stored_uid))
        .await?;
    let plan = backend
        .find_symbols_pending_enrichment_internal(10)
        .await?
        .into_iter()
        .find(|plan| plan.symbol.symbol_uid == stored_uid)
        .expect("symbol plan should remain after references sentinel");
    assert!(!plan.needs_references);
    assert!(plan.needs_implementations);
    assert!(plan.needs_call_hierarchy);

    backend
        .store_edges(&create_none_implementation_edges(&stored_uid))
        .await?;
    let plan = backend
        .find_symbols_pending_enrichment_internal(10)
        .await?
        .into_iter()
        .find(|plan| plan.symbol.symbol_uid == stored_uid)
        .expect("symbol plan should remain after implementation sentinel");
    assert!(!plan.needs_references);
    assert!(!plan.needs_implementations);
    assert!(plan.needs_call_hierarchy);

    backend
        .store_edges(&create_none_call_hierarchy_edges(&stored_uid))
        .await?;
    let plans = backend.find_symbols_pending_enrichment_internal(10).await?;
    assert!(
        !plans
            .iter()
            .any(|plan| plan.symbol.symbol_uid == stored_uid),
        "symbol should no longer require enrichment once all operations are satisfied"
    );

    Ok(())
}

#[tokio::test]
#[ignore = "Pending enrichment counts not reported by simplified backend in legacy mode"]
async fn test_get_pending_enrichment_counts_reflects_database_state() -> Result<()> {
    let temp_dir = tempdir()?;
    let db_path = temp_dir.path().join("counts.db");
    let backend = SQLiteBackend::new(test_database_config(db_path)).await?;

    let symbol_uid = "demo::symbol";
    let symbol = make_symbol(symbol_uid);
    backend.store_symbols(&[symbol.clone()]).await?;

    let counts = backend.get_pending_enrichment_counts().await?;
    assert_eq!(counts.symbols_pending, 1);
    assert_eq!(counts.references_pending, 1);
    assert_eq!(counts.implementations_pending, 1);
    assert_eq!(counts.call_hierarchy_pending, 1);
    assert_eq!(counts.high_priority_pending, 1);
    assert_eq!(counts.medium_priority_pending, 0);
    assert_eq!(counts.low_priority_pending, 0);

    backend
        .store_edges(&create_none_reference_edges(symbol_uid))
        .await?;
    let counts = backend.get_pending_enrichment_counts().await?;
    assert_eq!(counts.symbols_pending, 1, "still pending other operations");
    assert_eq!(counts.references_pending, 0);
    assert_eq!(counts.implementations_pending, 1);
    assert_eq!(counts.call_hierarchy_pending, 1);

    backend
        .store_edges(&create_none_implementation_edges(symbol_uid))
        .await?;
    let counts = backend.get_pending_enrichment_counts().await?;
    assert_eq!(counts.symbols_pending, 1);
    assert_eq!(counts.implementations_pending, 0);
    assert_eq!(counts.call_hierarchy_pending, 1);

    backend
        .store_edges(&create_none_call_hierarchy_edges(symbol_uid))
        .await?;
    let counts = backend.get_pending_enrichment_counts().await?;
    assert_eq!(counts.symbols_pending, 0);
    assert_eq!(counts.references_pending, 0);
    assert_eq!(counts.implementations_pending, 0);
    assert_eq!(counts.call_hierarchy_pending, 0);

    Ok(())
}

#[tokio::test]
#[ignore = "Pending enrichment counts not reported by simplified backend in legacy mode"]
async fn test_get_pending_enrichment_counts_deduplicates_union() -> Result<()> {
    let temp_dir = tempdir()?;
    let db_path = temp_dir.path().join("counts_union.db");
    let backend = SQLiteBackend::new(test_database_config(db_path)).await?;

    // sym_x: pending for all three operations (no sentinels)
    let sym_x = make_symbol("demo::sym_x");
    backend.store_symbols(&[sym_x.clone()]).await?;

    // sym_y: only call hierarchy pending (refs + impls satisfied via sentinel edges)
    let sym_y = make_symbol("demo::sym_y");
    backend.store_symbols(&[sym_y.clone()]).await?;
    backend
        .store_edges(&create_none_reference_edges(&sym_y.symbol_uid))
        .await?;
    backend
        .store_edges(&create_none_implementation_edges(&sym_y.symbol_uid))
        .await?;

    // Validate per-op counts and overall deduped total
    let counts = backend.get_pending_enrichment_counts().await?;

    // sym_x contributes to refs/impls/calls; sym_y contributes to calls only
    assert_eq!(counts.references_pending, 1, "only sym_x pending refs");
    assert_eq!(
        counts.implementations_pending, 1,
        "only sym_x pending impls"
    );
    assert_eq!(
        counts.call_hierarchy_pending, 2,
        "sym_x and sym_y pending calls"
    );

    // symbols_pending must count distinct symbols across all pending sets → {sym_x, sym_y} = 2
    assert_eq!(counts.symbols_pending, 2, "dedup across pending sets");

    // Both are functions → high priority bucket should equal 2
    assert_eq!(counts.high_priority_pending, 2);
    assert_eq!(counts.medium_priority_pending, 0);
    assert_eq!(counts.low_priority_pending, 0);

    Ok(())
}
