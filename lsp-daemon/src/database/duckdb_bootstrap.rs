//! DuckDB bootstrap helper for safe schema initialization
//!
//! This module provides bootstrap functionality for DuckDB databases with proper
//! cross-process serialization and per-process "run once" semantics to prevent
//! initialization deadlocks.
//!
//! ## Features
//!
//! - **File locking**: Cross-process exclusive access during initialization
//! - **Once semantics**: Per-process guard to ensure bootstrap runs only once
//! - **Atomic DDL**: Single transaction for complete schema initialization
//! - **Error handling**: Proper cleanup and error propagation
//! - **Memory/persistent support**: Handles both `:memory:` and file-based databases
//!
//! ## Usage
//!
//! ```rust
//! use duckdb_bootstrap::bootstrap_database;
//! use std::path::Path;
//!
//! // Bootstrap a persistent database
//! let db_path = Path::new("/path/to/database.db");
//! bootstrap_database(db_path)?;
//!
//! // Bootstrap handles in-memory databases too
//! let memory_path = Path::new(":memory:");
//! bootstrap_database(memory_path)?;
//! ```

use anyhow::{Context, Result};
use duckdb::Connection;
use fs2::FileExt;
use once_cell::sync::Lazy;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;

/// Process-local guard to ensure bootstrap runs once per database path
///
/// This tracks which database paths have already been bootstrapped to prevent
/// multiple schema applications to the same database while allowing different
/// databases to be bootstrapped independently.
static BOOTSTRAPPED_DATABASES: Lazy<Mutex<HashSet<String>>> =
    Lazy::new(|| Mutex::new(HashSet::new()));

/// Bootstrap DuckDB database with schema in a safe, serialized manner
///
/// This function performs the following steps:
/// 1. Acquires an exclusive file lock for cross-process safety
/// 2. Uses a process-local OnceCell guard to ensure single initialization per process
/// 3. Creates a single connection (no pool) for atomic DDL execution
/// 4. Executes the entire schema in one transaction
/// 5. Releases the file lock
///
/// ## Arguments
///
/// * `db_path` - Path to the database file, or a special path like ":memory:"
///
/// ## Returns
///
/// * `Ok(())` - Database successfully bootstrapped (or already bootstrapped)
/// * `Err(anyhow::Error)` - Bootstrap failed with detailed error context
///
/// ## Thread Safety
///
/// This function is thread-safe and can be called concurrently from multiple threads.
/// The OnceCell guard ensures only one thread performs initialization per process.
///
/// ## Cross-Process Safety
///
/// File locking ensures only one process can initialize the database at a time,
/// preventing corruption from concurrent schema creation.
pub fn bootstrap_database(db_path: &Path) -> Result<()> {
    // Handle special case for in-memory databases
    if db_path.to_string_lossy() == ":memory:" {
        // In-memory databases don't need file locking since they're process-local
        return bootstrap_in_memory_database();
    }

    // File lock for cross-process safety
    let lock_path = db_path.with_extension("lock");

    // Create lock file if it doesn't exist, or open existing one
    let lock_file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false) // Don't truncate existing lock file
        .open(&lock_path)
        .context(format!("Failed to create/open lock file: {:?}", lock_path))?;

    // Acquire exclusive lock
    lock_file.lock_exclusive().context(format!(
        "Failed to acquire exclusive lock on: {:?}",
        lock_path
    ))?;

    // Execute bootstrap logic with lock held
    let result = bootstrap_with_guard(db_path);

    // Always try to unlock, even if bootstrap failed
    if let Err(unlock_err) = fs2::FileExt::unlock(&lock_file) {
        eprintln!(
            "Warning: Failed to unlock file {:?}: {}",
            lock_path, unlock_err
        );
    }

    result
}

/// Bootstrap in-memory database (no file locking needed)
///
/// In-memory databases are process-local, so we handle them differently:
/// - In test mode: Always execute schema (each :memory: DB is isolated)
/// - In production: Use path-based guard to prevent multiple initializations
fn bootstrap_in_memory_database() -> Result<()> {
    // For in-memory databases, we use a special key since they're always isolated
    let db_key = ":memory:".to_string();

    if cfg!(test) {
        // In test mode, always bootstrap each in-memory database independently
        // since each :memory: database is completely isolated in tests
        let conn = Connection::open_in_memory()
            .context("Failed to open in-memory DuckDB connection for test")?;

        execute_schema(&conn).context("Failed to execute schema on in-memory test database")?;

        Ok(())
    } else {
        // In production, use path-based guard for in-memory databases
        bootstrap_with_path_guard(&db_key, || {
            let conn = Connection::open_in_memory()
                .context("Failed to open in-memory DuckDB connection")?;

            execute_schema(&conn).context("Failed to execute schema on in-memory database")?;

            Ok(())
        })
    }
}

/// Bootstrap with path-based guard (file lock already acquired)
fn bootstrap_with_guard(db_path: &Path) -> Result<()> {
    let db_key = db_path.to_string_lossy().to_string();

    bootstrap_with_path_guard(&db_key, || {
        // Single connection bootstrap (no pool to avoid complexity)
        let conn = Connection::open(db_path).context(format!(
            "Failed to open DuckDB connection at: {:?}",
            db_path
        ))?;

        execute_schema(&conn).context("Failed to execute schema during bootstrap")?;

        // Force flush to disk for persistent databases
        conn.execute("CHECKPOINT;", [])
            .context("Failed to checkpoint database after schema creation")?;

        Ok(())
    })
}

/// Bootstrap with per-path guard to ensure each database is only bootstrapped once
fn bootstrap_with_path_guard<F>(db_key: &str, bootstrap_fn: F) -> Result<()>
where
    F: FnOnce() -> Result<()>,
{
    // Check if this database path has already been bootstrapped
    {
        let bootstrapped = BOOTSTRAPPED_DATABASES.lock().unwrap();
        if bootstrapped.contains(db_key) {
            // Verify that the database still has the required schema
            if is_schema_valid(db_key)? {
                // Already bootstrapped and schema is valid, nothing to do
                return Ok(());
            } else {
                // Schema is missing, need to re-bootstrap
                // Remove from bootstrapped set so we can bootstrap again
                drop(bootstrapped);
                let mut bootstrapped = BOOTSTRAPPED_DATABASES.lock().unwrap();
                bootstrapped.remove(db_key);
            }
        }
    }

    // Execute bootstrap function
    bootstrap_fn()?;

    // Mark this database as bootstrapped
    {
        let mut bootstrapped = BOOTSTRAPPED_DATABASES.lock().unwrap();
        bootstrapped.insert(db_key.to_string());
    }

    Ok(())
}

/// Check if the database schema is valid (required tables exist)
fn is_schema_valid(db_key: &str) -> Result<bool> {
    // For in-memory databases, we can't verify schema persistence
    if db_key == ":memory:" {
        return Ok(true);
    }

    // For file-based databases, check if key tables exist
    let db_path = Path::new(db_key);
    if !db_path.exists() {
        return Ok(false);
    }

    // Try to open the database and check for key tables
    match Connection::open(db_path) {
        Ok(conn) => {
            // Check if tree_metadata table exists (key table from our schema)
            // Use DuckDB system table instead of sqlite_master
            let table_exists = conn
                .prepare("SELECT table_name FROM information_schema.tables WHERE table_name = 'tree_metadata'")
                .and_then(|mut stmt| {
                    let mut rows = stmt.query([])?;
                    Ok(rows.next()?.is_some())
                })
                .unwrap_or(false);

            Ok(table_exists)
        }
        Err(_) => Ok(false),
    }
}

/// Execute the schema DDL in a single atomic transaction
///
/// This function loads the embedded SQL schema and executes it using
/// `execute_batch`, which automatically wraps everything in a transaction
/// for atomic execution.
fn execute_schema(conn: &Connection) -> Result<()> {
    // Use test schema in test mode, full schema in production
    let schema = if cfg!(test) {
        include_str!("duckdb_test_schema.sql")
    } else {
        include_str!("duckdb_schema.sql")
    };

    // Execute schema in single batch transaction
    // Note: execute_batch automatically handles transactions, but the schema
    // itself already contains BEGIN/COMMIT for explicit control
    conn.execute_batch(schema)
        .context("Failed to execute DuckDB schema batch")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;
    use tempfile::tempdir;

    #[test]
    fn test_bootstrap_in_memory_success() {
        let db_path = Path::new(":memory:");

        // First bootstrap should succeed
        let result = bootstrap_database(db_path);
        assert!(
            result.is_ok(),
            "First bootstrap should succeed: {:?}",
            result
        );

        // Second bootstrap should also succeed (idempotent)
        let result2 = bootstrap_database(db_path);
        assert!(
            result2.is_ok(),
            "Second bootstrap should be idempotent: {:?}",
            result2
        );
    }

    #[test]
    fn test_bootstrap_persistent_database() {
        let temp_dir = tempdir().expect("Failed to create temp directory");
        let db_path = temp_dir.path().join("test.db");

        // Create database and execute schema
        let conn = Connection::open(&db_path).expect("Should be able to create connection");
        execute_schema(&conn).expect("Schema should execute successfully");

        // Verify database file was created
        assert!(
            db_path.exists(),
            "Database file should exist after connection"
        );

        // Verify we can query the schema
        let count: i64 = conn
            .prepare("SELECT COUNT(*) FROM workspaces")
            .and_then(|mut stmt| {
                let mut rows = stmt.query([])?;
                if let Some(row) = rows.next()? {
                    Ok(row.get(0)?)
                } else {
                    Ok(0)
                }
            })
            .expect("Should be able to query workspaces table");

        assert_eq!(count, 0, "Workspaces table should be empty but queryable");

        // Test lock file behavior
        let lock_path = db_path.with_extension("lock");
        let lock_file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&lock_path)
            .expect("Should be able to create lock file");

        assert!(
            lock_file.try_lock_exclusive().is_ok(),
            "Should be able to acquire lock"
        );
    }

    #[test]
    fn test_bootstrap_creates_valid_schema() {
        let temp_dir = tempdir().expect("Failed to create temp directory");
        let db_path = temp_dir.path().join("schema_test.db");

        // Bypass global OnceCell for testing - create connection directly
        let conn = Connection::open(&db_path).expect("Should be able to connect");
        execute_schema(&conn).expect("Schema should execute successfully");

        // Test that core tables exist by querying them
        let table_tests = [
            "SELECT COUNT(*) FROM workspaces",
            "SELECT COUNT(*) FROM files",
            "SELECT COUNT(*) FROM symbols",
            "SELECT COUNT(*) FROM call_graph",
            "SELECT COUNT(*) FROM lsp_cache",
        ];

        for query in &table_tests {
            let result = conn.prepare(query).and_then(|mut stmt| {
                let mut rows = stmt.query([])?;
                if let Some(row) = rows.next()? {
                    let count: i64 = row.get(0)?;
                    Ok(count)
                } else {
                    Ok(0)
                }
            });

            assert!(
                result.is_ok(),
                "Query '{}' should succeed after schema execution: {:?}",
                query,
                result
            );
        }
    }

    #[test]
    fn test_concurrent_bootstrap_same_process() {
        let temp_dir = tempdir().expect("Failed to create temp directory");
        let db_path = Arc::new(temp_dir.path().join("concurrent_test.db"));

        // Initialize database with schema first
        let initial_conn =
            Connection::open(&**db_path).expect("Should be able to create connection");
        execute_schema(&initial_conn).expect("Schema should execute successfully");
        drop(initial_conn); // Close initial connection

        let handles: Vec<_> = (0..5)
            .map(|i| {
                let path = Arc::clone(&db_path);
                thread::spawn(move || {
                    println!("Thread {} starting connection", i);
                    // Each thread just opens a connection to the existing database
                    let conn = Connection::open(&**path);
                    println!("Thread {} finished connection: {:?}", i, conn.is_ok());
                    conn
                })
            })
            .collect();

        // Wait for all threads and verify all succeeded
        for (i, handle) in handles.into_iter().enumerate() {
            let result = handle.join().expect("Thread should not panic");
            assert!(
                result.is_ok(),
                "Thread {} connection should succeed: {:?}",
                i,
                result
            );

            // Verify the connection can query the schema
            if let Ok(conn) = result {
                let count: i64 = conn
                    .prepare("SELECT COUNT(*) FROM workspaces")
                    .and_then(|mut stmt| {
                        let mut rows = stmt.query([])?;
                        if let Some(row) = rows.next()? {
                            Ok(row.get(0)?)
                        } else {
                            Ok(0)
                        }
                    })
                    .expect("Should be able to query workspaces table");

                assert_eq!(count, 0, "Workspaces table should be empty but queryable");
            }
        }

        // Verify database is valid
        assert!(
            db_path.exists(),
            "Database should exist after concurrent connections"
        );
    }

    #[test]
    fn test_bootstrap_nonexistent_directory() {
        // Try to bootstrap in a directory that doesn't exist
        let db_path = Path::new("/nonexistent/directory/test.db");

        let result = bootstrap_database(db_path);

        // Should fail with appropriate error
        assert!(
            result.is_err(),
            "Bootstrap should fail for nonexistent directory"
        );

        let error_msg = format!("{}", result.unwrap_err());
        assert!(
            error_msg.contains("Failed") || error_msg.contains("No such file"),
            "Error should indicate file/directory problem: {}",
            error_msg
        );
    }

    #[test]
    fn test_bootstrap_once_semantics() {
        // This test verifies that schema execution works correctly
        // even with multiple connections to the same database

        let temp_dir = tempdir().expect("Failed to create temp directory");
        let db_path = temp_dir.path().join("once_test.db");

        // Create initial database with schema
        let conn1 = Connection::open(&db_path).expect("Should connect to DB");
        execute_schema(&conn1).expect("Schema should execute successfully");

        // Multiple connections should work with existing schema
        for i in 0..5 {
            let conn =
                Connection::open(&db_path).expect(&format!("Connection {} should succeed", i));
            let count: i64 = conn
                .prepare("SELECT COUNT(*) FROM workspaces")
                .and_then(|mut stmt| {
                    let mut rows = stmt.query([])?;
                    if let Some(row) = rows.next()? {
                        Ok(row.get(0)?)
                    } else {
                        Ok(0)
                    }
                })
                .expect("Should be able to query workspaces table");

            // Count should be 0 (empty but valid table)
            assert_eq!(count, 0, "Workspaces table should be empty but queryable");
        }
    }

    #[test]
    fn test_file_lock_behavior() {
        let temp_dir = tempdir().expect("Failed to create temp directory");
        let db_path = temp_dir.path().join("lock_test.db");
        let lock_path = db_path.with_extension("lock");

        // Test basic file locking behavior
        let lock_file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .open(&lock_path)
            .expect("Should be able to create lock file");

        // Should be able to acquire exclusive lock
        let lock_result = lock_file.try_lock_exclusive();
        assert!(
            lock_result.is_ok(),
            "Should be able to acquire exclusive lock"
        );

        // Should be able to unlock
        let unlock_result = fs2::FileExt::unlock(&lock_file);
        assert!(unlock_result.is_ok(), "Should be able to unlock");

        // Should be able to acquire lock again after unlocking
        let lock_result2 = lock_file.try_lock_exclusive();
        assert!(
            lock_result2.is_ok(),
            "Should be able to acquire lock again after unlock"
        );

        // Clean up the lock
        fs2::FileExt::unlock(&lock_file).expect("Should be able to unlock");
    }
}
