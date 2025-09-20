//! Simplified workspace-aware database routing for LSP cache management
//!
//! The WorkspaceDatabaseRouter provides simple database routing for LSP operations
//! across multiple workspaces, implementing:
//!
//! - Per-workspace database isolation
//! - Stable workspace IDs based on content hashing
//! - Direct database cache creation per workspace

use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::database_cache_adapter::{DatabaseCacheAdapter, DatabaseCacheConfig};
use crate::git_service::GitService;
use crate::server_manager::SingleServerManager;

/// Configuration for workspace database router
#[derive(Debug, Clone)]
pub struct WorkspaceDatabaseRouterConfig {
    /// Base directory for all workspace caches
    pub base_cache_dir: PathBuf,
    /// Maximum number of parent directories to search for reads
    pub max_parent_lookup_depth: usize,
    /// Cache configuration template for new workspace caches
    pub cache_config_template: DatabaseCacheConfig,
    /// Force in-memory mode for all workspace caches
    pub force_memory_only: bool,
    // Ignored fields for compatibility
    #[allow(dead_code)]
    pub max_open_caches: usize,
}

impl Default for WorkspaceDatabaseRouterConfig {
    fn default() -> Self {
        Self {
            base_cache_dir: PathBuf::from(".probe-temp-cache"),
            max_parent_lookup_depth: 3,
            cache_config_template: DatabaseCacheConfig::default(),
            force_memory_only: false,
            max_open_caches: 8, // Ignored but kept for compatibility
        }
    }
}

/// Lazily compute the default cache directory to avoid early filesystem access on Windows CI.
fn default_cache_directory() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")))
        .join("probe")
        .join("lsp")
        .join("workspaces")
}

/// Simple per-workspace database routing without memory management complexity
pub struct WorkspaceDatabaseRouter {
    /// Configuration
    config: WorkspaceDatabaseRouterConfig,
    /// Open cache instances: workspace_id -> cache
    open_caches: Arc<RwLock<HashMap<String, Arc<DatabaseCacheAdapter>>>>,
    /// Server manager for workspace resolution
    #[allow(dead_code)]
    server_manager: Arc<SingleServerManager>,
    /// Workspace root discovery cache: file_path -> nearest_workspace_root
    workspace_cache: Arc<RwLock<HashMap<PathBuf, Option<PathBuf>>>>,
    /// Centralized workspace resolver for consistent workspace detection
    workspace_resolver:
        Option<std::sync::Arc<tokio::sync::Mutex<crate::workspace_resolver::WorkspaceResolver>>>,
    /// Dedicated reverse mapping: workspace_id -> workspace_root
    workspace_id_to_root: Arc<RwLock<HashMap<String, PathBuf>>>,
}

impl WorkspaceDatabaseRouter {
    /// Create a new workspace database router without workspace resolver (for backward compatibility)
    pub fn new(
        config: WorkspaceDatabaseRouterConfig,
        server_manager: Arc<SingleServerManager>,
    ) -> Self {
        Self::new_with_workspace_resolver(config, server_manager, None)
    }

    /// Create a new workspace database router with workspace resolver integration
    pub fn new_with_workspace_resolver(
        mut config: WorkspaceDatabaseRouterConfig,
        server_manager: Arc<SingleServerManager>,
        workspace_resolver: Option<
            std::sync::Arc<tokio::sync::Mutex<crate::workspace_resolver::WorkspaceResolver>>,
        >,
    ) -> Self {
        // Initialize proper cache directory at runtime
        if config.base_cache_dir == PathBuf::from(".probe-temp-cache") {
            config.base_cache_dir = default_cache_directory();
        }

        info!(
            "Initializing WorkspaceDatabaseRouter with base dir: {:?}, memory_only: {}",
            config.base_cache_dir, config.force_memory_only
        );

        Self {
            config,
            open_caches: Arc::new(RwLock::new(HashMap::new())),
            server_manager,
            workspace_cache: Arc::new(RwLock::new(HashMap::new())),
            workspace_resolver,
            workspace_id_to_root: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get or create a cache for a specific workspace
    pub async fn cache_for_workspace<P: AsRef<Path>>(
        &self,
        workspace_root: P,
    ) -> Result<Arc<DatabaseCacheAdapter>> {
        let workspace_root = workspace_root.as_ref().to_path_buf();

        let workspace_id = self.workspace_id_for(&workspace_root)?;

        // Check if cache is already open
        {
            let caches = self.open_caches.read().await;
            if let Some(cache) = caches.get(&workspace_id) {
                debug!(
                    "Cache hit for workspace '{}' ({})",
                    workspace_id,
                    workspace_root.display()
                );
                return Ok(cache.clone());
            }
        }

        debug!(
            "Cache miss for workspace '{}' ({}), creating new cache",
            workspace_id,
            workspace_root.display()
        );

        // Create cache directory path for this workspace
        let cache_dir = self.config.base_cache_dir.join(&workspace_id);

        // Ensure the cache directory exists
        if !cache_dir.exists() {
            std::fs::create_dir_all(&cache_dir).context(format!(
                "Failed to create cache directory for workspace '{workspace_id}': {cache_dir:?}"
            ))?;
        }

        // Create cache configuration for this workspace
        let mut cache_config = self.config.cache_config_template.clone();

        // Configure cache path and type
        if self.config.force_memory_only {
            cache_config.database_config.temporary = true;
            cache_config.database_config.path = None;
            debug!("Creating in-memory cache for workspace '{}'", workspace_id);
        } else {
            let db_path = cache_dir.join("cache.db");
            cache_config.database_config.temporary = false;
            cache_config.database_config.path = Some(db_path.clone());
            debug!(
                "Creating persistent cache at '{}' for workspace '{}'",
                db_path.display(),
                workspace_id
            );
        }

        // Create the cache instance
        let cache = DatabaseCacheAdapter::new_with_workspace_id(cache_config, &workspace_id)
            .await
            .context(format!(
                "Failed to create cache for workspace '{workspace_id}' at {cache_dir:?}"
            ))?;

        let cache_arc = Arc::new(cache);

        // Store the cache and maintain reverse mapping
        {
            let mut caches = self.open_caches.write().await;
            caches.insert(workspace_id.clone(), cache_arc.clone());
        }

        {
            let mut workspace_mapping = self.workspace_id_to_root.write().await;
            workspace_mapping.insert(workspace_id.clone(), workspace_root.clone());
        }

        info!(
            "Opened new cache for workspace '{}' ({})",
            workspace_id,
            workspace_root.display()
        );

        Ok(cache_arc)
    }

    /// Generate a stable workspace ID for a given workspace root path
    /// First tries to use git remote URL, falls back to hash-based approach
    pub fn workspace_id_for<P: AsRef<Path>>(&self, workspace_root: P) -> Result<String> {
        let path = workspace_root.as_ref();
        // Canonicalize path with fallback to original path for robustness
        let canonical_path = self.canonicalize_path(path);

        // Check if the path is a file and handle it properly
        let workspace_path = if canonical_path.is_file() {
            warn!(
                "workspace_id_for() received file path {:?} - using parent directory instead. \
                This may indicate a bug in the caller.",
                canonical_path
            );
            canonical_path
                .parent()
                .unwrap_or(&canonical_path)
                .to_path_buf()
        } else {
            canonical_path.clone()
        };

        // Try to get git remote URL for git-based workspace ID
        if let Ok(git_service) = GitService::discover_repo(&workspace_path, &workspace_path) {
            if let Ok(Some(remote_url)) = git_service.get_remote_url("origin") {
                debug!(
                    "Found git remote URL for workspace {}: {}",
                    workspace_path.display(),
                    remote_url
                );
                let sanitized_url = self.sanitize_remote_url(&remote_url);
                if !sanitized_url.is_empty() {
                    return Ok(sanitized_url);
                }
            }
        }

        // Fallback to hash-based approach if git remote not available
        debug!(
            "Using hash-based workspace ID for workspace {}",
            workspace_path.display()
        );

        // Normalize path for consistent hashing across platforms
        let normalized_path = self.normalize_path_for_hashing(&workspace_path);

        // Compute hash of the normalized path
        let hash = self.compute_path_hash(&normalized_path);

        // Extract folder name
        let folder_name = workspace_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        // Sanitize folder name for filesystem safety
        let safe_folder_name = self.sanitize_filename(&folder_name);

        Ok(format!("{}_{}", hash, safe_folder_name))
    }

    /// Find workspace root for a given file path
    pub async fn workspace_root_for<P: AsRef<Path>>(&self, file_path: P) -> Result<PathBuf> {
        let workspace_root = self.find_nearest_workspace(file_path.as_ref()).await?;
        Ok(workspace_root)
    }

    /// Clear all caches
    pub async fn clear_all(&self) -> Result<()> {
        let mut caches = self.open_caches.write().await;
        for (workspace_id, cache) in caches.drain() {
            debug!("Clearing cache for workspace '{}'", workspace_id);
            if let Err(e) = cache.clear().await {
                warn!(
                    "Failed to clear cache for workspace '{}': {}",
                    workspace_id, e
                );
            }
        }

        // Clear mappings
        {
            let mut workspace_mapping = self.workspace_id_to_root.write().await;
            workspace_mapping.clear();
        }
        {
            let mut workspace_cache = self.workspace_cache.write().await;
            workspace_cache.clear();
        }

        info!("Cleared all workspace caches");
        Ok(())
    }

    // Private helper methods

    fn canonicalize_path(&self, path: &Path) -> PathBuf {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    }

    fn normalize_path_for_hashing(&self, path: &Path) -> String {
        let path_str = path.to_string_lossy();
        // On Windows, convert to lowercase for consistent hashing
        #[cfg(windows)]
        {
            path_str.to_lowercase()
        }
        // On Unix-like systems, use as-is
        #[cfg(not(windows))]
        {
            path_str.to_string()
        }
    }

    fn compute_path_hash(&self, normalized_path: &str) -> String {
        // Use Blake3 for consistent workspace ID generation across restarts
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"workspace_id:");
        hasher.update(normalized_path.as_bytes());
        let hash = hasher.finalize();
        // Use first 8 characters to match the format used elsewhere
        hash.to_hex().to_string()[..8].to_string()
    }

    fn sanitize_filename(&self, name: &str) -> String {
        name.chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>()
            .chars()
            .take(32) // Limit length
            .collect()
    }

    /// Sanitize a remote URL to create a valid workspace ID
    /// Converts "https://github.com/user/repo.git" to "github_com_user_repo"
    fn sanitize_remote_url(&self, url: &str) -> String {
        let mut sanitized = url.to_lowercase();

        // Remove common protocols
        sanitized = sanitized
            .strip_prefix("https://")
            .or_else(|| sanitized.strip_prefix("http://"))
            .or_else(|| sanitized.strip_prefix("ssh://"))
            .or_else(|| sanitized.strip_prefix("git@"))
            .unwrap_or(&sanitized)
            .to_string();

        // Replace colon with slash (for git@ URLs like git@github.com:user/repo.git)
        sanitized = sanitized.replace(':', "/");

        // Remove .git extension
        if sanitized.ends_with(".git") {
            sanitized = sanitized[..sanitized.len() - 4].to_string();
        }

        // Replace all special characters with underscores
        sanitized = sanitized
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
            .collect();

        // Remove consecutive underscores and trim
        while sanitized.contains("__") {
            sanitized = sanitized.replace("__", "_");
        }
        sanitized = sanitized.trim_matches('_').to_string();

        // Limit length for filesystem safety
        if sanitized.len() > 64 {
            sanitized.truncate(64);
            sanitized = sanitized.trim_end_matches('_').to_string();
        }

        sanitized
    }

    /// Find the nearest workspace root for a given file path
    async fn find_nearest_workspace(&self, file_path: &Path) -> Result<PathBuf> {
        // Check cache first
        {
            let cache = self.workspace_cache.read().await;
            if let Some(result) = cache.get(file_path) {
                return match result {
                    Some(workspace_root) => Ok(workspace_root.clone()),
                    None => Err(anyhow!(
                        "No workspace found for path: {}",
                        file_path.display()
                    )),
                };
            }
        }

        // Resolve workspace using workspace resolver if available
        if let Some(resolver) = &self.workspace_resolver {
            let mut resolver_guard = resolver.lock().await;
            match resolver_guard.resolve_workspace(file_path, None) {
                Ok(workspace_root) => {
                    // Cache the result
                    {
                        let mut cache = self.workspace_cache.write().await;
                        cache.insert(file_path.to_path_buf(), Some(workspace_root.clone()));
                    }
                    return Ok(workspace_root);
                }
                Err(e) => {
                    debug!(
                        "Workspace resolver failed for {}: {}",
                        file_path.display(),
                        e
                    );
                    // Fall through to manual detection
                }
            }
        }

        // Manual workspace detection - walk up directory tree
        let mut current_path = if file_path.is_file() {
            file_path.parent().unwrap_or(file_path).to_path_buf()
        } else {
            file_path.to_path_buf()
        };

        let mut depth = 0;
        while depth < self.config.max_parent_lookup_depth {
            if self.is_workspace_root(&current_path) {
                // Cache the result
                {
                    let mut cache = self.workspace_cache.write().await;
                    cache.insert(file_path.to_path_buf(), Some(current_path.clone()));
                }
                return Ok(current_path);
            }

            // Move to parent directory
            if let Some(parent) = current_path.parent() {
                current_path = parent.to_path_buf();
                depth += 1;
            } else {
                break;
            }
        }

        // No workspace found - cache the negative result
        {
            let mut cache = self.workspace_cache.write().await;
            cache.insert(file_path.to_path_buf(), None);
        }

        Err(anyhow!(
            "No workspace found for path: {}",
            file_path.display()
        ))
    }

    fn is_workspace_root(&self, path: &Path) -> bool {
        // Check for common workspace markers
        let workspace_markers = [
            "Cargo.toml",
            "package.json",
            "tsconfig.json",
            "pyproject.toml",
            "setup.py",
            "requirements.txt",
            "go.mod",
            "pom.xml",
            "build.gradle",
            "CMakeLists.txt",
            ".git",
            "README.md",
        ];

        workspace_markers
            .iter()
            .any(|marker| path.join(marker).exists())
    }

    // Essential methods for daemon compatibility (simplified without LRU complexity)

    /// Get basic stats about workspace caches (without LRU/access tracking complexity)
    pub async fn get_stats(&self) -> crate::workspace_cache_router::WorkspaceCacheRouterStats {
        let caches = self.open_caches.read().await;
        let mut workspace_stats = Vec::new();

        for (workspace_id, cache) in caches.iter() {
            let workspace_root = {
                let mapping = self.workspace_id_to_root.read().await;
                mapping
                    .get(workspace_id)
                    .cloned()
                    .unwrap_or_else(|| PathBuf::from("unknown"))
            };

            let cache_stats = match cache.get_stats().await {
                Ok(stats) => Some(stats),
                Err(e) => {
                    warn!("Failed to get stats for cache '{}': {}", workspace_id, e);
                    None
                }
            };

            workspace_stats.push(crate::workspace_cache_router::WorkspaceStats {
                workspace_id: workspace_id.clone(),
                workspace_root,
                opened_at: std::time::Instant::now(), // Simplified: no access tracking
                last_accessed: std::time::Instant::now(),
                access_count: 1, // Simplified: no access counting
                cache_stats,
            });
        }

        crate::workspace_cache_router::WorkspaceCacheRouterStats {
            max_open_caches: 0, // No limit in simplified router
            current_open_caches: caches.len(),
            total_workspaces_seen: workspace_stats.len(),
            workspace_stats,
        }
    }

    /// List all workspace caches
    pub async fn list_all_workspace_caches(
        &self,
    ) -> Result<Vec<crate::protocol::WorkspaceCacheEntry>> {
        use std::time::SystemTime;
        let mut entries = Vec::new();

        if !self.config.base_cache_dir.exists() {
            return Ok(entries);
        }

        let mut read_dir = tokio::fs::read_dir(&self.config.base_cache_dir).await?;
        while let Some(entry) = read_dir.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                let workspace_id = entry.file_name().to_string_lossy().to_string();
                let cache_dir = entry.path();

                let workspace_root = {
                    let mapping = self.workspace_id_to_root.read().await;
                    mapping
                        .get(&workspace_id)
                        .cloned()
                        .unwrap_or_else(|| PathBuf::from("unknown"))
                };

                let mut total_size_bytes = 0u64;
                let mut total_files = 0usize;

                // Calculate directory size
                let mut dir_entries = tokio::fs::read_dir(&cache_dir).await?;
                while let Some(file_entry) = dir_entries.next_entry().await? {
                    if file_entry.file_type().await?.is_file() {
                        if let Ok(metadata) = file_entry.metadata().await {
                            total_size_bytes += metadata.len();
                            total_files += 1;
                        }
                    }
                }

                let _last_modified = SystemTime::UNIX_EPOCH; // Simplified

                entries.push(crate::protocol::WorkspaceCacheEntry {
                    workspace_id,
                    workspace_root,
                    cache_path: cache_dir.clone(),
                    size_bytes: total_size_bytes,
                    file_count: total_files,
                    last_accessed: "1970-01-01T00:00:00Z".to_string(), // Simplified
                    created_at: "1970-01-01T00:00:00Z".to_string(),    // Simplified
                });
            }
        }

        Ok(entries)
    }

    /// Get workspace cache info
    pub async fn get_workspace_cache_info(
        &self,
        workspace_path: Option<PathBuf>,
    ) -> Result<Vec<crate::protocol::WorkspaceCacheInfo>> {
        let mut info_list = Vec::new();

        if let Some(workspace_path) = workspace_path {
            // Get info for specific workspace
            let workspace_id = self.workspace_id_for(&workspace_path)?;
            let cache_path = self.config.base_cache_dir.join(&workspace_id);

            if cache_path.exists() {
                let cache_stats = if let Some(cache) = {
                    let caches = self.open_caches.read().await;
                    caches.get(&workspace_id).cloned()
                } {
                    cache.get_stats().await.ok()
                } else {
                    None
                };

                let cache_stats_proto = cache_stats.map(|stats| crate::protocol::CacheStatistics {
                    total_size_bytes: stats.total_size_bytes,
                    disk_size_bytes: stats.disk_size_bytes,
                    total_entries: stats.total_nodes,
                    entries_per_file: std::collections::HashMap::new(),
                    entries_per_language: std::collections::HashMap::new(),
                    hit_rate: stats.hit_count as f64
                        / (stats.hit_count + stats.miss_count).max(1) as f64,
                    miss_rate: stats.miss_count as f64
                        / (stats.hit_count + stats.miss_count).max(1) as f64,
                    age_distribution: crate::protocol::AgeDistribution {
                        entries_last_hour: 0,
                        entries_last_day: 0,
                        entries_last_week: 0,
                        entries_last_month: 0,
                        entries_older: 0,
                    },
                    most_accessed: Vec::new(),
                    memory_usage: crate::protocol::MemoryUsage {
                        in_memory_cache_bytes: 0,
                        persistent_cache_bytes: 0,
                        metadata_bytes: 0,
                        index_bytes: 0,
                    },
                    per_workspace_stats: None,
                    per_operation_totals: None,
                });

                info_list.push(crate::protocol::WorkspaceCacheInfo {
                    workspace_id,
                    workspace_root: workspace_path,
                    cache_path: cache_path.clone(),
                    size_bytes: 0, // Simplified
                    file_count: 0, // Simplified
                    last_accessed: "1970-01-01T00:00:00Z".to_string(),
                    created_at: "1970-01-01T00:00:00Z".to_string(),
                    disk_size_bytes: 0,    // Simplified
                    files_indexed: 0,      // Simplified
                    languages: Vec::new(), // Simplified
                    router_stats: None,    // Simplified
                    cache_stats: cache_stats_proto,
                });
            }
        } else {
            // Get info for all workspaces
            let entries = self.list_all_workspace_caches().await?;
            for entry in entries {
                let cache_path = self.config.base_cache_dir.join(&entry.workspace_id);
                info_list.push(crate::protocol::WorkspaceCacheInfo {
                    workspace_id: entry.workspace_id,
                    workspace_root: entry.workspace_root,
                    cache_path,
                    size_bytes: entry.size_bytes,
                    file_count: entry.file_count,
                    last_accessed: entry.last_accessed,
                    created_at: entry.created_at,
                    disk_size_bytes: entry.size_bytes, // Same as size_bytes for simplicity
                    files_indexed: entry.file_count as u64, // Same as file_count for simplicity
                    languages: Vec::new(),             // Simplified
                    router_stats: None,
                    cache_stats: None, // Simplified for list view
                });
            }
        }

        Ok(info_list)
    }

    /// Clear workspace cache(s)
    pub async fn clear_workspace_cache(
        &self,
        workspace_path: Option<PathBuf>,
        _older_than_seconds: Option<u64>, // Simplified: ignore age filter
    ) -> Result<crate::protocol::WorkspaceClearResult> {
        let mut cleared_workspaces = Vec::new();
        let mut total_size_freed_bytes = 0u64;
        let mut total_files_removed = 0usize;
        let mut errors = Vec::new();

        if let Some(workspace_path) = workspace_path {
            // Clear specific workspace
            let workspace_id = self.workspace_id_for(&workspace_path)?;
            match self.clear_single_workspace(&workspace_id).await {
                Ok((size_freed, files_removed)) => {
                    let workspace_root = {
                        let mapping = self.workspace_id_to_root.read().await;
                        mapping
                            .get(&workspace_id)
                            .cloned()
                            .unwrap_or_else(|| PathBuf::from("unknown"))
                    };
                    cleared_workspaces.push(crate::protocol::WorkspaceClearEntry {
                        workspace_id,
                        workspace_root,
                        success: true,
                        size_freed_bytes: size_freed,
                        files_removed,
                        error: None,
                    });
                    total_size_freed_bytes += size_freed;
                    total_files_removed += files_removed;
                }
                Err(e) => {
                    let workspace_root = {
                        let mapping = self.workspace_id_to_root.read().await;
                        mapping
                            .get(&workspace_id)
                            .cloned()
                            .unwrap_or_else(|| PathBuf::from("unknown"))
                    };
                    cleared_workspaces.push(crate::protocol::WorkspaceClearEntry {
                        workspace_id: workspace_id.clone(),
                        workspace_root,
                        success: false,
                        size_freed_bytes: 0,
                        files_removed: 0,
                        error: Some(e.to_string()),
                    });
                    errors.push(format!("Failed to clear workspace {}: {}", workspace_id, e));
                }
            }
        } else {
            // Clear all workspaces
            let caches = {
                let caches_guard = self.open_caches.read().await;
                caches_guard.keys().cloned().collect::<Vec<_>>()
            };

            for workspace_id in caches {
                match self.clear_single_workspace(&workspace_id).await {
                    Ok((size_freed, files_removed)) => {
                        let workspace_root = {
                            let mapping = self.workspace_id_to_root.read().await;
                            mapping
                                .get(&workspace_id)
                                .cloned()
                                .unwrap_or_else(|| PathBuf::from("unknown"))
                        };
                        cleared_workspaces.push(crate::protocol::WorkspaceClearEntry {
                            workspace_id,
                            workspace_root,
                            success: true,
                            size_freed_bytes: size_freed,
                            files_removed,
                            error: None,
                        });
                        total_size_freed_bytes += size_freed;
                        total_files_removed += files_removed;
                    }
                    Err(e) => {
                        let workspace_root = {
                            let mapping = self.workspace_id_to_root.read().await;
                            mapping
                                .get(&workspace_id)
                                .cloned()
                                .unwrap_or_else(|| PathBuf::from("unknown"))
                        };
                        cleared_workspaces.push(crate::protocol::WorkspaceClearEntry {
                            workspace_id: workspace_id.clone(),
                            workspace_root,
                            success: false,
                            size_freed_bytes: 0,
                            files_removed: 0,
                            error: Some(e.to_string()),
                        });
                        errors.push(format!("Failed to clear workspace {}: {}", workspace_id, e));
                    }
                }
            }
        }

        Ok(crate::protocol::WorkspaceClearResult {
            cleared_workspaces,
            total_size_freed_bytes,
            total_files_removed,
            errors,
        })
    }

    /// Clear a single workspace cache
    async fn clear_single_workspace(&self, workspace_id: &str) -> Result<(u64, usize)> {
        let mut size_freed = 0u64;
        let mut files_removed = 0usize;

        // Clear from memory if open
        {
            let mut caches = self.open_caches.write().await;
            if let Some(cache) = caches.remove(workspace_id) {
                let _ = cache.clear().await;
            }
        }

        // Clear from disk
        let cache_dir = self.config.base_cache_dir.join(workspace_id);
        if cache_dir.exists() {
            let mut dir_entries = tokio::fs::read_dir(&cache_dir).await?;
            while let Some(entry) = dir_entries.next_entry().await? {
                if entry.file_type().await?.is_file() {
                    if let Ok(metadata) = entry.metadata().await {
                        size_freed += metadata.len();
                        files_removed += 1;
                    }
                    let _ = tokio::fs::remove_file(entry.path()).await;
                }
            }
            let _ = tokio::fs::remove_dir(&cache_dir).await;
        }

        // Remove from mappings
        {
            let mut mapping = self.workspace_id_to_root.write().await;
            mapping.remove(workspace_id);
        }

        Ok((size_freed, files_removed))
    }

    /// Migrate existing workspace caches to use git-based naming where possible
    /// This is called during daemon initialization to upgrade old hash-based cache names
    pub async fn migrate_workspace_caches(&self) -> Result<()> {
        if !self.config.base_cache_dir.exists() {
            debug!("Cache directory doesn't exist yet, skipping migration");
            return Ok(());
        }

        info!(
            "Starting workspace cache migration in {}",
            self.config.base_cache_dir.display()
        );

        let mut migrated_count = 0;
        let mut skipped_count = 0;

        let mut read_dir = match tokio::fs::read_dir(&self.config.base_cache_dir).await {
            Ok(rd) => rd,
            Err(e) => {
                warn!("Failed to read cache directory for migration: {}", e);
                return Ok(());
            }
        };

        while let Some(entry) = read_dir.next_entry().await? {
            if !entry.file_type().await?.is_dir() {
                continue;
            }

            let old_workspace_id = entry.file_name().to_string_lossy().to_string();
            let old_cache_dir = entry.path();

            // Skip directories that already use git-based naming
            if old_workspace_id.contains("github_")
                || old_workspace_id.contains("gitlab_")
                || old_workspace_id.contains("bitbucket_")
                || old_workspace_id.contains("codeberg_")
                || old_workspace_id.starts_with("ssh_")
                || old_workspace_id.starts_with("https_")
                || old_workspace_id.starts_with("http_")
            {
                debug!(
                    "Skipping already git-based workspace ID: {}",
                    old_workspace_id
                );
                skipped_count += 1;
                continue;
            }

            // Try to find the workspace root from the reverse mapping
            let workspace_root = {
                let mapping = self.workspace_id_to_root.read().await;
                mapping.get(&old_workspace_id).cloned()
            };

            let workspace_root = match workspace_root {
                Some(root) => root,
                None => {
                    // We don't have the workspace root in memory, so we can't migrate
                    debug!(
                        "No workspace root found for {}, skipping migration",
                        old_workspace_id
                    );
                    skipped_count += 1;
                    continue;
                }
            };

            // Try to get the git-based workspace ID
            match GitService::discover_repo(&workspace_root, &workspace_root) {
                Ok(git_service) => {
                    match git_service.get_remote_url("origin") {
                        Ok(Some(remote_url)) => {
                            let new_workspace_id = self.sanitize_remote_url(&remote_url);
                            if !new_workspace_id.is_empty() && new_workspace_id != old_workspace_id
                            {
                                let new_cache_dir =
                                    self.config.base_cache_dir.join(&new_workspace_id);

                                // Only migrate if the new path doesn't already exist
                                if !new_cache_dir.exists() {
                                    match tokio::fs::rename(&old_cache_dir, &new_cache_dir).await {
                                        Ok(()) => {
                                            info!(
                                                "Migrated workspace cache: {} -> {} ({})",
                                                old_workspace_id,
                                                new_workspace_id,
                                                workspace_root.display()
                                            );

                                            // Update the reverse mapping
                                            {
                                                let mut mapping =
                                                    self.workspace_id_to_root.write().await;
                                                mapping.remove(&old_workspace_id);
                                                mapping.insert(
                                                    new_workspace_id.clone(),
                                                    workspace_root.clone(),
                                                );
                                            }

                                            // Update the open caches map if the old cache was open
                                            {
                                                let mut caches = self.open_caches.write().await;
                                                if let Some(cache) =
                                                    caches.remove(&old_workspace_id)
                                                {
                                                    caches.insert(new_workspace_id, cache);
                                                }
                                            }

                                            migrated_count += 1;
                                        }
                                        Err(e) => {
                                            warn!(
                                                "Failed to migrate cache {} to {}: {}",
                                                old_workspace_id, new_workspace_id, e
                                            );
                                            skipped_count += 1;
                                        }
                                    }
                                } else {
                                    debug!(
                                        "Target cache directory {} already exists, skipping migration",
                                        new_cache_dir.display()
                                    );
                                    skipped_count += 1;
                                }
                            } else {
                                debug!(
                                    "No git-based ID available for workspace {}, keeping existing ID",
                                    workspace_root.display()
                                );
                                skipped_count += 1;
                            }
                        }
                        Ok(None) | Err(_) => {
                            debug!(
                                "No git remote found for workspace {}, keeping hash-based ID",
                                workspace_root.display()
                            );
                            skipped_count += 1;
                        }
                    }
                }
                Err(_) => {
                    debug!(
                        "Not a git repository: {}, keeping hash-based ID",
                        workspace_root.display()
                    );
                    skipped_count += 1;
                }
            }
        }

        if migrated_count > 0 || skipped_count > 0 {
            info!(
                "Workspace cache migration completed: {} migrated, {} skipped",
                migrated_count, skipped_count
            );
        }

        Ok(())
    }
}

// Maintain compatibility by re-exporting the old type name
pub use WorkspaceDatabaseRouter as WorkspaceCacheRouter;
pub use WorkspaceDatabaseRouterConfig as WorkspaceCacheRouterConfig;
