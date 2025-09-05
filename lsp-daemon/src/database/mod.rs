//! Database abstraction layer for LSP daemon
//!
//! This module provides a clean database abstraction interface using SQLite (via libSQL) for fast,
//! local storage with minimal compilation overhead. It supports both persistent and
//! in-memory modes, with comprehensive error handling and async support.
//!
//! ## Architecture
//!
//! The abstraction is built around the `DatabaseBackend` trait which provides a
//! database-agnostic interface for key-value operations with additional features:
//!
//! - **Key-value operations**: get, set, remove
//! - **Prefix scanning**: for efficient cache clearing operations
//! - **Tree operations**: hierarchical data organization
//! - **Maintenance operations**: clear, flush, size reporting
//! - **Storage modes**: persistent disk storage or temporary in-memory
//!
//! ## Usage
//!
//! ```rust
//! use database::{DatabaseBackend, SQLiteBackend, DatabaseConfig};
//!
//! // Create a persistent database
//! let config = DatabaseConfig {
//!     path: Some(PathBuf::from("/tmp/my-cache.db")),
//!     temporary: false,
//!     compression: true,
//!     cache_capacity: 64 * 1024 * 1024,
//! };
//! let db = SQLiteBackend::new(config).await?;
//!
//! // Basic operations
//! db.set(b"key", b"value").await?;
//! let value = db.get(b"key").await?;
//!
//! // Tree operations (for organized data)
//! let tree = db.open_tree("my_tree").await?;
//! tree.set(b"tree_key", b"tree_value").await?;
//! ```

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;

pub mod migrations;
pub mod sqlite_backend;
pub use migrations::{Migration, MigrationError, MigrationRunner};
pub use sqlite_backend::SQLiteBackend;
// Legacy DuckDB exports removed - SQLite is now the primary backend

/// Database error types specific to database operations
#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("Database corruption detected: {message}")]
    Corruption { message: String },

    #[error("Database operation failed: {message}")]
    OperationFailed { message: String },

    #[error("Serialization error: {0}")]
    Serialization(#[from] Box<bincode::ErrorKind>),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database configuration error: {message}")]
    Configuration { message: String },

    #[error("Tree not found: {name}")]
    TreeNotFound { name: String },
}

/// Configuration for database backends
#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    /// Path to the database file (None for temporary/in-memory)
    pub path: Option<PathBuf>,
    /// Whether to use temporary/in-memory storage
    pub temporary: bool,
    /// Enable compression if supported by backend
    pub compression: bool,
    /// Cache capacity in bytes
    pub cache_capacity: u64,
    /// Compression factor (higher = more compression)
    pub compression_factor: i32,
    /// Flush interval in milliseconds (None to disable periodic flushes)
    pub flush_every_ms: Option<u64>,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: None,
            temporary: false,
            compression: false,
            cache_capacity: 64 * 1024 * 1024, // 64MB default
            compression_factor: 5,            // Balanced compression
            flush_every_ms: Some(1000),       // Flush every second
        }
    }
}

/// Database statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseStats {
    /// Total number of entries across all trees
    pub total_entries: u64,
    /// Estimated total size in bytes
    pub total_size_bytes: u64,
    /// Database size on disk (0 for in-memory)
    pub disk_size_bytes: u64,
    /// Number of trees
    pub tree_count: usize,
    /// Whether the database is in-memory/temporary
    pub is_temporary: bool,
}

/// Workspace representation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Workspace {
    pub workspace_id: i64,
    pub project_id: i64,
    pub name: String,
    pub description: Option<String>,
    pub branch_hint: Option<String>,
    pub is_active: bool,
    pub created_at: String,
}

/// File version representation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileVersion {
    pub file_version_id: i64,
    pub file_id: i64,
    pub content_digest: String,
    pub git_blob_oid: Option<String>,
    pub size_bytes: u64,
    pub line_count: Option<u32>,
    pub detected_language: Option<String>,
    pub mtime: Option<i64>,
}

/// Symbol state representation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SymbolState {
    pub symbol_uid: String,
    pub file_version_id: i64,
    pub language: String, // Language for direct language-based detection
    pub name: String,
    pub fqn: Option<String>,
    pub kind: String,
    pub signature: Option<String>,
    pub visibility: Option<String>,
    pub def_start_line: u32,
    pub def_start_char: u32,
    pub def_end_line: u32,
    pub def_end_char: u32,
    pub is_definition: bool,
    pub documentation: Option<String>,
    pub metadata: Option<String>,
}

/// Edge relationship types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum EdgeRelation {
    HasChild,
    InheritsFrom,
    Implements,
    Overrides,
    References,
    Calls,
    Instantiates,
    Imports,
    Includes,
    DependsOn,
}

impl EdgeRelation {
    /// Convert to string for database storage
    pub fn to_string(&self) -> &'static str {
        match self {
            EdgeRelation::HasChild => "has_child",
            EdgeRelation::InheritsFrom => "inherits_from",
            EdgeRelation::Implements => "implements",
            EdgeRelation::Overrides => "overrides",
            EdgeRelation::References => "references",
            EdgeRelation::Calls => "calls",
            EdgeRelation::Instantiates => "instantiates",
            EdgeRelation::Imports => "imports",
            EdgeRelation::Includes => "includes",
            EdgeRelation::DependsOn => "depends_on",
        }
    }

    /// Parse from database string
    pub fn from_string(s: &str) -> Result<Self, DatabaseError> {
        match s {
            "has_child" => Ok(EdgeRelation::HasChild),
            "inherits_from" => Ok(EdgeRelation::InheritsFrom),
            "implements" => Ok(EdgeRelation::Implements),
            "overrides" => Ok(EdgeRelation::Overrides),
            "references" => Ok(EdgeRelation::References),
            "calls" => Ok(EdgeRelation::Calls),
            "instantiates" => Ok(EdgeRelation::Instantiates),
            "imports" => Ok(EdgeRelation::Imports),
            "includes" => Ok(EdgeRelation::Includes),
            "depends_on" => Ok(EdgeRelation::DependsOn),
            _ => Err(DatabaseError::OperationFailed {
                message: format!("Unknown edge relation: {}", s),
            }),
        }
    }
}

/// Call direction for graph traversal
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CallDirection {
    Incoming,
    Outgoing,
    Both,
}

/// Edge representation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Edge {
    pub relation: EdgeRelation,
    pub source_symbol_uid: String,
    pub target_symbol_uid: String,
    pub anchor_file_version_id: i64,
    pub start_line: Option<u32>,
    pub start_char: Option<u32>,
    pub confidence: f32,
    pub language: String,         // Language for direct language-based detection
    pub metadata: Option<String>, // Additional metadata
}

/// Graph path for traversal results
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphPath {
    pub symbol_uid: String,
    pub depth: u32,
    pub path: Vec<String>,
    pub relation_chain: Vec<EdgeRelation>,
}

/// Analysis progress information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnalysisProgress {
    pub workspace_id: i64,
    pub total_files: u64,
    pub analyzed_files: u64,
    pub failed_files: u64,
    pub pending_files: u64,
    pub completion_percentage: f32,
}

/// Represents a database tree (hierarchical namespace for keys)
#[async_trait]
pub trait DatabaseTree: Send + Sync {
    /// Get a value by key from this tree
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, DatabaseError>;

    /// Set a key-value pair in this tree
    async fn set(&self, key: &[u8], value: &[u8]) -> Result<(), DatabaseError>;

    /// Remove a key from this tree
    async fn remove(&self, key: &[u8]) -> Result<bool, DatabaseError>;

    /// Scan all keys with a given prefix in this tree
    async fn scan_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, DatabaseError>;

    /// Clear all entries in this tree
    async fn clear(&self) -> Result<(), DatabaseError>;

    /// Get the number of entries in this tree
    async fn len(&self) -> Result<u64, DatabaseError>;

    /// Check if this tree is empty
    async fn is_empty(&self) -> Result<bool, DatabaseError> {
        Ok(self.len().await? == 0)
    }
}

/// Main database backend trait that all implementations must support
#[async_trait]
pub trait DatabaseBackend: Send + Sync {
    /// Associated tree type for this backend
    type Tree: DatabaseTree;

    /// Create a new database instance with the given configuration
    async fn new(config: DatabaseConfig) -> Result<Self, DatabaseError>
    where
        Self: Sized;

    /// Get a value by key from the default tree
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, DatabaseError>;

    /// Set a key-value pair in the default tree
    async fn set(&self, key: &[u8], value: &[u8]) -> Result<(), DatabaseError>;

    /// Remove a key from the default tree
    async fn remove(&self, key: &[u8]) -> Result<bool, DatabaseError>;

    /// Scan all keys with a given prefix in the default tree
    async fn scan_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, DatabaseError>;

    /// Open or create a named tree (hierarchical namespace)
    async fn open_tree(&self, name: &str) -> Result<Arc<Self::Tree>, DatabaseError>;

    /// List all available tree names
    async fn tree_names(&self) -> Result<Vec<String>, DatabaseError>;

    /// Clear all data from the database (all trees)
    async fn clear(&self) -> Result<(), DatabaseError>;

    /// Force flush pending changes to disk (no-op for in-memory)
    async fn flush(&self) -> Result<(), DatabaseError>;

    /// Get database statistics
    async fn stats(&self) -> Result<DatabaseStats, DatabaseError>;

    /// Get the size of the database on disk in bytes (0 for in-memory)
    async fn size_on_disk(&self) -> Result<u64, DatabaseError>;

    /// Check if this database is temporary/in-memory
    fn is_temporary(&self) -> bool;

    // ===================
    // Workspace Management
    // ===================

    /// Create a new workspace
    async fn create_workspace(
        &self,
        name: &str,
        project_id: i64,
        branch_hint: Option<&str>,
    ) -> Result<i64, DatabaseError>;

    /// Get workspace by ID
    async fn get_workspace(&self, workspace_id: i64) -> Result<Option<Workspace>, DatabaseError>;

    /// List workspaces, optionally filtered by project
    async fn list_workspaces(
        &self,
        project_id: Option<i64>,
    ) -> Result<Vec<Workspace>, DatabaseError>;

    /// Update workspace branch hint
    async fn update_workspace_branch(
        &self,
        workspace_id: i64,
        branch: &str,
    ) -> Result<(), DatabaseError>;

    // ===================
    // File Version Management
    // ===================

    /// Create a new file version
    async fn create_file_version(
        &self,
        file_id: i64,
        content_digest: &str,
        size_bytes: u64,
        mtime: Option<i64>,
    ) -> Result<i64, DatabaseError>;

    /// Get file version by content digest
    async fn get_file_version_by_digest(
        &self,
        content_digest: &str,
    ) -> Result<Option<FileVersion>, DatabaseError>;

    /// Link file to workspace (add to workspace)
    async fn link_file_to_workspace(
        &self,
        workspace_id: i64,
        file_id: i64,
        file_version_id: i64,
    ) -> Result<(), DatabaseError>;

    // ===================
    // Symbol Storage & Retrieval
    // ===================

    /// Store multiple symbols from analysis
    async fn store_symbols(&self, symbols: &[SymbolState]) -> Result<(), DatabaseError>;

    /// Get symbols by file version and language
    async fn get_symbols_by_file(
        &self,
        file_version_id: i64,
        language: &str,
    ) -> Result<Vec<SymbolState>, DatabaseError>;

    /// Find symbols by name within workspace
    async fn find_symbol_by_name(
        &self,
        workspace_id: i64,
        name: &str,
    ) -> Result<Vec<SymbolState>, DatabaseError>;

    /// Find symbol by fully qualified name
    async fn find_symbol_by_fqn(
        &self,
        workspace_id: i64,
        fqn: &str,
    ) -> Result<Option<SymbolState>, DatabaseError>;

    // ===================
    // Relationship Storage & Querying
    // ===================

    /// Store multiple edges (relationships) from analysis
    async fn store_edges(&self, edges: &[Edge]) -> Result<(), DatabaseError>;

    /// Get all references to a symbol (incoming edges)
    async fn get_symbol_references(
        &self,
        workspace_id: i64,
        symbol_uid: &str,
    ) -> Result<Vec<Edge>, DatabaseError>;

    /// Get call relationships for a symbol (incoming/outgoing/both)
    async fn get_symbol_calls(
        &self,
        workspace_id: i64,
        symbol_uid: &str,
        direction: CallDirection,
    ) -> Result<Vec<Edge>, DatabaseError>;

    /// Traverse graph starting from symbol with maximum depth and relation filters
    async fn traverse_graph(
        &self,
        start_symbol: &str,
        max_depth: u32,
        relations: &[EdgeRelation],
    ) -> Result<Vec<GraphPath>, DatabaseError>;

    // ===================
    // Analysis Management
    // ===================

    /// Create new analysis run
    async fn create_analysis_run(
        &self,
        analyzer_name: &str,
        analyzer_version: &str,
        language: &str,
        config: &str,
    ) -> Result<i64, DatabaseError>;

    /// Get analysis progress for workspace
    async fn get_analysis_progress(
        &self,
        workspace_id: i64,
    ) -> Result<AnalysisProgress, DatabaseError>;

    /// Queue file for analysis
    async fn queue_file_analysis(
        &self,
        file_version_id: i64,
        language: &str,
        priority: i32,
    ) -> Result<(), DatabaseError>;
}

/// Convenience functions for serializable types
#[allow(async_fn_in_trait)]
pub trait DatabaseBackendExt: DatabaseBackend {
    /// Get and deserialize a value
    async fn get_serialized<T>(&self, key: &[u8]) -> Result<Option<T>, DatabaseError>
    where
        T: for<'de> Deserialize<'de>,
    {
        if let Some(data) = self.get(key).await? {
            let value = bincode::deserialize(&data)?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    /// Serialize and set a value
    async fn set_serialized<T>(&self, key: &[u8], value: &T) -> Result<(), DatabaseError>
    where
        T: Serialize,
    {
        let data = bincode::serialize(value)?;
        self.set(key, &data).await
    }
}

/// Implement the extension trait for all DatabaseBackend implementations
impl<T: DatabaseBackend> DatabaseBackendExt for T {}

/// Convenience functions for DatabaseTree with serialization
#[allow(async_fn_in_trait)]
pub trait DatabaseTreeExt: DatabaseTree {
    /// Get and deserialize a value from this tree
    async fn get_serialized<T>(&self, key: &[u8]) -> Result<Option<T>, DatabaseError>
    where
        T: for<'de> Deserialize<'de>,
    {
        if let Some(data) = self.get(key).await? {
            let value = bincode::deserialize(&data)?;
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    /// Serialize and set a value in this tree
    async fn set_serialized<T>(&self, key: &[u8], value: &T) -> Result<(), DatabaseError>
    where
        T: Serialize,
    {
        let data = bincode::serialize(value)?;
        self.set(key, &data).await
    }
}

/// Implement the extension trait for all DatabaseTree implementations
impl<T: DatabaseTree> DatabaseTreeExt for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_database_config_default() {
        let config = DatabaseConfig::default();
        assert_eq!(config.path, None);
        assert!(!config.temporary);
        assert!(!config.compression);
        assert_eq!(config.cache_capacity, 64 * 1024 * 1024);
    }

    // Additional integration tests will be added in the backend implementations
}
