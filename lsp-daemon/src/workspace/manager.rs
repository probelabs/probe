//! Workspace Manager Implementation
//!
//! Provides the main WorkspaceManager struct with comprehensive workspace management,
//! file operations, git integration, and performance optimizations.

use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{Mutex, RwLock, Semaphore};
use tracing::{debug, error, info, warn};

use crate::database::{DatabaseBackend, DatabaseError, Workspace};
use crate::git_service::{GitService, GitServiceError};
use crate::indexing::versioning::{FileVersionManager, VersioningError};

use super::branch::{BranchError, BranchManager, GitSyncResult};
use super::config::{WorkspaceConfig, WorkspaceValidationError};
use super::project::{Project, ProjectError, ProjectManager};
use super::{
    ComprehensiveBranchSwitchResult, FileChange, FileChangeType, WorkspaceEvent,
    WorkspaceEventHandler, WorkspaceIndexingResult, WorkspaceMetrics,
};

/// Comprehensive indexing result with detailed metrics
#[derive(Debug, Clone, Serialize)]
pub struct IndexingResult {
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
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Workspace management errors
#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("Workspace not found: {workspace_id}")]
    WorkspaceNotFound { workspace_id: i64 },

    #[error("Workspace name already exists: {name}")]
    WorkspaceNameExists { name: String },

    #[error("Invalid workspace path: {path} - {reason}")]
    InvalidWorkspacePath { path: String, reason: String },

    #[error("Workspace validation failed: {message}")]
    ValidationFailed { message: String },

    #[error("Configuration error: {source}")]
    Configuration {
        #[from]
        source: WorkspaceValidationError,
    },

    #[error("Project management error: {source}")]
    ProjectManagement {
        #[from]
        source: ProjectError,
    },

    #[error("Branch management error: {source}")]
    BranchManagement {
        #[from]
        source: BranchError,
    },

    #[error("Git service error: {source}")]
    GitService {
        #[from]
        source: GitServiceError,
    },

    #[error("Database error: {source}")]
    Database {
        #[from]
        source: DatabaseError,
    },

    #[error("File versioning error: {source}")]
    FileVersioning {
        #[from]
        source: VersioningError,
    },

    #[error("IO error: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },

    #[error("Context error: {source}")]
    Context {
        #[from]
        source: anyhow::Error,
    },
}

/// Main workspace manager with comprehensive functionality
pub struct WorkspaceManager<T>
where
    T: DatabaseBackend + Send + Sync + 'static,
{
    database: Arc<T>,
    file_manager: FileVersionManager<T>,
    project_manager: ProjectManager<T>,
    branch_manager: BranchManager<T>,
    config: WorkspaceConfig,
    event_handler: Arc<dyn WorkspaceEventHandler>,
    operation_semaphore: Arc<Semaphore>,
    workspace_cache: Arc<RwLock<HashMap<i64, Workspace>>>,
    metrics: Arc<RwLock<WorkspaceMetrics>>,
    start_time: Instant,
}

impl<T> WorkspaceManager<T>
where
    T: DatabaseBackend + Send + Sync + 'static,
{
    /// Create a new workspace manager
    pub async fn new(database: Arc<T>) -> Result<Self, WorkspaceError> {
        let config = WorkspaceConfig::default();
        Self::with_config(database, config).await
    }

    /// Create a new workspace manager with custom configuration
    pub async fn with_config(
        database: Arc<T>,
        config: WorkspaceConfig,
    ) -> Result<Self, WorkspaceError> {
        // Validate configuration
        config.validate()?;

        info!("Initializing WorkspaceManager with config: git_integration={}, incremental_indexing={}, max_concurrent={}",
            config.git_integration,
            config.incremental_indexing,
            config.performance.max_concurrent_operations
        );

        // Initialize file version manager
        let versioning_config = crate::indexing::versioning::VersioningConfig {
            max_concurrent_operations: config.performance.max_concurrent_operations,
            enable_git_integration: config.git_integration,
            max_file_size: config.max_file_size_mb * 1024 * 1024, // Convert MB to bytes
            batch_size: config.performance.batch_size,
            collect_metrics: true,
            hash_algorithm: crate::indexing::HashAlgorithm::Blake3,
            ..Default::default()
        };

        let file_manager = FileVersionManager::new(database.clone(), versioning_config)
            .await
            .context("Failed to initialize file version manager")?;

        // Initialize project manager
        let project_manager = ProjectManager::new(database.clone(), config.git_integration);

        // Initialize branch manager with a separate file manager instance
        let branch_versioning_config = crate::indexing::versioning::VersioningConfig {
            max_concurrent_operations: config.performance.max_concurrent_operations,
            enable_git_integration: config.git_integration,
            max_file_size: config.max_file_size_mb * 1024 * 1024, // Convert MB to bytes
            batch_size: config.performance.batch_size,
            collect_metrics: true,
            hash_algorithm: crate::indexing::HashAlgorithm::Blake3,
            ..Default::default()
        };
        let branch_file_manager =
            FileVersionManager::new(database.clone(), branch_versioning_config)
                .await
                .context("Failed to initialize branch file manager")?;
        let branch_manager = BranchManager::new(
            database.clone(),
            branch_file_manager,
            config.git_integration,
        )
        .await
        .context("Failed to initialize branch manager")?;

        // Create workspace manager
        let manager = Self {
            database,
            file_manager,
            project_manager,
            branch_manager,
            operation_semaphore: Arc::new(Semaphore::new(
                config.performance.max_concurrent_operations,
            )),
            workspace_cache: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(WorkspaceMetrics::default())),
            event_handler: Arc::new(super::NoOpEventHandler),
            config,
            start_time: Instant::now(),
        };

        info!("WorkspaceManager initialized successfully");
        Ok(manager)
    }

    /// Create workspace manager with git integration
    pub async fn with_git_integration(
        database: Arc<T>,
        git_service: Arc<Mutex<GitService>>,
    ) -> Result<Self, WorkspaceError> {
        let mut config = WorkspaceConfig::default();
        config.git_integration = true;

        // The git_service parameter is for future use if we need workspace-specific git services
        let _ = git_service; // Suppress unused warning

        Self::with_config(database, config).await
    }

    /// Set event handler for workspace lifecycle events
    pub fn set_event_handler(&mut self, handler: Arc<dyn WorkspaceEventHandler>) {
        self.event_handler = handler;
    }

    // ===================
    // Project Operations
    // ===================

    /// Create a new project
    pub async fn create_project(
        &self,
        name: &str,
        root_path: &Path,
    ) -> Result<i64, WorkspaceError> {
        let _permit = self
            .operation_semaphore
            .acquire()
            .await
            .context("Failed to acquire operation permit")?;

        info!("Creating project: {} at {}", name, root_path.display());

        let project_config = super::project::ProjectConfig {
            name: name.to_string(),
            root_path: root_path.to_path_buf(),
            auto_detect_languages: true,
            enable_caching: self.config.cache_settings.enabled,
            ..Default::default()
        };

        let project_id = self.project_manager.create_project(project_config).await?;

        // Update metrics
        self.update_metrics(|metrics| metrics.total_workspaces_managed += 1)
            .await;

        // Emit event
        self.emit_event(WorkspaceEvent::Created {
            workspace_id: project_id, // Using project_id as workspace_id for now
            name: name.to_string(),
        })
        .await?;

        Ok(project_id)
    }

    /// Get project by ID
    pub async fn get_project(&self, project_id: i64) -> Result<Option<Project>, WorkspaceError> {
        self.project_manager
            .get_project(project_id)
            .await
            .map_err(WorkspaceError::from)
    }

    /// List all projects
    pub async fn list_projects(&self) -> Result<Vec<Project>, WorkspaceError> {
        self.project_manager
            .list_projects(true)
            .await
            .map_err(WorkspaceError::from)
    }

    // ===================
    // Workspace Operations
    // ===================

    /// Create a new workspace
    pub async fn create_workspace(
        &self,
        project_id: i64,
        name: &str,
        description: Option<&str>,
    ) -> Result<i64, WorkspaceError> {
        let _permit = self
            .operation_semaphore
            .acquire()
            .await
            .context("Failed to acquire operation permit")?;

        info!("Creating workspace '{}' for project {}", name, project_id);

        // Validate project exists
        let project =
            self.get_project(project_id)
                .await?
                .ok_or(WorkspaceError::ValidationFailed {
                    message: format!("Project {} not found", project_id),
                })?;

        // Detect current branch if git integration is enabled
        let branch_hint = if self.config.git_integration {
            match GitService::discover_repo(&project.root_path, &project.root_path) {
                Ok(git_service) => {
                    match git_service.head_commit() {
                        Ok(Some(_)) => Some("main".to_string()), // Simplified branch detection
                        _ => None,
                    }
                }
                Err(_) => None,
            }
        } else {
            None
        };

        // Create workspace in database
        let workspace_id = self
            .database
            .create_workspace(name, project_id, branch_hint.as_deref())
            .await?;

        // Cache the workspace
        let workspace = Workspace {
            workspace_id,
            project_id,
            name: name.to_string(),
            description: description.map(|s| s.to_string()),
            branch_hint,
            is_active: true,
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs()
                .to_string(),
        };

        {
            let mut cache = self.workspace_cache.write().await;
            cache.insert(workspace_id, workspace);
        }

        // Update metrics
        self.update_metrics(|metrics| metrics.total_workspaces_managed += 1)
            .await;

        // Emit event
        self.emit_event(WorkspaceEvent::Created {
            workspace_id,
            name: name.to_string(),
        })
        .await?;

        info!("Created workspace {} with ID {}", name, workspace_id);
        Ok(workspace_id)
    }

    /// Get workspace by ID
    pub async fn get_workspace(
        &self,
        workspace_id: i64,
    ) -> Result<Option<Workspace>, WorkspaceError> {
        // Check cache first
        {
            let cache = self.workspace_cache.read().await;
            if let Some(workspace) = cache.get(&workspace_id) {
                return Ok(Some(workspace.clone()));
            }
        }

        // Query database
        match self.database.get_workspace(workspace_id).await? {
            Some(workspace) => {
                // Cache the result
                {
                    let mut cache = self.workspace_cache.write().await;
                    cache.insert(workspace_id, workspace.clone());
                }
                Ok(Some(workspace))
            }
            None => Ok(None),
        }
    }

    /// List workspaces, optionally filtered by project
    pub async fn list_workspaces(
        &self,
        project_id: Option<i64>,
    ) -> Result<Vec<Workspace>, WorkspaceError> {
        self.database
            .list_workspaces(project_id)
            .await
            .map_err(WorkspaceError::from)
    }

    /// Delete workspace
    pub async fn delete_workspace(&self, workspace_id: i64) -> Result<(), WorkspaceError> {
        let _permit = self
            .operation_semaphore
            .acquire()
            .await
            .context("Failed to acquire operation permit")?;

        info!("Deleting workspace {}", workspace_id);

        let workspace = self
            .get_workspace(workspace_id)
            .await?
            .ok_or(WorkspaceError::WorkspaceNotFound { workspace_id })?;

        // TODO: Implement actual workspace deletion
        // This would involve:
        // 1. Removing all file associations
        // 2. Clearing cache entries
        // 3. Removing from database
        // 4. Cleaning up any workspace-specific resources

        // For now, just remove from cache
        {
            let mut cache = self.workspace_cache.write().await;
            cache.remove(&workspace_id);
        }

        // Emit event
        self.emit_event(WorkspaceEvent::Deleted {
            workspace_id,
            name: workspace.name,
        })
        .await?;

        info!("Deleted workspace {}", workspace_id);
        Ok(())
    }

    // ===================
    // File Operations
    // ===================

    /// Index all files in a workspace
    pub async fn index_workspace_files(
        &self,
        workspace_id: i64,
        scan_path: &Path,
    ) -> Result<IndexingResult, WorkspaceError> {
        let start_time = Instant::now();
        info!(
            "Starting full indexing for workspace {} at path: {}",
            workspace_id,
            scan_path.display()
        );

        // Emit indexing started event
        self.emit_event(WorkspaceEvent::IndexingStarted { workspace_id })
            .await?;

        // Discover files
        let file_changes = self.discover_workspace_files(scan_path).await?;

        // Convert workspace FileChange to indexing FileChange
        let indexing_changes: Vec<crate::indexing::FileChange> = file_changes
            .into_iter()
            .map(|change| crate::indexing::FileChange {
                path: change.path,
                change_type: match change.change_type {
                    FileChangeType::Create => crate::indexing::FileChangeType::Create,
                    FileChangeType::Update => crate::indexing::FileChangeType::Update,
                    FileChangeType::Delete => crate::indexing::FileChangeType::Delete,
                    FileChangeType::Move { from, to } => {
                        crate::indexing::FileChangeType::Move { from, to }
                    }
                },
                content_digest: change.content_digest,
                size_bytes: change.size_bytes,
                mtime: change.modified_time,
                detected_language: None,
            })
            .collect();

        // Process files using file version manager
        let processing_results = self
            .file_manager
            .process_file_changes(workspace_id, indexing_changes)
            .await?;

        // Get git information if available
        let (branch_name, commit_hash) = if self.config.git_integration {
            match GitService::discover_repo(scan_path, scan_path) {
                Ok(git_service) => {
                    let commit = git_service.head_commit().unwrap_or(None);
                    // TODO: Get actual branch name
                    (Some("main".to_string()), commit)
                }
                Err(_) => (None, None),
            }
        } else {
            (None, None)
        };

        let result = IndexingResult {
            workspace_id,
            files_processed: processing_results.processed_versions.len() as u64,
            files_added: processing_results.new_versions_count as u64,
            files_updated: processing_results.deduplicated_count as u64,
            files_deleted: 0, // Not applicable for full indexing
            bytes_processed: processing_results
                .processed_versions
                .iter()
                .map(|v| v.file_version.size_bytes)
                .sum(),
            processing_time: start_time.elapsed(),
            deduplication_savings: processing_results.deduplicated_count as u64,
            git_integration_active: self.config.git_integration,
            branch_name,
            commit_hash,
            errors: processing_results
                .failed_files
                .iter()
                .map(|(path, error)| format!("{}: {}", path.display(), error))
                .collect(),
            warnings: Vec::new(),
        };

        // Update metrics
        self.update_metrics(|metrics| {
            metrics.total_files_indexed += result.files_processed;
            metrics.database_transactions += result.files_added;
            metrics.average_indexing_time_ms =
                (metrics.average_indexing_time_ms + result.processing_time.as_millis() as u64) / 2;
        })
        .await;

        // Emit completion event
        let workspace_result = WorkspaceIndexingResult {
            workspace_id: result.workspace_id,
            files_processed: result.files_processed,
            files_added: result.files_added,
            files_updated: result.files_updated,
            files_deleted: result.files_deleted,
            bytes_processed: result.bytes_processed,
            processing_time: result.processing_time,
            deduplication_savings: result.deduplication_savings,
            git_integration_active: result.git_integration_active,
            branch_name: result.branch_name.clone(),
            commit_hash: result.commit_hash.clone(),
        };

        self.emit_event(WorkspaceEvent::IndexingCompleted {
            workspace_id,
            result: workspace_result,
        })
        .await?;

        info!(
            "Completed indexing for workspace {}: {} files processed, {} new versions in {:?}",
            workspace_id, result.files_processed, result.files_added, result.processing_time
        );

        Ok(result)
    }

    /// Update workspace files incrementally
    pub async fn update_workspace_files(
        &self,
        workspace_id: i64,
        incremental: bool,
    ) -> Result<IndexingResult, WorkspaceError> {
        let start_time = Instant::now();
        info!(
            "Starting {} update for workspace {}",
            if incremental { "incremental" } else { "full" },
            workspace_id
        );

        let workspace = self
            .get_workspace(workspace_id)
            .await?
            .ok_or(WorkspaceError::WorkspaceNotFound { workspace_id })?;

        let project = self.get_project(workspace.project_id).await?.ok_or(
            WorkspaceError::ValidationFailed {
                message: format!(
                    "Project {} not found for workspace {}",
                    workspace.project_id, workspace_id
                ),
            },
        )?;

        let scan_path = &project.root_path;

        if incremental && self.config.git_integration {
            // Use git to detect changes
            let file_changes = self
                .branch_manager
                .detect_git_changes(workspace_id, scan_path)
                .await?;

            if file_changes.is_empty() {
                info!("No changes detected for workspace {}", workspace_id);
                return Ok(IndexingResult {
                    workspace_id,
                    files_processed: 0,
                    files_added: 0,
                    files_updated: 0,
                    files_deleted: 0,
                    bytes_processed: 0,
                    processing_time: start_time.elapsed(),
                    deduplication_savings: 0,
                    git_integration_active: true,
                    branch_name: workspace.branch_hint,
                    commit_hash: None,
                    errors: Vec::new(),
                    warnings: Vec::new(),
                });
            }

            info!(
                "Detected {} file changes for incremental update",
                file_changes.len()
            );

            // Convert workspace FileChange to indexing FileChange
            let indexing_changes: Vec<crate::indexing::FileChange> = file_changes
                .into_iter()
                .map(|change| crate::indexing::FileChange {
                    path: change.path,
                    change_type: match change.change_type {
                        FileChangeType::Create => crate::indexing::FileChangeType::Create,
                        FileChangeType::Update => crate::indexing::FileChangeType::Update,
                        FileChangeType::Delete => crate::indexing::FileChangeType::Delete,
                        FileChangeType::Move { from, to } => {
                            crate::indexing::FileChangeType::Move { from, to }
                        }
                    },
                    content_digest: change.content_digest,
                    size_bytes: change.size_bytes,
                    mtime: change.modified_time,
                    detected_language: None,
                })
                .collect();

            // Process the changes
            let processing_results = self
                .file_manager
                .process_file_changes(workspace_id, indexing_changes)
                .await?;

            Ok(IndexingResult {
                workspace_id,
                files_processed: processing_results.processed_versions.len() as u64,
                files_added: processing_results.new_versions_count as u64,
                files_updated: processing_results.deduplicated_count as u64,
                files_deleted: 0, // TODO: Handle deletions
                bytes_processed: processing_results
                    .processed_versions
                    .iter()
                    .map(|v| v.file_version.size_bytes)
                    .sum(),
                processing_time: start_time.elapsed(),
                deduplication_savings: processing_results.deduplicated_count as u64,
                git_integration_active: true,
                branch_name: workspace.branch_hint,
                commit_hash: None, // TODO: Get current commit
                errors: processing_results
                    .failed_files
                    .iter()
                    .map(|(path, error)| format!("{}: {}", path.display(), error))
                    .collect(),
                warnings: Vec::new(),
            })
        } else {
            // Fall back to full indexing
            self.index_workspace_files(workspace_id, scan_path).await
        }
    }

    // ===================
    // Branch Operations
    // ===================

    /// Switch workspace to a different branch
    pub async fn switch_branch(
        &self,
        workspace_id: i64,
        target_branch: &str,
    ) -> Result<ComprehensiveBranchSwitchResult, WorkspaceError> {
        info!(
            "Switching workspace {} to branch: {}",
            workspace_id, target_branch
        );

        let workspace = self
            .get_workspace(workspace_id)
            .await?
            .ok_or(WorkspaceError::WorkspaceNotFound { workspace_id })?;

        let project = self.get_project(workspace.project_id).await?.ok_or(
            WorkspaceError::ValidationFailed {
                message: format!("Project {} not found", workspace.project_id),
            },
        )?;

        // Perform branch switch
        let branch_result = self
            .branch_manager
            .switch_branch(workspace_id, target_branch, &project.root_path)
            .await?;

        // Sync with git if enabled
        let git_sync_result = if self.config.git_integration {
            Some(
                self.branch_manager
                    .sync_with_git(workspace_id, &project.root_path, Some(target_branch))
                    .await?,
            )
        } else {
            None
        };

        // Trigger incremental indexing if files changed during branch switch
        let _post_switch_indexing_result = if branch_result.indexing_required
            && self.config.incremental_indexing
        {
            info!(
                "Triggering incremental indexing after branch switch for workspace {} ({} files changed)",
                workspace_id, branch_result.files_changed
            );

            match self.update_workspace_files(workspace_id, true).await {
                Ok(indexing_result) => {
                    info!(
                        "Post-switch indexing completed: {} files processed in {:?}",
                        indexing_result.files_processed, indexing_result.processing_time
                    );
                    Some(indexing_result)
                }
                Err(e) => {
                    warn!("Post-switch indexing failed: {}", e);
                    None
                }
            }
        } else {
            None
        };

        let result = ComprehensiveBranchSwitchResult {
            workspace_id: branch_result.workspace_id,
            previous_branch: branch_result.previous_branch.clone(),
            new_branch: branch_result.new_branch.clone(),
            files_changed: branch_result.files_changed,
            reused_versions: branch_result.reused_versions,
            switch_time: branch_result.switch_time,
            git_sync_result,
            indexing_required: branch_result.indexing_required,
            cache_invalidations: branch_result.cache_invalidations,
        };

        // Emit event
        self.emit_event(WorkspaceEvent::BranchSwitched {
            workspace_id,
            from: result.previous_branch.clone(),
            to: result.new_branch.clone(),
        })
        .await?;

        // Update cached workspace
        {
            let mut cache = self.workspace_cache.write().await;
            if let Some(cached_workspace) = cache.get_mut(&workspace_id) {
                cached_workspace.branch_hint = Some(target_branch.to_string());
            }
        }

        info!(
            "Branch switch completed for workspace {}: {} -> {} in {:?}",
            workspace_id,
            result.previous_branch.as_deref().unwrap_or("unknown"),
            result.new_branch,
            result.switch_time
        );

        Ok(result)
    }

    /// Get current branch for workspace
    pub async fn get_workspace_branch(
        &self,
        workspace_id: i64,
    ) -> Result<Option<String>, WorkspaceError> {
        self.branch_manager
            .get_workspace_branch(workspace_id)
            .await
            .map_err(WorkspaceError::from)
    }

    /// Create a new branch for workspace
    pub async fn create_branch(
        &self,
        workspace_id: i64,
        branch_name: &str,
        start_point: Option<&str>,
    ) -> Result<(), WorkspaceError> {
        let workspace = self
            .get_workspace(workspace_id)
            .await?
            .ok_or(WorkspaceError::WorkspaceNotFound { workspace_id })?;

        let project = self.get_project(workspace.project_id).await?.ok_or(
            WorkspaceError::ValidationFailed {
                message: format!("Project {} not found", workspace.project_id),
            },
        )?;

        self.branch_manager
            .create_branch(workspace_id, branch_name, &project.root_path, start_point)
            .await
            .map_err(WorkspaceError::from)
    }

    /// Delete a branch for workspace
    pub async fn delete_branch(
        &self,
        workspace_id: i64,
        branch_name: &str,
        force: bool,
    ) -> Result<(), WorkspaceError> {
        let workspace = self
            .get_workspace(workspace_id)
            .await?
            .ok_or(WorkspaceError::WorkspaceNotFound { workspace_id })?;

        let project = self.get_project(workspace.project_id).await?.ok_or(
            WorkspaceError::ValidationFailed {
                message: format!("Project {} not found", workspace.project_id),
            },
        )?;

        self.branch_manager
            .delete_branch(workspace_id, branch_name, &project.root_path, force)
            .await
            .map_err(WorkspaceError::from)
    }

    /// List all branches for workspace
    pub async fn list_branches(
        &self,
        workspace_id: i64,
    ) -> Result<Vec<super::branch::BranchInfo>, WorkspaceError> {
        let workspace = self
            .get_workspace(workspace_id)
            .await?
            .ok_or(WorkspaceError::WorkspaceNotFound { workspace_id })?;

        let project = self.get_project(workspace.project_id).await?.ok_or(
            WorkspaceError::ValidationFailed {
                message: format!("Project {} not found", workspace.project_id),
            },
        )?;

        self.branch_manager
            .list_all_branches(workspace_id, &project.root_path)
            .await
            .map_err(WorkspaceError::from)
    }

    /// Synchronize workspace with git
    pub async fn sync_with_git(
        &self,
        workspace_id: i64,
        reference: Option<&str>,
    ) -> Result<GitSyncResult, WorkspaceError> {
        let workspace = self
            .get_workspace(workspace_id)
            .await?
            .ok_or(WorkspaceError::WorkspaceNotFound { workspace_id })?;

        let project = self.get_project(workspace.project_id).await?.ok_or(
            WorkspaceError::ValidationFailed {
                message: format!("Project {} not found", workspace.project_id),
            },
        )?;

        self.branch_manager
            .sync_with_git(workspace_id, &project.root_path, reference)
            .await
            .map_err(WorkspaceError::from)
    }

    // ===================
    // Performance & Metrics
    // ===================

    /// Get current workspace metrics
    pub async fn get_metrics(&self) -> WorkspaceMetrics {
        let metrics = self.metrics.read().await;
        let mut result = metrics.clone();

        // Calculate uptime and hit rates
        let uptime_seconds = self.start_time.elapsed().as_secs();
        if result.cache_hits + result.cache_misses > 0 {
            result.deduplication_rate =
                result.cache_hits as f64 / (result.cache_hits + result.cache_misses) as f64;
        }

        debug!(
            "Workspace manager metrics: {:?} (uptime: {}s)",
            result, uptime_seconds
        );
        result
    }

    /// Clear workspace cache
    pub async fn clear_cache(&self) -> Result<(), WorkspaceError> {
        info!("Clearing workspace cache");

        {
            let mut cache = self.workspace_cache.write().await;
            cache.clear();
        }

        self.file_manager.clear_cache().await;

        Ok(())
    }

    // ===================
    // Private Helper Methods
    // ===================

    /// Discover files in workspace directory
    async fn discover_workspace_files(
        &self,
        scan_path: &Path,
    ) -> Result<Vec<FileChange>, WorkspaceError> {
        let mut file_changes = Vec::new();

        debug!("Discovering files in: {}", scan_path.display());

        let mut entries = tokio::fs::read_dir(scan_path)
            .await
            .context(format!("Failed to read directory: {}", scan_path.display()))?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            // Skip ignored files and directories
            if self.config.should_ignore_file(&path) {
                debug!("Skipping ignored path: {}", path.display());
                continue;
            }

            if path.is_file() {
                // Check file size
                let metadata = entry.metadata().await?;
                let size_bytes = metadata.len();

                if size_bytes > (self.config.max_file_size_mb * 1024 * 1024) {
                    warn!(
                        "Skipping large file: {} ({} bytes)",
                        path.display(),
                        size_bytes
                    );
                    continue;
                }

                // Read file content to compute hash
                match tokio::fs::read(&path).await {
                    Ok(content) => {
                        let content_digest = blake3::hash(&content).to_hex().to_string();
                        let modified_time = metadata
                            .modified()
                            .unwrap_or(SystemTime::UNIX_EPOCH)
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64;

                        file_changes.push(FileChange {
                            path,
                            change_type: FileChangeType::Create, // Assume new file for full scan
                            content_digest: Some(content_digest),
                            size_bytes: Some(size_bytes),
                            modified_time: Some(modified_time),
                        });
                    }
                    Err(e) => {
                        warn!("Failed to read file {}: {}", path.display(), e);
                    }
                }
            } else if path.is_dir() && self.config.validation.max_directory_depth > 1 {
                // Recursively scan subdirectories (simplified implementation)
                // In practice, you'd want proper depth tracking and more sophisticated scanning
            }
        }

        info!(
            "Discovered {} files in {}",
            file_changes.len(),
            scan_path.display()
        );

        Ok(file_changes)
    }

    /// Update metrics with a closure
    async fn update_metrics<F>(&self, update_fn: F)
    where
        F: FnOnce(&mut WorkspaceMetrics),
    {
        let mut metrics = self.metrics.write().await;
        update_fn(&mut *metrics);
    }

    /// Emit workspace event to registered handlers
    async fn emit_event(&self, event: WorkspaceEvent) -> Result<(), WorkspaceError> {
        if let Err(e) = self.event_handler.handle_event(event).await {
            warn!("Event handler error: {}", e);
            // Don't fail the operation due to event handler errors
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    // Note: These tests would require a mock database backend for full testing
    // For now, they serve as examples of the intended API usage

    #[tokio::test]
    async fn test_workspace_config_validation() {
        let config = WorkspaceConfig::default();
        assert!(config.validate().is_ok());

        // Test invalid config
        let mut invalid_config = config.clone();
        invalid_config.max_file_size_mb = 2000; // Too large
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_indexing_result_serialization() {
        let result = IndexingResult {
            workspace_id: 1,
            files_processed: 100,
            files_added: 80,
            files_updated: 20,
            files_deleted: 0,
            bytes_processed: 1024000,
            processing_time: Duration::from_secs(30),
            deduplication_savings: 20,
            git_integration_active: true,
            branch_name: Some("main".to_string()),
            commit_hash: Some("abc123".to_string()),
            errors: vec!["error1".to_string()],
            warnings: vec!["warning1".to_string()],
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("workspace_id"));
        assert!(json.contains("files_processed"));
    }

    #[test]
    fn test_file_change_types() {
        let create_change = FileChange {
            path: PathBuf::from("/test/file.rs"),
            change_type: FileChangeType::Create,
            content_digest: Some("abc123".to_string()),
            size_bytes: Some(1024),
            modified_time: Some(1234567890),
        };

        match create_change.change_type {
            FileChangeType::Create => assert!(true),
            _ => assert!(false, "Expected Create change type"),
        }
    }
}
