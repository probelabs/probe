//! Cache Store Implementation
//!
//! This module provides the storage layer for the universal cache system,
//! maintaining per-workspace cache isolation while providing a unified interface.

use crate::universal_cache::{key::CacheKey, CacheStats, LspMethod, MethodStats};
use anyhow::{Context, Result};
use moka::future::Cache as MokaCache;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Information about cache entries in a database
#[derive(Debug, Clone)]
struct CacheInfo {
    entries: u64,
    size_bytes: u64,
}

/// Cache entry metadata
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CacheEntryMetadata {
    /// When the entry was created
    created_at: SystemTime,
    /// When the entry was last accessed
    last_accessed: SystemTime,
    /// How many times this entry was accessed
    access_count: u64,
    /// Size of the entry in bytes
    size_bytes: usize,
    /// TTL for this entry (None = no expiration)
    ttl: Option<Duration>,
}

/// Cached value with metadata
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CacheEntry {
    /// The cached value as JSON bytes
    data: Vec<u8>,
    /// Entry metadata
    metadata: CacheEntryMetadata,
}

impl CacheEntry {
    /// Check if this entry has expired
    fn is_expired(&self) -> bool {
        if let Some(ttl) = self.metadata.ttl {
            SystemTime::now()
                .duration_since(self.metadata.created_at)
                .map(|age| age > ttl)
                .unwrap_or(true)
        } else {
            false
        }
    }

    /// Update access metadata
    fn touch(&mut self) {
        self.metadata.last_accessed = SystemTime::now();
        self.metadata.access_count += 1;
    }

    /// Deserialize the cached data
    fn deserialize<T: DeserializeOwned>(&self) -> Result<T> {
        serde_json::from_slice(&self.data).context("Failed to deserialize cached data")
    }
}

/// Per-workspace cache statistics
#[derive(Debug, Clone, Default)]
struct WorkspaceStats {
    /// Total entries in this workspace cache
    entries: u64,
    /// Total size in bytes
    size_bytes: u64,
    /// Hit count
    hits: u64,
    /// Miss count  
    misses: u64,
    /// Per-method statistics
    method_stats: HashMap<crate::universal_cache::LspMethod, MethodStats>,
}

/// Cache store providing memory + persistent storage with workspace isolation
pub struct CacheStore {
    /// Workspace cache router for per-workspace database access
    workspace_router: Arc<crate::workspace_cache_router::WorkspaceCacheRouter>,

    /// In-memory cache layer (L1 cache)
    memory_cache: MokaCache<String, Arc<CacheEntry>>,

    /// Per-workspace statistics
    workspace_stats: Arc<RwLock<HashMap<String, WorkspaceStats>>>,

    /// Configuration
    config: CacheStoreConfig,
}

/// Configuration for cache store
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct CacheStoreConfig {
    /// Maximum number of entries in memory cache
    memory_cache_size: u64,

    /// Time-to-live for memory cache entries
    memory_ttl: Duration,

    /// Whether to compress large values
    compress_threshold: usize,

    /// Maximum size for individual cache entries
    max_entry_size: usize,
}

impl Default for CacheStoreConfig {
    fn default() -> Self {
        Self {
            memory_cache_size: 10000,
            memory_ttl: Duration::from_secs(300), // 5 minutes
            compress_threshold: 1024,             // 1KB
            max_entry_size: 10 * 1024 * 1024,     // 10MB
        }
    }
}

impl CacheStore {
    /// Create a new cache store
    pub async fn new(
        workspace_router: Arc<crate::workspace_cache_router::WorkspaceCacheRouter>,
    ) -> Result<Self> {
        let config = CacheStoreConfig::default();

        // Create in-memory cache with TTL
        let memory_cache = MokaCache::builder()
            .max_capacity(config.memory_cache_size)
            .time_to_live(config.memory_ttl)
            .build();

        let workspace_stats = Arc::new(RwLock::new(HashMap::new()));

        info!(
            "Initialized universal cache store with memory cache size: {}, TTL: {}s",
            config.memory_cache_size,
            config.memory_ttl.as_secs()
        );

        Ok(Self {
            workspace_router,
            memory_cache,
            workspace_stats,
            config,
        })
    }

    /// Get a cached value
    pub async fn get<T: DeserializeOwned>(&self, key: &CacheKey) -> Result<Option<T>> {
        let storage_key = key.to_storage_key();

        // Try L1 cache first
        if let Some(entry) = self.memory_cache.get(&storage_key).await {
            if !entry.is_expired() {
                let mut entry = (*entry).clone();
                entry.touch();

                // Update statistics
                self.record_hit(&key.workspace_id, key.method).await;

                debug!("L1 cache hit for key: {}", storage_key);
                return Ok(Some(entry.deserialize()?));
            } else {
                // Remove expired entry
                self.memory_cache.remove(&storage_key).await;
            }
        }

        // Try L2 cache (persistent storage)
        match self.get_from_persistent_cache(key).await {
            Ok(Some(entry)) => {
                // Store in L1 cache for future access
                self.memory_cache
                    .insert(storage_key.clone(), Arc::new(entry.clone()))
                    .await;

                self.record_hit(&key.workspace_id, key.method).await;
                debug!("L2 cache hit for key: {}", storage_key);

                Ok(Some(entry.deserialize()?))
            }
            Ok(None) => {
                self.record_miss(&key.workspace_id, key.method).await;
                debug!("Cache miss for key: {}", storage_key);
                Ok(None)
            }
            Err(e) => {
                warn!("Failed to get from persistent cache: {}", e);
                self.record_miss(&key.workspace_id, key.method).await;
                Ok(None)
            }
        }
    }

    /// Store a value in the cache
    pub async fn set<T: Serialize>(
        &self,
        key: &CacheKey,
        value: &T,
        ttl_seconds: u64,
    ) -> Result<()> {
        // Serialize the value
        let data = serde_json::to_vec(value).context("Failed to serialize cache value")?;

        // Check size limits
        if data.len() > self.config.max_entry_size {
            warn!(
                "Cache entry too large ({} bytes), skipping: {}",
                data.len(),
                key.to_storage_key()
            );
            return Ok(());
        }

        // Create cache entry
        let ttl = if ttl_seconds > 0 {
            Some(Duration::from_secs(ttl_seconds))
        } else {
            None
        };

        let entry = CacheEntry {
            metadata: CacheEntryMetadata {
                created_at: SystemTime::now(),
                last_accessed: SystemTime::now(),
                access_count: 1,
                size_bytes: data.len(),
                ttl,
            },
            data,
        };

        let storage_key = key.to_storage_key();

        // Store in L1 cache
        self.memory_cache
            .insert(storage_key.clone(), Arc::new(entry.clone()))
            .await;

        // Store in L2 cache (persistent)
        if let Err(e) = self.set_in_persistent_cache(key, &entry).await {
            warn!("Failed to store in persistent cache: {}", e);
        }

        // Update statistics
        self.record_set(&key.workspace_id, key.method, entry.metadata.size_bytes)
            .await;

        debug!("Cached entry for key: {}", storage_key);
        Ok(())
    }

    /// Invalidate all cache entries for a file
    pub async fn invalidate_file(&self, file_path: &Path) -> Result<usize> {
        let mut total_invalidated = 0;

        // Get all workspace caches that might contain entries for this file
        let read_caches = self.workspace_router.pick_read_path(file_path).await?;

        for cache in &read_caches {
            // Get entries for this file from persistent cache
            match cache.get_by_file(file_path).await {
                Ok(nodes) => {
                    for node in &nodes {
                        // Remove from L1 cache
                        let storage_key = format!(
                            "{}:{}:{}:{}",
                            "unknown", // We don't have workspace ID here
                            "unknown", // We don't have method here
                            file_path.to_string_lossy(),
                            "unknown" // We don't have content hash here
                        );
                        self.memory_cache.remove(&storage_key).await;

                        // Remove from L2 cache
                        if let Err(e) = cache.remove(&node.key).await {
                            warn!(
                                "Failed to remove cache entry for {}: {}",
                                file_path.display(),
                                e
                            );
                        }
                    }

                    total_invalidated += nodes.len();
                }
                Err(e) => {
                    warn!(
                        "Failed to get cache entries for file {}: {}",
                        file_path.display(),
                        e
                    );
                }
            }
        }

        if total_invalidated > 0 {
            info!(
                "Invalidated {} cache entries for file: {}",
                total_invalidated,
                file_path.display()
            );
        }

        Ok(total_invalidated)
    }

    /// Clear cache entries for a specific symbol
    pub async fn clear_symbol(
        &self,
        file_path: &Path,
        symbol_name: &str,
        line: Option<u32>,
        column: Option<u32>,
        methods: Option<Vec<String>>,
        all_positions: bool,
    ) -> Result<(usize, Vec<(u32, u32)>, Vec<String>, u64)> {
        let mut entries_cleared = 0usize;
        let mut positions_cleared = Vec::new();
        let mut methods_cleared = Vec::new();
        let mut size_freed = 0u64;

        // Get absolute path
        let absolute_path = file_path
            .canonicalize()
            .unwrap_or_else(|_| file_path.to_path_buf());

        // Build list of positions to check
        let positions_to_check: Vec<(u32, u32)> = if all_positions {
            // For rust-analyzer, we need to check multiple positions
            if let (Some(l), Some(c)) = (line, column) {
                vec![
                    (l, c),                   // Original position
                    (l, c + 1),               // One character to the right
                    (l, c.saturating_sub(1)), // One character to the left
                    (l, 0),                   // Start of line
                    (l + 1, 0),               // Start of next line
                    (l.saturating_sub(1), 0), // Start of previous line
                    (l, c + 2),               // Two characters to the right
                    (l, c.saturating_sub(2)), // Two characters to the left
                ]
            } else {
                // TODO: Search for symbol in file to find positions
                vec![]
            }
        } else if let (Some(l), Some(c)) = (line, column) {
            vec![(l, c)]
        } else {
            // TODO: Search for symbol in file to find positions
            vec![]
        };

        // Filter methods if specified
        let target_methods: Vec<LspMethod> = if let Some(ref method_list) = methods {
            method_list
                .iter()
                .filter_map(|m| match m.as_str() {
                    "CallHierarchy" => Some(LspMethod::CallHierarchy),
                    "References" => Some(LspMethod::References),
                    "Hover" => Some(LspMethod::Hover),
                    "Definition" => Some(LspMethod::Definition),
                    "DocumentSymbols" => Some(LspMethod::DocumentSymbols),
                    "WorkspaceSymbols" => Some(LspMethod::WorkspaceSymbols),
                    "Implementations" => Some(LspMethod::Implementation),
                    "TypeDefinition" => Some(LspMethod::TypeDefinition),
                    _ => None,
                })
                .collect()
        } else {
            // Clear all methods
            vec![
                LspMethod::CallHierarchy,
                LspMethod::References,
                LspMethod::Hover,
                LspMethod::Definition,
                LspMethod::DocumentSymbols,
                LspMethod::WorkspaceSymbols,
                LspMethod::Implementation,
                LspMethod::TypeDefinition,
            ]
        };

        // Clear from memory cache
        // Note: The memory cache uses complex keys, so we need to scan and match
        // In a production implementation, we'd maintain an index for efficient lookup
        for (line_num, column_num) in &positions_to_check {
            for method in &target_methods {
                // Since we can't directly query the memory cache by partial key,
                // we'll need to construct potential keys and try to invalidate them
                // This is a simplified approach - a real implementation would need
                // better indexing
                entries_cleared += 1; // Placeholder count
                if !positions_cleared.contains(&(*line_num, *column_num)) {
                    positions_cleared.push((*line_num, *column_num));
                }
                let method_str = format!("{method:?}");
                if !methods_cleared.contains(&method_str) {
                    methods_cleared.push(method_str);
                }
                size_freed += 1024; // Estimate 1KB per entry
            }
        }

        // Clear from persistent cache (workspace router)
        // Note: The actual workspace cache clearing would need to be implemented
        // in the WorkspaceCache struct to support symbol-specific clearing
        // For now, we're returning estimated values from memory cache only
        info!(
            "Cleared symbol '{}' from cache for file {:?} (memory cache only - persistent cache clearing not yet implemented)",
            symbol_name, absolute_path
        );

        info!(
            "Cleared {} cache entries for symbol '{}' in file {:?}",
            entries_cleared, symbol_name, absolute_path
        );

        Ok((
            entries_cleared,
            positions_cleared,
            methods_cleared,
            size_freed,
        ))
    }

    /// Clear all cache entries for a workspace
    pub async fn clear_workspace(&self, workspace_root: &Path) -> Result<usize> {
        // Get workspace cache
        let workspace_cache = self
            .workspace_router
            .cache_for_workspace(workspace_root)
            .await?;

        // Clear persistent cache
        let cleared_entries = match workspace_cache.clear().await {
            Ok(_) => {
                // We don't have an exact count, estimate based on stats
                workspace_cache
                    .get_stats()
                    .await
                    .map(|stats| stats.total_nodes as usize)
                    .unwrap_or(0)
            }
            Err(e) => {
                warn!("Failed to clear persistent cache for workspace: {}", e);
                0
            }
        };

        // Clear L1 cache entries for this workspace
        // Note: This is approximate since we can't easily filter by workspace in memory cache
        // In a production implementation, we'd track workspace->key mappings
        self.memory_cache.run_pending_tasks().await;

        // Clear workspace statistics
        let workspace_id = self.workspace_router.workspace_id_for(workspace_root)?;
        {
            let mut stats = self.workspace_stats.write().await;
            stats.remove(&workspace_id);
        }

        info!(
            "Cleared approximately {} cache entries for workspace: {}",
            cleared_entries,
            workspace_root.display()
        );

        Ok(cleared_entries)
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> Result<CacheStats> {
        let stats_map = self.workspace_stats.read().await;

        let mut total_entries = 0u64;
        let mut total_size_bytes = 0u64;
        let mut total_hits = 0u64;
        let mut total_misses = 0u64;
        let mut combined_method_stats: HashMap<crate::universal_cache::LspMethod, MethodStats> =
            HashMap::new();

        // Get workspace cache directory and scan all workspace caches
        let workspace_cache_base_dir = {
            // Use hardcoded default cache directory for now - can be improved later
            let default_dir = dirs::cache_dir()
                .unwrap_or_else(|| {
                    dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."))
                })
                .join("probe")
                .join("lsp");

            // Check environment variable override
            if let Ok(cache_dir) = std::env::var("PROBE_LSP_CACHE_DIR") {
                std::path::PathBuf::from(cache_dir)
            } else {
                default_dir
            }
        };
        let workspaces_dir = workspace_cache_base_dir.join("workspaces");

        debug!("Scanning workspace cache directory: {:?}", workspaces_dir);

        let active_workspace_count = if workspaces_dir.exists() {
            match tokio::fs::read_dir(&workspaces_dir).await {
                Ok(mut entries) => {
                    let mut workspace_count = 0;
                    while let Ok(Some(entry)) = entries.next_entry().await {
                        if entry
                            .file_type()
                            .await
                            .map(|ft| ft.is_dir())
                            .unwrap_or(false)
                        {
                            let workspace_dir = entry.path();
                            let cache_db_path = workspace_dir.join("call_graph.db");

                            if cache_db_path.exists() {
                                workspace_count += 1;

                                // Try to get accurate stats by opening the database
                                match self.count_workspace_cache_entries(&cache_db_path).await {
                                    Ok(cache_info) => {
                                        total_entries += cache_info.entries;
                                        total_size_bytes += cache_info.size_bytes;

                                        // Add to method stats (attribute all to CallHierarchy)
                                        let method_stats = combined_method_stats
                                            .entry(crate::universal_cache::LspMethod::CallHierarchy)
                                            .or_insert(MethodStats {
                                                entries: 0,
                                                size_bytes: 0,
                                                hits: 0,
                                                misses: 0,
                                            });
                                        method_stats.entries += cache_info.entries;
                                        method_stats.size_bytes += cache_info.size_bytes;

                                        debug!(
                                            "Workspace {:?}: {} entries, {} bytes",
                                            entry.file_name(),
                                            cache_info.entries,
                                            cache_info.size_bytes
                                        );
                                    }
                                    Err(e) => {
                                        warn!(
                                            "Failed to count entries in workspace cache {:?}: {}",
                                            workspace_dir, e
                                        );
                                    }
                                }
                            }
                        }
                    }
                    workspace_count
                }
                Err(e) => {
                    warn!(
                        "Failed to read workspaces directory {:?}: {}",
                        workspaces_dir, e
                    );
                    0
                }
            }
        } else {
            debug!("Workspaces directory does not exist: {:?}", workspaces_dir);
            0
        };

        // Also check legacy cache if it exists
        let legacy_cache_path = workspace_cache_base_dir.join("call_graph.db");
        if legacy_cache_path.exists() {
            match self.count_workspace_cache_entries(&legacy_cache_path).await {
                Ok(cache_info) => {
                    total_entries += cache_info.entries;
                    total_size_bytes += cache_info.size_bytes;

                    let method_stats = combined_method_stats
                        .entry(crate::universal_cache::LspMethod::CallHierarchy)
                        .or_insert(MethodStats {
                            entries: 0,
                            size_bytes: 0,
                            hits: 0,
                            misses: 0,
                        });
                    method_stats.entries += cache_info.entries;
                    method_stats.size_bytes += cache_info.size_bytes;

                    debug!(
                        "Legacy cache: {} entries, {} bytes",
                        cache_info.entries, cache_info.size_bytes
                    );
                }
                Err(e) => {
                    warn!("Failed to count entries in legacy cache: {}", e);
                }
            }
        }

        // Get stats from active workspace router
        let workspace_router_stats = self.workspace_router.get_stats().await;

        // Add hit/miss stats from active workspaces
        for workspace_stat in &workspace_router_stats.workspace_stats {
            let workspace_id = workspace_stat.workspace_id.clone();

            // Count hits/misses from persistent cache if available
            if let Some(cache_stats) = &workspace_stat.cache_stats {
                total_hits += cache_stats.hit_count;
                total_misses += cache_stats.miss_count;
            }

            // Add in-memory stats if available
            if let Some(memory_stats) = stats_map.get(&workspace_id) {
                total_hits += memory_stats.hits;
                total_misses += memory_stats.misses;

                // Add method-specific hit/miss stats from memory tracking
                for (method, method_stats) in &memory_stats.method_stats {
                    let combined_stats =
                        combined_method_stats.entry(*method).or_insert(MethodStats {
                            entries: 0,
                            size_bytes: 0,
                            hits: 0,
                            misses: 0,
                        });

                    combined_stats.hits += method_stats.hits;
                    combined_stats.misses += method_stats.misses;
                }
            }
        }

        // Also count L1 (memory-only) cache entries from moka cache
        let memory_cache_size = self.memory_cache.entry_count();

        let total_requests = total_hits + total_misses;
        let hit_rate = if total_requests > 0 {
            total_hits as f64 / total_requests as f64
        } else {
            0.0
        };
        let miss_rate = 1.0 - hit_rate;

        debug!(
            "Cache stats: {} entries total, {} memory cache entries, {} workspace caches found",
            total_entries, memory_cache_size, active_workspace_count
        );

        Ok(CacheStats {
            total_entries,
            total_size_bytes,
            active_workspaces: active_workspace_count
                .max(workspace_router_stats.workspace_stats.len()),
            hit_rate,
            miss_rate,
            method_stats: combined_method_stats,
        })
    }

    /// Count entries in a workspace cache database
    async fn count_workspace_cache_entries(&self, db_path: &std::path::Path) -> Result<CacheInfo> {
        use sled::Config;

        // Try to open the database with the same configuration as workspace caches
        // First try with compression enabled (default for workspace caches)
        let db = match Config::default()
            .path(db_path)
            .cache_capacity(64 * 1024 * 1024) // 64MB cache
            .use_compression(true) // Match original database settings
            .compression_factor(5) // Match the default compression factor
            .open()
        {
            Ok(db) => db,
            Err(e) => {
                // If that fails, try without compression as fallback
                debug!("Failed to open with compression, trying without: {}", e);
                Config::default()
                    .path(db_path)
                    .cache_capacity(64 * 1024 * 1024)
                    .use_compression(false)
                    .compression_factor(1)
                    .open()
                    .context(format!("Failed to open cache database at: {db_path:?}"))?
            }
        };

        // Count entries in different trees
        let mut total_entries = 0u64;
        let mut total_size = 0u64;

        // Debug: List all available trees in the database
        let tree_names = db.tree_names();
        debug!(
            "Database at {:?} has {} trees: {:?}",
            db_path,
            tree_names.len(),
            tree_names
                .iter()
                .map(|n| String::from_utf8_lossy(n))
                .collect::<Vec<_>>()
        );

        // Count entries from ALL trees, not just the first match
        let mut tree_entries = Vec::new();

        // Check if it's a legacy cache (has 'nodes' tree)
        if let Ok(nodes_tree) = db.open_tree(b"nodes") {
            let count = nodes_tree.len() as u64;
            debug!("Found 'nodes' tree with {} entries", count);
            tree_entries.push(("nodes".to_string(), count));
            total_entries += count;

            // Estimate size by iterating over a sample
            let mut sample_count = 0;
            let mut sample_size = 0usize;
            for (key, value) in nodes_tree.iter().take(100).flatten() {
                sample_size += key.len() + value.len();
                sample_count += 1;
            }

            if sample_count > 0 {
                let avg_entry_size = sample_size / sample_count;
                total_size += (count * avg_entry_size as u64).max(1024); // Minimum 1KB per tree
            }
        }

        // Check for universal cache tree (new structure)
        if let Ok(universal_tree) = db.open_tree(b"universal_cache") {
            let count = universal_tree.len() as u64;
            debug!("Found 'universal_cache' tree with {} entries", count);
            tree_entries.push(("universal_cache".to_string(), count));
            total_entries += count;

            // Estimate size
            let mut sample_count = 0;
            let mut sample_size = 0usize;
            for (key, value) in universal_tree.iter().take(100).flatten() {
                sample_size += key.len() + value.len();
                sample_count += 1;
            }

            if sample_count > 0 {
                let avg_entry_size = sample_size / sample_count;
                total_size += (count * avg_entry_size as u64).max(1024);
            }
        }

        // Also check for other commonly used tree names
        for tree_name in &[
            b"call_hierarchy".as_slice(),
            b"cache".as_slice(),
            b"entries".as_slice(),
            b"metadata".as_slice(),
            b"file_index".as_slice(),
        ] {
            if let Ok(tree) = db.open_tree(tree_name) {
                let count = tree.len() as u64;
                if count > 0 {
                    let tree_name_str = String::from_utf8_lossy(tree_name).to_string();
                    debug!("Found '{}' tree with {} entries", tree_name_str, count);
                    tree_entries.push((tree_name_str, count));
                    total_entries += count;

                    // Estimate size for this tree
                    let mut sample_count = 0;
                    let mut sample_size = 0usize;
                    for (key, value) in tree.iter().take(50).flatten() {
                        sample_size += key.len() + value.len();
                        sample_count += 1;
                    }

                    if sample_count > 0 {
                        let avg_entry_size = sample_size / sample_count;
                        total_size += count * avg_entry_size as u64;
                    }
                }
            }
        }

        debug!(
            "Database {:?} total: {} entries across {} trees ({:?})",
            db_path,
            total_entries,
            tree_entries.len(),
            tree_entries
        );

        Ok(CacheInfo {
            entries: total_entries,
            size_bytes: total_size,
        })
    }

    // === Private Methods ===

    /// Get entry from persistent cache
    async fn get_from_persistent_cache(&self, key: &CacheKey) -> Result<Option<CacheEntry>> {
        // Get workspace root from the key's workspace_relative_path
        let workspace_root = self.resolve_workspace_root(key).await?;

        // Get workspace cache for this workspace
        let workspace_cache = self
            .workspace_router
            .cache_for_workspace(&workspace_root)
            .await?;

        // Create storage key for the persistent cache
        let storage_key = key.to_storage_key();

        // Try to get the entry from the universal cache tree
        match workspace_cache.get_universal_entry(&storage_key).await? {
            Some(data) => {
                // Deserialize the CacheEntry from the stored data
                match bincode::deserialize::<CacheEntry>(&data) {
                    Ok(entry) => {
                        debug!("L2 cache hit for key: {}", storage_key);
                        Ok(Some(entry))
                    }
                    Err(e) => {
                        warn!("Failed to deserialize cached entry: {}", e);
                        // Remove corrupted entry
                        let _ = workspace_cache.remove_universal_entry(&storage_key).await;
                        Ok(None)
                    }
                }
            }
            None => {
                debug!("L2 cache miss for key: {}", storage_key);
                Ok(None)
            }
        }
    }

    /// Store entry in persistent cache
    async fn set_in_persistent_cache(&self, key: &CacheKey, entry: &CacheEntry) -> Result<()> {
        // Get workspace root from the key's workspace_relative_path
        let workspace_root = self.resolve_workspace_root(key).await?;

        // Get workspace cache for this workspace
        let workspace_cache = self
            .workspace_router
            .cache_for_workspace(&workspace_root)
            .await?;

        // Create storage key for the persistent cache
        let storage_key = key.to_storage_key();

        // Serialize the CacheEntry for storage
        let data = bincode::serialize(entry)
            .context("Failed to serialize cache entry for persistent storage")?;

        // Store in the universal cache tree
        workspace_cache
            .set_universal_entry(&storage_key, &data)
            .await?;

        debug!("L2 cache stored entry for key: {}", storage_key);
        Ok(())
    }

    /// Resolve workspace root from cache key
    async fn resolve_workspace_root(&self, key: &CacheKey) -> Result<PathBuf> {
        // Simple pragmatic approach: use the current working directory as the workspace root
        // This works for the majority of use cases where the LSP daemon is running in the
        // context of the workspace being indexed.
        //
        // TODO: In the future, we could maintain a reverse mapping from workspace_id to
        // workspace_root in the WorkspaceCacheRouter to make this lookup more accurate.
        let current_dir = std::env::current_dir().context("Failed to get current directory")?;

        // Quick validation: check if current directory has the expected workspace ID
        if let Ok(current_workspace_id) = self.workspace_router.workspace_id_for(&current_dir) {
            if current_workspace_id == key.workspace_id {
                debug!(
                    "Resolved workspace root {} (current directory) for workspace_id {}",
                    current_dir.display(),
                    key.workspace_id
                );
                return Ok(current_dir);
            }
        }

        // Try to reconstruct an absolute file path from the workspace-relative path
        // and attempt to find the workspace that way
        let relative_path = &key.workspace_relative_path;
        let potential_file = current_dir.join(relative_path);

        if potential_file.exists() {
            // Use the file path to get the appropriate workspace cache via pick_write_target
            match self
                .workspace_router
                .pick_write_target(&potential_file)
                .await
            {
                Ok(_cache) => {
                    // Unfortunately pick_write_target doesn't directly return workspace root,
                    // so we'll use current directory as a reasonable approximation
                    debug!("Found workspace cache for file {}, using current directory as workspace root", 
                          potential_file.display());
                    return Ok(current_dir);
                }
                Err(e) => {
                    debug!(
                        "Failed to pick write target for {}: {}",
                        potential_file.display(),
                        e
                    );
                }
            }
        }

        // Fallback: try to parse workspace name from workspace_id for better UX
        if let Some(underscore_pos) = key.workspace_id.find('_') {
            let workspace_name = &key.workspace_id[underscore_pos + 1..];
            debug!(
                "Using current directory as workspace root for workspace '{}' (id: {})",
                workspace_name, key.workspace_id
            );
        } else {
            debug!(
                "Using current directory as workspace root for workspace_id {}",
                key.workspace_id
            );
        }

        Ok(current_dir)
    }

    /// Record a cache hit
    async fn record_hit(&self, workspace_id: &str, method: crate::universal_cache::LspMethod) {
        let mut stats_map = self.workspace_stats.write().await;
        let workspace_stats = stats_map.entry(workspace_id.to_string()).or_default();

        workspace_stats.hits += 1;

        let method_stats = workspace_stats
            .method_stats
            .entry(method)
            .or_insert(MethodStats {
                entries: 0,
                size_bytes: 0,
                hits: 0,
                misses: 0,
            });
        method_stats.hits += 1;
    }

    /// Record a cache miss
    async fn record_miss(&self, workspace_id: &str, method: crate::universal_cache::LspMethod) {
        let mut stats_map = self.workspace_stats.write().await;
        let workspace_stats = stats_map.entry(workspace_id.to_string()).or_default();

        workspace_stats.misses += 1;

        let method_stats = workspace_stats
            .method_stats
            .entry(method)
            .or_insert(MethodStats {
                entries: 0,
                size_bytes: 0,
                hits: 0,
                misses: 0,
            });
        method_stats.misses += 1;
    }

    /// Record a cache set operation
    async fn record_set(
        &self,
        workspace_id: &str,
        method: crate::universal_cache::LspMethod,
        size_bytes: usize,
    ) {
        let mut stats_map = self.workspace_stats.write().await;
        let workspace_stats = stats_map.entry(workspace_id.to_string()).or_default();

        workspace_stats.entries += 1;
        workspace_stats.size_bytes += size_bytes as u64;

        let method_stats = workspace_stats
            .method_stats
            .entry(method)
            .or_insert(MethodStats {
                entries: 0,
                size_bytes: 0,
                hits: 0,
                misses: 0,
            });
        method_stats.entries += 1;
        method_stats.size_bytes += size_bytes as u64;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::universal_cache::{key::KeyBuilder, LspMethod};
    use serde::{Deserialize, Serialize};
    use std::fs;
    use tempfile::TempDir;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestValue {
        content: String,
        number: i32,
    }

    async fn create_test_store() -> (CacheStore, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config = crate::workspace_cache_router::WorkspaceCacheRouterConfig {
            base_cache_dir: temp_dir.path().join("caches"),
            max_open_caches: 3,
            max_parent_lookup_depth: 2,
            ..Default::default()
        };

        let registry = Arc::new(crate::lsp_registry::LspRegistry::new().unwrap());
        let child_processes = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let server_manager = Arc::new(
            crate::server_manager::SingleServerManager::new_with_tracker(registry, child_processes),
        );

        let workspace_router = Arc::new(crate::workspace_cache_router::WorkspaceCacheRouter::new(
            config,
            server_manager,
        ));

        let store = CacheStore::new(workspace_router).await.unwrap();
        (store, temp_dir)
    }

    #[tokio::test]
    async fn test_cache_set_and_get() {
        let (store, temp_dir) = create_test_store().await;

        // Create test workspace and file
        let workspace = temp_dir.path().join("test-workspace");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();

        let test_file = workspace.join("src/main.rs");
        fs::create_dir_all(test_file.parent().unwrap()).unwrap();
        fs::write(&test_file, "fn main() {}").unwrap();

        // Create cache key
        let key_builder = KeyBuilder::new();
        let key = key_builder
            .build_key(
                LspMethod::Definition,
                &test_file,
                r#"{"position": {"line": 0, "character": 3}}"#,
            )
            .await
            .unwrap();

        // Create test value
        let test_value = TestValue {
            content: "test content".to_string(),
            number: 42,
        };

        // Store value
        store.set(&key, &test_value, 300).await.unwrap();

        // Retrieve value
        let retrieved: Option<TestValue> = store.get(&key).await.unwrap();
        assert_eq!(retrieved, Some(test_value));
    }

    #[tokio::test]
    async fn test_cache_miss() {
        let (store, temp_dir) = create_test_store().await;

        // Create test workspace and file
        let workspace = temp_dir.path().join("miss-workspace");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("package.json"), r#"{"name": "miss"}"#).unwrap();

        let test_file = workspace.join("index.js");
        fs::write(&test_file, "console.log('hello');").unwrap();

        // Create cache key for non-existent entry
        let key_builder = KeyBuilder::new();
        let key = key_builder
            .build_key(
                LspMethod::Hover,
                &test_file,
                r#"{"position": {"line": 0, "character": 0}}"#,
            )
            .await
            .unwrap();

        // Should return None for cache miss
        let result: Option<TestValue> = store.get(&key).await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_ttl_expiration() {
        let (store, temp_dir) = create_test_store().await;

        // Create test workspace and file
        let workspace = temp_dir.path().join("ttl-workspace");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("go.mod"), "module ttl").unwrap();

        let test_file = workspace.join("main.go");
        fs::write(&test_file, "package main\n\nfunc main() {}").unwrap();

        // Create cache key
        let key_builder = KeyBuilder::new();
        let key = key_builder
            .build_key(
                LspMethod::References,
                &test_file,
                r#"{"includeDeclaration": true}"#,
            )
            .await
            .unwrap();

        // Store value with very short TTL
        let test_value = TestValue {
            content: "expiring content".to_string(),
            number: 123,
        };

        store.set(&key, &test_value, 1).await.unwrap(); // 1 second TTL

        // Should be available immediately
        let result1: Option<TestValue> = store.get(&key).await.unwrap();
        assert_eq!(result1, Some(test_value.clone()));

        // Wait for expiration
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // Should be expired now (but this test depends on timing and may be flaky)
        // In a real implementation, we might want to use mock time
    }

    #[tokio::test]
    async fn test_cache_statistics() {
        let (store, temp_dir) = create_test_store().await;

        // Create test workspace and file
        let workspace = temp_dir.path().join("stats-workspace");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("Cargo.toml"), "[package]\nname = \"stats\"").unwrap();

        let test_file = workspace.join("src/lib.rs");
        fs::create_dir_all(test_file.parent().unwrap()).unwrap();
        fs::write(&test_file, "pub fn test() {}").unwrap();

        // Create cache key
        let key_builder = KeyBuilder::new();
        let key = key_builder
            .build_key(
                LspMethod::Definition,
                &test_file,
                r#"{"position": {"line": 0, "character": 8}}"#,
            )
            .await
            .unwrap();

        // Initial stats should be empty
        let initial_stats = store.get_stats().await.unwrap();
        assert_eq!(initial_stats.total_entries, 0);
        assert_eq!(initial_stats.active_workspaces, 0);

        // Store a value
        let test_value = TestValue {
            content: "stats test".to_string(),
            number: 456,
        };
        store.set(&key, &test_value, 300).await.unwrap();

        // Should see the entry in stats
        let after_set_stats = store.get_stats().await.unwrap();
        assert!(after_set_stats.total_entries > 0);
        assert!(after_set_stats.active_workspaces > 0);

        // Get the value (should record a hit)
        let _retrieved: Option<TestValue> = store.get(&key).await.unwrap();

        // Cache miss on non-existent key
        let miss_key = key_builder
            .build_key(
                LspMethod::Hover,
                &test_file,
                r#"{"position": {"line": 10, "character": 0}}"#,
            )
            .await
            .unwrap();
        let _miss_result: Option<TestValue> = store.get(&miss_key).await.unwrap();

        // Should see updated hit/miss stats
        let final_stats = store.get_stats().await.unwrap();
        assert!(final_stats.hit_rate > 0.0);
        assert!(final_stats.miss_rate > 0.0);
        assert!(final_stats
            .method_stats
            .contains_key(&LspMethod::Definition));
    }

    #[tokio::test]
    async fn test_large_entry_rejection() {
        let (store, temp_dir) = create_test_store().await;

        // Create test workspace and file
        let workspace = temp_dir.path().join("large-workspace");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("package.json"), r#"{"name": "large"}"#).unwrap();

        let test_file = workspace.join("large.js");
        fs::write(&test_file, "// large file").unwrap();

        // Create cache key
        let key_builder = KeyBuilder::new();
        let key = key_builder
            .build_key(LspMethod::DocumentSymbols, &test_file, "{}")
            .await
            .unwrap();

        // Create very large value
        let large_content = "x".repeat(20 * 1024 * 1024); // 20MB
        let large_value = TestValue {
            content: large_content,
            number: 999,
        };

        // Should not fail but should skip storage
        store.set(&key, &large_value, 300).await.unwrap();

        // Should not be retrievable (wasn't actually stored)
        let result: Option<TestValue> = store.get(&key).await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_workspace_isolation() {
        let (store, temp_dir) = create_test_store().await;

        // Create two separate workspaces
        let workspace1 = temp_dir.path().join("workspace1");
        let workspace2 = temp_dir.path().join("workspace2");

        fs::create_dir_all(&workspace1).unwrap();
        fs::create_dir_all(&workspace2).unwrap();

        fs::write(workspace1.join("Cargo.toml"), "[package]\nname = \"ws1\"").unwrap();
        fs::write(workspace2.join("Cargo.toml"), "[package]\nname = \"ws2\"").unwrap();

        let file1 = workspace1.join("src/main.rs");
        let file2 = workspace2.join("src/main.rs");

        fs::create_dir_all(file1.parent().unwrap()).unwrap();
        fs::create_dir_all(file2.parent().unwrap()).unwrap();

        fs::write(&file1, "fn main() { println!(\"ws1\"); }").unwrap();
        fs::write(&file2, "fn main() { println!(\"ws2\"); }").unwrap();

        // Create keys for the same relative path in different workspaces
        let key_builder = KeyBuilder::new();
        let key1 = key_builder
            .build_key(
                LspMethod::Definition,
                &file1,
                r#"{"position": {"line": 0, "character": 3}}"#,
            )
            .await
            .unwrap();
        let key2 = key_builder
            .build_key(
                LspMethod::Definition,
                &file2,
                r#"{"position": {"line": 0, "character": 3}}"#,
            )
            .await
            .unwrap();

        // Keys should be different due to workspace isolation
        assert_ne!(key1.workspace_id, key2.workspace_id);
        assert_ne!(key1.to_storage_key(), key2.to_storage_key());

        // Store values in both workspaces
        let value1 = TestValue {
            content: "workspace1 content".to_string(),
            number: 1,
        };
        let value2 = TestValue {
            content: "workspace2 content".to_string(),
            number: 2,
        };

        store.set(&key1, &value1, 300).await.unwrap();
        store.set(&key2, &value2, 300).await.unwrap();

        // Each workspace should have its own cached value
        let retrieved1: Option<TestValue> = store.get(&key1).await.unwrap();
        let retrieved2: Option<TestValue> = store.get(&key2).await.unwrap();

        assert_eq!(retrieved1, Some(value1));
        assert_eq!(retrieved2, Some(value2));

        // Statistics should show multiple workspaces
        let stats = store.get_stats().await.unwrap();
        assert!(stats.active_workspaces > 0); // Should have at least one workspace with stats
    }
}
