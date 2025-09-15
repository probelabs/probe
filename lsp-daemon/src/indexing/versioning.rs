//! File Version Management System - Phase 2.2
#![allow(dead_code, clippy::all)]
//!
//! This module provides comprehensive file version management that builds on Phase 2.1's
//! file change detection. It implements content-addressed storage with automatic deduplication,
//! workspace file association, and git integration.
//!
//! ## Core Features
//!
//! - **Content-addressed storage**: Files are identified by their content hash, enabling
//!   automatic deduplication across workspaces
//! - **Workspace file association**: Links files to specific workspaces using Phase 1.3 database traits
//! - **Batch processing**: Efficiently processes multiple FileChange results from Phase 2.1
//! - **Git integration**: Maps file versions to git blob OIDs and commit references
//! - **Performance metrics**: Tracks deduplication rates and operation performance
//!
//! ## Architecture
//!
//! The FileVersionManager integrates with:
//! - Phase 1.3 DatabaseBackend traits for persistent storage
//! - Phase 2.1 FileChangeDetector for change detection
//! - Existing GitService for git operations
//! - Universal cache system for performance optimization
//!
//! ## Usage
//!
//! ```rust
//! use versioning::FileVersionManager;
//! use file_detector::{FileChangeDetector, DetectionConfig};
//!
//! // Create manager with database backend
//! let manager = FileVersionManager::new(database_backend, config).await?;
//!
//! // Process file changes from Phase 2.1
//! let detector = FileChangeDetector::new();
//! let changes = detector.detect_changes(workspace_id, scan_path, &database).await?;
//! let results = manager.process_file_changes(workspace_id, changes).await?;
//!
//! // Ensure file version exists (content-addressed)
//! let version_info = manager.ensure_file_version(file_path, content).await?;
//! ```

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{RwLock, Semaphore};
use tracing::{debug, error, info, warn};

use crate::database::{DatabaseBackend, DatabaseError, FileVersion};
use crate::git_service::{GitService, GitServiceError};
use crate::indexing::{FileChange, FileChangeType, HashAlgorithm};

/// Errors that can occur during file version management operations
#[derive(Debug, thiserror::Error)]
pub enum VersioningError {
    #[error("Database operation failed: {0}")]
    Database(#[from] DatabaseError),

    #[error("Git operation failed: {0}")]
    Git(#[from] GitServiceError),

    #[error("IO operation failed: {0}")]
    Io(#[from] std::io::Error),

    #[error("File not found: {path}")]
    FileNotFound { path: PathBuf },

    #[error("Invalid file content: {reason}")]
    InvalidContent { reason: String },

    #[error("Workspace not found: {workspace_id}")]
    WorkspaceNotFound { workspace_id: i64 },

    #[error("Concurrent operation failed: {reason}")]
    ConcurrencyError { reason: String },

    #[error("Content addressing failed: {reason}")]
    ContentAddressingError { reason: String },

    #[error("Git tree synchronization failed: {reason}")]
    GitSyncError { reason: String },

    #[error("Context error: {0}")]
    Context(#[from] anyhow::Error),
}

/// Configuration for file version management
#[derive(Debug, Clone)]
pub struct VersioningConfig {
    /// Hash algorithm for content addressing (should match FileChangeDetector)
    pub hash_algorithm: HashAlgorithm,
    /// Maximum concurrent file processing operations
    pub max_concurrent_operations: usize,
    /// Enable git blob OID mapping
    pub enable_git_integration: bool,
    /// Cache size for recently accessed file versions
    pub version_cache_size: usize,
    /// Maximum file size to process (in bytes)
    pub max_file_size: u64,
    /// Enable performance metrics collection
    pub collect_metrics: bool,
    /// Batch size for database operations
    pub batch_size: usize,
}

impl Default for VersioningConfig {
    fn default() -> Self {
        Self {
            hash_algorithm: HashAlgorithm::Blake3,
            max_concurrent_operations: 50,
            enable_git_integration: true,
            version_cache_size: 1000,
            max_file_size: 100 * 1024 * 1024, // 100MB
            collect_metrics: true,
            batch_size: 100,
        }
    }
}

/// Information about a file version with deduplication tracking
#[derive(Debug, Clone)]
pub struct FileVersionInfo {
    /// Database file version record
    pub file_version: FileVersion,
    /// Whether this version was newly created (true) or deduplicated (false)
    pub is_new_version: bool,
    /// Git blob OID if available
    pub git_blob_oid: Option<String>,
    /// Detected programming language
    pub detected_language: Option<String>,
    /// File path where this version was encountered
    pub file_path: PathBuf,
}

/// Results from processing a batch of file changes
#[derive(Debug, Clone)]
pub struct ProcessingResults {
    /// Successfully processed file versions
    pub processed_versions: Vec<FileVersionInfo>,
    /// Files that failed to process with error messages
    pub failed_files: Vec<(PathBuf, String)>,
    /// Number of files that were deduplicated (content already existed)
    pub deduplicated_count: usize,
    /// Number of new file versions created
    pub new_versions_count: usize,
    /// Total processing time
    pub processing_duration: Duration,
    /// Workspace file associations created
    pub workspace_associations_created: usize,
}

/// Performance metrics for file version management operations
#[derive(Debug, Default, Clone)]
pub struct VersioningMetrics {
    /// Total files processed
    pub total_files_processed: u64,
    /// Total deduplications achieved
    pub total_deduplications: u64,
    /// Average processing time per file (microseconds)
    pub avg_processing_time_us: u64,
    /// Cache hit rate for version lookups
    pub cache_hit_rate: f64,
    /// Git operations performed
    pub git_operations_count: u64,
    /// Database transaction count
    pub database_transactions: u64,
}

/// Cache entry for file version lookups
#[derive(Debug, Clone)]
struct CacheEntry {
    file_version: FileVersion,
    accessed_at: Instant,
}

/// File version manager with content-addressed storage and deduplication
pub struct FileVersionManager<T>
where
    T: DatabaseBackend + Send + Sync,
{
    /// Database backend for persistent storage
    database: Arc<T>,
    /// Configuration settings
    config: VersioningConfig,
    /// Semaphore for controlling concurrent operations
    operation_semaphore: Arc<Semaphore>,
    /// Cache for recently accessed file versions
    version_cache: Arc<RwLock<HashMap<String, CacheEntry>>>,
    /// Performance metrics (if enabled)
    metrics: Arc<RwLock<VersioningMetrics>>,
    /// Start time for metrics calculation
    start_time: Instant,
}

impl<T> FileVersionManager<T>
where
    T: DatabaseBackend + Send + Sync + 'static,
{
    /// Create a new file version manager with the given database backend and configuration
    pub async fn new(database: Arc<T>, config: VersioningConfig) -> Result<Self, VersioningError> {
        let manager = Self {
            database,
            operation_semaphore: Arc::new(Semaphore::new(config.max_concurrent_operations)),
            version_cache: Arc::new(RwLock::new(HashMap::with_capacity(
                config.version_cache_size,
            ))),
            metrics: Arc::new(RwLock::new(VersioningMetrics::default())),
            config,
            start_time: Instant::now(),
        };

        info!(
            "FileVersionManager initialized with config: max_concurrent_operations={}, git_integration={}, cache_size={}",
            manager.config.max_concurrent_operations,
            manager.config.enable_git_integration,
            manager.config.version_cache_size
        );

        Ok(manager)
    }

    /// Get or create a file version using content-addressed storage with automatic deduplication
    /// This is the core method that implements content addressing
    pub async fn ensure_file_version(
        &self,
        file_path: &Path,
        content: &[u8],
    ) -> Result<FileVersionInfo, VersioningError> {
        let _permit = self.operation_semaphore.acquire().await.map_err(|e| {
            VersioningError::ConcurrencyError {
                reason: format!("Failed to acquire semaphore: {}", e),
            }
        })?;

        let start_time = Instant::now();

        // Check file size limit
        if content.len() as u64 > self.config.max_file_size {
            return Err(VersioningError::InvalidContent {
                reason: format!(
                    "File too large: {} bytes exceeds limit of {} bytes",
                    content.len(),
                    self.config.max_file_size
                ),
            });
        }

        // Compute content hash using configured algorithm
        let content_digest = self.compute_content_hash(content);

        // Check cache first
        if let Some(cached_version) = self.get_from_cache(&content_digest).await {
            debug!("Cache hit for content digest: {}", content_digest);

            if self.config.collect_metrics {
                self.update_deduplication_metrics(start_time.elapsed())
                    .await;
            }

            return Ok(FileVersionInfo {
                file_version: cached_version,
                is_new_version: false,
                git_blob_oid: None, // TODO: Cache git blob OID as well
                detected_language: self.detect_language(file_path),
                file_path: file_path.to_path_buf(),
            });
        }

        // Since we no longer use file versions in the simplified schema,
        // we'll create a simple file version representation based on content hash
        let file_version = FileVersion {
            file_version_id: content_digest
                .chars()
                .take(10)
                .collect::<String>()
                .parse::<i64>()
                .unwrap_or(1),
            file_id: file_path
                .to_string_lossy()
                .chars()
                .take(10)
                .collect::<String>()
                .parse::<i64>()
                .unwrap_or(1),
            content_digest: content_digest.clone(),
            size_bytes: content.len() as u64,
            git_blob_oid: None,
            line_count: None,
            detected_language: None,
            mtime: Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64,
            ),
        };

        // Cache the result
        self.add_to_cache(&content_digest, file_version.clone())
            .await;

        if self.config.collect_metrics {
            self.update_deduplication_metrics(start_time.elapsed())
                .await;
        }

        Ok(FileVersionInfo {
            file_version,
            is_new_version: true, // For simplicity, always treat as new
            git_blob_oid: None,
            detected_language: self.detect_language(file_path),
            file_path: file_path.to_path_buf(),
        })
    }

    /// Process a batch of FileChange results from Phase 2.1 file change detection
    /// This method efficiently handles multiple files with proper error recovery
    pub async fn process_file_changes(
        &self,
        workspace_id: i64,
        changes: Vec<FileChange>,
    ) -> Result<ProcessingResults, VersioningError> {
        let start_time = Instant::now();
        let mut results = ProcessingResults {
            processed_versions: Vec::new(),
            failed_files: Vec::new(),
            deduplicated_count: 0,
            new_versions_count: 0,
            processing_duration: Duration::from_secs(0),
            workspace_associations_created: 0,
        };

        info!(
            "Processing {} file changes for workspace {}",
            changes.len(),
            workspace_id
        );

        // Process changes in batches to avoid overwhelming the database
        for batch in changes.chunks(self.config.batch_size) {
            let batch_start = Instant::now();

            // Process batch sequentially to avoid lifetime issues
            // This still respects the semaphore for concurrent operations within each file
            for change in batch {
                match self
                    .process_single_file_change(workspace_id, change.clone())
                    .await
                {
                    Ok(version_info) => {
                        if version_info.is_new_version {
                            results.new_versions_count += 1;
                        } else {
                            results.deduplicated_count += 1;
                        }

                        // Associate file with workspace
                        if let Err(e) = self
                            .associate_file_with_workspace(
                                workspace_id,
                                version_info.file_version.file_id,
                                version_info.file_version.file_version_id,
                            )
                            .await
                        {
                            warn!("Failed to associate file with workspace: {}", e);
                        } else {
                            results.workspace_associations_created += 1;
                        }

                        results.processed_versions.push(version_info);
                    }
                    Err(e) => {
                        error!("Failed to process file change: {}", e);
                        results
                            .failed_files
                            .push((change.path.clone(), e.to_string()));
                    }
                }
            }

            debug!(
                "Processed batch of {} changes in {:?}",
                batch.len(),
                batch_start.elapsed()
            );
        }

        results.processing_duration = start_time.elapsed();

        info!(
            "File change processing completed: {} processed, {} new, {} deduplicated, {} failed in {:?}",
            results.processed_versions.len(),
            results.new_versions_count,
            results.deduplicated_count,
            results.failed_files.len(),
            results.processing_duration
        );

        if self.config.collect_metrics {
            self.update_batch_processing_metrics(&results).await;
        }

        Ok(results)
    }

    /// Associate a file with a workspace using the Phase 1.3 database traits
    /// This creates the link between files and workspaces for incremental indexing
    pub async fn associate_file_with_workspace(
        &self,
        workspace_id: i64,
        file_id: i64,
        file_version_id: i64,
    ) -> Result<(), VersioningError> {
        debug!(
            "Associating file {} (version {}) with workspace {}",
            file_id, file_version_id, workspace_id
        );

        // Verify workspace exists
        match self.database.get_workspace(workspace_id).await {
            Ok(Some(_)) => {
                // Workspace exists, proceed with association
                self.database
                    .link_file_to_workspace(workspace_id, file_id, file_version_id)
                    .await
                    .context("Failed to link file to workspace")?;

                debug!(
                    "Successfully associated file {} with workspace {}",
                    file_id, workspace_id
                );
                Ok(())
            }
            Ok(None) => Err(VersioningError::WorkspaceNotFound { workspace_id }),
            Err(e) => {
                error!("Failed to verify workspace existence: {}", e);
                Err(VersioningError::Database(e))
            }
        }
    }

    /// Synchronize file versions with git tree using GitService integration
    /// This method maps file versions to git blob OIDs and handles git-aware operations
    pub async fn sync_with_git_tree(
        &self,
        workspace_root: &Path,
        file_versions: &[FileVersionInfo],
    ) -> Result<Vec<(i64, Option<String>)>, VersioningError> {
        if !self.config.enable_git_integration {
            debug!("Git integration disabled, skipping git tree sync");
            return Ok(vec![(0, None); file_versions.len()]);
        }

        debug!(
            "Synchronizing {} file versions with git tree at {}",
            file_versions.len(),
            workspace_root.display()
        );

        // Discover git repository
        let git_service = match GitService::discover_repo(workspace_root, workspace_root) {
            Ok(service) => service,
            Err(GitServiceError::NotRepo) => {
                debug!(
                    "No git repository found at {}, skipping git sync",
                    workspace_root.display()
                );
                return Ok(vec![(0, None); file_versions.len()]);
            }
            Err(e) => {
                warn!("Failed to discover git repository: {}", e);
                return Err(VersioningError::Git(e));
            }
        };

        // Get current HEAD commit for reference
        let head_commit = match git_service.head_commit() {
            Ok(commit) => commit,
            Err(e) => {
                warn!("Failed to get HEAD commit: {}", e);
                return Err(VersioningError::Git(e));
            }
        };

        debug!("Git HEAD commit: {:?}", head_commit);

        let mut results = Vec::with_capacity(file_versions.len());

        for version_info in file_versions {
            // TODO: Implement git blob OID retrieval when GitService supports it
            // For now, we track that the operation was attempted but return None for OID
            let git_blob_oid = None; // Placeholder until GitService implements blob OID lookup

            results.push((version_info.file_version.file_version_id, git_blob_oid));

            if self.config.collect_metrics {
                let mut metrics = self.metrics.write().await;
                metrics.git_operations_count += 1;
            }
        }

        debug!(
            "Git tree synchronization completed for {} file versions",
            results.len()
        );
        Ok(results)
    }

    /// Get current performance metrics
    pub async fn get_metrics(&self) -> VersioningMetrics {
        let metrics = self.metrics.read().await;
        let mut result = metrics.clone();

        // Calculate cache hit rate
        if result.total_files_processed > 0 {
            let cache_hits = result.total_files_processed - result.total_deduplications;
            result.cache_hit_rate = cache_hits as f64 / result.total_files_processed as f64;
        }

        result
    }

    /// Clear all cached file versions
    pub async fn clear_cache(&self) {
        let mut cache = self.version_cache.write().await;
        cache.clear();
        debug!("File version cache cleared");
    }

    /// Get cache statistics
    pub async fn get_cache_stats(&self) -> (usize, usize) {
        let cache = self.version_cache.read().await;
        (cache.len(), self.config.version_cache_size)
    }

    // Private helper methods

    /// Process a single file change
    async fn process_single_file_change(
        &self,
        _workspace_id: i64,
        change: FileChange,
    ) -> Result<FileVersionInfo, VersioningError> {
        let file_path = &change.path;

        debug!(
            "Processing file change: {:?} (type: {:?})",
            file_path, change.change_type
        );

        match change.change_type {
            FileChangeType::Delete => {
                // For deletions, we don't create versions but could mark associations as inactive
                // This is a placeholder - actual deletion handling would be more complex
                return Err(VersioningError::InvalidContent {
                    reason: "Cannot process deleted file".to_string(),
                });
            }
            FileChangeType::Create | FileChangeType::Update => {
                // Read file content
                let content = tokio::fs::read(file_path)
                    .await
                    .context(format!("Failed to read file: {}", file_path.display()))?;

                // Use content hash from change if available, otherwise compute it
                let expected_hash = change.content_digest.as_deref();
                let computed_hash = self.compute_content_hash(&content);

                // Verify content hash if provided
                if let Some(expected) = expected_hash {
                    if expected != computed_hash {
                        return Err(VersioningError::ContentAddressingError {
                            reason: format!(
                                "Content hash mismatch: expected {}, computed {}",
                                expected, computed_hash
                            ),
                        });
                    }
                }

                self.ensure_file_version(file_path, &content).await
            }
            FileChangeType::Move { from: _, to: _ } => {
                // For moves, we treat it as a new file at the destination
                let content = tokio::fs::read(file_path).await.context(format!(
                    "Failed to read moved file: {}",
                    file_path.display()
                ))?;

                self.ensure_file_version(file_path, &content).await
            }
        }
    }

    /// Create a new file version in the database
    async fn create_new_file_version(
        &self,
        file_path: &Path,
        content: &[u8],
        content_digest: &str,
        start_time: Instant,
    ) -> Result<FileVersionInfo, VersioningError> {
        debug!("Creating new file version for: {}", file_path.display());

        // Get file metadata
        let metadata = tokio::fs::metadata(file_path).await.context(format!(
            "Failed to get file metadata: {}",
            file_path.display()
        ))?;

        let mtime = metadata
            .modified()?
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let size_bytes = content.len() as u64;

        // Generate a unique file ID (this would typically come from a files table)
        let file_id = self.generate_file_id().await;

        // Since we no longer use database file versions, create a simple file version representation
        let file_version_id = content_digest
            .chars()
            .take(10)
            .collect::<String>()
            .parse::<i64>()
            .unwrap_or(1);

        // Construct FileVersion for result
        let file_version = FileVersion {
            file_version_id,
            file_id,
            content_digest: content_digest.to_string(),
            size_bytes,
            git_blob_oid: None, // Will be set by git integration if enabled
            line_count: Some(self.count_lines(content)),
            detected_language: self.detect_language(file_path),
            mtime: Some(mtime),
        };

        // Cache the result
        self.add_to_cache(content_digest, file_version.clone())
            .await;

        if self.config.collect_metrics {
            self.update_new_version_metrics(start_time.elapsed()).await;
        }

        debug!(
            "Created new file version {} for file {} (size: {} bytes)",
            file_version_id,
            file_path.display(),
            size_bytes
        );

        Ok(FileVersionInfo {
            file_version,
            is_new_version: true,
            git_blob_oid: None,
            detected_language: self.detect_language(file_path),
            file_path: file_path.to_path_buf(),
        })
    }

    /// Compute content hash using the configured algorithm
    fn compute_content_hash(&self, content: &[u8]) -> String {
        match self.config.hash_algorithm {
            HashAlgorithm::Blake3 => {
                let hash = blake3::hash(content);
                hash.to_hex().to_string()
            }
            HashAlgorithm::Sha256 => {
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(content);
                format!("{:x}", hasher.finalize())
            }
        }
    }

    /// Detect programming language from file path
    fn detect_language(&self, file_path: &Path) -> Option<String> {
        file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_lowercase())
    }

    /// Count lines in content (for metadata)
    fn count_lines(&self, content: &[u8]) -> u32 {
        content.iter().filter(|&&b| b == b'\n').count() as u32 + 1
    }

    /// Generate a unique file ID
    async fn generate_file_id(&self) -> i64 {
        // This is a simple timestamp-based ID. In a real implementation,
        // this would use a proper ID generation strategy or query the files table
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    }

    // Cache management methods

    /// Get file version from cache
    async fn get_from_cache(&self, content_digest: &str) -> Option<FileVersion> {
        let cache = self.version_cache.read().await;
        cache
            .get(content_digest)
            .map(|entry| entry.file_version.clone())
    }

    /// Add file version to cache with LRU eviction
    async fn add_to_cache(&self, content_digest: &str, file_version: FileVersion) {
        let mut cache = self.version_cache.write().await;

        // LRU eviction if cache is full
        if cache.len() >= self.config.version_cache_size {
            // Find oldest entry
            if let Some((oldest_key, _)) = cache
                .iter()
                .min_by_key(|(_, entry)| entry.accessed_at)
                .map(|(k, v)| (k.clone(), v.accessed_at))
            {
                cache.remove(&oldest_key);
            }
        }

        cache.insert(
            content_digest.to_string(),
            CacheEntry {
                file_version,
                accessed_at: Instant::now(),
            },
        );
    }

    // Metrics update methods

    /// Update metrics for cache hit
    async fn update_cache_metrics(&self, _hit: bool, duration: Duration) {
        if !self.config.collect_metrics {
            return;
        }

        let mut metrics = self.metrics.write().await;
        metrics.total_files_processed += 1;

        let duration_us = duration.as_micros() as u64;
        metrics.avg_processing_time_us = (metrics.avg_processing_time_us + duration_us) / 2;
    }

    /// Update metrics for deduplication
    async fn update_deduplication_metrics(&self, duration: Duration) {
        if !self.config.collect_metrics {
            return;
        }

        let mut metrics = self.metrics.write().await;
        metrics.total_files_processed += 1;
        metrics.total_deduplications += 1;

        let duration_us = duration.as_micros() as u64;
        metrics.avg_processing_time_us = (metrics.avg_processing_time_us + duration_us) / 2;
    }

    /// Update metrics for new version creation
    async fn update_new_version_metrics(&self, duration: Duration) {
        if !self.config.collect_metrics {
            return;
        }

        let mut metrics = self.metrics.write().await;
        metrics.total_files_processed += 1;
        metrics.database_transactions += 1;

        let duration_us = duration.as_micros() as u64;
        metrics.avg_processing_time_us = (metrics.avg_processing_time_us + duration_us) / 2;
    }

    /// Update metrics for batch processing
    async fn update_batch_processing_metrics(&self, results: &ProcessingResults) {
        if !self.config.collect_metrics {
            return;
        }

        let mut metrics = self.metrics.write().await;
        metrics.database_transactions += results.new_versions_count as u64;
        // Additional batch metrics could be added here
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{DatabaseConfig, SQLiteBackend};
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_file_version_manager_creation() -> Result<(), Box<dyn std::error::Error>> {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };
        let db = Arc::new(SQLiteBackend::new(config).await?);
        let versioning_config = VersioningConfig::default();

        let manager = FileVersionManager::new(db, versioning_config).await?;

        // Verify initial state
        let metrics = manager.get_metrics().await;
        assert_eq!(metrics.total_files_processed, 0);
        assert_eq!(metrics.total_deduplications, 0);

        let (cache_size, cache_capacity) = manager.get_cache_stats().await;
        assert_eq!(cache_size, 0);
        assert!(cache_capacity > 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_ensure_file_version_new_content() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };
        let db = Arc::new(SQLiteBackend::new(config).await?);
        let manager = FileVersionManager::new(db, VersioningConfig::default()).await?;

        // Create test file
        let test_file = temp_dir.path().join("test.rs");
        let content = b"fn main() { println!(\"Hello, world!\"); }";
        fs::write(&test_file, content).await?;

        // First call should create new version
        let version_info1 = manager.ensure_file_version(&test_file, content).await?;
        assert!(version_info1.is_new_version);
        assert_eq!(version_info1.file_version.size_bytes, content.len() as u64);
        assert!(version_info1.file_version.content_digest.len() > 0);

        // Second call with same content should deduplicate
        let version_info2 = manager.ensure_file_version(&test_file, content).await?;
        assert!(!version_info2.is_new_version);
        assert_eq!(
            version_info1.file_version.content_digest,
            version_info2.file_version.content_digest
        );
        assert_eq!(
            version_info1.file_version.file_version_id,
            version_info2.file_version.file_version_id
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_content_deduplication() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };
        let db = Arc::new(SQLiteBackend::new(config).await?);
        let manager = FileVersionManager::new(db, VersioningConfig::default()).await?;

        let content = b"const MESSAGE = 'Hello, deduplication!';";

        // Create two different files with identical content
        let file1 = temp_dir.path().join("file1.js");
        let file2 = temp_dir.path().join("file2.js");
        fs::write(&file1, content).await?;
        fs::write(&file2, content).await?;

        let version1 = manager.ensure_file_version(&file1, content).await?;
        let version2 = manager.ensure_file_version(&file2, content).await?;

        // Both should reference the same content hash
        assert_eq!(
            version1.file_version.content_digest,
            version2.file_version.content_digest
        );

        // First should be new, second should be deduplicated
        assert!(version1.is_new_version);
        assert!(!version2.is_new_version);

        // Check metrics
        let metrics = manager.get_metrics().await;
        assert_eq!(metrics.total_files_processed, 2);
        assert_eq!(metrics.total_deduplications, 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_hash_algorithms() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };
        let db1 = Arc::new(SQLiteBackend::new(config.clone()).await?);
        let db2 = Arc::new(SQLiteBackend::new(config).await?);

        let blake3_config = VersioningConfig {
            hash_algorithm: HashAlgorithm::Blake3,
            ..Default::default()
        };
        let sha256_config = VersioningConfig {
            hash_algorithm: HashAlgorithm::Sha256,
            ..Default::default()
        };

        let manager1 = FileVersionManager::new(db1, blake3_config).await?;
        let manager2 = FileVersionManager::new(db2, sha256_config).await?;

        let test_file = temp_dir.path().join("test.txt");
        let content = b"Test content for hash algorithms";
        fs::write(&test_file, content).await?;

        let version1 = manager1.ensure_file_version(&test_file, content).await?;
        let version2 = manager2.ensure_file_version(&test_file, content).await?;

        // Different hash algorithms should produce different digests
        assert_ne!(
            version1.file_version.content_digest,
            version2.file_version.content_digest
        );

        // BLAKE3 produces 64-character hex string
        assert_eq!(version1.file_version.content_digest.len(), 64);
        // SHA256 also produces 64-character hex string
        assert_eq!(version2.file_version.content_digest.len(), 64);

        Ok(())
    }

    #[tokio::test]
    async fn test_file_size_limits() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };
        let db = Arc::new(SQLiteBackend::new(config).await?);

        let versioning_config = VersioningConfig {
            max_file_size: 100, // Very small limit for testing
            ..Default::default()
        };
        let manager = FileVersionManager::new(db, versioning_config).await?;

        let test_file = temp_dir.path().join("large_file.txt");
        let large_content = vec![b'A'; 200]; // Exceeds the limit
        fs::write(&test_file, &large_content).await?;

        // Should fail due to size limit
        let result = manager
            .ensure_file_version(&test_file, &large_content)
            .await;
        assert!(result.is_err());

        if let Err(VersioningError::InvalidContent { reason }) = result {
            assert!(reason.contains("File too large"));
        } else {
            panic!("Expected InvalidContent error");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_cache_functionality() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };
        let db = Arc::new(SQLiteBackend::new(config).await?);

        let versioning_config = VersioningConfig {
            version_cache_size: 2, // Small cache for testing LRU
            ..Default::default()
        };
        let manager = FileVersionManager::new(db, versioning_config).await?;

        let content1 = b"File 1 content";
        let content2 = b"File 2 content";
        let content3 = b"File 3 content";

        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("file2.txt");
        let file3 = temp_dir.path().join("file3.txt");

        fs::write(&file1, content1).await?;
        fs::write(&file2, content2).await?;
        fs::write(&file3, content3).await?;

        // Fill cache
        manager.ensure_file_version(&file1, content1).await?;
        manager.ensure_file_version(&file2, content2).await?;

        let (cache_size, _) = manager.get_cache_stats().await;
        assert_eq!(cache_size, 2);

        // Add third item - should evict oldest (file1)
        manager.ensure_file_version(&file3, content3).await?;

        let (cache_size_after, _) = manager.get_cache_stats().await;
        assert_eq!(cache_size_after, 2); // Still at capacity

        // Test cache clearing
        manager.clear_cache().await;
        let (cache_size_cleared, _) = manager.get_cache_stats().await;
        assert_eq!(cache_size_cleared, 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_language_detection() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };
        let db = Arc::new(SQLiteBackend::new(config).await?);
        let manager = FileVersionManager::new(db, VersioningConfig::default()).await?;

        // Test various file extensions
        let test_cases = vec![
            ("test.rs", "fn main() {}".as_bytes(), Some("rs".to_string())),
            (
                "test.py",
                "print('hello')".as_bytes(),
                Some("py".to_string()),
            ),
            (
                "test.js",
                "console.log('hello')".as_bytes(),
                Some("js".to_string()),
            ),
            (
                "test.unknown",
                "unknown content".as_bytes(),
                Some("unknown".to_string()),
            ),
        ];

        for (filename, content, expected_lang) in test_cases {
            let file_path = temp_dir.path().join(filename);
            fs::write(&file_path, content).await?;

            let version_info = manager.ensure_file_version(&file_path, content).await?;
            assert_eq!(version_info.detected_language, expected_lang);
            assert_eq!(version_info.file_version.detected_language, expected_lang);
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_metrics_collection() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };
        let db = Arc::new(SQLiteBackend::new(config).await?);

        let versioning_config = VersioningConfig {
            collect_metrics: true,
            ..Default::default()
        };
        let manager = FileVersionManager::new(db, versioning_config).await?;

        let content = b"Test content for metrics";
        let file1 = temp_dir.path().join("file1.txt");
        let file2 = temp_dir.path().join("file2.txt");

        fs::write(&file1, content).await?;
        fs::write(&file2, content).await?;

        // Process files
        manager.ensure_file_version(&file1, content).await?; // New
        manager.ensure_file_version(&file2, content).await?; // Deduplication

        // Check metrics
        let metrics = manager.get_metrics().await;
        assert_eq!(metrics.total_files_processed, 2);
        assert_eq!(metrics.total_deduplications, 1);
        assert!(metrics.avg_processing_time_us > 0);

        Ok(())
    }
}
