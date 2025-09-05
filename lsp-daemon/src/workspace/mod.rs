//! Workspace Management Module
//!
//! Provides comprehensive workspace management APIs for managing projects, workspaces,
//! and their file associations with support for content-addressed storage, git integration,
//! and incremental indexing.

pub mod branch;
pub mod config;
pub mod manager;
pub mod project;

#[cfg(test)]
mod tests;

// Re-export main types and APIs
pub use branch::{BranchError, BranchManager, BranchSwitchResult, GitSyncResult};
pub use config::{CacheConfig, WorkspaceConfig, WorkspaceConfigBuilder, WorkspaceValidationError};
pub use manager::{IndexingResult, WorkspaceError, WorkspaceManager};
pub use project::{Project, ProjectConfig, ProjectError, ProjectManager};

// Re-export commonly used types
pub use crate::database::{AnalysisProgress, FileVersion, Workspace};
pub use crate::indexing::versioning::{FileVersionInfo, FileVersionManager, ProcessingResults};

// Note: WorkspaceIndexingResult, ComprehensiveBranchSwitchResult, FileChange, FileChangeType,
// WorkspaceMetrics, WorkspaceEvent, WorkspaceEventHandler, WorkspaceManagementError, and NoOpEventHandler
// are defined in this module and automatically exported

use std::time::Duration;

/// Comprehensive result type for workspace indexing operations
#[derive(Debug, Clone)]
pub struct WorkspaceIndexingResult {
    pub workspace_id: i64,
    pub files_processed: u64,
    pub files_added: u64,
    pub files_updated: u64,
    pub files_deleted: u64,
    pub bytes_processed: u64,
    pub processing_time: Duration,
    pub deduplication_savings: u64,
    pub git_integration_active: bool,
    pub branch_name: Option<String>,
    pub commit_hash: Option<String>,
}

/// Result of branch switching operations with comprehensive metrics
#[derive(Debug, Clone)]
pub struct ComprehensiveBranchSwitchResult {
    pub workspace_id: i64,
    pub previous_branch: Option<String>,
    pub new_branch: String,
    pub files_changed: u64,
    pub reused_versions: u64,
    pub switch_time: Duration,
    pub git_sync_result: Option<GitSyncResult>,
    pub indexing_required: bool,
    pub cache_invalidations: u64,
}

/// Workspace file change information for incremental updates
#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: std::path::PathBuf,
    pub change_type: FileChangeType,
    pub content_digest: Option<String>,
    pub size_bytes: Option<u64>,
    pub modified_time: Option<i64>,
}

/// Types of file changes detected during workspace operations
#[derive(Debug, Clone, PartialEq)]
pub enum FileChangeType {
    Create,
    Update,
    Delete,
    Move {
        from: std::path::PathBuf,
        to: std::path::PathBuf,
    },
}

/// Workspace operation metrics for monitoring and optimization
#[derive(Debug, Clone, Default)]
pub struct WorkspaceMetrics {
    pub total_workspaces_managed: u64,
    pub total_files_indexed: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub git_operations: u64,
    pub database_transactions: u64,
    pub average_indexing_time_ms: u64,
    pub deduplication_rate: f64,
}

/// Workspace lifecycle events for monitoring and hooks
#[derive(Debug, Clone)]
pub enum WorkspaceEvent {
    Created {
        workspace_id: i64,
        name: String,
    },
    Deleted {
        workspace_id: i64,
        name: String,
    },
    IndexingStarted {
        workspace_id: i64,
    },
    IndexingCompleted {
        workspace_id: i64,
        result: WorkspaceIndexingResult,
    },
    BranchSwitched {
        workspace_id: i64,
        from: Option<String>,
        to: String,
    },
    FilesUpdated {
        workspace_id: i64,
        file_count: u64,
    },
    Error {
        workspace_id: Option<i64>,
        error: String,
    },
}

/// Workspace management error types with comprehensive context
#[derive(Debug, thiserror::Error)]
pub enum WorkspaceManagementError {
    #[error("Workspace operation failed: {operation} - {context}")]
    OperationFailed { operation: String, context: String },

    #[error("Configuration error: {message}")]
    Configuration { message: String },

    #[error("Git integration error: {source}")]
    GitIntegration {
        #[from]
        source: crate::git_service::GitServiceError,
    },

    #[error("Database error: {source}")]
    Database {
        #[from]
        source: crate::database::DatabaseError,
    },

    #[error("File versioning error: {source}")]
    FileVersioning {
        #[from]
        source: crate::indexing::versioning::VersioningError,
    },

    #[error("Project management error: {source}")]
    ProjectManagement {
        #[from]
        source: ProjectError,
    },

    #[error("Context error: {source}")]
    Context {
        #[from]
        source: anyhow::Error,
    },
}

/// Event handler trait for workspace lifecycle events
pub trait WorkspaceEventHandler: Send + Sync {
    fn handle_event(
        &self,
        event: WorkspaceEvent,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<(), WorkspaceManagementError>> + Send + '_>,
    >;
}

/// Default no-op event handler
pub struct NoOpEventHandler;

impl WorkspaceEventHandler for NoOpEventHandler {
    fn handle_event(
        &self,
        _event: WorkspaceEvent,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<(), WorkspaceManagementError>> + Send + '_>,
    > {
        Box::pin(async { Ok(()) })
    }
}
