//! Branch Management Module
//!
//! Provides git-aware branch management with workspace synchronization,
//! file change detection, and cache management for branch switching operations.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use crate::database::{DatabaseBackend, DatabaseError};
use crate::git_service::{GitService, GitServiceError};
use crate::indexing::versioning::{FileVersionManager, VersioningError};

/// Result of branch switching operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchSwitchResult {
    pub workspace_id: i64,
    pub previous_branch: Option<String>,
    pub new_branch: String,
    pub files_changed: u64,
    pub reused_versions: u64,
    pub switch_time: Duration,
    pub cache_invalidations: u64,
    pub indexing_required: bool,
}

/// Result of git synchronization operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitSyncResult {
    pub workspace_id: i64,
    pub previous_commit: Option<String>,
    pub new_commit: String,
    pub files_modified: Vec<String>,
    pub files_added: Vec<String>,
    pub files_deleted: Vec<String>,
    pub sync_time: Duration,
    pub conflicts_detected: Vec<String>,
}

/// Branch information with workspace association
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchInfo {
    pub branch_name: String,
    pub commit_hash: Option<String>,
    pub last_updated: String,
    pub workspace_id: i64,
    pub is_current: bool,
    pub file_count: u64,
    pub indexed_files: u64,
    pub cache_entries: u64,
}

/// Branch management errors
#[derive(Debug, thiserror::Error)]
pub enum BranchError {
    #[error("Branch not found: {branch_name}")]
    BranchNotFound { branch_name: String },

    #[error("Invalid branch name: {branch_name} - {reason}")]
    InvalidBranchName { branch_name: String, reason: String },

    #[error("Branch switch failed: {from} -> {to} - {reason}")]
    BranchSwitchFailed {
        from: String,
        to: String,
        reason: String,
    },

    #[error("Git synchronization failed: {reason}")]
    GitSyncFailed { reason: String },

    #[error("Working directory has uncommitted changes")]
    UncommittedChanges,

    #[error("Branch conflicts detected: {conflicts:?}")]
    BranchConflicts { conflicts: Vec<String> },

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

    #[error("Context error: {source}")]
    Context {
        #[from]
        source: anyhow::Error,
    },
}

/// Branch manager for git-aware workspace operations
pub struct BranchManager<T>
where
    T: DatabaseBackend + Send + Sync + 'static,
{
    database: Arc<T>,
    file_manager: FileVersionManager<T>,
    git_integration_enabled: bool,
    branch_cache: Arc<Mutex<HashMap<i64, BranchInfo>>>,
}

impl<T> BranchManager<T>
where
    T: DatabaseBackend + Send + Sync + 'static,
{
    /// Create a new branch manager
    pub async fn new(
        database: Arc<T>,
        file_manager: FileVersionManager<T>,
        git_integration_enabled: bool,
    ) -> Result<Self, BranchError> {
        info!(
            "BranchManager initialized with git_integration={}",
            git_integration_enabled
        );

        Ok(Self {
            database,
            file_manager,
            git_integration_enabled,
            branch_cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Switch workspace to a different branch
    pub async fn switch_branch(
        &self,
        workspace_id: i64,
        target_branch: &str,
        workspace_root: &Path,
    ) -> Result<BranchSwitchResult, BranchError> {
        if !self.git_integration_enabled {
            return Err(BranchError::GitSyncFailed {
                reason: "Git integration is disabled".to_string(),
            });
        }

        let start_time = Instant::now();
        info!(
            "Switching workspace {} to branch: {}",
            workspace_id, target_branch
        );

        // Validate branch name
        self.validate_branch_name(target_branch)?;

        // Initialize git service
        let mut git_service = GitService::discover_repo(workspace_root, workspace_root)
            .context("Failed to discover git repository")?;

        // Get current branch from git (more reliable than database)
        let current_branch = git_service.current_branch()?;

        // Skip if already on target branch
        if let Some(ref current) = current_branch {
            if current == target_branch {
                info!(
                    "Workspace {} already on branch {}",
                    workspace_id, target_branch
                );
                return Ok(BranchSwitchResult {
                    workspace_id,
                    previous_branch: current_branch,
                    new_branch: target_branch.to_string(),
                    files_changed: 0,
                    reused_versions: 0,
                    switch_time: start_time.elapsed(),
                    cache_invalidations: 0,
                    indexing_required: false,
                });
            }
        }

        // Check for uncommitted changes
        let modified_files = git_service.modified_files()?;
        if !modified_files.is_empty() {
            warn!(
                "Workspace {} has {} uncommitted changes: {:?}",
                workspace_id,
                modified_files.len(),
                modified_files.iter().take(5).collect::<Vec<_>>()
            );

            // For production, we might want to stash changes automatically
            // For now, return an error to prevent data loss
            return Err(BranchError::UncommittedChanges);
        }

        // Check if target branch exists
        if !git_service.branch_exists(target_branch)? {
            return Err(BranchError::BranchNotFound {
                branch_name: target_branch.to_string(),
            });
        }

        // Get files that will change between branches
        let changed_files = if let Some(ref current) = current_branch {
            git_service.files_changed_between(current, Some(target_branch))?
        } else {
            // Detached HEAD or unborn repo - get all files in target branch
            warn!("Current branch is unknown, assuming all files may change");
            Vec::new()
        };

        debug!("Branch switch will affect {} files", changed_files.len());

        // Invalidate affected cache entries before checkout
        let cache_invalidations = self
            .invalidate_branch_cache(workspace_id, &changed_files)
            .await?;

        // Perform actual git checkout
        info!("Performing git checkout to branch: {}", target_branch);
        git_service
            .checkout(target_branch, false)
            .map_err(|e| match e {
                GitServiceError::BranchNotFound { branch } => BranchError::BranchNotFound {
                    branch_name: branch,
                },
                GitServiceError::DirtyWorkingDirectory { files } => {
                    BranchError::BranchConflicts { conflicts: files }
                }
                GitServiceError::CheckoutFailed { reason } => BranchError::BranchSwitchFailed {
                    from: current_branch.as_deref().unwrap_or("unknown").to_string(),
                    to: target_branch.to_string(),
                    reason,
                },
                _ => BranchError::GitService { source: e },
            })?;

        // Update workspace branch in database after successful checkout
        self.database
            .update_workspace_branch(workspace_id, target_branch)
            .await
            .context("Failed to update workspace branch")?;

        // Update branch cache with new branch information
        self.update_branch_cache(workspace_id, target_branch, &git_service)
            .await?;

        // Determine if reindexing is required
        let indexing_required = !changed_files.is_empty();
        let files_changed = changed_files.len() as u64;

        // Calculate reused versions (approximation based on file changes)
        let reused_versions = if indexing_required {
            // Some files changed, so we'll need to reprocess
            0
        } else {
            // No files changed, all versions can be reused
            files_changed
        };

        let result = BranchSwitchResult {
            workspace_id,
            previous_branch: current_branch,
            new_branch: target_branch.to_string(),
            files_changed,
            reused_versions,
            switch_time: start_time.elapsed(),
            cache_invalidations,
            indexing_required,
        };

        info!(
            "Completed branch switch for workspace {} in {:?}: {} files changed, indexing {}",
            workspace_id,
            result.switch_time,
            result.files_changed,
            if result.indexing_required {
                "required"
            } else {
                "not required"
            }
        );

        Ok(result)
    }

    /// Get the current branch for a workspace
    pub async fn get_workspace_branch(
        &self,
        workspace_id: i64,
    ) -> Result<Option<String>, BranchError> {
        debug!("Getting current branch for workspace {}", workspace_id);

        // Check cache first
        {
            let cache = self.branch_cache.lock().await;
            if let Some(branch_info) = cache.get(&workspace_id) {
                if branch_info.is_current {
                    return Ok(Some(branch_info.branch_name.clone()));
                }
            }
        }

        // Query database
        match self.database.get_workspace(workspace_id).await? {
            Some(workspace) => Ok(workspace.branch_hint),
            None => Ok(None),
        }
    }

    /// Synchronize workspace with git repository
    pub async fn sync_with_git(
        &self,
        workspace_id: i64,
        workspace_root: &Path,
        git_ref: Option<&str>,
    ) -> Result<GitSyncResult, BranchError> {
        if !self.git_integration_enabled {
            return Err(BranchError::GitSyncFailed {
                reason: "Git integration is disabled".to_string(),
            });
        }

        let start_time = Instant::now();
        info!(
            "Synchronizing workspace {} with git (ref: {:?})",
            workspace_id, git_ref
        );

        // Initialize git service
        let git_service = GitService::discover_repo(workspace_root, workspace_root)
            .context("Failed to discover git repository")?;

        // Get current commit
        let previous_commit = git_service.head_commit()?;

        // Get current branch if no specific ref provided
        let current_branch = if git_ref.is_none() {
            self.get_workspace_branch(workspace_id).await?
        } else {
            None
        };

        let reference = git_ref.unwrap_or_else(|| current_branch.as_deref().unwrap_or("HEAD"));

        // Get file changes
        let _modified_files = git_service.modified_files()?;
        let changed_files = if let Some(ref prev_commit) = previous_commit {
            git_service.files_changed_between(prev_commit, Some(reference))?
        } else {
            Vec::new()
        };

        // Get current commit after potential changes
        let new_commit = git_service
            .head_commit()?
            .unwrap_or_else(|| "unknown".to_string());

        // Categorize file changes
        let (files_added, files_modified, files_deleted) = self
            .categorize_file_changes(&changed_files, workspace_root)
            .await?;

        // Detect conflicts (simplified)
        let conflicts_detected = Vec::new(); // TODO: Implement conflict detection

        let result = GitSyncResult {
            workspace_id,
            previous_commit,
            new_commit: new_commit.clone(),
            files_modified: files_modified.clone(),
            files_added: files_added.clone(),
            files_deleted: files_deleted.clone(),
            sync_time: start_time.elapsed(),
            conflicts_detected,
        };

        // Update workspace with new commit information
        if let Some(workspace) = self.database.get_workspace(workspace_id).await? {
            // Update branch information in cache
            self.update_branch_cache_with_commit(workspace_id, &workspace.branch_hint, &new_commit)
                .await?;
        }

        info!(
            "Git sync completed for workspace {} in {:?}: {} modified, {} added, {} deleted",
            workspace_id,
            result.sync_time,
            files_modified.len(),
            files_added.len(),
            files_deleted.len()
        );

        Ok(result)
    }

    /// Get git file list for a specific reference
    pub async fn get_git_file_list(
        &self,
        workspace_id: i64,
        workspace_root: &Path,
        git_ref: &str,
    ) -> Result<Vec<PathBuf>, BranchError> {
        if !self.git_integration_enabled {
            return Ok(Vec::new());
        }

        debug!(
            "Getting file list for workspace {} at ref {}",
            workspace_id, git_ref
        );

        let git_service = GitService::discover_repo(workspace_root, workspace_root)
            .context("Failed to discover git repository")?;

        // TODO: Implement actual git tree traversal when GitService supports it
        // For now, return an empty list as placeholder
        let _head_commit = git_service.head_commit()?;

        // This would typically involve:
        // 1. Resolve git_ref to a commit
        // 2. Get the tree object for that commit
        // 3. Recursively traverse the tree to get all file paths
        // 4. Convert git paths to filesystem paths

        Ok(Vec::new()) // Placeholder implementation
    }

    /// Detect git changes since last sync
    pub async fn detect_git_changes(
        &self,
        workspace_id: i64,
        workspace_root: &Path,
    ) -> Result<Vec<super::FileChange>, BranchError> {
        if !self.git_integration_enabled {
            return Ok(Vec::new());
        }

        debug!("Detecting git changes for workspace {}", workspace_id);

        let git_service = GitService::discover_repo(workspace_root, workspace_root)
            .context("Failed to discover git repository")?;

        // Get modified files from git
        let _modified_files = git_service.modified_files()?;

        let mut changes = Vec::new();

        for file_path in _modified_files {
            let full_path = workspace_root.join(&file_path);

            // Determine change type
            let change_type = if full_path.exists() {
                // File exists - could be create or update
                // TODO: Check git status to determine if it's new or modified
                super::FileChangeType::Update
            } else {
                super::FileChangeType::Delete
            };

            // Get file metadata if it exists
            let (content_digest, size_bytes, modified_time) = if full_path.exists() {
                match tokio::fs::read(&full_path).await {
                    Ok(content) => {
                        let digest = blake3::hash(&content).to_hex().to_string();
                        let metadata = tokio::fs::metadata(&full_path)
                            .await
                            .context("Failed to get file metadata")?;
                        let mtime = metadata
                            .modified()
                            .context("Failed to get modification time")?
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64;

                        (Some(digest), Some(content.len() as u64), Some(mtime))
                    }
                    Err(_) => (None, None, None),
                }
            } else {
                (None, None, None)
            };

            changes.push(super::FileChange {
                path: full_path,
                change_type,
                content_digest,
                size_bytes,
                modified_time,
            });
        }

        info!(
            "Detected {} git changes for workspace {}",
            changes.len(),
            workspace_id
        );
        Ok(changes)
    }

    /// List branches tracked for a workspace
    pub async fn list_workspace_branches(
        &self,
        workspace_id: i64,
    ) -> Result<Vec<BranchInfo>, BranchError> {
        debug!("Listing branches for workspace {}", workspace_id);

        let cache = self.branch_cache.lock().await;
        let branches: Vec<BranchInfo> = cache
            .values()
            .filter(|branch| branch.workspace_id == workspace_id)
            .cloned()
            .collect();

        Ok(branches)
    }

    /// Clear branch cache for workspace
    pub async fn clear_branch_cache(&self, workspace_id: i64) -> Result<(), BranchError> {
        debug!("Clearing branch cache for workspace {}", workspace_id);

        let mut cache = self.branch_cache.lock().await;
        cache.retain(|&id, _| id != workspace_id);

        Ok(())
    }

    /// Create a new branch from current HEAD or specified commit
    pub async fn create_branch(
        &self,
        workspace_id: i64,
        branch_name: &str,
        workspace_root: &Path,
        start_point: Option<&str>,
    ) -> Result<(), BranchError> {
        if !self.git_integration_enabled {
            return Err(BranchError::GitSyncFailed {
                reason: "Git integration is disabled".to_string(),
            });
        }

        info!(
            "Creating branch '{}' for workspace {} (start_point: {:?})",
            branch_name, workspace_id, start_point
        );

        // Validate branch name
        self.validate_branch_name(branch_name)?;

        // Initialize git service
        let git_service = GitService::discover_repo(workspace_root, workspace_root)
            .context("Failed to discover git repository")?;

        // Create the branch
        git_service.create_branch(branch_name, start_point)?;

        // Update branch cache
        self.update_branch_cache_for_creation(workspace_id, branch_name, &git_service)
            .await?;

        info!("Successfully created branch: {}", branch_name);
        Ok(())
    }

    /// Delete a branch (cannot be the current branch)
    pub async fn delete_branch(
        &self,
        workspace_id: i64,
        branch_name: &str,
        workspace_root: &Path,
        force: bool,
    ) -> Result<(), BranchError> {
        if !self.git_integration_enabled {
            return Err(BranchError::GitSyncFailed {
                reason: "Git integration is disabled".to_string(),
            });
        }

        info!(
            "Deleting branch '{}' for workspace {} (force: {})",
            branch_name, workspace_id, force
        );

        // Validate branch name
        self.validate_branch_name(branch_name)?;

        // Initialize git service
        let git_service = GitService::discover_repo(workspace_root, workspace_root)
            .context("Failed to discover git repository")?;

        // Check if it's the current branch
        if let Ok(Some(current)) = git_service.current_branch() {
            if current == branch_name {
                return Err(BranchError::BranchSwitchFailed {
                    from: current,
                    to: "N/A".to_string(),
                    reason: "Cannot delete current branch".to_string(),
                });
            }
        }

        // Delete the branch
        git_service.delete_branch(branch_name, force)?;

        // Remove from cache
        self.remove_branch_from_cache(workspace_id, branch_name)
            .await?;

        info!("Successfully deleted branch: {}", branch_name);
        Ok(())
    }

    /// List all branches for a workspace
    pub async fn list_all_branches(
        &self,
        workspace_id: i64,
        workspace_root: &Path,
    ) -> Result<Vec<BranchInfo>, BranchError> {
        if !self.git_integration_enabled {
            return Ok(Vec::new());
        }

        debug!("Listing all branches for workspace {}", workspace_id);

        // Initialize git service
        let git_service = GitService::discover_repo(workspace_root, workspace_root)
            .context("Failed to discover git repository")?;

        // Get current branch to mark it as current
        let current_branch = git_service.current_branch().unwrap_or(None);

        // Get all branches from git
        let git_branches = git_service.list_branches()?;

        let mut branch_infos = Vec::new();
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();

        for (branch_name, commit_hash) in git_branches {
            let is_current = current_branch.as_deref() == Some(&branch_name);

            let branch_info = BranchInfo {
                branch_name: branch_name.clone(),
                commit_hash,
                last_updated: current_time.clone(),
                workspace_id,
                is_current,
                file_count: 0,    // TODO: Calculate actual file count from git
                indexed_files: 0, // TODO: Get from database
                cache_entries: 0, // TODO: Get from cache system
            };

            branch_infos.push(branch_info);
        }

        // Update cache with fresh information
        self.update_branch_cache_bulk(workspace_id, &branch_infos)
            .await?;

        Ok(branch_infos)
    }

    // Private helper methods

    /// Validate branch name format
    fn validate_branch_name(&self, branch_name: &str) -> Result<(), BranchError> {
        if branch_name.is_empty() {
            return Err(BranchError::InvalidBranchName {
                branch_name: branch_name.to_string(),
                reason: "Branch name cannot be empty".to_string(),
            });
        }

        if branch_name.len() > 100 {
            return Err(BranchError::InvalidBranchName {
                branch_name: branch_name.to_string(),
                reason: "Branch name too long (max 100 characters)".to_string(),
            });
        }

        // Check for invalid characters
        if branch_name.contains("..") || branch_name.starts_with('/') || branch_name.ends_with('/')
        {
            return Err(BranchError::InvalidBranchName {
                branch_name: branch_name.to_string(),
                reason: "Invalid branch name format".to_string(),
            });
        }

        Ok(())
    }

    /// Invalidate cache entries affected by branch switch
    async fn invalidate_branch_cache(
        &self,
        workspace_id: i64,
        changed_files: &[String],
    ) -> Result<u64, BranchError> {
        debug!(
            "Invalidating cache for {} changed files in workspace {}",
            changed_files.len(),
            workspace_id
        );

        let mut invalidation_count = 0u64;

        // 1. Invalidate file-specific cache entries
        for file_path in changed_files {
            // Invalidate file version cache entries
            // Clear the entire file cache since we don't have per-file invalidation
            // This is less efficient but ensures consistency
            self.file_manager.clear_cache().await;
            invalidation_count += 1; // Count the cache clear operation
            debug!("Cleared file cache due to changes in: {}", file_path);
        }

        // 2. Invalidate workspace-level cache entries that depend on branch
        // This includes:
        // - Symbol index cache
        // - Cross-reference cache
        // - Dependency analysis cache
        // - Search index cache
        // Note: DatabaseBackend doesn't have clear_workspace_cache method
        // This would need to be implemented if workspace-specific cache clearing is needed
        debug!("Workspace-level cache clearing not implemented in DatabaseBackend");
        // TODO: Implement workspace-specific cache clearing in DatabaseBackend trait

        // 3. Clear branch-specific cache entries from our local cache
        {
            let mut cache = self.branch_cache.lock().await;
            let initial_len = cache.len();
            cache.retain(|_, branch_info| {
                // Keep entries for other workspaces, but invalidate this workspace's cache
                if branch_info.workspace_id == workspace_id {
                    // Mark as needing refresh rather than removing completely
                    false
                } else {
                    true
                }
            });
            let removed_count = initial_len - cache.len();
            invalidation_count += removed_count as u64;
            debug!(
                "Removed {} branch cache entries for workspace {}",
                removed_count, workspace_id
            );
        }

        // 4. If we have access to the universal cache system, invalidate entries there
        // TODO: When universal cache integration is available, add:
        // - LSP cache invalidation for affected files
        // - Symbol cache invalidation
        // - Cross-reference cache invalidation

        info!(
            "Invalidated {} total cache entries for workspace {} branch switch",
            invalidation_count, workspace_id
        );

        Ok(invalidation_count)
    }

    /// Update branch cache with current information
    async fn update_branch_cache(
        &self,
        workspace_id: i64,
        branch_name: &str,
        git_service: &GitService,
    ) -> Result<(), BranchError> {
        let commit_hash = git_service.head_commit()?;
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();

        let branch_info = BranchInfo {
            branch_name: branch_name.to_string(),
            commit_hash,
            last_updated: current_time,
            workspace_id,
            is_current: true,
            file_count: 0,    // TODO: Calculate actual file count
            indexed_files: 0, // TODO: Calculate indexed files
            cache_entries: 0, // TODO: Calculate cache entries
        };

        let mut cache = self.branch_cache.lock().await;

        // Mark other branches for this workspace as not current
        for (_, branch) in cache.iter_mut() {
            if branch.workspace_id == workspace_id {
                branch.is_current = false;
            }
        }

        // Insert/update the current branch
        cache.insert(workspace_id, branch_info);

        Ok(())
    }

    /// Update branch cache with commit information
    async fn update_branch_cache_with_commit(
        &self,
        workspace_id: i64,
        branch_hint: &Option<String>,
        commit_hash: &str,
    ) -> Result<(), BranchError> {
        let mut cache = self.branch_cache.lock().await;

        if let Some(branch_info) = cache.get_mut(&workspace_id) {
            branch_info.commit_hash = Some(commit_hash.to_string());
            branch_info.last_updated = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                .to_string();
        } else if let Some(branch_name) = branch_hint {
            // Create new cache entry
            let branch_info = BranchInfo {
                branch_name: branch_name.clone(),
                commit_hash: Some(commit_hash.to_string()),
                last_updated: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    .to_string(),
                workspace_id,
                is_current: true,
                file_count: 0,
                indexed_files: 0,
                cache_entries: 0,
            };

            cache.insert(workspace_id, branch_info);
        }

        Ok(())
    }

    /// Categorize file changes into added, modified, and deleted
    async fn categorize_file_changes(
        &self,
        changed_files: &[String],
        workspace_root: &Path,
    ) -> Result<(Vec<String>, Vec<String>, Vec<String>), BranchError> {
        let files_added = Vec::new();
        let mut files_modified = Vec::new();
        let mut files_deleted = Vec::new();

        for file_path in changed_files {
            let full_path = workspace_root.join(file_path);

            if full_path.exists() {
                // TODO: Check if this is a new file or modified file
                // For now, assume all existing files are modified
                files_modified.push(file_path.clone());
            } else {
                files_deleted.push(file_path.clone());
            }
        }

        Ok((files_added, files_modified, files_deleted))
    }

    /// Update branch cache for newly created branch
    async fn update_branch_cache_for_creation(
        &self,
        workspace_id: i64,
        branch_name: &str,
        git_service: &GitService,
    ) -> Result<(), BranchError> {
        let commit_hash = git_service.head_commit()?;
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .to_string();

        let branch_info = BranchInfo {
            branch_name: branch_name.to_string(),
            commit_hash,
            last_updated: current_time,
            workspace_id,
            is_current: false, // New branch is not current unless we switch to it
            file_count: 0,
            indexed_files: 0,
            cache_entries: 0,
        };

        let mut cache = self.branch_cache.lock().await;

        // Use a unique key based on workspace and branch name
        let cache_key = workspace_id * 1000 + branch_name.len() as i64;
        cache.insert(cache_key, branch_info);

        Ok(())
    }

    /// Remove branch from cache
    async fn remove_branch_from_cache(
        &self,
        workspace_id: i64,
        branch_name: &str,
    ) -> Result<(), BranchError> {
        let mut cache = self.branch_cache.lock().await;

        // Find and remove the branch entry
        cache.retain(|_, branch_info| {
            !(branch_info.workspace_id == workspace_id && branch_info.branch_name == branch_name)
        });

        Ok(())
    }

    /// Update branch cache with multiple branches
    async fn update_branch_cache_bulk(
        &self,
        workspace_id: i64,
        branch_infos: &[BranchInfo],
    ) -> Result<(), BranchError> {
        let mut cache = self.branch_cache.lock().await;

        // Remove existing entries for this workspace
        cache.retain(|_, branch_info| branch_info.workspace_id != workspace_id);

        // Add new entries
        for (index, branch_info) in branch_infos.iter().enumerate() {
            let cache_key = workspace_id * 1000 + index as i64;
            cache.insert(cache_key, branch_info.clone());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    async fn create_mock_branch_manager(
    ) -> BranchManager<crate::database::sqlite_backend::SQLiteBackend> {
        use crate::database::{DatabaseBackend, DatabaseConfig, SQLiteBackend};
        use crate::indexing::versioning::FileVersionManager;

        // Create a temporary in-memory SQLite database for testing
        let config = DatabaseConfig {
            path: None,
            temporary: true,
            compression: false,
            cache_capacity: 1024 * 1024, // 1MB
            compression_factor: 0,
            flush_every_ms: None,
        };

        let database = Arc::new(
            SQLiteBackend::new(config)
                .await
                .expect("Failed to create test database"),
        );
        let file_manager = FileVersionManager::new(database.clone(), Default::default())
            .await
            .expect("Failed to create file version manager");

        BranchManager {
            database,
            file_manager,
            git_integration_enabled: true,
            branch_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    #[tokio::test]
    async fn test_branch_name_validation() {
        let manager = create_mock_branch_manager().await;

        // Valid branch names
        assert!(manager.validate_branch_name("main").is_ok());
        assert!(manager.validate_branch_name("feature/new-feature").is_ok());
        assert!(manager.validate_branch_name("hotfix-123").is_ok());
        assert!(manager.validate_branch_name("release/v1.0.0").is_ok());
        assert!(manager.validate_branch_name("bugfix/issue-42").is_ok());

        // Invalid branch names
        assert!(manager.validate_branch_name("").is_err());
        assert!(manager.validate_branch_name("branch..name").is_err());
        assert!(manager.validate_branch_name("/invalid").is_err());
        assert!(manager.validate_branch_name("invalid/").is_err());

        // Too long branch name
        let long_name = "a".repeat(101);
        assert!(manager.validate_branch_name(&long_name).is_err());

        // Edge cases
        assert!(manager.validate_branch_name("a").is_ok()); // Single character
        assert!(manager.validate_branch_name("feature/sub/branch").is_ok()); // Nested
        assert!(manager.validate_branch_name("123-numeric-start").is_ok()); // Numeric start
    }

    #[test]
    fn test_branch_info_serialization() {
        let branch_info = BranchInfo {
            branch_name: "main".to_string(),
            commit_hash: Some("abc123".to_string()),
            last_updated: "1234567890".to_string(),
            workspace_id: 1,
            is_current: true,
            file_count: 100,
            indexed_files: 95,
            cache_entries: 50,
        };

        let serialized = serde_json::to_string(&branch_info).unwrap();
        let deserialized: BranchInfo = serde_json::from_str(&serialized).unwrap();

        assert_eq!(branch_info.branch_name, deserialized.branch_name);
        assert_eq!(branch_info.commit_hash, deserialized.commit_hash);
        assert_eq!(branch_info.workspace_id, deserialized.workspace_id);
        assert_eq!(branch_info.is_current, deserialized.is_current);
        assert_eq!(branch_info.file_count, deserialized.file_count);
        assert_eq!(branch_info.indexed_files, deserialized.indexed_files);
        assert_eq!(branch_info.cache_entries, deserialized.cache_entries);
    }

    #[test]
    fn test_branch_info_edge_cases() {
        // Test with None commit hash
        let branch_info = BranchInfo {
            branch_name: "detached".to_string(),
            commit_hash: None,
            last_updated: "0".to_string(),
            workspace_id: 0,
            is_current: false,
            file_count: 0,
            indexed_files: 0,
            cache_entries: 0,
        };

        let serialized = serde_json::to_string(&branch_info).unwrap();
        let deserialized: BranchInfo = serde_json::from_str(&serialized).unwrap();

        assert_eq!(branch_info.commit_hash, deserialized.commit_hash);
        assert_eq!(None, deserialized.commit_hash);
    }

    #[test]
    fn test_branch_switch_result_serialization() {
        let result = BranchSwitchResult {
            workspace_id: 1,
            previous_branch: Some("main".to_string()),
            new_branch: "feature/test".to_string(),
            files_changed: 5,
            reused_versions: 10,
            switch_time: Duration::from_millis(250),
            cache_invalidations: 15,
            indexing_required: true,
        };

        let serialized = serde_json::to_string(&result).unwrap();
        let deserialized: BranchSwitchResult = serde_json::from_str(&serialized).unwrap();

        assert_eq!(result.workspace_id, deserialized.workspace_id);
        assert_eq!(result.previous_branch, deserialized.previous_branch);
        assert_eq!(result.new_branch, deserialized.new_branch);
        assert_eq!(result.files_changed, deserialized.files_changed);
        assert_eq!(result.indexing_required, deserialized.indexing_required);
    }

    #[test]
    fn test_git_sync_result_serialization() {
        let result = GitSyncResult {
            workspace_id: 1,
            previous_commit: Some("abc123".to_string()),
            new_commit: "def456".to_string(),
            files_modified: vec!["file1.txt".to_string(), "file2.txt".to_string()],
            files_added: vec!["file3.txt".to_string()],
            files_deleted: vec!["old_file.txt".to_string()],
            sync_time: Duration::from_millis(100),
            conflicts_detected: Vec::new(),
        };

        let serialized = serde_json::to_string(&result).unwrap();
        let deserialized: GitSyncResult = serde_json::from_str(&serialized).unwrap();

        assert_eq!(result.workspace_id, deserialized.workspace_id);
        assert_eq!(result.previous_commit, deserialized.previous_commit);
        assert_eq!(result.new_commit, deserialized.new_commit);
        assert_eq!(result.files_modified, deserialized.files_modified);
        assert_eq!(result.files_added, deserialized.files_added);
        assert_eq!(result.files_deleted, deserialized.files_deleted);
        assert_eq!(result.conflicts_detected, deserialized.conflicts_detected);
    }

    #[tokio::test]
    async fn test_branch_cache_operations() {
        let manager = create_mock_branch_manager().await;
        let workspace_id = 1;
        let branch_name = "test-branch";

        // Test cache clearing
        manager.clear_branch_cache(workspace_id).await.unwrap();

        let branches = manager.list_workspace_branches(workspace_id).await.unwrap();
        assert!(branches.is_empty());
    }

    #[test]
    fn test_branch_error_display() {
        let errors = vec![
            BranchError::BranchNotFound {
                branch_name: "missing".to_string(),
            },
            BranchError::InvalidBranchName {
                branch_name: "bad..name".to_string(),
                reason: "contains double dots".to_string(),
            },
            BranchError::BranchSwitchFailed {
                from: "main".to_string(),
                to: "feature".to_string(),
                reason: "merge conflicts".to_string(),
            },
            BranchError::UncommittedChanges,
            BranchError::BranchConflicts {
                conflicts: vec!["file1.txt".to_string(), "file2.txt".to_string()],
            },
        ];

        for error in errors {
            let error_str = error.to_string();
            assert!(!error_str.is_empty());
            println!("Error: {}", error_str);
        }
    }
}
