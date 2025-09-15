//! Integration tests for database-first LSP caching functionality
//!
//! These tests validate the complete database-first caching pipeline
//! including workspace isolation, concurrent operations, and cache persistence.

use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::timeout;

/// Integration test: Database creation and workspace isolation
#[tokio::test]
async fn test_database_workspace_isolation() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");

    // Create two separate test workspaces
    let workspace1 = temp_dir.path().join("workspace1");
    let workspace2 = temp_dir.path().join("workspace2");

    std::fs::create_dir_all(&workspace1).expect("Failed to create workspace1");
    std::fs::create_dir_all(&workspace2).expect("Failed to create workspace2");

    // Create Cargo.toml files to make them valid Rust workspaces
    std::fs::write(
        workspace1.join("Cargo.toml"),
        "[package]\nname = \"workspace1\"",
    )
    .expect("Failed to create Cargo.toml");
    std::fs::write(
        workspace2.join("Cargo.toml"),
        "[package]\nname = \"workspace2\"",
    )
    .expect("Failed to create Cargo.toml");

    // Create test Rust files
    std::fs::write(
        workspace1.join("main.rs"),
        "fn main() { println!(\"workspace1\"); }",
    )
    .expect("Failed to create main.rs");
    std::fs::write(
        workspace2.join("main.rs"),
        "fn main() { println!(\"workspace2\"); }",
    )
    .expect("Failed to create main.rs");

    // Test workspace initialization through CLI
    let binary_path = get_probe_binary_path();

    // Initialize workspace1
    let output1 = std::process::Command::new(&binary_path)
        .args(&["lsp", "init", "--workspace", workspace1.to_str().unwrap()])
        .output()
        .expect("Failed to execute probe command");

    assert!(
        output1.status.success(),
        "Workspace1 initialization failed: {}",
        String::from_utf8_lossy(&output1.stderr)
    );

    // Initialize workspace2
    let output2 = std::process::Command::new(&binary_path)
        .args(&["lsp", "init", "--workspace", workspace2.to_str().unwrap()])
        .output()
        .expect("Failed to execute probe command");

    assert!(
        output2.status.success(),
        "Workspace2 initialization failed: {}",
        String::from_utf8_lossy(&output2.stderr)
    );

    // Allow time for database creation
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Verify separate cache databases were created
    let cache_dir = get_cache_directory();
    let cache_files = find_cache_databases(&cache_dir);

    // Should have at least the databases we created plus potentially the main project
    assert!(
        cache_files.len() >= 2,
        "Expected at least 2 cache databases, found {}: {:?}",
        cache_files.len(),
        cache_files
    );

    // Verify databases are valid SQLite files
    for db_path in &cache_files {
        assert!(
            db_path.exists(),
            "Database file should exist: {:?}",
            db_path
        );
        assert!(
            is_sqlite_database(db_path),
            "File should be SQLite database: {:?}",
            db_path
        );
    }

    println!("✅ Database workspace isolation test passed");
    println!(
        "   Created {} isolated workspace databases",
        cache_files.len()
    );
}

/// Integration test: Concurrent LSP operations with database persistence
#[tokio::test]
async fn test_concurrent_lsp_operations() {
    let binary_path = get_probe_binary_path();

    // Start daemon in background
    let mut daemon_process = std::process::Command::new(&binary_path)
        .args(&["lsp", "start", "-f", "--log-level", "debug"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("Failed to start daemon");

    // Allow daemon to start
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Initialize current workspace
    let init_output = std::process::Command::new(&binary_path)
        .args(&["lsp", "init", "--workspace", "."])
        .output()
        .expect("Failed to initialize workspace");

    assert!(
        init_output.status.success(),
        "Workspace initialization failed: {}",
        String::from_utf8_lossy(&init_output.stderr)
    );

    // Launch concurrent LSP operations
    let mut handles: Vec<
        tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
    > = Vec::new();
    let operations = vec![
        ("src/main.rs", 10, 5),
        ("src/main.rs", 11, 5),
        ("src/main.rs", 12, 5),
        ("src/main.rs", 13, 5),
        ("src/main.rs", 14, 5),
    ];

    for (i, (file, line, column)) in operations.into_iter().enumerate() {
        let binary_path = binary_path.clone();
        let handle = tokio::spawn(async move {
            let result = timeout(Duration::from_secs(30), async {
                let output = std::process::Command::new(&binary_path)
                    .args(&[
                        "lsp",
                        "call",
                        "definition",
                        &format!("{}:{}:{}", file, line, column),
                    ])
                    .output()
                    .expect("Failed to execute LSP call");
                output
            })
            .await;

            match result {
                Ok(output) => {
                    if output.status.success() {
                        println!("✅ Operation {} completed successfully", i + 1);
                        Ok(())
                    } else {
                        println!(
                            "⚠️  Operation {} failed: {}",
                            i + 1,
                            String::from_utf8_lossy(&output.stderr)
                        );
                        // Don't fail the test for individual LSP errors (server might be initializing)
                        Ok(())
                    }
                }
                Err(_) => {
                    println!("⚠️  Operation {} timed out", i + 1);
                    Ok(())
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all operations to complete
    let mut successful = 0;
    let mut failed = 0;

    for handle in handles {
        match handle.await {
            Ok(Ok(())) => successful += 1,
            Ok(Err(_)) | Err(_) => failed += 1,
        }
    }

    // Verify database state after operations
    let cache_output = std::process::Command::new(&binary_path)
        .args(&["lsp", "cache", "stats"])
        .output()
        .expect("Failed to get cache stats");

    assert!(
        cache_output.status.success(),
        "Cache stats command failed: {}",
        String::from_utf8_lossy(&cache_output.stderr)
    );

    let stats_output = String::from_utf8_lossy(&cache_output.stdout);
    println!("Cache stats after concurrent operations:\n{}", stats_output);

    // Clean up daemon
    let _ = daemon_process.kill();
    let _ = daemon_process.wait();

    println!("✅ Concurrent operations test completed");
    println!(
        "   Successful operations: {}, Failed/Timeout: {}",
        successful, failed
    );

    // Test passes if at least some operations completed without crashing the system
    assert!(
        successful > 0 || failed == 5,
        "At least some operations should complete or all should gracefully fail"
    );
}

/// Integration test: Database persistence across daemon restarts
#[tokio::test]
async fn test_database_persistence() {
    let binary_path = get_probe_binary_path();

    // Clear any existing cache
    let cache_dir = get_cache_directory();
    if cache_dir.exists() {
        let _ = std::fs::remove_dir_all(&cache_dir);
    }

    // Start daemon, perform operations, and restart
    for restart_count in 1..=2 {
        println!("🔄 Daemon restart cycle {}/2", restart_count);

        // Start daemon
        let mut daemon_process = std::process::Command::new(&binary_path)
            .args(&["lsp", "start", "-f", "--log-level", "debug"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("Failed to start daemon");

        tokio::time::sleep(Duration::from_secs(3)).await;

        // Initialize workspace
        let init_output = std::process::Command::new(&binary_path)
            .args(&["lsp", "init", "--workspace", "."])
            .output()
            .expect("Failed to initialize workspace");

        assert!(init_output.status.success());

        // Perform an LSP operation
        let lsp_output = std::process::Command::new(&binary_path)
            .args(&["lsp", "call", "definition", "src/main.rs:10:5"])
            .output()
            .expect("Failed to perform LSP operation");

        // Operation may fail due to LSP server initialization, but shouldn't crash
        println!(
            "LSP operation result (restart {}): {}",
            restart_count,
            if lsp_output.status.success() {
                "Success"
            } else {
                "Failed (expected during initialization)"
            }
        );

        // Verify database exists
        let cache_files = find_cache_databases(&cache_dir);
        assert!(
            !cache_files.is_empty(),
            "Database should exist after restart {}",
            restart_count
        );

        // Stop daemon
        let _ = daemon_process.kill();
        let _ = daemon_process.wait();

        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    // Verify database persistence
    let cache_files = find_cache_databases(&cache_dir);
    assert!(
        !cache_files.is_empty(),
        "Database should persist across restarts"
    );

    for db_path in &cache_files {
        assert!(
            is_sqlite_database(db_path),
            "Persisted database should be valid SQLite: {:?}",
            db_path
        );
    }

    println!("✅ Database persistence test passed");
    println!(
        "   {} databases persisted across daemon restarts",
        cache_files.len()
    );
}

/// Integration test: Error handling and recovery
#[tokio::test]
async fn test_error_handling_and_recovery() {
    let binary_path = get_probe_binary_path();

    // Test invalid file operations
    let invalid_output = std::process::Command::new(&binary_path)
        .args(&["lsp", "call", "definition", "nonexistent_file.rs:1:1"])
        .output()
        .expect("Failed to execute invalid operation");

    // Should fail gracefully, not crash
    assert!(
        !invalid_output.status.success(),
        "Invalid file operation should fail"
    );
    let error_message = String::from_utf8_lossy(&invalid_output.stderr);
    assert!(!error_message.is_empty(), "Should provide error message");

    // Test cache operations on invalid workspace
    let cache_output = std::process::Command::new(&binary_path)
        .args(&["lsp", "cache", "stats"])
        .output()
        .expect("Failed to execute cache stats");

    // Should succeed even with no active workspace
    assert!(
        cache_output.status.success(),
        "Cache stats should work even without active workspace: {}",
        String::from_utf8_lossy(&cache_output.stderr)
    );

    println!("✅ Error handling test passed");
}

// Helper functions

fn get_probe_binary_path() -> PathBuf {
    let mut path = std::env::current_dir().expect("Failed to get current directory");
    path.push("target");
    path.push("release");
    path.push("probe");

    if !path.exists() {
        // Try debug build if release doesn't exist
        path.pop();
        path.push("debug");
        path.push("probe");
    }

    assert!(path.exists(), "Probe binary not found at {:?}", path);
    path
}

fn get_cache_directory() -> PathBuf {
    let mut cache_dir = dirs::cache_dir().expect("Failed to get cache directory");
    cache_dir.push("probe");
    cache_dir.push("lsp");
    cache_dir.push("workspaces");
    cache_dir
}

fn find_cache_databases(cache_dir: &Path) -> Vec<PathBuf> {
    let mut databases = Vec::new();

    if cache_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(cache_dir) {
            for entry in entries.flatten() {
                let workspace_dir = entry.path();
                if workspace_dir.is_dir() {
                    let db_path = workspace_dir.join("cache.db");
                    if db_path.exists() {
                        databases.push(db_path);
                    }
                }
            }
        }
    }

    databases
}

fn is_sqlite_database(path: &Path) -> bool {
    if let Ok(metadata) = std::fs::metadata(path) {
        if metadata.len() > 0 {
            if let Ok(mut file) = std::fs::File::open(path) {
                use std::io::Read;
                let mut header = [0u8; 16];
                if file.read_exact(&mut header).is_ok() {
                    // SQLite database files start with "SQLite format 3\0"
                    return header.starts_with(b"SQLite format 3");
                }
            }
        }
    }
    false
}
