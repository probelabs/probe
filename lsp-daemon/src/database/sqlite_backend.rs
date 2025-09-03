//! SQLite backend implementation using Turso
//!
//! This module provides a SQLite-based implementation of the DatabaseBackend trait
//! using Turso for fast, local database operations. It's designed to be a drop-in
//! replacement for DuckDB with much faster compilation times.

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::info;
use turso::{Builder, Connection, Database};

use crate::database::{
    DatabaseBackend, DatabaseConfig, DatabaseError, DatabaseStats, DatabaseTree,
};

/// SQLite-specific configuration
#[derive(Debug, Clone)]
pub struct SQLiteConfig {
    /// Database file path (or ":memory:" for in-memory)
    pub path: String,
    /// Whether this is a temporary/in-memory database
    pub temporary: bool,
    /// Enable WAL mode for better concurrency
    pub enable_wal: bool,
    /// SQLite page size in bytes
    pub page_size: u32,
    /// SQLite cache size in pages
    pub cache_size: i32,
}

impl Default for SQLiteConfig {
    fn default() -> Self {
        Self {
            path: ":memory:".to_string(),
            temporary: true,
            enable_wal: false, // Disabled for in-memory databases
            page_size: 4096,   // 4KB pages
            cache_size: 2000,  // ~8MB cache
        }
    }
}

/// Connection pool for managing SQLite connections
struct ConnectionPool {
    /// The libSQL database instance
    database: Database,
    /// Available connections
    available: Vec<Connection>,
    /// Maximum pool size
    max_size: usize,
    /// Configuration
    config: SQLiteConfig,
}

impl ConnectionPool {
    /// Create a new connection pool
    async fn new(config: SQLiteConfig) -> Result<Self, DatabaseError> {
        let database = if config.path == ":memory:" {
            Builder::new_local(":memory:")
        } else {
            Builder::new_local(&config.path)
        }
        .build()
        .await
        .map_err(|e| DatabaseError::Configuration {
            message: format!("Failed to create database: {e}"),
        })?;

        // Initialize the database with our schema
        let conn = database
            .connect()
            .map_err(|e| DatabaseError::Configuration {
                message: format!("Failed to get initial connection: {e}"),
            })?;

        Self::initialize_schema(&conn, &config).await?;

        // Pre-populate with some connections
        let initial_size = if config.temporary { 1 } else { 2 };
        let mut available = Vec::with_capacity(initial_size);
        for _ in 0..initial_size {
            if let Ok(conn) = database.connect() {
                Self::configure_connection(&conn, &config).await?;
                available.push(conn);
            }
        }

        Ok(Self {
            database,
            available,
            max_size: 8,
            config,
        })
    }

    /// Initialize database schema
    async fn initialize_schema(
        conn: &Connection,
        config: &SQLiteConfig,
    ) -> Result<(), DatabaseError> {
        // Configure SQLite settings
        // Note: WAL mode is not fully supported in Turso, so we skip it
        if config.enable_wal && config.path != ":memory:" {
            // Try to enable WAL mode, but don't fail if it's not supported
            if conn.execute("PRAGMA journal_mode = WAL", ()).await.is_err() {
                eprintln!("Warning: WAL mode not supported, continuing with default journal mode");
            }
        }

        // Note: page_size and cache_size pragmas are not supported in Turso
        // The database handles these settings automatically

        // Create main key-value store table
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS kv_store (
                key TEXT PRIMARY KEY,
                value BLOB NOT NULL,
                created_at INTEGER DEFAULT (strftime('%s','now')),
                updated_at INTEGER DEFAULT (strftime('%s','now'))
            )
            "#,
            (),
        )
        .await
        .map_err(|e| DatabaseError::Configuration {
            message: format!("Failed to create kv_store table: {e}"),
        })?;

        // Create metadata table for tracking trees
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS tree_metadata (
                tree_name TEXT PRIMARY KEY,
                created_at INTEGER DEFAULT (strftime('%s','now'))
            )
            "#,
            (),
        )
        .await
        .map_err(|e| DatabaseError::Configuration {
            message: format!("Failed to create tree_metadata table: {e}"),
        })?;

        Ok(())
    }

    /// Configure a connection with optimal settings
    async fn configure_connection(
        _conn: &Connection,
        _config: &SQLiteConfig,
    ) -> Result<(), DatabaseError> {
        // Most SQLite pragmas are not supported in Turso
        // The database handles optimization automatically
        Ok(())
    }

    /// Get a connection from the pool
    async fn get_connection(&mut self) -> Result<Connection, DatabaseError> {
        if let Some(conn) = self.available.pop() {
            Ok(conn)
        } else {
            // Create a new connection if we haven't hit the max
            let conn = self
                .database
                .connect()
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to create new connection: {e}"),
                })?;
            Self::configure_connection(&conn, &self.config).await?;
            Ok(conn)
        }
    }

    /// Return a connection to the pool
    fn return_connection(&mut self, conn: Connection) {
        if self.available.len() < self.max_size {
            self.available.push(conn);
        }
        // If pool is full, just drop the connection
    }
}

/// SQLite-based implementation of DatabaseTree
pub struct SQLiteTree {
    /// Tree name (used as table suffix)
    name: String,
    /// Connection pool reference
    pool: Arc<Mutex<ConnectionPool>>,
}

#[async_trait]
impl DatabaseTree for SQLiteTree {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, DatabaseError> {
        let key_str = String::from_utf8_lossy(key);
        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        let table_name = format!("tree_{}", sanitize_table_name(&self.name));
        let sql = format!("SELECT value FROM {table_name} WHERE key = ?");

        let mut rows = conn
            .query(&sql, [turso::Value::Text(key_str.to_string())])
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to get key from tree '{}': {}", self.name, e),
            })?;

        let value = if let Some(row) =
            rows.next()
                .await
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to iterate rows in tree '{}': {}", self.name, e),
                })? {
            match row.get_value(0) {
                Ok(turso::Value::Blob(blob)) => Some(blob),
                _ => None,
            }
        } else {
            None
        };

        pool.return_connection(conn);
        Ok(value)
    }

    async fn set(&self, key: &[u8], value: &[u8]) -> Result<(), DatabaseError> {
        let key_str = String::from_utf8_lossy(key);
        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        let table_name = format!("tree_{}", sanitize_table_name(&self.name));
        // Use UPDATE/INSERT pattern since Turso doesn't support OR REPLACE
        let update_sql = format!(
            "UPDATE {table_name} SET value = ?, updated_at = strftime('%s','now') WHERE key = ?"
        );
        let insert_sql = format!(
            "INSERT INTO {table_name} (key, value, created_at, updated_at) VALUES (?, ?, strftime('%s','now'), strftime('%s','now'))"
        );

        // Try update first
        let rows_updated = conn
            .execute(
                &update_sql,
                [
                    turso::Value::Blob(value.to_vec()),
                    turso::Value::Text(key_str.to_string()),
                ],
            )
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to update key in tree '{}': {}", self.name, e),
            })?;

        // If no rows were updated, insert new record
        if rows_updated == 0 {
            conn.execute(
                &insert_sql,
                [
                    turso::Value::Text(key_str.to_string()),
                    turso::Value::Blob(value.to_vec()),
                ],
            )
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to insert key in tree '{}': {}", self.name, e),
            })?;
        }

        pool.return_connection(conn);
        Ok(())
    }

    async fn remove(&self, key: &[u8]) -> Result<bool, DatabaseError> {
        let key_str = String::from_utf8_lossy(key);
        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        let table_name = format!("tree_{}", sanitize_table_name(&self.name));
        let sql = format!("DELETE FROM {table_name} WHERE key = ?");

        let rows_affected = conn
            .execute(&sql, [turso::Value::Text(key_str.to_string())])
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to remove key from tree '{}': {}", self.name, e),
            })?;

        pool.return_connection(conn);
        Ok(rows_affected > 0)
    }

    async fn scan_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, DatabaseError> {
        let prefix_str = String::from_utf8_lossy(prefix);
        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        let table_name = format!("tree_{}", sanitize_table_name(&self.name));
        let sql = if prefix.is_empty() {
            format!("SELECT key, value FROM {table_name} ORDER BY key")
        } else {
            format!("SELECT key, value FROM {table_name} WHERE key GLOB ? || '*' ORDER BY key")
        };

        let params = if prefix.is_empty() {
            Vec::new()
        } else {
            vec![turso::Value::Text(prefix_str.to_string())]
        };

        let mut rows =
            conn.query(&sql, params)
                .await
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to scan prefix in tree '{}': {}", self.name, e),
                })?;

        let mut results = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to iterate rows in tree '{}': {}", self.name, e),
            })?
        {
            if let (Ok(turso::Value::Text(key)), Ok(turso::Value::Blob(value))) =
                (row.get_value(0), row.get_value(1))
            {
                results.push((key.as_bytes().to_vec(), value));
            }
            // Skip malformed rows
        }

        pool.return_connection(conn);
        Ok(results)
    }

    async fn clear(&self) -> Result<(), DatabaseError> {
        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        let table_name = format!("tree_{}", sanitize_table_name(&self.name));
        let sql = format!("DELETE FROM {table_name}");

        conn.execute(&sql, ())
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to clear tree '{}': {}", self.name, e),
            })?;

        pool.return_connection(conn);
        Ok(())
    }

    async fn len(&self) -> Result<u64, DatabaseError> {
        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        let table_name = format!("tree_{}", sanitize_table_name(&self.name));
        let sql = format!("SELECT COUNT(*) FROM {table_name}");

        let mut rows = conn
            .query(&sql, ())
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to get length of tree '{}': {}", self.name, e),
            })?;

        let count = if let Some(row) =
            rows.next()
                .await
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to iterate rows in tree '{}': {}", self.name, e),
                })? {
            match row.get_value(0) {
                Ok(turso::Value::Integer(n)) => n as u64,
                _ => 0,
            }
        } else {
            0
        };

        pool.return_connection(conn);
        Ok(count)
    }
}

/// SQLite database backend implementation
pub struct SQLiteBackend {
    /// Connection pool
    pool: Arc<Mutex<ConnectionPool>>,
    /// SQLite-specific configuration
    sqlite_config: SQLiteConfig,
    /// Cache of opened trees
    trees: RwLock<HashMap<String, Arc<SQLiteTree>>>,
}

impl SQLiteBackend {
    /// Create a new SQLiteBackend with custom SQLite configuration
    pub async fn with_sqlite_config(
        _config: DatabaseConfig,
        sqlite_config: SQLiteConfig,
    ) -> Result<Self, DatabaseError> {
        let pool = ConnectionPool::new(sqlite_config.clone()).await?;

        let backend = Self {
            pool: Arc::new(Mutex::new(pool)),
            sqlite_config: sqlite_config.clone(),
            trees: RwLock::new(HashMap::new()),
        };

        if sqlite_config.temporary {
            info!("Initialized temporary SQLite database (in-memory)");
        } else {
            info!(
                "Initialized persistent SQLite database at: {}",
                sqlite_config.path
            );
        }

        Ok(backend)
    }

    /// Create a new tree table if it doesn't exist
    async fn ensure_tree_table(&self, tree_name: &str) -> Result<(), DatabaseError> {
        let sanitized_name = sanitize_table_name(tree_name);
        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        let table_name = format!("tree_{sanitized_name}");
        let sql = format!(
            r#"
            CREATE TABLE IF NOT EXISTS {table_name} (
                key TEXT PRIMARY KEY,
                value BLOB NOT NULL,
                created_at INTEGER DEFAULT (strftime('%s','now')),
                updated_at INTEGER DEFAULT (strftime('%s','now'))
            )
            "#
        );

        conn.execute(&sql, ())
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to create tree table '{tree_name}': {e}"),
            })?;

        // Create index for the tree with unique suffix to avoid conflicts
        // Use a hash of the tree name and a random component to ensure uniqueness
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        tree_name.hash(&mut hasher);
        // Add current time to ensure uniqueness across test runs
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .hash(&mut hasher);
        let unique_suffix = hasher.finish();

        let index_name = format!("idx_{sanitized_name}_{unique_suffix:x}_key");
        let index_sql = format!("CREATE INDEX IF NOT EXISTS {index_name} ON {table_name}(key)");

        conn.execute(&index_sql, ())
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to create index for tree '{tree_name}': {e}"),
            })?;

        // Update metadata - check if exists first, then insert if needed
        let mut rows = conn
            .query(
                "SELECT tree_name FROM tree_metadata WHERE tree_name = ?",
                [turso::Value::Text(tree_name.to_string())],
            )
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to check tree metadata for '{tree_name}': {e}"),
            })?;

        if rows
            .next()
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to iterate metadata check for '{tree_name}': {e}"),
            })?
            .is_none()
        {
            // Tree doesn't exist in metadata, insert it
            conn.execute(
                "INSERT INTO tree_metadata (tree_name) VALUES (?)",
                [turso::Value::Text(tree_name.to_string())],
            )
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to insert tree metadata for '{tree_name}': {e}"),
            })?;
        }

        pool.return_connection(conn);
        Ok(())
    }
}

#[async_trait]
impl DatabaseBackend for SQLiteBackend {
    type Tree = SQLiteTree;

    async fn new(config: DatabaseConfig) -> Result<Self, DatabaseError>
    where
        Self: Sized,
    {
        let sqlite_config = SQLiteConfig {
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
            enable_wal: !config.temporary, // Enable WAL for persistent databases
            page_size: 4096,
            cache_size: (config.cache_capacity / 4096).max(100) as i32, // Convert bytes to pages
        };

        Self::with_sqlite_config(config, sqlite_config).await
    }

    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, DatabaseError> {
        let key_str = String::from_utf8_lossy(key);
        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        let mut rows = conn
            .query(
                "SELECT value FROM kv_store WHERE key = ?",
                [turso::Value::Text(key_str.to_string())],
            )
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to get key from default store: {e}"),
            })?;

        let value = if let Some(row) =
            rows.next()
                .await
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to iterate rows in default store: {e}"),
                })? {
            match row.get_value(0) {
                Ok(turso::Value::Blob(blob)) => Some(blob),
                _ => None,
            }
        } else {
            None
        };

        pool.return_connection(conn);
        Ok(value)
    }

    async fn set(&self, key: &[u8], value: &[u8]) -> Result<(), DatabaseError> {
        let key_str = String::from_utf8_lossy(key);
        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        // Try update first
        let rows_updated = conn
            .execute(
                "UPDATE kv_store SET value = ?, updated_at = strftime('%s','now') WHERE key = ?",
                [
                    turso::Value::Blob(value.to_vec()),
                    turso::Value::Text(key_str.to_string()),
                ],
            )
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to update key in default store: {e}"),
            })?;

        // If no rows were updated, insert new record
        if rows_updated == 0 {
            conn.execute(
                "INSERT INTO kv_store (key, value, created_at, updated_at) VALUES (?, ?, strftime('%s','now'), strftime('%s','now'))",
                [
                    turso::Value::Text(key_str.to_string()),
                    turso::Value::Blob(value.to_vec()),
                ],
            )
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to insert key in default store: {e}"),
            })?;
        }

        pool.return_connection(conn);
        Ok(())
    }

    async fn remove(&self, key: &[u8]) -> Result<bool, DatabaseError> {
        let key_str = String::from_utf8_lossy(key);
        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        let rows_affected = conn
            .execute(
                "DELETE FROM kv_store WHERE key = ?",
                [turso::Value::Text(key_str.to_string())],
            )
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to remove key from default store: {e}"),
            })?;

        pool.return_connection(conn);
        Ok(rows_affected > 0)
    }

    async fn scan_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, DatabaseError> {
        let prefix_str = String::from_utf8_lossy(prefix);
        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        let (sql, params) = if prefix.is_empty() {
            (
                "SELECT key, value FROM kv_store ORDER BY key".to_string(),
                Vec::new(),
            )
        } else {
            (
                "SELECT key, value FROM kv_store WHERE key GLOB ? || '*' ORDER BY key".to_string(),
                vec![turso::Value::Text(prefix_str.to_string())],
            )
        };

        let mut rows =
            conn.query(&sql, params)
                .await
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to scan prefix in default store: {e}"),
                })?;

        let mut results = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to iterate rows in default store: {e}"),
            })?
        {
            if let (Ok(turso::Value::Text(key)), Ok(turso::Value::Blob(value))) =
                (row.get_value(0), row.get_value(1))
            {
                results.push((key.as_bytes().to_vec(), value));
            }
            // Skip malformed rows
        }

        pool.return_connection(conn);
        Ok(results)
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
        let tree = Arc::new(SQLiteTree {
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
        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        let mut rows = conn
            .query("SELECT tree_name FROM tree_metadata ORDER BY tree_name", ())
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to get tree names: {e}"),
            })?;

        let mut names = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to iterate tree names: {e}"),
            })?
        {
            if let Ok(turso::Value::Text(name)) = row.get_value(0) {
                names.push(name);
            }
            // Skip malformed rows
        }

        pool.return_connection(conn);
        Ok(names)
    }

    async fn clear(&self) -> Result<(), DatabaseError> {
        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        // Clear default key-value store
        conn.execute("DELETE FROM kv_store", ())
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to clear default store: {e}"),
            })?;

        // Clear all tree tables
        let tree_names = {
            let trees = self.trees.read().await;
            trees.keys().cloned().collect::<Vec<_>>()
        };

        for tree_name in &tree_names {
            let sanitized_name = sanitize_table_name(tree_name);
            let table_name = format!("tree_{sanitized_name}");
            let sql = format!("DELETE FROM {table_name}");
            conn.execute(&sql, ())
                .await
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to clear tree '{tree_name}': {e}"),
                })?;
        }

        pool.return_connection(conn);
        Ok(())
    }

    async fn flush(&self) -> Result<(), DatabaseError> {
        if !self.sqlite_config.temporary {
            // For Turso, flush is handled automatically by the underlying database
            // Most pragmas are not supported, so we'll just do a no-op for persistent databases
            // The database will be automatically flushed when connections are closed
        }
        Ok(())
    }

    async fn stats(&self) -> Result<DatabaseStats, DatabaseError> {
        let mut pool = self.pool.lock().await;
        let conn = pool.get_connection().await?;

        // Count entries in default store
        let mut rows = conn
            .query("SELECT COUNT(*) FROM kv_store", ())
            .await
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to count default store entries: {e}"),
            })?;

        let default_count = if let Some(row) =
            rows.next()
                .await
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to iterate count result: {e}"),
                })? {
            match row.get_value(0) {
                Ok(turso::Value::Integer(n)) => n as u64,
                _ => 0,
            }
        } else {
            0
        };

        // Count entries in all trees
        let tree_names = {
            let trees = self.trees.read().await;
            trees.keys().cloned().collect::<Vec<_>>()
        };

        let mut total_entries = default_count;
        for tree_name in &tree_names {
            let sanitized_name = sanitize_table_name(tree_name);
            let table_name = format!("tree_{sanitized_name}");
            let sql = format!("SELECT COUNT(*) FROM {table_name}");

            let mut rows =
                conn.query(&sql, ())
                    .await
                    .map_err(|e| DatabaseError::OperationFailed {
                        message: format!("Failed to count entries in tree '{tree_name}': {e}"),
                    })?;

            if let Some(row) = rows
                .next()
                .await
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to iterate count result for tree '{tree_name}': {e}"),
                })?
            {
                if let Ok(turso::Value::Integer(n)) = row.get_value(0) {
                    total_entries += n as u64;
                }
            }
        }

        // Estimate total size (rough estimate)
        let estimated_avg_entry_size = 256; // bytes per entry
        let total_size_bytes = total_entries * estimated_avg_entry_size;

        let disk_size_bytes = if self.sqlite_config.temporary {
            0
        } else {
            self.size_on_disk().await?
        };

        pool.return_connection(conn);

        Ok(DatabaseStats {
            total_entries,
            total_size_bytes,
            disk_size_bytes,
            tree_count: tree_names.len(),
            is_temporary: self.sqlite_config.temporary,
        })
    }

    async fn size_on_disk(&self) -> Result<u64, DatabaseError> {
        if self.sqlite_config.temporary || self.sqlite_config.path == ":memory:" {
            return Ok(0);
        }

        let path = PathBuf::from(&self.sqlite_config.path);
        if path.exists() {
            std::fs::metadata(&path)
                .map(|metadata| metadata.len())
                .map_err(|e| DatabaseError::OperationFailed {
                    message: format!("Failed to get database file size: {e}"),
                })
        } else {
            Ok(0)
        }
    }

    fn is_temporary(&self) -> bool {
        self.sqlite_config.temporary
    }
}

/// Sanitize table names for SQL safety
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::DatabaseConfig;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_sqlite_backend_basic_operations() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let backend = SQLiteBackend::new(config).await.unwrap();

        // Test basic key-value operations
        backend.set(b"test_key", b"test_value").await.unwrap();
        let value = backend.get(b"test_key").await.unwrap();
        assert_eq!(value, Some(b"test_value".to_vec()));

        // Test removal
        let removed = backend.remove(b"test_key").await.unwrap();
        assert!(removed);

        let value = backend.get(b"test_key").await.unwrap();
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_sqlite_tree_operations() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let backend = SQLiteBackend::new(config).await.unwrap();
        let tree = backend.open_tree("test_tree").await.unwrap();

        // Test tree operations
        tree.set(b"tree_key", b"tree_value").await.unwrap();
        let value = tree.get(b"tree_key").await.unwrap();
        assert_eq!(value, Some(b"tree_value".to_vec()));

        // Test tree length
        let len = tree.len().await.unwrap();
        assert_eq!(len, 1);

        // Test prefix scan
        tree.set(b"prefix_1", b"value_1").await.unwrap();
        tree.set(b"prefix_2", b"value_2").await.unwrap();
        let results = tree.scan_prefix(b"prefix").await.unwrap();
        assert_eq!(results.len(), 2);

        // Test clear
        tree.clear().await.unwrap();
        let len = tree.len().await.unwrap();
        assert_eq!(len, 0);
    }

    #[tokio::test]
    async fn test_sqlite_persistence() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.db");

        let config = DatabaseConfig {
            path: Some(db_path.clone()),
            temporary: false,
            ..Default::default()
        };

        {
            let backend = SQLiteBackend::new(config.clone()).await.unwrap();
            backend.set(b"persist_key", b"persist_value").await.unwrap();
            backend.flush().await.unwrap();
        }

        // Reopen database
        {
            let backend = SQLiteBackend::new(config).await.unwrap();
            let value = backend.get(b"persist_key").await.unwrap();
            assert_eq!(value, Some(b"persist_value".to_vec()));
        }
    }

    #[tokio::test]
    async fn test_sqlite_stats() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let backend = SQLiteBackend::new(config).await.unwrap();

        // Add some data
        backend.set(b"key1", b"value1").await.unwrap();
        backend.set(b"key2", b"value2").await.unwrap();

        let tree = backend.open_tree("test_tree").await.unwrap();
        tree.set(b"tree_key", b"tree_value").await.unwrap();

        let stats = backend.stats().await.unwrap();
        assert_eq!(stats.total_entries, 3); // 2 in default + 1 in tree
        assert!(stats.is_temporary);
        assert_eq!(stats.tree_count, 1);
    }
}
