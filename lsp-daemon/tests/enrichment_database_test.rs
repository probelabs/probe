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
