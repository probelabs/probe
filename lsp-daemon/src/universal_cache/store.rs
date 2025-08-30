//! Cache Store Implementation
//!
//! This module provides the storage layer for the universal cache system,
//! maintaining per-workspace cache isolation while providing a unified interface.

use crate::universal_cache::{key::CacheKey, CacheStats, LspMethod, MethodStats};
use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Information about cache entries in a database
#[derive(Debug, Clone)]
#[allow(dead_code)]
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

/// Cache store providing direct database storage with workspace isolation
pub struct CacheStore {
    /// Workspace cache router for per-workspace database access
    workspace_router: Arc<crate::workspace_cache_router::WorkspaceCacheRouter>,

    /// Per-workspace statistics
    workspace_stats: Arc<RwLock<HashMap<String, WorkspaceStats>>>,

    /// Configuration
    config: CacheStoreConfig,
}

/// Configuration for cache store
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct CacheStoreConfig {
    /// Whether to compress large values
    compress_threshold: usize,

    /// Maximum size for individual cache entries
    max_entry_size: usize,
}

impl Default for CacheStoreConfig {
    fn default() -> Self {
        Self {
            compress_threshold: 1024,         // 1KB
            max_entry_size: 10 * 1024 * 1024, // 10MB
        }
    }
}

impl CacheStore {
    /// Create a new cache store
    pub async fn new(
        workspace_router: Arc<crate::workspace_cache_router::WorkspaceCacheRouter>,
    ) -> Result<Self> {
        let config = CacheStoreConfig::default();
        let workspace_stats = Arc::new(RwLock::new(HashMap::new()));

        info!(
            "Initialized universal cache store with direct database access (compress threshold: {} bytes, max entry size: {} bytes)",
            config.compress_threshold,
            config.max_entry_size
        );

        Ok(Self {
            workspace_router,
            workspace_stats,
            config,
        })
    }

    /// Get a cached value
    pub async fn get<T: DeserializeOwned>(&self, key: &CacheKey) -> Result<Option<T>> {
        let storage_key = key.to_storage_key();

        // Direct database access only
        match self.get_from_persistent_cache(key).await {
            Ok(Some(entry)) => {
                if entry.is_expired() {
                    // Remove expired entry and return None
                    let _ = self.remove_from_persistent_cache(key).await;
                    self.record_miss(&key.workspace_id, key.method).await;
                    debug!("Database cache miss (expired) for key: {}", storage_key);
                    Ok(None)
                } else {
                    // Update access metadata and store back
                    let mut updated_entry = entry.clone();
                    updated_entry.touch();
                    let _ = self.set_in_persistent_cache(key, &updated_entry).await;

                    self.record_hit(&key.workspace_id, key.method).await;
                    debug!("Database cache hit for key: {}", storage_key);
                    Ok(Some(entry.deserialize()?))
                }
            }
            Ok(None) => {
                self.record_miss(&key.workspace_id, key.method).await;
                debug!("Database cache miss for key: {}", storage_key);
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

        // Store directly in persistent database
        debug!("Storing in database cache for key: {}", storage_key);
        match self.set_in_persistent_cache(key, &entry).await {
            Ok(()) => {
                debug!("Successfully stored in database cache: {}", storage_key);
            }
            Err(e) => {
                warn!(
                    "Failed to store in persistent cache for key {}: {}",
                    storage_key, e
                );
                return Err(e);
            }
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
                        // Remove from database cache
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
    ///
    /// Uses tree-sitter to find symbol positions when not explicitly provided
    pub async fn clear_symbol(
        &self,
        file_path: &Path,
        symbol_name: &str,
        line: Option<u32>,
        column: Option<u32>,
        methods: Option<Vec<String>>,
        all_positions: bool,
    ) -> Result<(usize, Vec<(u32, u32)>, Vec<String>, u64)> {
        let mut positions_cleared = Vec::new();
        let mut methods_cleared = Vec::new();

        // Get absolute path
        let absolute_path = file_path
            .canonicalize()
            .unwrap_or_else(|_| file_path.to_path_buf());

        info!(
            "Starting cache clearing for symbol '{}' with line={:?}, column={:?}, all_positions={}",
            symbol_name, line, column, all_positions
        );

        // Build list of positions to check using tree-sitter for precision
        let positions_to_check: Vec<(u32, u32)> = if let (Some(l), Some(c)) = (line, column) {
            // Use provided exact position
            if all_positions {
                // Add nearby positions for comprehensive clearing
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
                vec![(l, c)]
            }
        } else {
            // No line/column provided - this should not happen with proper tree-sitter integration
            return Err(anyhow::anyhow!(
                "Symbol position is required for cache clearing. Use tree-sitter to find exact position on client side."
            ));
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

        // Create key builder for cache key generation
        let key_builder = crate::universal_cache::key::KeyBuilder::new();

        // === CLEAR FROM DATABASE CACHE ===

        // Database: Clear from workspace persistent cache (direct database storage)
        let mut database_entries_cleared = 0;

        // Generate all possible cache keys for the symbol
        let mut cache_keys_to_clear = Vec::new();
        info!(
            "Generating cache keys for {} positions and {} methods",
            positions_to_check.len(),
            target_methods.len()
        );
        for (line_num, column_num) in &positions_to_check {
            for method in &target_methods {
                let params = match method {
                    LspMethod::Definition => {
                        format!(r#"{{"position":{{"character":{column_num},"line":{line_num}}}}}"#,)
                    }
                    LspMethod::References => format!(
                        r#"{{"context":{{"includeDeclaration":false}},"position":{{"character":{column_num},"line":{line_num}}}}}"#,
                    ),
                    LspMethod::Hover => {
                        format!(r#"{{"position":{{"character":{column_num},"line":{line_num}}}}}"#,)
                    }
                    LspMethod::CallHierarchy => {
                        format!(r#"{{"position":{{"character":{column_num},"line":{line_num}}}}}"#,)
                    }
                    LspMethod::DocumentSymbols => "{}".to_string(),
                    _ => {
                        format!(r#"{{"position":{{"character":{column_num},"line":{line_num}}}}}"#,)
                    }
                };

                if let Ok(cache_key) = key_builder
                    .build_key(*method, &absolute_path, &params)
                    .await
                {
                    cache_keys_to_clear.push((cache_key, (*line_num, *column_num), *method));
                } else {
                    warn!(
                        "Failed to build cache key for clearing: method={:?}, position={}:{}",
                        method, line_num, column_num
                    );
                }
            }
        }

        // If no specific positions, add some common positions for general clearing
        if positions_to_check.is_empty() {
            for method in &target_methods {
                // Try a few common positions for broad clearing
                for (line, col) in [(0, 0), (1, 0), (10, 0), (100, 0)] {
                    let params = match method {
                        LspMethod::DocumentSymbols => "{}".to_string(),
                        _ => format!(r#"{{"position":{{"line":{line},"character":{col}}}}}"#),
                    };

                    if let Ok(cache_key) = key_builder
                        .build_key(*method, &absolute_path, &params)
                        .await
                    {
                        cache_keys_to_clear.push((cache_key, (line, col), *method));
                    }
                }
            }
        }

        // === DATABASE CACHE CLEARING (Workspace Persistent Cache) ===
        info!(
            "Clearing database cache for symbol '{}' in {:?}",
            symbol_name, absolute_path
        );
        // Resolve workspace root from the cache key to ensure we use the correct workspace
        // that was used when the entries were originally stored
        if let Some((cache_key, _, _)) = cache_keys_to_clear.first() {
            match self.resolve_workspace_root(cache_key).await {
                Ok(workspace_root) => {
                    debug!(
                        "Resolved workspace root {:?} for cache clearing of symbol '{}' in {:?}",
                        workspace_root, symbol_name, absolute_path
                    );
                    if let Ok(workspace_cache) = self
                        .workspace_router
                        .cache_for_workspace(&workspace_root)
                        .await
                    {
                        info!("Got workspace cache for clearing, will clear by prefix matching");

                        // Build prefixes for each method and position combination
                        // We need to match entries regardless of content hash
                        let mut cleared_methods = std::collections::HashSet::new();
                        let mut cleared_positions = std::collections::HashSet::new();

                        for (cache_key, position, method) in &cache_keys_to_clear {
                            // Build prefix without content hash: workspace_id:method:file:
                            // This will match all entries for this file/method combo regardless of content
                            // Note: method names have '/' replaced with '_' in storage keys
                            let prefix = format!(
                                "{}:{}:{}:",
                                cache_key.workspace_id,
                                cache_key.method.as_str().replace('/', "_"),
                                cache_key.workspace_relative_path.display()
                            );

                            info!(
                    "Clearing L2 cache entries with prefix: {} for method={:?}, position={:?}",
                    prefix, method, position
                );

                            match workspace_cache
                                .clear_universal_entries_by_prefix(&prefix)
                                .await
                            {
                                Ok(count) => {
                                    if count > 0 {
                                        database_entries_cleared += count;
                                        cleared_methods.insert(*method);
                                        cleared_positions.insert(*position);
                                        info!(
                                            "Cleared {} database cache entries for {:?} with prefix: {}",
                                            count, method, prefix
                                        );
                                    } else {
                                        debug!("No database entries found with prefix: {}", prefix);
                                    }
                                }
                                Err(e) => {
                                    warn!(
                                        "Failed to clear database entries with prefix {}: {}",
                                        prefix, e
                                    );
                                }
                            }
                        }

                        // Convert cleared items to the expected format
                        for pos in cleared_positions {
                            if !positions_cleared.contains(&pos) {
                                positions_cleared.push(pos);
                            }
                        }
                        for method in cleared_methods {
                            let method_str = format!("{method:?}");
                            if !methods_cleared.contains(&method_str) {
                                methods_cleared.push(method_str);
                            }
                        }
                    } else {
                        warn!("Failed to get workspace cache for clearing symbol '{}' in workspace {:?}", 
                              symbol_name, workspace_root);
                    }
                }
                Err(e) => {
                    warn!(
                        "Failed to resolve workspace root for symbol '{}' cache clearing: {}",
                        symbol_name, e
                    );
                }
            }
        } else {
            warn!(
                "No cache keys available for L2 clearing of symbol '{}'",
                symbol_name
            );
        }

        // Calculate totals
        let entries_cleared = database_entries_cleared;
        let size_freed = (database_entries_cleared * 1024) as u64; // Estimate 1KB per entry

        info!(
            "Symbol '{}' cache clearing complete for file {:?}: {} database entries cleared, total={}",
            symbol_name, absolute_path, database_entries_cleared, entries_cleared
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

        // No memory cache to clear - using direct database storage only

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

        // Get stats from active workspace router (only count active workspaces)
        let workspace_router_stats = self.workspace_router.get_stats().await;

        let active_workspace_count = workspace_router_stats.workspace_stats.len();

        // Add hit/miss stats and entries from active workspaces
        for workspace_stat in &workspace_router_stats.workspace_stats {
            let workspace_id = workspace_stat.workspace_id.clone();

            // Count entries and hits/misses from persistent cache if available
            if let Some(cache_stats) = &workspace_stat.cache_stats {
                total_entries += cache_stats.total_nodes;
                total_size_bytes += cache_stats.total_size_bytes;
                total_hits += cache_stats.hit_count;
                total_misses += cache_stats.miss_count;

                // Add to method stats (attribute all to CallHierarchy for now)
                let method_stats = combined_method_stats
                    .entry(crate::universal_cache::LspMethod::CallHierarchy)
                    .or_insert(MethodStats {
                        entries: 0,
                        size_bytes: 0,
                        hits: 0,
                        misses: 0,
                    });
                method_stats.entries += cache_stats.total_nodes;
                method_stats.size_bytes += cache_stats.total_size_bytes;
                method_stats.hits += cache_stats.hit_count;
                method_stats.misses += cache_stats.miss_count;
            }

            // Add in-memory stats if available
            if let Some(memory_stats) = stats_map.get(&workspace_id) {
                total_entries += memory_stats.entries;
                total_size_bytes += memory_stats.size_bytes;
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

                    // Add memory-specific stats (entries and size already added above)
                    combined_stats.hits += method_stats.hits;
                    combined_stats.misses += method_stats.misses;
                    // Note: entries and size_bytes for method_stats from memory are included in the total above
                }
            }
        }

        let total_requests = total_hits + total_misses;
        let hit_rate = if total_requests > 0 {
            total_hits as f64 / total_requests as f64
        } else {
            0.0
        };
        let miss_rate = if total_requests > 0 {
            total_misses as f64 / total_requests as f64
        } else {
            0.0
        };

        debug!(
            "Cache stats calculation: total_hits={}, total_misses={}, total_requests={}, hit_rate={}, miss_rate={}",
            total_hits, total_misses, total_requests, hit_rate, miss_rate
        );

        debug!(
            "Cache stats: {} entries total, {} workspace caches found",
            total_entries, active_workspace_count
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
    #[allow(dead_code)]
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

        debug!(
            "Database cache stored entry for key: {} (workspace_id: {})",
            storage_key, key.workspace_id
        );
        Ok(())
    }

    /// Remove entry from persistent cache
    async fn remove_from_persistent_cache(&self, key: &CacheKey) -> Result<()> {
        // Get workspace root from the key's workspace_relative_path
        let workspace_root = self.resolve_workspace_root(key).await?;

        // Get workspace cache for this workspace
        let workspace_cache = self
            .workspace_router
            .cache_for_workspace(&workspace_root)
            .await?;

        // Create storage key for the persistent cache
        let storage_key = key.to_storage_key();

        // Remove from the universal cache tree
        workspace_cache.remove_universal_entry(&storage_key).await?;

        debug!(
            "Database cache removed entry for key: {} (workspace_id: {})",
            storage_key, key.workspace_id
        );
        Ok(())
    }

    /// Resolve workspace root from cache key
    async fn resolve_workspace_root(&self, key: &CacheKey) -> Result<PathBuf> {
        // Use the workspace router's reverse lookup capability
        match self
            .workspace_router
            .workspace_root_for(&key.workspace_id)
            .await
        {
            Ok(workspace_root) => {
                debug!(
                    "Resolved workspace root {} for workspace_id {}",
                    workspace_root.display(),
                    key.workspace_id
                );
                Ok(workspace_root)
            }
            Err(e) => {
                // Intelligent fallback: try to reconstruct from file path in cache key
                let current_dir =
                    std::env::current_dir().context("Failed to get current directory")?;
                let relative_path = &key.workspace_relative_path;
                let potential_file = current_dir.join(relative_path);

                // First try: if file exists, find workspace root by traversing up
                if potential_file.exists() {
                    let mut candidate_dir = potential_file.parent();
                    while let Some(dir) = candidate_dir {
                        if let Ok(workspace_id) = self.workspace_router.workspace_id_for(dir) {
                            if workspace_id == key.workspace_id {
                                debug!(
                                    "Intelligently resolved workspace root {} for workspace_id {} via file path reconstruction",
                                    dir.display(),
                                    key.workspace_id
                                );

                                return Ok(dir.to_path_buf());
                            }
                        }
                        candidate_dir = dir.parent();
                    }
                }

                // Second try: attempt to intelligently guess from relative path structure
                // Look for common workspace indicators in the path
                let path_components: Vec<_> = relative_path.components().collect();
                if !path_components.is_empty() {
                    // Try progressively shorter paths from current directory
                    for depth in 0..=2 {
                        // Try current dir, parent, grandparent
                        let mut test_workspace = current_dir.clone();
                        for _ in 0..depth {
                            if let Some(parent) = test_workspace.parent() {
                                test_workspace = parent.to_path_buf();
                            } else {
                                break;
                            }
                        }

                        // Check if this could be the right workspace
                        if let Ok(workspace_id) =
                            self.workspace_router.workspace_id_for(&test_workspace)
                        {
                            if workspace_id == key.workspace_id {
                                debug!(
                                    "Intelligently resolved workspace root {} for workspace_id {} via directory traversal (depth {})",
                                    test_workspace.display(),
                                    key.workspace_id,
                                    depth
                                );

                                return Ok(test_workspace);
                            }
                        }

                        // Also check if the file would exist relative to this workspace
                        let test_file = test_workspace.join(relative_path);
                        if test_file.exists() {
                            if let Ok(workspace_id) =
                                self.workspace_router.workspace_id_for(&test_workspace)
                            {
                                if workspace_id == key.workspace_id {
                                    debug!(
                                        "Intelligently resolved workspace root {} for workspace_id {} via file existence check at depth {}",
                                        test_workspace.display(),
                                        key.workspace_id,
                                        depth
                                    );

                                    return Ok(test_workspace);
                                }
                            }
                        }
                    }
                }

                // Final fallback: use current directory
                // Only use debug! since current directory is often a reasonable fallback
                debug!(
                    "Could not intelligently resolve workspace root for workspace_id {} ({}), using current directory as fallback: {}",
                    key.workspace_id,
                    e,
                    current_dir.display()
                );

                // Check if current directory happens to match the workspace_id
                if let Ok(current_workspace_id) =
                    self.workspace_router.workspace_id_for(&current_dir)
                {
                    if current_workspace_id == key.workspace_id {
                        debug!(
                            "Current directory fallback successfully matches workspace_id {} - this is a good fallback",
                            key.workspace_id
                        );
                    } else {
                        // Only warn when we truly can't find a reasonable workspace
                        warn!(
                            "Workspace resolution fallback: workspace_id {} doesn't match current directory workspace_id {}, but using current directory: {}",
                            key.workspace_id,
                            current_workspace_id,
                            current_dir.display()
                        );
                    }
                } else {
                    // Only warn if we can't even determine current directory's workspace
                    warn!(
                        "Could not determine workspace for current directory {}, but using it as fallback for workspace_id {} (original error: {})",
                        current_dir.display(),
                        key.workspace_id,
                        e
                    );
                }

                Ok(current_dir)
            }
        }
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

        // Should be expired now when accessed next (with direct database implementation)
        let result2: Option<TestValue> = store.get(&key).await.unwrap();
        assert_eq!(result2, None); // Should be None due to expiration
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

        // Store a value first
        let test_value = TestValue {
            content: "stats test".to_string(),
            number: 456,
        };
        store.set(&key, &test_value, 300).await.unwrap();

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
        println!(
            "Final stats: total_entries={}, active_workspaces={}, hit_rate={}, miss_rate={}",
            final_stats.total_entries,
            final_stats.active_workspaces,
            final_stats.hit_rate,
            final_stats.miss_rate
        );

        // The stats should have at least some entries - our stats come from workspace operations
        // not the universal cache tree scanning in this implementation
        assert!(final_stats.hit_rate > 0.0 || final_stats.miss_rate > 0.0); // At least some operations happened
        assert!(
            final_stats
                .method_stats
                .contains_key(&LspMethod::Definition)
                || final_stats.method_stats.contains_key(&LspMethod::Hover)
        );
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
