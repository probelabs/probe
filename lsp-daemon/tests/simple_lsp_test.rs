#![cfg(feature = "legacy-tests")]
use anyhow::Result;
use lsp_daemon::{
    database::sqlite_backend::SQLiteBackend,
    database::{DatabaseBackend, DatabaseConfig},
    language_detector::Language,
    lsp_registry::{LspRegistry, LspServerCapabilities, LspServerConfig},
    server_manager::SingleServerManager,
};
use std::sync::Arc;

/// Simple LSP integration test focused on basic server functionality
#[tokio::test]
async fn test_lsp_server_basic_functionality() -> Result<()> {
    // Create a temporary directory for testing
    let temp_dir = tempfile::tempdir()?;
    let workspace_root = temp_dir.path().to_path_buf();

    // Create a simple Rust file for testing
    let test_file = workspace_root.join("test.rs");
    tokio::fs::write(
        &test_file,
        r#"
fn hello_world() {
    println!("Hello, world!");
}

fn main() {
    hello_world();
}
"#,
    )
    .await?;

    // Initialize database
    let db_path = workspace_root.join("test.db");
    let config = DatabaseConfig {
        path: Some(db_path),
        temporary: false,
        compression: false,
        cache_capacity: 1024 * 1024, // 1MB
        compression_factor: 0,
        flush_every_ms: Some(1000),
    };
    let db = SQLiteBackend::new(config).await?;

    // Create LSP registry
    let registry = Arc::new(LspRegistry::new()?);

    // Create server manager
    let server_manager = SingleServerManager::new(registry);

    println!("✓ Simple LSP test setup completed successfully");
    println!("  - Workspace: {:?}", workspace_root);
    println!("  - Test file: {:?}", test_file);

    // This test just validates that we can create the basic infrastructure
    // without actual LSP server communication

    Ok(())
}

/// Test LSP server configuration
#[tokio::test]
async fn test_lsp_server_config() -> Result<()> {
    let config = LspServerConfig {
        language: Language::Rust,
        command: "rust-analyzer".to_string(),
        args: vec![],
        initialization_options: None,
        root_markers: vec!["Cargo.toml".to_string()],
        initialization_timeout_secs: 30,
        capabilities: LspServerCapabilities::default(),
    };

    println!("✓ LSP server config created successfully");
    println!("  - Command: {}", config.command);
    println!("  - Language: {:?}", config.language);
    println!("  - Root markers: {:?}", config.root_markers);

    Ok(())
}

/// Test basic structures and types compilation
#[tokio::test]
async fn test_lsp_types_compilation() -> Result<()> {
    use lsp_daemon::protocol::{Location, Position, Range};

    let position = Position {
        line: 0,
        character: 0,
    };

    let range = Range {
        start: position.clone(),
        end: Position {
            line: 0,
            character: 10,
        },
    };

    let location = Location {
        uri: "file:///test.rs".to_string(),
        range,
    };

    println!("✓ LSP types compilation successful");
    println!("  - Position: {:?}", position);
    println!("  - Location URI: {}", location.uri);

    Ok(())
}
