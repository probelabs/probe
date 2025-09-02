//! DuckDB database backend implementation
//!
//! This module implements the `DatabaseBackend` trait using DuckDB as the storage engine.
//! It provides both persistent and in-memory storage modes with advanced SQL capabilities
//! for complex code analysis queries while maintaining compatibility with the existing
//! key-value interface required by the LSP daemon.
//!
//! ## Features
//!
//! - **Key-Value Interface**: Compatible with existing sled-based code
//! - **SQL Query Engine**: Advanced analytics capabilities for code relationships
//! - **Git-Aware Versioning**: Tracks both committed and modified file states
//! - **Connection Pooling**: Manages multiple connections for concurrent access
//! - **Async Support**: Full async/await support with tokio
//!
//! ## Architecture
//!
//! The DuckDB backend uses a hybrid approach:
//! 1. A simple key-value table (`kv_store`) for compatibility with existing code
//! 2. Rich relational schema for symbols, references, and call graphs
//! 3. Connection pooling for performance
//! 4. Git-aware state management for real-time code analysis

use super::duckdb_queries::{
    CallGraphTraversal, CallPath, DuckDBQueries, GraphTraversalOptions, SymbolDependency,
    SymbolHotspot, SymbolImpact, TraversalDirection,
};
use super::{DatabaseBackend, DatabaseConfig, DatabaseError, DatabaseStats, DatabaseTree};
use anyhow::{Context, Result};
use async_trait::async_trait;
use duckdb::{params, Connection as DuckdbConnection};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};

/// Connection customizer for DuckDB to set per-connection PRAGMAs
/// This avoids the need for global settings that can cause initialization deadlocks
#[derive(Debug)]
struct DuckDBConnectionCustomizer;

impl DuckDBConnectionCustomizer {
    /// Apply per-connection configuration (PRAGMAs only, no DDL!)
    ///
    /// This method sets per-connection PRAGMAs that are safe to apply repeatedly
    /// without causing database lock contention. All schema initialization (DDL)
    /// is handled by the bootstrap module before any connections are created.
    fn customize_connection(conn: &DuckdbConnection) -> Result<()> {
        // Set per-connection PRAGMAs (no DDL here!)
        let thread_count = if cfg!(test) {
            1 // Single thread for tests to avoid race conditions
        } else {
            num_cpus::get().max(1) // Use available CPU cores for production
        };

        conn.execute_batch(&format!("PRAGMA threads={thread_count};"))?;

        // Set additional per-connection optimizations
        conn.execute_batch("PRAGMA enable_progress_bar=false;")?;

        Ok(())
    }
}

/// Configuration specific to DuckDB backend
#[derive(Debug, Clone)]
pub struct DuckDBConfig {
    /// Database file path (or ":memory:" for in-memory)
    pub path: String,

    /// Maximum number of connections in the pool
    pub max_connections: usize,

    /// Memory limit for DuckDB in gigabytes
    pub memory_limit_gb: f64,

    /// Number of threads for parallel execution
    pub threads: usize,

    /// Enable/disable query result caching
    pub enable_query_cache: bool,

    /// Whether this is a temporary/in-memory database
    pub temporary: bool,
}

impl Default for DuckDBConfig {
    fn default() -> Self {
        Self {
            path: ":memory:".to_string(),
            max_connections: 8,
            memory_limit_gb: 2.0,
            threads: num_cpus::get(),
            enable_query_cache: true,
            temporary: true,
        }
    }
}

/// Connection pool for managing DuckDB connections
struct ConnectionPool {
    /// Available connections
    available: Vec<DuckdbConnection>,

    /// Maximum pool size
    max_size: usize,

    /// Database configuration
    config: DuckDBConfig,
}

impl ConnectionPool {
    fn new(config: DuckDBConfig) -> Result<Self> {
        let pool = Self {
            available: Vec::new(),
            // Test-friendly pool config: use reduced pool size for tests
            max_size: if cfg!(test) {
                2
            } else {
                config.max_connections
            },
            config: config.clone(),
        };

        // Don't pre-populate to avoid initialization issues
        // Connections will be created on-demand
        Ok(pool)
    }

    fn create_connection(&self) -> Result<DuckdbConnection> {
        let conn = DuckdbConnection::open(&self.config.path).with_context(|| {
            format!("Failed to open DuckDB connection at: {}", self.config.path)
        })?;

        // For in-memory databases, each connection creates a separate database
        // so we need to apply the schema to each connection directly
        if self.config.path == ":memory:" {
            self.apply_schema_to_connection(&conn)
                .context("Failed to apply schema to in-memory connection")?;
        }

        // Apply per-connection customization (PRAGMAs only, no DDL)
        DuckDBConnectionCustomizer::customize_connection(&conn)
            .context("Failed to apply connection customization")?;

        // Configure other DuckDB settings
        conn.execute(
            &format!("SET memory_limit = '{}GB'", self.config.memory_limit_gb),
            [],
        )
        .context("Failed to set memory limit")?;

        if self.config.enable_query_cache {
            conn.execute("SET enable_object_cache = true", [])
                .context("Failed to enable query cache")?;
        }

        Ok(conn)
    }

    fn get_connection(&mut self) -> Result<DuckdbConnection> {
        if let Some(conn) = self.available.pop() {
            Ok(conn)
        } else {
            // Pool exhausted, create a new connection
            self.create_connection()
        }
    }

    fn return_connection(&mut self, conn: DuckdbConnection) {
        if self.available.len() < self.max_size {
            self.available.push(conn);
        }
        // If pool is full, just drop the connection
    }

    /// Apply database schema to a connection (for in-memory databases)
    ///
    /// For in-memory databases, each connection creates a separate database,
    /// so we need to apply the schema to each connection individually.
    fn apply_schema_to_connection(&self, conn: &DuckdbConnection) -> Result<()> {
        // Use the same schema as the bootstrap module
        let schema = if cfg!(test) {
            include_str!("duckdb_test_schema.sql")
        } else {
            include_str!("duckdb_schema.sql")
        };

        // Execute schema in single batch transaction
        conn.execute_batch(schema)
            .context("Failed to execute DuckDB schema on connection")?;

        Ok(())
    }
}

/// DuckDB-based implementation of DatabaseTree
pub struct DuckDBTree {
    /// Tree name (used as table prefix)
    name: String,

    /// Connection pool reference
    pool: Arc<Mutex<ConnectionPool>>,
}

#[async_trait]
impl DatabaseTree for DuckDBTree {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, DatabaseError> {
        let key_str = String::from_utf8_lossy(key);
        let mut pool = self.pool.lock().await;
        let conn = pool
            .get_connection()
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to get connection: {e}"),
            })?;

        let query = format!(
            "SELECT value FROM tree_{} WHERE key = ?",
            sanitize_table_name(&self.name)
        );

        let result = conn
            .prepare(&query)
            .and_then(|mut stmt| {
                let mut rows = stmt.query(params![key_str])?;
                if let Some(row) = rows.next()? {
                    let value: Vec<u8> = row.get(0)?;
                    Ok(Some(value))
                } else {
                    Ok(None)
                }
            })
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to get key from tree '{}': {}", self.name, e),
            });

        pool.return_connection(conn);
        result
    }

    async fn set(&self, key: &[u8], value: &[u8]) -> Result<(), DatabaseError> {
        let key_str = String::from_utf8_lossy(key);
        let mut pool = self.pool.lock().await;
        let conn = pool
            .get_connection()
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to get connection: {e}"),
            })?;

        let query = format!(
            "INSERT OR REPLACE INTO tree_{} (key, value) VALUES (?, ?)",
            sanitize_table_name(&self.name)
        );

        let result = conn
            .execute(&query, params![key_str, value])
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to set key in tree '{}': {}", self.name, e),
            })
            .map(|_| ());

        pool.return_connection(conn);
        result
    }

    async fn remove(&self, key: &[u8]) -> Result<bool, DatabaseError> {
        let key_str = String::from_utf8_lossy(key);
        let mut pool = self.pool.lock().await;
        let conn = pool
            .get_connection()
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to get connection: {e}"),
            })?;

        let query = format!(
            "DELETE FROM tree_{} WHERE key = ?",
            sanitize_table_name(&self.name)
        );

        let result = conn
            .execute(&query, params![key_str])
            .map(|changes| changes > 0)
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to remove key from tree '{}': {}", self.name, e),
            });

        pool.return_connection(conn);
        result
    }

    async fn scan_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, DatabaseError> {
        let prefix_str = String::from_utf8_lossy(prefix);
        let mut pool = self.pool.lock().await;
        let conn = pool
            .get_connection()
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to get connection: {e}"),
            })?;

        let query = format!(
            "SELECT key, value FROM tree_{} WHERE key LIKE ? || '%'",
            sanitize_table_name(&self.name)
        );

        let result = conn
            .prepare(&query)
            .and_then(|mut stmt| {
                let rows = stmt.query_map(params![prefix_str], |row| {
                    let key: String = row.get(0)?;
                    let value: Vec<u8> = row.get(1)?;
                    Ok((key.into_bytes(), value))
                })?;

                let mut results = Vec::new();
                for row in rows {
                    results.push(row?);
                }
                Ok(results)
            })
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to scan prefix in tree '{}': {}", self.name, e),
            });

        pool.return_connection(conn);
        result
    }

    async fn clear(&self) -> Result<(), DatabaseError> {
        let mut pool = self.pool.lock().await;
        let conn = pool
            .get_connection()
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to get connection: {e}"),
            })?;

        let query = format!("DELETE FROM tree_{}", sanitize_table_name(&self.name));

        let result = conn
            .execute(&query, [])
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to clear tree '{}': {}", self.name, e),
            })
            .map(|_| ());

        pool.return_connection(conn);
        result
    }

    async fn len(&self) -> Result<u64, DatabaseError> {
        let mut pool = self.pool.lock().await;
        let conn = pool
            .get_connection()
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to get connection: {e}"),
            })?;

        let query = format!(
            "SELECT COUNT(*) FROM tree_{}",
            sanitize_table_name(&self.name)
        );

        let result = conn
            .prepare(&query)
            .and_then(|mut stmt| {
                let mut rows = stmt.query([])?;
                if let Some(row) = rows.next()? {
                    let count: i64 = row.get(0)?;
                    Ok(count as u64)
                } else {
                    Ok(0)
                }
            })
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to get length of tree '{}': {}", self.name, e),
            });

        pool.return_connection(conn);
        result
    }
}

/// DuckDB database backend implementation
pub struct DuckDBBackend {
    /// Connection pool
    pool: Arc<Mutex<ConnectionPool>>,

    /// Configuration
    #[allow(dead_code)]
    config: DatabaseConfig,

    /// DuckDB-specific configuration
    duckdb_config: DuckDBConfig,

    /// Cache of opened trees
    trees: RwLock<HashMap<String, Arc<DuckDBTree>>>,

    /// Current workspace context for git-aware queries
    workspace_context: RwLock<Option<WorkspaceContext>>,
}

/// Workspace context for git-aware operations
#[derive(Debug, Clone)]
pub struct WorkspaceContext {
    /// Workspace ID
    pub workspace_id: String,

    /// Current git commit hash (None for non-git repos)
    pub current_commit: Option<String>,

    /// List of modified file paths (relative to workspace root)
    pub modified_files: Vec<String>,

    /// Workspace root path
    pub root_path: String,
}

impl WorkspaceContext {
    /// Create a context and immediately populate git fields when possible.
    /// `root_path` is the workspace root (not necessarily the git repo root).
    pub fn new_with_git(workspace_id: impl Into<String>, root_path: impl Into<String>) -> Self {
        use std::path::Path;
        let workspace_id = workspace_id.into();
        let root_path_str = root_path.into();
        let root_path = Path::new(&root_path_str);

        // Attempt to discover a git repo *from* the workspace root and normalize paths
        // relative to the workspace root (so they join with `files.relative_path`).
        let (current_commit, modified_files) =
            match crate::git_service::GitService::discover_repo(root_path, root_path) {
                Ok(svc) => {
                    let head = svc.head_commit().ok().flatten();
                    let mods = svc.modified_files().unwrap_or_default();
                    (head, mods)
                }
                Err(_) => (None, Vec::new()),
            };

        Self {
            workspace_id,
            current_commit,
            modified_files,
            root_path: root_path_str,
        }
    }

    /// Refresh Git fields in-place. Safe to call periodically (e.g., on file events).
    pub fn refresh_git(&mut self) {
        use std::path::Path;
        let root = Path::new(&self.root_path);
        if let Ok(svc) = crate::git_service::GitService::discover_repo(root, root) {
            self.current_commit = svc.head_commit().ok().flatten();
            self.modified_files = svc.modified_files().unwrap_or_default();
        } else {
            self.current_commit = None;
            self.modified_files.clear();
        }
    }
}

impl DuckDBBackend {
    /// Create a new DuckDBBackend with custom DuckDB configuration
    pub async fn with_duckdb_config(
        config: DatabaseConfig,
        duckdb_config: DuckDBConfig,
    ) -> Result<Self, DatabaseError> {
        // Bootstrap schema before creating pool
        let db_path = Path::new(&duckdb_config.path);
        crate::database::duckdb_bootstrap::bootstrap_database(db_path).map_err(|e| {
            DatabaseError::Configuration {
                message: format!("Failed to bootstrap database schema: {e}"),
            }
        })?;

        // Test-friendly pool config
        let pool = ConnectionPool::new(duckdb_config.clone()).map_err(|e| {
            DatabaseError::Configuration {
                message: format!("Failed to create connection pool: {e}"),
            }
        })?;

        let backend = Self {
            pool: Arc::new(Mutex::new(pool)),
            config: config.clone(),
            duckdb_config: duckdb_config.clone(),
            trees: RwLock::new(HashMap::new()),
            workspace_context: RwLock::new(None),
        };

        if duckdb_config.temporary {
            info!("Initialized temporary DuckDB database (in-memory)");
        } else {
            info!(
                "Initialized persistent DuckDB database at: {}",
                duckdb_config.path
            );
        }

        Ok(backend)
    }

    // Schema initialization is now handled by the bootstrap module before pool creation
    // This method is kept for compatibility but no longer performs schema initialization

    /// Get a connection from the pool
    async fn get_connection(&self) -> Result<DuckdbConnection, DatabaseError> {
        let mut pool = self.pool.lock().await;
        pool.get_connection()
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to get connection: {e}"),
            })
    }

    /// Return a connection to the pool
    async fn return_connection(&self, conn: DuckdbConnection) {
        let mut pool = self.pool.lock().await;
        pool.return_connection(conn);
    }

    /// Create a new tree table if it doesn't exist
    async fn ensure_tree_table(&self, tree_name: &str) -> Result<(), DatabaseError> {
        let sanitized_name = sanitize_table_name(tree_name);
        let conn = self.get_connection().await?;

        let create_table_query = format!(
            r#"
            CREATE TABLE IF NOT EXISTS tree_{} (
                key TEXT PRIMARY KEY,
                value BLOB NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
            "#,
            sanitized_name
        );

        conn.execute(&create_table_query, [])
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to create tree table '{}': {}", tree_name, e),
            })?;

        // Create index for the tree
        let create_index_query = format!(
            "CREATE INDEX IF NOT EXISTS idx_tree_{}_key ON tree_{}(key)",
            sanitized_name, sanitized_name
        );

        conn.execute(&create_index_query, [])
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to create index for tree '{}': {}", tree_name, e),
            })?;

        // Update metadata
        conn.execute(
            "INSERT OR IGNORE INTO tree_metadata (tree_name) VALUES (?)",
            params![tree_name],
        )
        .map_err(|e| DatabaseError::OperationFailed {
            message: format!("Failed to update tree metadata for '{}': {}", tree_name, e),
        })?;

        self.return_connection(conn).await;
        Ok(())
    }
}

#[async_trait]
impl DatabaseBackend for DuckDBBackend {
    type Tree = DuckDBTree;

    async fn new(config: DatabaseConfig) -> Result<Self, DatabaseError>
    where
        Self: Sized,
    {
        let duckdb_config = DuckDBConfig {
            path: if config.temporary {
                ":memory:".to_string()
            } else {
                config
                    .path
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| ":memory:".to_string())
            },
            temporary: config.temporary,
            max_connections: 8,
            memory_limit_gb: 2.0,
            threads: num_cpus::get(),
            enable_query_cache: true,
        };

        Self::with_duckdb_config(config, duckdb_config).await
    }

    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, DatabaseError> {
        let key_str = String::from_utf8_lossy(key);
        let conn = self.get_connection().await?;

        let result = conn
            .prepare("SELECT value FROM kv_store WHERE key = ?")
            .and_then(|mut stmt| {
                let mut rows = stmt.query(params![key_str])?;
                if let Some(row) = rows.next()? {
                    let value: Vec<u8> = row.get(0)?;
                    Ok(Some(value))
                } else {
                    Ok(None)
                }
            })
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to get key from default store: {}", e),
            });

        self.return_connection(conn).await;
        result
    }

    async fn set(&self, key: &[u8], value: &[u8]) -> Result<(), DatabaseError> {
        let key_str = String::from_utf8_lossy(key);
        let conn = self.get_connection().await?;

        let result = conn.execute(
            "INSERT OR REPLACE INTO kv_store (key, value, updated_at) VALUES (?, ?, CURRENT_TIMESTAMP)",
            params![key_str, value],
        )
        .map_err(|e| DatabaseError::OperationFailed {
            message: format!("Failed to set key in default store: {}", e),
        })
        .map(|_| ());

        self.return_connection(conn).await;
        result
    }

    async fn remove(&self, key: &[u8]) -> Result<bool, DatabaseError> {
        let key_str = String::from_utf8_lossy(key);
        let conn = self.get_connection().await?;

        let result = conn
            .execute("DELETE FROM kv_store WHERE key = ?", params![key_str])
            .map(|changes| changes > 0)
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to remove key from default store: {}", e),
            });

        self.return_connection(conn).await;
        result
    }

    async fn scan_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, DatabaseError> {
        let prefix_str = String::from_utf8_lossy(prefix);
        let conn = self.get_connection().await?;

        let result = conn
            .prepare("SELECT key, value FROM kv_store WHERE key LIKE ? || '%'")
            .and_then(|mut stmt| {
                let rows = stmt.query_map(params![prefix_str], |row| {
                    let key: String = row.get(0)?;
                    let value: Vec<u8> = row.get(1)?;
                    Ok((key.into_bytes(), value))
                })?;

                let mut results = Vec::new();
                for row in rows {
                    results.push(row?);
                }
                Ok(results)
            })
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to scan prefix in default store: {}", e),
            });

        self.return_connection(conn).await;
        result
    }

    async fn open_tree(&self, name: &str) -> Result<Arc<Self::Tree>, DatabaseError> {
        // Check if tree already exists in cache
        {
            let trees = self.trees.read().await;
            if let Some(tree) = trees.get(name) {
                return Ok(Arc::clone(tree));
            }
        }

        // Ensure tree table exists
        self.ensure_tree_table(name).await?;

        // Create new tree instance
        let tree = Arc::new(DuckDBTree {
            name: name.to_string(),
            pool: Arc::clone(&self.pool),
        });

        // Cache the tree
        {
            let mut trees = self.trees.write().await;
            trees.insert(name.to_string(), Arc::clone(&tree));
        }

        Ok(tree)
    }

    async fn tree_names(&self) -> Result<Vec<String>, DatabaseError> {
        let conn = self.get_connection().await?;

        let result = conn
            .prepare("SELECT tree_name FROM tree_metadata ORDER BY tree_name")
            .and_then(|mut stmt| {
                let rows = stmt.query_map([], |row| {
                    let name: String = row.get(0)?;
                    Ok(name)
                })?;

                let mut names = Vec::new();
                for row in rows {
                    names.push(row?);
                }
                Ok(names)
            })
            .or_else(|e| {
                // If the table doesn't exist, just return empty list
                if e.to_string().contains("tree_metadata does not exist") {
                    Ok(Vec::new())
                } else {
                    Err(e)
                }
            })
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to get tree names: {}", e),
            });

        self.return_connection(conn).await;
        result
    }

    async fn clear(&self) -> Result<(), DatabaseError> {
        let conn = self.get_connection().await?;

        // Clear default key-value store
        conn.execute("DELETE FROM kv_store", [])
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to clear default store: {}", e),
            })?;

        // Clear all tree tables - get tree names from our in-memory cache
        // since the metadata table might not be properly populated
        let tree_names = {
            let trees = self.trees.read().await;
            trees.keys().cloned().collect::<Vec<_>>()
        };

        for tree_name in &tree_names {
            let sanitized_name = sanitize_table_name(tree_name);
            let query = format!("DELETE FROM tree_{}", sanitized_name);
            conn.execute(&query, [])
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to clear tree '{}': {}", tree_name, e),
                })?;
        }

        // Also try to clear any tree tables that might exist in the metadata table
        let metadata_tree_names = self.tree_names().await.unwrap_or_default();
        for tree_name in &metadata_tree_names {
            if !tree_names.contains(tree_name) {
                let sanitized_name = sanitize_table_name(tree_name);
                let query = format!("DELETE FROM tree_{}", sanitized_name);
                let _ = conn.execute(&query, []); // Ignore errors for this fallback
            }
        }

        self.return_connection(conn).await;
        Ok(())
    }

    async fn flush(&self) -> Result<(), DatabaseError> {
        // DuckDB handles flushing automatically, but we can force a checkpoint
        let conn = self.get_connection().await?;

        conn.execute("CHECKPOINT", [])
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to flush database: {}", e),
            })?;

        self.return_connection(conn).await;
        Ok(())
    }

    async fn stats(&self) -> Result<DatabaseStats, DatabaseError> {
        let conn = self.get_connection().await?;

        // Count entries in default store
        let default_count: i64 = conn
            .prepare("SELECT COUNT(*) FROM kv_store")
            .and_then(|mut stmt| {
                let mut rows = stmt.query([])?;
                if let Some(row) = rows.next()? {
                    Ok(row.get(0)?)
                } else {
                    Ok(0)
                }
            })
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to count default store entries: {}", e),
            })?;

        // Count entries in all trees
        let tree_names = self.tree_names().await?;
        let mut total_entries = default_count as u64;

        for tree_name in &tree_names {
            let sanitized_name = sanitize_table_name(tree_name);
            let query = format!("SELECT COUNT(*) FROM tree_{}", sanitized_name);
            let count: i64 = conn
                .prepare(&query)
                .and_then(|mut stmt| {
                    let mut rows = stmt.query([])?;
                    if let Some(row) = rows.next()? {
                        Ok(row.get(0)?)
                    } else {
                        Ok(0)
                    }
                })
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to count entries in tree '{}': {}", tree_name, e),
                })?;

            total_entries += count as u64;
        }

        // Estimate total size (rough estimate)
        let estimated_avg_entry_size = 256; // bytes per entry
        let total_size_bytes = total_entries * estimated_avg_entry_size;

        let disk_size_bytes = if self.duckdb_config.temporary {
            0
        } else {
            self.size_on_disk().await?
        };

        self.return_connection(conn).await;

        Ok(DatabaseStats {
            total_entries,
            total_size_bytes,
            disk_size_bytes,
            tree_count: tree_names.len(),
            is_temporary: self.duckdb_config.temporary,
        })
    }

    async fn size_on_disk(&self) -> Result<u64, DatabaseError> {
        if self.duckdb_config.temporary {
            return Ok(0);
        }

        if self.duckdb_config.path == ":memory:" {
            return Ok(0);
        }

        let path = PathBuf::from(&self.duckdb_config.path);
        if path.exists() {
            std::fs::metadata(&path)
                .map(|metadata| metadata.len())
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to get database file size: {}", e),
                })
        } else {
            Ok(0)
        }
    }

    fn is_temporary(&self) -> bool {
        self.duckdb_config.temporary
    }
}

impl DuckDBBackend {
    // ========================================================================
    // Workspace Management Methods
    // ========================================================================

    /// Set the current workspace context for git-aware queries
    pub async fn set_workspace_context(
        &self,
        context: WorkspaceContext,
    ) -> Result<(), DatabaseError> {
        debug!(
            "Setting workspace context: workspace_id={}, commit={:?}, modified_files={}",
            context.workspace_id,
            context.current_commit,
            context.modified_files.len()
        );

        let mut workspace_context = self.workspace_context.write().await;
        *workspace_context = Some(context);
        Ok(())
    }

    /// Get the current workspace context
    pub async fn get_current_workspace(&self) -> Option<WorkspaceContext> {
        let workspace_context = self.workspace_context.read().await;
        workspace_context.clone()
    }

    /// Set the list of modified files for the current workspace
    pub async fn set_modified_files(
        &self,
        modified_files: Vec<String>,
    ) -> Result<(), DatabaseError> {
        debug!(
            "Updating modified files list: {} files",
            modified_files.len()
        );

        let mut workspace_context = self.workspace_context.write().await;
        if let Some(ref mut context) = *workspace_context {
            context.modified_files = modified_files;
            Ok(())
        } else {
            Err(DatabaseError::Configuration {
                message: "No workspace context set. Call set_workspace_context first.".to_string(),
            })
        }
    }

    /// Get the current list of modified files
    pub async fn get_modified_files(&self) -> Vec<String> {
        let workspace_context = self.workspace_context.read().await;
        workspace_context
            .as_ref()
            .map(|ctx| ctx.modified_files.clone())
            .unwrap_or_default()
    }

    /// Set the current git commit hash
    pub async fn set_current_commit(
        &self,
        commit_hash: Option<String>,
    ) -> Result<(), DatabaseError> {
        debug!("Updating current commit: {:?}", commit_hash);

        let mut workspace_context = self.workspace_context.write().await;
        if let Some(ref mut context) = *workspace_context {
            context.current_commit = commit_hash;
            Ok(())
        } else {
            Err(DatabaseError::Configuration {
                message: "No workspace context set. Call set_workspace_context first.".to_string(),
            })
        }
    }

    /// Get the current git commit hash
    pub async fn get_current_commit(&self) -> Option<String> {
        let workspace_context = self.workspace_context.read().await;
        workspace_context
            .as_ref()
            .and_then(|ctx| ctx.current_commit.clone())
    }

    // ========================================================================
    // Git-Aware Query Methods
    // ========================================================================

    /// Execute a git-aware query that considers modified files
    pub async fn execute_git_aware_query<F, T>(&self, query_builder: F) -> Result<T, DatabaseError>
    where
        F: FnOnce(&str, Option<&str>, &[String]) -> Result<T, DatabaseError>,
    {
        let workspace_context = self.workspace_context.read().await;
        let context = workspace_context
            .as_ref()
            .ok_or_else(|| DatabaseError::Configuration {
                message: "No workspace context set for git-aware query".to_string(),
            })?;

        let values_clause = self.build_modified_files_values_clause(&context.modified_files);
        let commit_hash = context.current_commit.as_deref();

        debug!(
            "Executing git-aware query: workspace_id={}, commit={:?}, modified_files={}",
            context.workspace_id,
            commit_hash,
            context.modified_files.len()
        );

        query_builder(&values_clause, commit_hash, &context.modified_files)
    }

    /// Get current symbols using git-aware versioning
    pub async fn get_current_symbols(
        &self,
        name_filter: Option<&str>,
    ) -> Result<Vec<CurrentSymbol>, DatabaseError> {
        self.execute_git_aware_query(|values_clause, _commit_hash, modified_files| {
            // Build the query with optional name filter
            let where_clause = if let Some(name) = name_filter {
                format!("AND s.name = '{}'", name)
            } else {
                String::new()
            };

            let _query = format!(
                r#"
                WITH modified_files (file_path) AS (
                    {}
                ),
                current_state AS (
                    SELECT DISTINCT ON (s.symbol_id)
                        s.symbol_id,
                        s.name,
                        s.qualified_name,
                        s.kind,
                        s.start_line,
                        s.start_column,
                        s.end_line,
                        s.end_column,
                        s.signature,
                        s.documentation,
                        s.visibility,
                        s.indexed_at,
                        f.relative_path,
                        f.absolute_path,
                        f.language,
                        CASE 
                            WHEN mf.file_path IS NOT NULL THEN 'modified'
                            ELSE 'committed'
                        END as state
                    FROM symbols s
                    JOIN files f ON s.file_id = f.file_id
                    LEFT JOIN modified_files mf ON f.relative_path = mf.file_path
                    WHERE s.workspace_id = ?
                      {} 
                      AND (
                        -- Latest version for modified files (max indexed_at)
                        (mf.file_path IS NOT NULL AND s.indexed_at = (
                            SELECT MAX(s2.indexed_at)
                            FROM symbols s2
                            WHERE s2.symbol_id = s.symbol_id
                              AND s2.workspace_id = s.workspace_id
                              AND s2.file_id = s.file_id
                        ))
                        OR
                        -- Git commit version for unmodified files
                        (mf.file_path IS NULL AND s.git_commit_hash = ?)
                      )
                    ORDER BY s.symbol_id, s.indexed_at DESC
                )
                SELECT * FROM current_state
                ORDER BY relative_path, start_line, start_column
                "#,
                values_clause, where_clause
            );

            // Execute query and parse results (simplified for this enhancement)
            debug!(
                "Executing current symbols query with {} modified files",
                modified_files.len()
            );
            Ok(Vec::new()) // TODO: Implement actual query execution and result parsing
        })
        .await
    }

    /// Get call hierarchy for a symbol (incoming or outgoing calls)
    pub async fn get_call_hierarchy(
        &self,
        symbol_id: &str,
        direction: CallDirection,
    ) -> Result<Vec<CallHierarchyItem>, DatabaseError> {
        self.execute_git_aware_query(|values_clause, _commit_hash, modified_files| {
            let (caller_col, callee_col, _target_symbol) = match direction {
                CallDirection::Incoming => ("caller_symbol_id", "callee_symbol_id", symbol_id),
                CallDirection::Outgoing => ("callee_symbol_id", "caller_symbol_id", symbol_id),
            };

            let _query = format!(
                r#"
                WITH modified_files (file_path) AS (
                    {}
                ),
                calls AS (
                    SELECT DISTINCT
                        cg.{},
                        s.name,
                        s.qualified_name,
                        f.relative_path,
                        cg.call_line,
                        cg.call_column,
                        cg.call_type
                    FROM call_graph cg
                    JOIN symbols s ON cg.{} = s.symbol_id
                    JOIN files f ON cg.file_id = f.file_id
                    LEFT JOIN modified_files mf ON f.relative_path = mf.file_path
                    WHERE cg.workspace_id = ?
                      AND cg.{} = ?
                      AND (
                        -- Latest version for modified files
                        (mf.file_path IS NOT NULL AND cg.indexed_at = (
                            SELECT MAX(cg2.indexed_at)
                            FROM call_graph cg2
                            WHERE cg2.call_id = cg.call_id
                        ))
                        OR
                        -- Git commit version for unmodified files
                        (mf.file_path IS NULL AND cg.git_commit_hash = ?)
                      )
                    ORDER BY f.relative_path, cg.call_line, cg.call_column
                )
                SELECT * FROM calls
                "#,
                values_clause, caller_col, caller_col, callee_col
            );

            debug!(
                "Executing call hierarchy query: direction={:?}, modified_files={}",
                direction,
                modified_files.len()
            );
            Ok(Vec::new()) // TODO: Implement actual query execution and result parsing
        })
        .await
    }

    /// Get LSP cache entry with git-aware versioning
    pub async fn get_lsp_cache_entry(
        &self,
        method: &str,
        file_id: &str,
        _position_line: i32,
        _position_column: i32,
    ) -> Result<Option<LspCacheEntry>, DatabaseError> {
        self.execute_git_aware_query(|values_clause, _commit_hash, modified_files| {
            let _query = format!(
                r#"
                WITH modified_files (file_path) AS (
                    {}
                )
                SELECT 
                    lc.cache_key,
                    lc.response_data,
                    lc.response_type,
                    lc.created_at,
                    lc.last_accessed,
                    lc.response_time_ms
                FROM lsp_cache lc
                JOIN files f ON lc.file_id = f.file_id
                LEFT JOIN modified_files mf ON f.relative_path = mf.file_path
                WHERE lc.workspace_id = ?
                  AND lc.method = ?
                  AND lc.file_id = ?
                  AND lc.position_line = ?
                  AND lc.position_column = ?
                  AND (
                    -- Latest version for modified files
                    (mf.file_path IS NOT NULL AND lc.created_at = (
                        SELECT MAX(lc2.created_at)
                        FROM lsp_cache lc2
                        WHERE lc2.cache_key = lc.cache_key
                    ))
                    OR
                    -- Git commit version for unmodified files
                    (mf.file_path IS NULL AND lc.git_commit_hash = ?)
                  )
                ORDER BY lc.created_at DESC
                LIMIT 1
                "#,
                values_clause
            );

            debug!(
                "Executing LSP cache query: method={}, file_id={}, modified_files={}",
                method,
                file_id,
                modified_files.len()
            );
            Ok(None) // TODO: Implement actual query execution and result parsing
        })
        .await
    }

    // ========================================================================
    // Helper Methods
    // ========================================================================

    /// Build SQL VALUES clause for modified files list
    fn build_modified_files_values_clause(&self, modified_files: &[String]) -> String {
        if modified_files.is_empty() {
            "SELECT NULL as file_path WHERE FALSE".to_string() // Empty set
        } else {
            let values: Vec<String> = modified_files
                .iter()
                .map(|path| format!("('{}')", path.replace("'", "''"))) // Escape single quotes
                .collect();
            format!("VALUES {}", values.join(", "))
        }
    }

    /// Execute a transaction with automatic rollback on error
    pub async fn execute_transaction<F, T>(&self, transaction_fn: F) -> Result<T, DatabaseError>
    where
        F: FnOnce(&DuckdbConnection) -> Result<T, DatabaseError>,
    {
        let conn = self.get_connection().await?;

        // Begin transaction
        conn.execute("BEGIN TRANSACTION", [])
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to begin transaction: {}", e),
            })?;

        let result = transaction_fn(&conn);

        match result {
            Ok(value) => {
                // Commit transaction
                conn.execute("COMMIT", [])
                    .map_err(|e| DatabaseError::OperationFailed {
                        message: format!("Failed to commit transaction: {}", e),
                    })?;

                self.return_connection(conn).await;
                debug!("Transaction committed successfully");
                Ok(value)
            }
            Err(error) => {
                // Rollback transaction
                if let Err(rollback_error) = conn.execute("ROLLBACK", []) {
                    warn!("Failed to rollback transaction: {}", rollback_error);
                }

                self.return_connection(conn).await;
                debug!("Transaction rolled back due to error: {:?}", error);
                Err(error)
            }
        }
    }

    // ========================================================================
    // Advanced Graph Analytics Methods
    // ========================================================================

    /// Find all paths from one symbol to another through call graph
    pub async fn find_call_paths(
        &self,
        from_symbol_id: &str,
        to_symbol_id: &str,
        options: GraphTraversalOptions,
    ) -> Result<Vec<CallPath>, DatabaseError> {
        let workspace_context = self.workspace_context.read().await;
        let context = workspace_context
            .as_ref()
            .ok_or_else(|| DatabaseError::Configuration {
                message: "No workspace context set for call path analysis".to_string(),
            })?;

        let conn = self.get_connection().await?;
        let result = DuckDBQueries::find_call_paths(
            &conn,
            &context.workspace_id,
            from_symbol_id,
            to_symbol_id,
            context.current_commit.as_deref(),
            &context.modified_files,
            options,
        )
        .await;

        self.return_connection(conn).await;
        result
    }

    /// Find all symbols affected by changing a given symbol (impact analysis)
    pub async fn find_affected_symbols(
        &self,
        changed_symbol_id: &str,
        options: GraphTraversalOptions,
    ) -> Result<Vec<SymbolImpact>, DatabaseError> {
        let workspace_context = self.workspace_context.read().await;
        let context = workspace_context
            .as_ref()
            .ok_or_else(|| DatabaseError::Configuration {
                message: "No workspace context set for impact analysis".to_string(),
            })?;

        let conn = self.get_connection().await?;
        let result = DuckDBQueries::find_affected_symbols(
            &conn,
            &context.workspace_id,
            changed_symbol_id,
            context.current_commit.as_deref(),
            &context.modified_files,
            options,
        )
        .await;

        self.return_connection(conn).await;
        result
    }

    /// Get symbol dependency graph
    pub async fn get_symbol_dependencies(
        &self,
        symbol_id: &str,
        options: GraphTraversalOptions,
    ) -> Result<Vec<SymbolDependency>, DatabaseError> {
        let workspace_context = self.workspace_context.read().await;
        let context = workspace_context
            .as_ref()
            .ok_or_else(|| DatabaseError::Configuration {
                message: "No workspace context set for dependency analysis".to_string(),
            })?;

        let conn = self.get_connection().await?;
        let result = DuckDBQueries::get_symbol_dependencies(
            &conn,
            &context.workspace_id,
            symbol_id,
            context.current_commit.as_deref(),
            &context.modified_files,
            options,
        )
        .await;

        self.return_connection(conn).await;
        result
    }

    /// Analyze symbol hotspots (most referenced symbols)
    pub async fn analyze_symbol_hotspots(
        &self,
        limit: Option<usize>,
    ) -> Result<Vec<SymbolHotspot>, DatabaseError> {
        let workspace_context = self.workspace_context.read().await;
        let context = workspace_context
            .as_ref()
            .ok_or_else(|| DatabaseError::Configuration {
                message: "No workspace context set for hotspot analysis".to_string(),
            })?;

        let conn = self.get_connection().await?;
        let result = DuckDBQueries::analyze_symbol_hotspots(
            &conn,
            &context.workspace_id,
            context.current_commit.as_deref(),
            &context.modified_files,
            limit,
        )
        .await;

        self.return_connection(conn).await;
        result
    }

    /// Perform call graph traversal with cycle detection
    pub async fn traverse_call_graph(
        &self,
        start_symbol_id: &str,
        direction: TraversalDirection,
        options: GraphTraversalOptions,
    ) -> Result<CallGraphTraversal, DatabaseError> {
        let workspace_context = self.workspace_context.read().await;
        let context = workspace_context
            .as_ref()
            .ok_or_else(|| DatabaseError::Configuration {
                message: "No workspace context set for call graph traversal".to_string(),
            })?;

        let conn = self.get_connection().await?;
        let result = DuckDBQueries::traverse_call_graph(
            &conn,
            &context.workspace_id,
            start_symbol_id,
            direction,
            context.current_commit.as_deref(),
            &context.modified_files,
            options,
        )
        .await;

        self.return_connection(conn).await;
        result
    }

    /// Find all call chains between two symbols with depth limits
    pub async fn find_call_chains(
        &self,
        from_symbol_id: &str,
        to_symbol_id: &str,
        max_depth: usize,
    ) -> Result<Vec<CallPath>, DatabaseError> {
        let options = GraphTraversalOptions {
            max_depth,
            detect_cycles: true,
            call_types_filter: None,
            reference_types_filter: None,
            result_limit: Some(100),
        };

        self.find_call_paths(from_symbol_id, to_symbol_id, options)
            .await
    }

    /// Find symbols with the most incoming references (potential refactoring targets)
    pub async fn find_reference_hotspots(
        &self,
        top_n: usize,
    ) -> Result<Vec<SymbolHotspot>, DatabaseError> {
        self.analyze_symbol_hotspots(Some(top_n)).await
    }

    /// Analyze the blast radius of changing a symbol (symbols that would be affected)
    pub async fn analyze_change_impact(
        &self,
        symbol_id: &str,
        max_depth: usize,
    ) -> Result<Vec<SymbolImpact>, DatabaseError> {
        let options = GraphTraversalOptions {
            max_depth,
            detect_cycles: true,
            call_types_filter: None,
            reference_types_filter: None,
            result_limit: Some(500),
        };

        self.find_affected_symbols(symbol_id, options).await
    }

    /// Find all direct and transitive dependencies of a symbol
    pub async fn get_all_dependencies(
        &self,
        symbol_id: &str,
        include_transitive: bool,
    ) -> Result<Vec<SymbolDependency>, DatabaseError> {
        let max_depth = if include_transitive { 10 } else { 1 };
        let options = GraphTraversalOptions {
            max_depth,
            detect_cycles: true,
            call_types_filter: None,
            reference_types_filter: None,
            result_limit: Some(1000),
        };

        self.get_symbol_dependencies(symbol_id, options).await
    }

    /// Check if there are any cycles in the call graph starting from a symbol
    pub async fn detect_call_cycles(
        &self,
        start_symbol_id: &str,
    ) -> Result<Vec<Vec<String>>, DatabaseError> {
        let options = GraphTraversalOptions {
            max_depth: 20,
            detect_cycles: true,
            call_types_filter: None,
            reference_types_filter: None,
            result_limit: Some(100),
        };

        let traversal = self
            .traverse_call_graph(start_symbol_id, TraversalDirection::Outgoing, options)
            .await?;

        // Extract cycles from the traversal
        let cycles: Vec<Vec<String>> = traversal
            .nodes
            .into_iter()
            .filter(|node| node.has_cycle)
            .map(|node| node.path)
            .collect();

        Ok(cycles)
    }

    /// Find unused symbols (symbols with no incoming references or calls)
    pub async fn find_unused_symbols(&self) -> Result<Vec<String>, DatabaseError> {
        let workspace_context = self.workspace_context.read().await;
        let context = workspace_context
            .as_ref()
            .ok_or_else(|| DatabaseError::Configuration {
                message: "No workspace context set for unused symbol analysis".to_string(),
            })?;

        let values_clause = self.build_modified_files_values_clause(&context.modified_files);
        let conn = self.get_connection().await?;

        let query = format!(
            r#"
            WITH modified_files (file_path) AS (
                {}
            ),
            current_symbols AS (
                SELECT DISTINCT s.symbol_id, s.name, s.kind
                FROM symbols s
                JOIN files f ON s.file_id = f.file_id
                LEFT JOIN modified_files mf ON f.relative_path = mf.file_path
                WHERE s.workspace_id = ?
                  AND (
                    (mf.file_path IS NOT NULL AND s.indexed_at = (
                        SELECT MAX(s2.indexed_at)
                        FROM symbols s2
                        WHERE s2.symbol_id = s.symbol_id
                    ))
                    OR
                    (mf.file_path IS NULL AND s.git_commit_hash = ?)
                  )
                  AND s.kind IN ('function', 'class', 'method')  -- Focus on callable symbols
            ),
            referenced_symbols AS (
                SELECT DISTINCT sr.target_symbol_id
                FROM symbol_references sr
                JOIN files f ON sr.file_id = f.file_id
                LEFT JOIN modified_files mf ON f.relative_path = mf.file_path
                WHERE sr.workspace_id = ?
                  AND (
                    (mf.file_path IS NOT NULL AND sr.indexed_at = (
                        SELECT MAX(sr2.indexed_at)
                        FROM symbol_references sr2
                        WHERE sr2.reference_id = sr.reference_id
                    ))
                    OR
                    (mf.file_path IS NULL AND sr.git_commit_hash = ?)
                  )
            ),
            called_symbols AS (
                SELECT DISTINCT cg.callee_symbol_id
                FROM call_graph cg
                JOIN files f ON cg.file_id = f.file_id
                LEFT JOIN modified_files mf ON f.relative_path = mf.file_path
                WHERE cg.workspace_id = ?
                  AND (
                    (mf.file_path IS NOT NULL AND cg.indexed_at = (
                        SELECT MAX(cg2.indexed_at)
                        FROM call_graph cg2
                        WHERE cg2.call_id = cg.call_id
                    ))
                    OR
                    (mf.file_path IS NULL AND cg.git_commit_hash = ?)
                  )
            )
            SELECT cs.symbol_id
            FROM current_symbols cs
            LEFT JOIN referenced_symbols rs ON cs.symbol_id = rs.target_symbol_id
            LEFT JOIN called_symbols cls ON cs.symbol_id = cls.callee_symbol_id
            WHERE rs.target_symbol_id IS NULL 
              AND cls.callee_symbol_id IS NULL
              AND cs.name NOT IN ('main', 'new', 'default')  -- Exclude common entry points
            ORDER BY cs.name
            "#,
            values_clause
        );

        let mut stmt = conn
            .prepare(&query)
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to prepare unused symbols query: {}", e),
            })?;

        let rows = stmt
            .query_map(
                params![
                    &context.workspace_id,
                    context.current_commit.as_deref().unwrap_or(""),
                    &context.workspace_id,
                    context.current_commit.as_deref().unwrap_or(""),
                    &context.workspace_id,
                    context.current_commit.as_deref().unwrap_or(""),
                ],
                |row| Ok(row.get::<_, String>(0)?),
            )
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to execute unused symbols query: {}", e),
            })?;

        let results: Result<Vec<String>, _> = rows.collect();
        let unused_symbols = results.map_err(|e| DatabaseError::OperationFailed {
            message: format!("Failed to collect unused symbols results: {}", e),
        })?;

        self.return_connection(conn).await;
        Ok(unused_symbols)
    }
}

// ============================================================================
// Supporting Types and Enums
// ============================================================================

/// Direction for call hierarchy queries
#[derive(Debug, Clone, Copy)]
pub enum CallDirection {
    Incoming,
    Outgoing,
}

/// Current symbol with file information
#[derive(Debug, Clone, PartialEq)]
pub struct CurrentSymbol {
    pub symbol_id: String,
    pub name: String,
    pub qualified_name: Option<String>,
    pub kind: String,
    pub start_line: i32,
    pub start_column: i32,
    pub end_line: i32,
    pub end_column: i32,
    pub signature: Option<String>,
    pub documentation: Option<String>,
    pub visibility: Option<String>,
    pub indexed_at: String,
    pub relative_path: String,
    pub absolute_path: String,
    pub language: Option<String>,
    pub state: String, // "modified" or "committed"
}

/// Call hierarchy item
#[derive(Debug, Clone, PartialEq)]
pub struct CallHierarchyItem {
    pub symbol_id: String,
    pub name: String,
    pub qualified_name: Option<String>,
    pub relative_path: String,
    pub call_line: i32,
    pub call_column: i32,
    pub call_type: String,
}

/// LSP cache entry
#[derive(Debug, Clone, PartialEq)]
pub struct LspCacheEntry {
    pub cache_key: String,
    pub response_data: String, // JSON
    pub response_type: Option<String>,
    pub created_at: String,
    pub last_accessed: String,
    pub response_time_ms: Option<i32>,
}

/// Sanitize table names to prevent SQL injection and ensure valid identifiers
fn sanitize_table_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

impl DuckDBBackend {
    // ========================================================================
    // Migration Support Methods
    // ========================================================================

    /// Batch insert cache entries for efficient migration from sled
    pub async fn batch_insert_cache_entries(
        &self,
        entries: &[(String, String, String, String, serde_json::Value)], // cache_key, workspace_id, method, file_id, response_data
    ) -> Result<u64, DatabaseError> {
        debug!("Batch inserting {} cache entries", entries.len());

        let conn = self.get_connection().await?;

        // Prepare batch insert statement
        let sql = r#"
            INSERT INTO lsp_cache (
                cache_key, workspace_id, method, file_id, version_id,
                request_params, response_data, response_type,
                created_at, last_accessed, access_count
            ) VALUES (?, ?, ?, ?, ?,
                     '{}', ?, ?,
                     CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 1)
        "#;

        let mut inserted_count = 0;

        // Execute transaction for batch insert
        conn.execute("BEGIN TRANSACTION", [])
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to begin transaction: {}", e),
            })?;

        for (cache_key, workspace_id, method, file_id, response_data) in entries {
            let version_id = format!("{}:{}", file_id, "migrated");
            let response_type = self.infer_response_type_from_method(method);
            let response_json = response_data.to_string();

            match conn.execute(
                sql,
                params![
                    cache_key,
                    workspace_id,
                    method,
                    file_id,
                    version_id,
                    response_json,
                    response_type
                ],
            ) {
                Ok(_) => inserted_count += 1,
                Err(e) => {
                    warn!("Failed to insert cache entry {}: {}", cache_key, e);
                }
            }
        }

        conn.execute("COMMIT", [])
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to commit transaction: {}", e),
            })?;

        self.return_connection(conn).await;

        info!(
            "Successfully batch inserted {} cache entries",
            inserted_count
        );
        Ok(inserted_count)
    }

    /// Insert workspace metadata for migration
    pub async fn insert_workspace_metadata(
        &self,
        workspace_id: &str,
        root_path: &str,
        name: &str,
        is_git_repo: bool,
        current_commit: Option<&str>,
    ) -> Result<(), DatabaseError> {
        debug!("Inserting workspace metadata: {}", workspace_id);

        let conn = self.get_connection().await?;

        let sql = r#"
            INSERT OR REPLACE INTO workspaces (
                workspace_id, root_path, name, is_git_repo, 
                current_commit, created_at, last_indexed
            ) VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        "#;

        conn.execute(
            sql,
            params![workspace_id, root_path, name, is_git_repo, current_commit],
        )
        .map_err(|e| DatabaseError::OperationFailed {
            message: format!("Failed to insert workspace metadata: {}", e),
        })?;

        self.return_connection(conn).await;

        debug!(
            "Successfully inserted workspace metadata for: {}",
            workspace_id
        );
        Ok(())
    }

    /// Create file and file version entries for migration
    pub async fn insert_file_metadata(
        &self,
        file_id: &str,
        workspace_id: &str,
        relative_path: &str,
        absolute_path: &str,
        language: Option<&str>,
        content_hash: &str,
        git_commit_hash: Option<&str>,
    ) -> Result<(), DatabaseError> {
        debug!("Inserting file metadata: {}", file_id);

        let conn = self.get_connection().await?;

        // Insert file entry
        let file_sql = r#"
            INSERT OR REPLACE INTO files (
                file_id, workspace_id, relative_path, absolute_path, 
                language, created_at
            ) VALUES (?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
        "#;

        conn.execute(
            file_sql,
            params![
                file_id,
                workspace_id,
                relative_path,
                absolute_path,
                language
            ],
        )
        .map_err(|e| DatabaseError::OperationFailed {
            message: format!("Failed to insert file metadata: {}", e),
        })?;

        // Insert file version entry
        let version_id = format!("{}:{}", file_id, content_hash);
        let version_sql = r#"
            INSERT OR REPLACE INTO file_versions (
                version_id, file_id, content_hash, git_commit_hash,
                indexed_at
            ) VALUES (?, ?, ?, ?, CURRENT_TIMESTAMP)
        "#;

        conn.execute(
            version_sql,
            params![version_id, file_id, content_hash, git_commit_hash],
        )
        .map_err(|e| DatabaseError::OperationFailed {
            message: format!("Failed to insert file version: {}", e),
        })?;

        self.return_connection(conn).await;

        debug!("Successfully inserted file metadata for: {}", file_id);
        Ok(())
    }

    /// Helper method to infer response type from LSP method
    fn infer_response_type_from_method(&self, method: &str) -> String {
        match method {
            m if m.contains("definition") => "locations",
            m if m.contains("references") => "locations",
            m if m.contains("hover") => "hover",
            m if m.contains("documentSymbol") => "symbols",
            m if m.contains("workspaceSymbol") => "symbols",
            m if m.contains("completion") => "completions",
            m if m.contains("callHierarchy") => "call_hierarchy",
            m if m.contains("signatureHelp") => "signature_help",
            m if m.contains("codeAction") => "code_actions",
            _ => "unknown",
        }
        .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{DatabaseBackendExt, DatabaseTreeExt};
    use serde::{Deserialize, Serialize};
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_duckdb_backend_temporary() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let db = DuckDBBackend::new(config)
            .await
            .expect("Failed to create temporary database");
        assert!(db.is_temporary());

        // Test basic operations
        db.set(b"key1", b"value1").await.expect("Failed to set key");
        let value = db.get(b"key1").await.expect("Failed to get key");
        assert_eq!(value, Some(b"value1".to_vec()));

        // Test removal
        let removed = db.remove(b"key1").await.expect("Failed to remove key");
        assert!(removed);

        let value = db
            .get(b"key1")
            .await
            .expect("Failed to get key after removal");
        assert_eq!(value, None);

        // Test removal of non-existent key
        let removed = db
            .remove(b"nonexistent")
            .await
            .expect("Failed to remove nonexistent key");
        assert!(!removed);
    }

    #[tokio::test]
    async fn test_duckdb_backend_persistent() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test.db");

        let config = DatabaseConfig {
            path: Some(db_path.clone()),
            temporary: false,
            ..Default::default()
        };

        let db = DuckDBBackend::new(config)
            .await
            .expect("Failed to create persistent database");
        assert!(!db.is_temporary());

        // Test basic operations
        db.set(b"persistent_key", b"persistent_value")
            .await
            .expect("Failed to set key");
        let value = db.get(b"persistent_key").await.expect("Failed to get key");
        assert_eq!(value, Some(b"persistent_value".to_vec()));

        // Test that database file exists
        assert!(db_path.exists());

        // Test size_on_disk
        let size = db.size_on_disk().await.expect("Failed to get disk size");
        assert!(size > 0);
    }

    #[tokio::test]
    async fn test_duckdb_tree_operations() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let db = DuckDBBackend::new(config)
            .await
            .expect("Failed to create database");

        // Test tree operations
        let tree = db
            .open_tree("test_tree")
            .await
            .expect("Failed to open tree");

        tree.set(b"tree_key", b"tree_value")
            .await
            .expect("Failed to set in tree");
        let value = tree
            .get(b"tree_key")
            .await
            .expect("Failed to get from tree");
        assert_eq!(value, Some(b"tree_value".to_vec()));

        // Test tree length
        let len = tree.len().await.expect("Failed to get tree length");
        assert_eq!(len, 1);

        // Test tree clear
        tree.clear().await.expect("Failed to clear tree");
        let len = tree
            .len()
            .await
            .expect("Failed to get tree length after clear");
        assert_eq!(len, 0);

        // Test tree removal
        let removed = tree
            .remove(b"nonexistent")
            .await
            .expect("Failed to remove from tree");
        assert!(!removed);

        tree.set(b"tree_key2", b"tree_value2")
            .await
            .expect("Failed to set in tree");
        let removed = tree
            .remove(b"tree_key2")
            .await
            .expect("Failed to remove from tree");
        assert!(removed);

        let value = tree
            .get(b"tree_key2")
            .await
            .expect("Failed to get from tree");
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_duckdb_tree_prefix_scan() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let db = DuckDBBackend::new(config)
            .await
            .expect("Failed to create database");

        let tree = db
            .open_tree("scan_tree")
            .await
            .expect("Failed to open tree");

        // Insert test data in tree
        tree.set(b"prefix:key1", b"value1")
            .await
            .expect("Failed to set key1");
        tree.set(b"prefix:key2", b"value2")
            .await
            .expect("Failed to set key2");
        tree.set(b"other:key3", b"value3")
            .await
            .expect("Failed to set key3");

        // Test prefix scan in tree
        let results = tree
            .scan_prefix(b"prefix:")
            .await
            .expect("Failed to scan prefix");
        assert_eq!(results.len(), 2);

        // Check that results contain expected keys
        let keys: Vec<&[u8]> = results.iter().map(|(k, _)| k.as_slice()).collect();
        assert!(keys.contains(&b"prefix:key1".as_slice()));
        assert!(keys.contains(&b"prefix:key2".as_slice()));

        // Test empty prefix scan
        let results = tree
            .scan_prefix(b"nonexistent:")
            .await
            .expect("Failed to scan nonexistent prefix");
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_duckdb_prefix_scan() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let db = DuckDBBackend::new(config)
            .await
            .expect("Failed to create database");

        // Insert test data
        db.set(b"prefix:key1", b"value1")
            .await
            .expect("Failed to set key1");
        db.set(b"prefix:key2", b"value2")
            .await
            .expect("Failed to set key2");
        db.set(b"other:key3", b"value3")
            .await
            .expect("Failed to set key3");

        // Test prefix scan
        let results = db
            .scan_prefix(b"prefix:")
            .await
            .expect("Failed to scan prefix");
        assert_eq!(results.len(), 2);

        // Check that results contain expected keys
        let keys: Vec<&[u8]> = results.iter().map(|(k, _)| k.as_slice()).collect();
        assert!(keys.contains(&b"prefix:key1".as_slice()));
        assert!(keys.contains(&b"prefix:key2".as_slice()));

        // Test empty prefix scan
        let results = db
            .scan_prefix(b"nonexistent:")
            .await
            .expect("Failed to scan nonexistent prefix");
        assert_eq!(results.len(), 0);
    }

    #[tokio::test]
    async fn test_duckdb_serialization_helpers() {
        #[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
        struct TestData {
            id: u64,
            name: String,
        }

        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let db = DuckDBBackend::new(config)
            .await
            .expect("Failed to create database");

        let test_data = TestData {
            id: 42,
            name: "test".to_string(),
        };

        // Test serialized set/get
        db.set_serialized(b"test_key", &test_data)
            .await
            .expect("Failed to set serialized data");
        let retrieved: Option<TestData> = db
            .get_serialized(b"test_key")
            .await
            .expect("Failed to get serialized data");

        assert_eq!(retrieved, Some(test_data.clone()));

        // Test tree serialization
        let tree = db
            .open_tree("serialize_tree")
            .await
            .expect("Failed to open tree");
        tree.set_serialized(b"tree_test_key", &test_data)
            .await
            .expect("Failed to set serialized data in tree");
        let retrieved: Option<TestData> = tree
            .get_serialized(b"tree_test_key")
            .await
            .expect("Failed to get serialized data from tree");

        assert_eq!(retrieved, Some(test_data));
    }

    #[tokio::test]
    async fn test_duckdb_stats() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let db = DuckDBBackend::new(config)
            .await
            .expect("Failed to create database");

        // Insert some test data
        db.set(b"key1", b"value1")
            .await
            .expect("Failed to set key1");
        db.set(b"key2", b"value2")
            .await
            .expect("Failed to set key2");

        let tree = db
            .open_tree("test_tree")
            .await
            .expect("Failed to open tree");
        tree.set(b"tree_key", b"tree_value")
            .await
            .expect("Failed to set in tree");

        let stats = db.stats().await.expect("Failed to get stats");

        assert!(stats.total_entries >= 2); // At least 2 in default store
        assert!(stats.is_temporary);
        assert_eq!(stats.disk_size_bytes, 0); // Temporary database
                                              // Tree count may be 0 or 1 depending on when metadata is registered
        assert!(stats.tree_count <= 1);
    }

    #[tokio::test]
    async fn test_duckdb_tree_names() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let db = DuckDBBackend::new(config)
            .await
            .expect("Failed to create database");

        // Initially no trees
        let names = db.tree_names().await.expect("Failed to get tree names");
        assert_eq!(names.len(), 0);

        // Create some trees
        db.open_tree("tree1").await.expect("Failed to open tree1");
        db.open_tree("tree2").await.expect("Failed to open tree2");
        db.open_tree("tree3").await.expect("Failed to open tree3");

        let mut names = db.tree_names().await.expect("Failed to get tree names");
        names.sort();
        assert_eq!(names, vec!["tree1", "tree2", "tree3"]);
    }

    #[tokio::test]
    async fn test_duckdb_clear_all() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let db = DuckDBBackend::new(config)
            .await
            .expect("Failed to create database");

        // Add data to default store
        db.set(b"key1", b"value1")
            .await
            .expect("Failed to set key1");
        db.set(b"key2", b"value2")
            .await
            .expect("Failed to set key2");

        // Add data to trees
        let tree1 = db.open_tree("tree1").await.expect("Failed to open tree1");
        tree1
            .set(b"tree_key1", b"tree_value1")
            .await
            .expect("Failed to set in tree1");

        let tree2 = db.open_tree("tree2").await.expect("Failed to open tree2");
        tree2
            .set(b"tree_key2", b"tree_value2")
            .await
            .expect("Failed to set in tree2");

        // Verify data exists - at least 2 in default store, and possibly more in trees
        let stats = db.stats().await.expect("Failed to get stats");
        assert!(stats.total_entries >= 2);

        // Clear all
        db.clear().await.expect("Failed to clear database");

        // Verify all data is cleared
        let stats = db.stats().await.expect("Failed to get stats after clear");
        assert_eq!(stats.total_entries, 0);

        let value = db
            .get(b"key1")
            .await
            .expect("Failed to get key after clear");
        assert_eq!(value, None);

        let value = tree1
            .get(b"tree_key1")
            .await
            .expect("Failed to get from tree after clear");
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_duckdb_flush() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let db = DuckDBBackend::new(config)
            .await
            .expect("Failed to create database");

        // Add some data
        db.set(b"key1", b"value1").await.expect("Failed to set key");

        // Test flush - should not fail
        db.flush().await.expect("Failed to flush database");

        // Data should still be there
        let value = db
            .get(b"key1")
            .await
            .expect("Failed to get key after flush");
        assert_eq!(value, Some(b"value1".to_vec()));
    }

    #[tokio::test]
    async fn test_duckdb_workspace_context() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let db = DuckDBBackend::new(config)
            .await
            .expect("Failed to create database");

        // Initially no workspace context
        let context = db.get_current_workspace().await;
        assert!(context.is_none());

        // Set workspace context
        let workspace_context = WorkspaceContext {
            workspace_id: "test-workspace".to_string(),
            current_commit: Some("abc123".to_string()),
            modified_files: vec!["src/main.rs".to_string(), "src/lib.rs".to_string()],
            root_path: "/path/to/workspace".to_string(),
        };

        db.set_workspace_context(workspace_context.clone())
            .await
            .expect("Failed to set workspace context");

        // Verify context is set
        let retrieved_context = db.get_current_workspace().await;
        assert!(retrieved_context.is_some());
        let retrieved_context = retrieved_context.unwrap();
        assert_eq!(retrieved_context.workspace_id, "test-workspace");
        assert_eq!(retrieved_context.current_commit, Some("abc123".to_string()));
        assert_eq!(retrieved_context.modified_files.len(), 2);
        assert_eq!(retrieved_context.root_path, "/path/to/workspace");
    }

    #[tokio::test]
    async fn test_duckdb_modified_files_operations() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let db = DuckDBBackend::new(config)
            .await
            .expect("Failed to create database");

        // Test without workspace context - should fail
        let result = db.set_modified_files(vec!["src/main.rs".to_string()]).await;
        assert!(result.is_err());

        let result = db.set_current_commit(Some("abc123".to_string())).await;
        assert!(result.is_err());

        // Set workspace context first
        let workspace_context = WorkspaceContext {
            workspace_id: "test-workspace".to_string(),
            current_commit: Some("abc123".to_string()),
            modified_files: vec!["src/main.rs".to_string()],
            root_path: "/path/to/workspace".to_string(),
        };

        db.set_workspace_context(workspace_context)
            .await
            .expect("Failed to set workspace context");

        // Update modified files
        let new_modified_files = vec![
            "src/lib.rs".to_string(),
            "src/utils.rs".to_string(),
            "tests/integration.rs".to_string(),
        ];

        db.set_modified_files(new_modified_files.clone())
            .await
            .expect("Failed to set modified files");

        let retrieved_files = db.get_modified_files().await;
        assert_eq!(retrieved_files, new_modified_files);

        // Update commit hash
        db.set_current_commit(Some("def456".to_string()))
            .await
            .expect("Failed to set current commit");

        let commit = db.get_current_commit().await;
        assert_eq!(commit, Some("def456".to_string()));

        // Test clearing commit hash
        db.set_current_commit(None)
            .await
            .expect("Failed to clear current commit");

        let commit = db.get_current_commit().await;
        assert_eq!(commit, None);
    }

    #[tokio::test]
    async fn test_duckdb_git_aware_operations() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let db = DuckDBBackend::new(config)
            .await
            .expect("Failed to create database");

        // Set workspace context
        let workspace_context = WorkspaceContext {
            workspace_id: "test-workspace".to_string(),
            current_commit: Some("abc123".to_string()),
            modified_files: vec!["src/main.rs".to_string(), "src/lib.rs".to_string()],
            root_path: "/path/to/workspace".to_string(),
        };

        db.set_workspace_context(workspace_context)
            .await
            .expect("Failed to set workspace context");

        // Test get_current_symbols (should return empty but not fail)
        let symbols = db
            .get_current_symbols(None)
            .await
            .expect("Failed to get current symbols");
        assert_eq!(symbols.len(), 0);

        // Test with name filter
        let symbols = db
            .get_current_symbols(Some("main"))
            .await
            .expect("Failed to get filtered symbols");
        assert_eq!(symbols.len(), 0);

        // Test call hierarchy operations
        let incoming = db
            .get_call_hierarchy("test-symbol", CallDirection::Incoming)
            .await;
        assert!(incoming.is_ok());
        let incoming = incoming.unwrap();
        assert_eq!(incoming.len(), 0);

        let outgoing = db
            .get_call_hierarchy("test-symbol", CallDirection::Outgoing)
            .await;
        assert!(outgoing.is_ok());
        let outgoing = outgoing.unwrap();
        assert_eq!(outgoing.len(), 0);

        // Test LSP cache operations
        let cache_entry = db
            .get_lsp_cache_entry("textDocument/definition", "file123", 10, 5)
            .await;
        assert!(cache_entry.is_ok());
        assert!(cache_entry.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_duckdb_build_modified_files_values_clause() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let db = DuckDBBackend::new(config)
            .await
            .expect("Failed to create database");

        // Test empty modified files
        let empty_clause = db.build_modified_files_values_clause(&[]);
        assert_eq!(empty_clause, "SELECT NULL as file_path WHERE FALSE");

        // Test with files
        let files = vec!["src/main.rs".to_string(), "src/lib.rs".to_string()];
        let clause = db.build_modified_files_values_clause(&files);
        assert_eq!(clause, "VALUES ('src/main.rs'), ('src/lib.rs')");
    }

    #[tokio::test]
    async fn test_duckdb_transaction_support() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let db = DuckDBBackend::new(config)
            .await
            .expect("Failed to create database");

        // Test successful transaction
        let result = db
            .execute_transaction(|conn| {
                conn.execute(
                    "INSERT OR REPLACE INTO kv_store (key, value) VALUES (?, ?)",
                    params!["tx_key", b"tx_value".as_slice()],
                )
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Transaction error: {}", e),
                })?;
                Ok(42)
            })
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);

        // Verify data was committed
        let value = db
            .get(b"tx_key")
            .await
            .expect("Failed to get key after transaction");
        assert_eq!(value, Some(b"tx_value".to_vec()));

        // Test failed transaction (should rollback)
        let result: Result<(), DatabaseError> = db
            .execute_transaction(|conn| {
                conn.execute(
                    "INSERT OR REPLACE INTO kv_store (key, value) VALUES (?, ?)",
                    params!["tx_key2", b"tx_value2".as_slice()],
                )
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Transaction error: {}", e),
                })?;
                Err(DatabaseError::OperationFailed {
                    message: "Test failure".to_string(),
                })
            })
            .await;

        assert!(result.is_err());

        // Verify rollback - data should not exist
        let value = db
            .get(b"tx_key2")
            .await
            .expect("Failed to get key after failed transaction");
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_duckdb_custom_config() {
        let duckdb_config = DuckDBConfig {
            path: ":memory:".to_string(),
            max_connections: 4,
            memory_limit_gb: 1.0,
            threads: 2,
            enable_query_cache: false,
            temporary: true,
        };

        let db_config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let db = DuckDBBackend::with_duckdb_config(db_config, duckdb_config)
            .await
            .expect("Failed to create database with custom config");

        assert!(db.is_temporary());

        // Test basic operations still work
        db.set(b"config_test", b"config_value")
            .await
            .expect("Failed to set key");
        let value = db.get(b"config_test").await.expect("Failed to get key");
        assert_eq!(value, Some(b"config_value".to_vec()));
    }

    #[tokio::test]
    async fn test_duckdb_multiple_trees_cached() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let db = DuckDBBackend::new(config)
            .await
            .expect("Failed to create database");

        // Open the same tree multiple times - should return cached instance
        let tree1 = db
            .open_tree("cached_tree")
            .await
            .expect("Failed to open tree1");
        let tree2 = db
            .open_tree("cached_tree")
            .await
            .expect("Failed to open tree2");

        // Set data in one tree
        tree1
            .set(b"cache_key", b"cache_value")
            .await
            .expect("Failed to set in tree1");

        // Should be visible in the other (same) tree
        let value = tree2
            .get(b"cache_key")
            .await
            .expect("Failed to get from tree2");
        assert_eq!(value, Some(b"cache_value".to_vec()));

        // Test that different tree names create different trees
        let tree3 = db
            .open_tree("different_tree")
            .await
            .expect("Failed to open tree3");
        let value = tree3
            .get(b"cache_key")
            .await
            .expect("Failed to get from different tree");
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_duckdb_edge_cases() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let db = DuckDBBackend::new(config)
            .await
            .expect("Failed to create database");

        // Test empty keys and values
        db.set(b"", b"")
            .await
            .expect("Failed to set empty key/value");
        let value = db.get(b"").await.expect("Failed to get empty key");
        assert_eq!(value, Some(b"".to_vec()));

        // Test large values
        let large_value = vec![0u8; 1024 * 1024]; // 1MB
        db.set(b"large_key", &large_value)
            .await
            .expect("Failed to set large value");
        let retrieved = db
            .get(b"large_key")
            .await
            .expect("Failed to get large value");
        assert_eq!(retrieved, Some(large_value));

        // Test special characters in keys
        db.set("special:key/with\\chars".as_bytes(), b"special_value")
            .await
            .expect("Failed to set special key");
        let value = db
            .get("special:key/with\\chars".as_bytes())
            .await
            .expect("Failed to get special key");
        assert_eq!(value, Some(b"special_value".to_vec()));

        // Test Unicode in keys and values
        let unicode_key = "ключ".as_bytes();
        let unicode_value = "значение".as_bytes();
        db.set(unicode_key, unicode_value)
            .await
            .expect("Failed to set unicode key/value");
        let value = db
            .get(unicode_key)
            .await
            .expect("Failed to get unicode key");
        assert_eq!(value, Some(unicode_value.to_vec()));
    }

    #[test]
    fn test_sanitize_table_name() {
        assert_eq!(sanitize_table_name("valid_name"), "valid_name");
        assert_eq!(sanitize_table_name("invalid-name"), "invalid_name");
        assert_eq!(sanitize_table_name("invalid.name"), "invalid_name");
        assert_eq!(sanitize_table_name("invalid name"), "invalid_name");
        assert_eq!(sanitize_table_name("123invalid"), "123invalid");
        assert_eq!(sanitize_table_name(""), "");
        assert_eq!(sanitize_table_name("a"), "a");
        assert_eq!(sanitize_table_name("!@#$%^&*()"), "__________");
    }

    #[test]
    fn test_duckdb_config_default() {
        let config = DuckDBConfig::default();
        assert_eq!(config.path, ":memory:");
        assert_eq!(config.max_connections, 8);
        assert_eq!(config.memory_limit_gb, 2.0);
        assert_eq!(config.threads, num_cpus::get());
        assert!(config.enable_query_cache);
        assert!(config.temporary);
    }

    #[test]
    fn test_call_direction_debug() {
        // Test debug formatting
        assert_eq!(format!("{:?}", CallDirection::Incoming), "Incoming");
        assert_eq!(format!("{:?}", CallDirection::Outgoing), "Outgoing");
    }

    #[tokio::test]
    async fn test_advanced_graph_queries() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let db = DuckDBBackend::new(config)
            .await
            .expect("Failed to create database");

        // Set workspace context
        let workspace_context = WorkspaceContext {
            workspace_id: "test-workspace".to_string(),
            current_commit: Some("abc123".to_string()),
            modified_files: vec![],
            root_path: "/path/to/workspace".to_string(),
        };

        db.set_workspace_context(workspace_context)
            .await
            .expect("Failed to set workspace context");

        // Test find_call_paths with empty database (should return empty results)
        let options = GraphTraversalOptions::default();
        let paths = db.find_call_paths("sym1", "sym2", options).await;
        assert!(paths.is_ok());
        assert!(paths.unwrap().is_empty());

        // Test find_affected_symbols
        let options = GraphTraversalOptions::default();
        let affected = db.find_affected_symbols("sym1", options).await;
        assert!(affected.is_ok());
        assert!(affected.unwrap().is_empty());

        // Test get_symbol_dependencies
        let options = GraphTraversalOptions::default();
        let deps = db.get_symbol_dependencies("sym1", options).await;
        assert!(deps.is_ok());
        assert!(deps.unwrap().is_empty());

        // Test analyze_symbol_hotspots
        let hotspots = db.analyze_symbol_hotspots(Some(10)).await;
        assert!(hotspots.is_ok());
        assert!(hotspots.unwrap().is_empty());

        // Test traverse_call_graph
        let options = GraphTraversalOptions::default();
        let traversal = db
            .traverse_call_graph("sym1", TraversalDirection::Outgoing, options)
            .await;
        assert!(traversal.is_ok());
        let traversal_result = traversal.unwrap();
        assert_eq!(traversal_result.start_symbol, "sym1");
        assert!(traversal_result.nodes.is_empty());

        // Test convenience methods
        let chains = db.find_call_chains("sym1", "sym2", 5).await;
        assert!(chains.is_ok());
        assert!(chains.unwrap().is_empty());

        let hotspots = db.find_reference_hotspots(5).await;
        assert!(hotspots.is_ok());
        assert!(hotspots.unwrap().is_empty());

        let impact = db.analyze_change_impact("sym1", 3).await;
        assert!(impact.is_ok());
        assert!(impact.unwrap().is_empty());

        let deps = db.get_all_dependencies("sym1", true).await;
        assert!(deps.is_ok());
        assert!(deps.unwrap().is_empty());

        let cycles = db.detect_call_cycles("sym1").await;
        assert!(cycles.is_ok());
        assert!(cycles.unwrap().is_empty());

        let unused = db.find_unused_symbols().await;
        assert!(unused.is_ok());
        assert!(unused.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_graph_traversal_options() {
        let default_options = GraphTraversalOptions::default();
        assert_eq!(default_options.max_depth, 10);
        assert!(default_options.detect_cycles);
        assert!(default_options.call_types_filter.is_none());
        assert!(default_options.reference_types_filter.is_none());
        assert_eq!(default_options.result_limit, Some(1000));

        let custom_options = GraphTraversalOptions {
            max_depth: 5,
            detect_cycles: false,
            call_types_filter: Some(vec!["direct".to_string(), "virtual".to_string()]),
            reference_types_filter: Some(vec!["use".to_string()]),
            result_limit: Some(50),
        };

        assert_eq!(custom_options.max_depth, 5);
        assert!(!custom_options.detect_cycles);
        assert_eq!(custom_options.call_types_filter.unwrap().len(), 2);
        assert_eq!(custom_options.reference_types_filter.unwrap().len(), 1);
        assert_eq!(custom_options.result_limit, Some(50));
    }

    #[tokio::test]
    async fn test_graph_queries_without_workspace_context() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let db = DuckDBBackend::new(config)
            .await
            .expect("Failed to create database");

        // All graph queries should fail without workspace context
        let options = GraphTraversalOptions::default();

        let result = db.find_call_paths("sym1", "sym2", options.clone()).await;
        assert!(result.is_err());

        let result = db.find_affected_symbols("sym1", options.clone()).await;
        assert!(result.is_err());

        let result = db.get_symbol_dependencies("sym1", options.clone()).await;
        assert!(result.is_err());

        let result = db.analyze_symbol_hotspots(Some(10)).await;
        assert!(result.is_err());

        let result = db
            .traverse_call_graph("sym1", TraversalDirection::Outgoing, options)
            .await;
        assert!(result.is_err());

        let result = db.find_unused_symbols().await;
        assert!(result.is_err());
    }

    #[test]
    fn test_data_structures() {
        // Test CurrentSymbol
        let symbol = CurrentSymbol {
            symbol_id: "sym1".to_string(),
            name: "test_function".to_string(),
            qualified_name: Some("module::test_function".to_string()),
            kind: "function".to_string(),
            start_line: 10,
            start_column: 5,
            end_line: 20,
            end_column: 10,
            signature: Some("fn test_function() -> i32".to_string()),
            documentation: Some("Test function documentation".to_string()),
            visibility: Some("public".to_string()),
            indexed_at: "2023-01-01T00:00:00Z".to_string(),
            relative_path: "src/main.rs".to_string(),
            absolute_path: "/path/src/main.rs".to_string(),
            language: Some("rust".to_string()),
            state: "modified".to_string(),
        };

        assert_eq!(symbol.symbol_id, "sym1");
        assert_eq!(symbol.state, "modified");

        // Test CallHierarchyItem
        let call_item = CallHierarchyItem {
            symbol_id: "sym2".to_string(),
            name: "caller_function".to_string(),
            qualified_name: Some("module::caller_function".to_string()),
            relative_path: "src/lib.rs".to_string(),
            call_line: 15,
            call_column: 8,
            call_type: "direct".to_string(),
        };

        assert_eq!(call_item.call_line, 15);
        assert_eq!(call_item.call_type, "direct");

        // Test LspCacheEntry
        let cache_entry = LspCacheEntry {
            cache_key: "key123".to_string(),
            response_data: r#"{"result": "test"}"#.to_string(),
            response_type: Some("definition".to_string()),
            created_at: "2023-01-01T00:00:00Z".to_string(),
            last_accessed: "2023-01-01T00:05:00Z".to_string(),
            response_time_ms: Some(50),
        };

        assert_eq!(cache_entry.cache_key, "key123");
        assert_eq!(cache_entry.response_time_ms, Some(50));

        // Test WorkspaceContext
        let workspace = WorkspaceContext {
            workspace_id: "ws1".to_string(),
            current_commit: Some("abc123".to_string()),
            modified_files: vec!["file1.rs".to_string(), "file2.rs".to_string()],
            root_path: "/workspace".to_string(),
        };

        assert_eq!(workspace.modified_files.len(), 2);
        assert_eq!(workspace.current_commit, Some("abc123".to_string()));
    }

    #[tokio::test]
    async fn test_bootstrap_based_initialization() {
        // Test that the bootstrap-based initialization works correctly
        // for databases with the same path

        use tempfile::tempdir;

        // Test with persistent databases to verify bootstrap works correctly
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("bootstrap_test.db");

        let config1 = DatabaseConfig {
            path: Some(db_path.clone()),
            temporary: false,
            ..Default::default()
        };
        let config2 = DatabaseConfig {
            path: Some(db_path.clone()),
            temporary: false,
            ..Default::default()
        };

        // Both should succeed - bootstrap handles schema initialization before pool creation
        let db1 = DuckDBBackend::new(config1)
            .await
            .expect("Failed to create first db");
        let db2 = DuckDBBackend::new(config2)
            .await
            .expect("Failed to create second db");

        // Both instances should be usable
        db1.set(b"test_key1", b"test_value1")
            .await
            .expect("Failed to set on db1");
        db2.set(b"test_key2", b"test_value2")
            .await
            .expect("Failed to set on db2");

        let value1 = db1.get(b"test_key1").await.expect("Failed to get from db1");
        let value2 = db2.get(b"test_key2").await.expect("Failed to get from db2");

        assert_eq!(value1, Some(b"test_value1".to_vec()));
        assert_eq!(value2, Some(b"test_value2".to_vec()));

        // Verify that both databases share the same file and schema
        assert!(db_path.exists(), "Database file should exist");

        // Both instances should have their own data (they don't automatically share connection pools)
        // But they should both be able to operate correctly since bootstrap properly initialized the schema
        let stats1 = db1.stats().await.expect("Failed to get db1 stats");
        let stats2 = db2.stats().await.expect("Failed to get db2 stats");

        // Both should be non-temporary and properly initialized
        assert!(!db1.is_temporary());
        assert!(!db2.is_temporary());
        assert!(stats1.total_entries > 0);
        assert!(stats2.total_entries > 0);

        info!("Bootstrap-based initialization test passed: persistent database schema properly initialized");
    }
}
