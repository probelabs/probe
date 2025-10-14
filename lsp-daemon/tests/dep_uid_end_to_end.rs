use anyhow::Result;
use tempfile::TempDir;

use lsp_daemon::database::{DatabaseBackend, DatabaseConfig, SQLiteBackend, SymbolState};

// Helper to create a test backend (file-backed to exercise full stack)
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

use lsp_daemon::symbol::dependency_path::classify_absolute_path;

#[tokio::test]
async fn dep_uid_normalization_end_to_end() -> Result<()> {
    // Make mapping of three ecosystems
    std::env::set_var("GOMODCACHE", "/gomodcache");

    let db = make_backend("dep_uid_e2e").await?;

    // 1) Rust registry path
    let rust_abs =
        "/home/user/.cargo/registry/src/index.crates.io-6f17d22bba15001f/serde-1.0.210/src/lib.rs";
    let rust_uid = format!("{}:{}:{}:{}", rust_abs, "testhash_rust", "TestRust", 123);
    let rust_symbol = SymbolState {
        symbol_uid: rust_uid,
        file_path: rust_abs.to_string(),
        language: "Rust".to_string(),
        name: "TestRust".to_string(),
        fqn: None,
        kind: "function".to_string(),
        signature: None,
        visibility: None,
        def_start_line: 123,
        def_start_char: 1,
        def_end_line: 124,
        def_end_char: 1,
        is_definition: true,
        documentation: None,
        metadata: None,
    };
    db.store_symbols(&[rust_symbol]).await?;

    // 2) JS node_modules path
    let js_abs = "/repo/node_modules/@types/node/fs.d.ts";
    let js_uid = format!("{}:{}:{}:{}", js_abs, "testhash_js", "TestJs", 10);
    let js_symbol = SymbolState {
        symbol_uid: js_uid,
        file_path: js_abs.to_string(),
        language: "JavaScript".to_string(),
        name: "TestJs".to_string(),
        fqn: None,
        kind: "function".to_string(),
        signature: None,
        visibility: None,
        def_start_line: 10,
        def_start_char: 1,
        def_end_line: 11,
        def_end_char: 1,
        is_definition: true,
        documentation: None,
        metadata: None,
    };
    db.store_symbols(&[js_symbol]).await?;

    // 3) Go module path
    let go_abs = "/gomodcache/github.com/gorilla/mux@v1.8.1/router.go";
    let go_uid = format!("{}:{}:{}:{}", go_abs, "testhash_go", "TestGo", 42);
    let go_symbol = SymbolState {
        symbol_uid: go_uid,
        file_path: go_abs.to_string(),
        language: "Go".to_string(),
        name: "TestGo".to_string(),
        fqn: None,
        kind: "function".to_string(),
        signature: None,
        visibility: None,
        def_start_line: 42,
        def_start_char: 1,
        def_end_line: 43,
        def_end_char: 1,
        is_definition: true,
        documentation: None,
        metadata: None,
    };
    db.store_symbols(&[go_symbol]).await?;

    // Fetch and assert
    let rust_dep_fp =
        classify_absolute_path(std::path::Path::new(rust_abs)).expect("rust dep path");
    let rust_rows = db.get_symbols_by_file(&rust_dep_fp, "Rust").await?;
    assert!(!rust_rows.is_empty(), "rust symbol not stored");
    let rust_uid_stored = &rust_rows[0].symbol_uid;
    let rust_fp = &rust_rows[0].file_path;
    assert!(
        rust_uid_stored.starts_with("/dep/rust/"),
        "UID not mapped to /dep/rust: {}",
        rust_uid_stored
    );
    assert!(
        rust_fp.starts_with("/dep/rust/"),
        "file_path not mapped to /dep/rust: {}",
        rust_fp
    );

    let js_dep_fp = classify_absolute_path(std::path::Path::new(js_abs)).expect("js dep path");
    let js_rows = db.get_symbols_by_file(&js_dep_fp, "JavaScript").await?;
    assert!(!js_rows.is_empty(), "js symbol not stored");
    let js_uid_stored = &js_rows[0].symbol_uid;
    let js_fp = &js_rows[0].file_path;
    assert!(
        js_uid_stored.starts_with("/dep/js/"),
        "UID not mapped to /dep/js: {}",
        js_uid_stored
    );
    assert!(
        js_fp.starts_with("/dep/js/"),
        "file_path not mapped to /dep/js: {}",
        js_fp
    );

    let go_dep_fp = classify_absolute_path(std::path::Path::new(go_abs)).expect("go dep path");
    let go_rows = db.get_symbols_by_file(&go_dep_fp, "Go").await?;
    assert!(!go_rows.is_empty(), "go symbol not stored");
    let go_uid_stored = &go_rows[0].symbol_uid;
    let go_fp = &go_rows[0].file_path;
    assert!(
        go_uid_stored.starts_with("/dep/go/"),
        "UID not mapped to /dep/go: {}",
        go_uid_stored
    );
    assert!(
        go_fp.starts_with("/dep/go/"),
        "file_path not mapped to /dep/go: {}",
        go_fp
    );

    Ok(())
}
