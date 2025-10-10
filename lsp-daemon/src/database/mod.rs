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

// Import protocol types for database query methods
use crate::protocol::{CallHierarchyResult, Location};

pub mod converters;
pub mod enrichment_tracking;
pub mod sqlite_backend;
pub use converters::ProtocolConverter;
pub use enrichment_tracking::{EnrichmentStatus, EnrichmentTracker, EnrichmentTracking};
pub use sqlite_backend::SQLiteBackend;
/// Engine-level checkpoint modes (database-agnostic)
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DbCheckpointMode {
    Passive,
    Full,
    Restart,
    Truncate,
}
// Using Turso (native SQLite implementation) as the primary backend

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

    #[error("Turso database error: {0}")]
    TursoError(#[from] turso::Error),
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
    pub file_path: String, // Relative path to the file (git-relative or workspace-relative)
    pub language: String,  // Language for direct language-based detection
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

/// Description of outstanding LSP enrichment operations for a symbol
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SymbolEnrichmentPlan {
    pub symbol: SymbolState,
    pub needs_references: bool,
    pub needs_implementations: bool,
    pub needs_call_hierarchy: bool,
}

/// Aggregated counts of pending LSP enrichment operations persisted in the database.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct PendingEnrichmentCounts {
    pub symbols_pending: u64,
    pub references_pending: u64,
    pub implementations_pending: u64,
    pub call_hierarchy_pending: u64,
    pub high_priority_pending: u64,
    pub medium_priority_pending: u64,
    pub low_priority_pending: u64,
}

impl SymbolEnrichmentPlan {
    /// Returns true if any LSP operation still needs to run for this symbol
    pub fn has_operations(&self) -> bool {
        self.needs_references || self.needs_implementations || self.needs_call_hierarchy
    }
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
    // LSP call hierarchy stored uniformly as 'calls'
    // LSP-specific definition relations
    Definition,
    Implementation,
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
            EdgeRelation::Definition => "definition",
            EdgeRelation::Implementation => "implementation",
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
            "definition" => Ok(EdgeRelation::Definition),
            "implementation" => Ok(EdgeRelation::Implementation),
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

/// Standard edge types for consistent relationship classification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StandardEdgeType {
    // Call relationships
    Calls,    // A calls B
    CalledBy, // A is called by B

    // Reference relationships
    References,   // A references B
    ReferencedBy, // A is referenced by B

    // Definition relationships
    Defines,   // A defines B
    DefinedBy, // A is defined by B

    // Implementation relationships
    Implements,    // A implements B
    ImplementedBy, // A is implemented by B

    // Type relationships
    HasType, // A has type B
    TypeOf,  // A is type of B

    // Inheritance relationships
    Extends,    // A extends B
    ExtendedBy, // A is extended by B
}

impl StandardEdgeType {
    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Calls => "calls",
            Self::CalledBy => "called_by",
            Self::References => "references",
            Self::ReferencedBy => "referenced_by",
            Self::Defines => "defines",
            Self::DefinedBy => "defined_by",
            Self::Implements => "implements",
            Self::ImplementedBy => "implemented_by",
            Self::HasType => "has_type",
            Self::TypeOf => "type_of",
            Self::Extends => "extends",
            Self::ExtendedBy => "extended_by",
        }
    }
}

/// Edge representation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Edge {
    pub relation: EdgeRelation,
    pub source_symbol_uid: String,
    pub target_symbol_uid: String,
    pub file_path: Option<String>, // File path from symbol_state for direct access
    pub start_line: Option<u32>,
    pub start_char: Option<u32>,
    pub confidence: f32,
    pub language: String,         // Language for direct language-based detection
    pub metadata: Option<String>, // Additional metadata
}

/// Create a "none" edge to mark a symbol as "analyzed but empty"
/// This prevents repeated LSP calls for symbols with no call hierarchy/references
pub fn create_none_edge(source_symbol_uid: &str, relation: EdgeRelation) -> Edge {
    Edge {
        relation,
        source_symbol_uid: source_symbol_uid.to_string(),
        target_symbol_uid: "none".to_string(), // Special marker for "analyzed but empty"
        file_path: None,                       // None edges don't need file path resolution
        start_line: None,
        start_char: None,
        confidence: 1.0,
        language: "unknown".to_string(), // Default language for none edges
        metadata: Some("null_edge".to_string()), // Mark as a special edge type
    }
}

/// Create "none" edges for empty call hierarchy results
/// Used when LSP returns {incoming: [], outgoing: []} (not null!)
pub fn create_none_call_hierarchy_edges(symbol_uid: &str) -> Vec<Edge> {
    // No outgoing: source symbol → none
    let outgoing = Edge {
        relation: EdgeRelation::Calls,
        source_symbol_uid: symbol_uid.to_string(),
        target_symbol_uid: "none".to_string(),
        file_path: None,
        start_line: None,
        start_char: None,
        confidence: 1.0,
        language: "unknown".to_string(),
        metadata: Some("lsp_call_hierarchy_empty_outgoing".to_string()),
    };

    // No incoming: none → target symbol
    let incoming = Edge {
        relation: EdgeRelation::Calls,
        source_symbol_uid: "none".to_string(),
        target_symbol_uid: symbol_uid.to_string(),
        file_path: None,
        start_line: None,
        start_char: None,
        confidence: 1.0,
        language: "unknown".to_string(),
        metadata: Some("lsp_call_hierarchy_empty_incoming".to_string()),
    };

    vec![incoming, outgoing]
}

/// Create "none" edges for empty references results  
/// Used when LSP returns [] for references (not null!)
pub fn create_none_reference_edges(symbol_uid: &str) -> Vec<Edge> {
    vec![create_none_edge(symbol_uid, EdgeRelation::References)]
}

/// Create "none" edges for empty definitions results
pub fn create_none_definition_edges(symbol_uid: &str) -> Vec<Edge> {
    vec![create_none_edge(symbol_uid, EdgeRelation::Definition)]
}

/// Create "none" edges for empty implementations results
pub fn create_none_implementation_edges(symbol_uid: &str) -> Vec<Edge> {
    vec![create_none_edge(symbol_uid, EdgeRelation::Implementation)]
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

/// Result of interpreting edges for a symbol and relation type
#[derive(Debug, Clone, PartialEq)]
pub enum EdgeInterpretation<T> {
    /// No edges found - need fresh LSP call
    Unknown,
    /// Single null edge found - LSP analyzed but found nothing (return [])
    AnalyzedEmpty,
    /// Real edges found (nulls ignored if mixed)
    HasData(Vec<T>),
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

    /// Perform an engine-direct checkpoint if the backend supports it.
    /// Default implementation returns OperationFailed.
    async fn engine_checkpoint(&self, _mode: DbCheckpointMode) -> Result<(), DatabaseError> {
        Err(DatabaseError::OperationFailed {
            message: "engine_checkpoint not supported by backend".to_string(),
        })
    }

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

    // File versioning methods removed

    /// Link file to workspace (deprecated - workspace_file table removed)
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
        file_path: &str,
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
        file_id: i64,
        language: &str,
        priority: i32,
    ) -> Result<(), DatabaseError>;

    // ===================
    // Graph Export Support
    // ===================

    /// Get all symbols in the database (for graph export)
    async fn get_all_symbols(&self) -> Result<Vec<SymbolState>, DatabaseError>;

    /// Get all edges in the database (for graph export)
    async fn get_all_edges(&self) -> Result<Vec<Edge>, DatabaseError>;

    // ===================
    // LSP Protocol Query Methods
    // ===================

    /// Get call hierarchy for a symbol, returns wire protocol type
    /// Note: Symbol resolution happens at daemon layer, not database layer
    async fn get_call_hierarchy_for_symbol(
        &self,
        workspace_id: i64,
        symbol_uid: &str,
    ) -> Result<Option<CallHierarchyResult>, DatabaseError>;

    /// Get references for a symbol, returns wire protocol type
    async fn get_references_for_symbol(
        &self,
        workspace_id: i64,
        symbol_uid: &str,
        include_declaration: bool,
    ) -> Result<Vec<Location>, DatabaseError>;

    /// Get definitions for a symbol
    async fn get_definitions_for_symbol(
        &self,
        workspace_id: i64,
        symbol_uid: &str,
    ) -> Result<Vec<Location>, DatabaseError>;

    /// Get implementations for a symbol
    async fn get_implementations_for_symbol(
        &self,
        workspace_id: i64,
        symbol_uid: &str,
    ) -> Result<Vec<Location>, DatabaseError>;

    // ===================
    // LSP Enrichment Support
    // ===================

    /// Find symbols that still require LSP enrichment operations along with pending operation flags
    async fn find_symbols_pending_enrichment(
        &self,
        limit: usize,
    ) -> Result<Vec<SymbolEnrichmentPlan>, DatabaseError>;
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

    #[test]
    fn test_standard_edge_type_as_str() {
        // Test call relationships
        assert_eq!(StandardEdgeType::Calls.as_str(), "calls");
        assert_eq!(StandardEdgeType::CalledBy.as_str(), "called_by");

        // Test reference relationships
        assert_eq!(StandardEdgeType::References.as_str(), "references");
        assert_eq!(StandardEdgeType::ReferencedBy.as_str(), "referenced_by");

        // Test definition relationships
        assert_eq!(StandardEdgeType::Defines.as_str(), "defines");
        assert_eq!(StandardEdgeType::DefinedBy.as_str(), "defined_by");

        // Test implementation relationships
        assert_eq!(StandardEdgeType::Implements.as_str(), "implements");
        assert_eq!(StandardEdgeType::ImplementedBy.as_str(), "implemented_by");

        // Test type relationships
        assert_eq!(StandardEdgeType::HasType.as_str(), "has_type");
        assert_eq!(StandardEdgeType::TypeOf.as_str(), "type_of");

        // Test inheritance relationships
        assert_eq!(StandardEdgeType::Extends.as_str(), "extends");
        assert_eq!(StandardEdgeType::ExtendedBy.as_str(), "extended_by");
    }

    #[test]
    fn test_standard_edge_type_serialization() {
        // Test that the enum can be serialized and deserialized
        let edge_type = StandardEdgeType::Calls;
        let serialized = serde_json::to_string(&edge_type).expect("Failed to serialize");
        let deserialized: StandardEdgeType =
            serde_json::from_str(&serialized).expect("Failed to deserialize");
        assert_eq!(edge_type, deserialized);

        // Test all variants
        let all_types = vec![
            StandardEdgeType::Calls,
            StandardEdgeType::CalledBy,
            StandardEdgeType::References,
            StandardEdgeType::ReferencedBy,
            StandardEdgeType::Defines,
            StandardEdgeType::DefinedBy,
            StandardEdgeType::Implements,
            StandardEdgeType::ImplementedBy,
            StandardEdgeType::HasType,
            StandardEdgeType::TypeOf,
            StandardEdgeType::Extends,
            StandardEdgeType::ExtendedBy,
        ];

        for edge_type in all_types {
            let serialized = serde_json::to_string(&edge_type).expect("Failed to serialize");
            let deserialized: StandardEdgeType =
                serde_json::from_str(&serialized).expect("Failed to deserialize");
            assert_eq!(edge_type, deserialized);
        }
    }

    // Additional integration tests will be added in the backend implementations
}
