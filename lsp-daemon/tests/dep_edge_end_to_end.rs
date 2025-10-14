use anyhow::Result;
use tempfile::TempDir;

use lsp_daemon::database::{DatabaseBackend, DatabaseConfig, Edge, EdgeRelation, SQLiteBackend};

async fn make_backend(temp_name: &str) -> Result<SQLiteBackend> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join(format!("{temp_name}.db"));

    let config = DatabaseConfig {
        path: None,
        temporary: true,
        compression: false,
        cache_capacity: 8 * 1024 * 1024,
        compression_factor: 5,
        flush_every_ms: Some(1000),
    };

    use lsp_daemon::database::sqlite_backend::SQLiteConfig;
    let sqlite_config = SQLiteConfig {
        path: db_path.to_string_lossy().to_string(),
        temporary: false,
        enable_wal: false,
        page_size: 4096,
        cache_size: 1024,
        enable_foreign_keys: false,
    };
    let db = SQLiteBackend::with_sqlite_config(config, sqlite_config).await?;
    Ok(db)
}

#[tokio::test]
async fn dep_edge_normalization_end_to_end() -> Result<()> {
    // Make sure go module path classification has env
    std::env::set_var("GOMODCACHE", "/gomodcache");

    let db = make_backend("dep_edge_e2e").await?;

    // Build a source inside workspace and a target outside (Rust registry)
    let source_uid = "src/main.rs:abcd1234:main:1".to_string();
    let target_abs =
        "/home/user/.cargo/registry/src/index.crates.io-6f17d22bba15001f/serde-1.0.210/src/lib.rs";
    let target_uid = format!("{}:{}:{}:{}", target_abs, "deadbeef", "serde_fn", 10);

    let edge = Edge {
        relation: EdgeRelation::References,
        source_symbol_uid: source_uid.clone(),
        target_symbol_uid: target_uid,
        file_path: Some("src/main.rs".to_string()),
        start_line: Some(1),
        start_char: Some(0),
        confidence: 1.0,
        language: "Rust".to_string(),
        metadata: None,
    };

    db.store_edges(&[edge]).await?;

    // Fetch references for the (workspace) source symbol
    let edges = db.get_symbol_references(1, &source_uid).await?;
    assert_eq!(edges.len(), 1, "expected one edge stored");
    let stored = &edges[0];
    assert!(
        stored.target_symbol_uid.starts_with("/dep/rust/"),
        "target UID not normalized to /dep/rust: {}",
        stored.target_symbol_uid
    );

    Ok(())
}
