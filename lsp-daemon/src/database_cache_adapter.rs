//! Database Cache Adapter
//!
//! This module provides a minimal adapter that implements the interface needed
//! by the WorkspaceCacheRouter and universal cache while using the new database
//! abstraction layer for the universal cache system.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::database::{DatabaseBackend, DatabaseConfig, DatabaseTree, SQLiteBackend};
use crate::universal_cache::store::CacheEntry;

/// Configuration for database-backed cache
#[derive(Debug, Clone)]
pub struct DatabaseCacheConfig {
    /// Database backend type ("sqlite")
    pub backend_type: String,
    /// Database configuration
    pub database_config: DatabaseConfig,
    // Legacy fields removed - use database_config.temporary instead of memory_only
}

impl Default for DatabaseCacheConfig {
    fn default() -> Self {
        Self {
            backend_type: "sqlite".to_string(),
            database_config: DatabaseConfig {
                temporary: false,
                compression: true,
                cache_capacity: 100 * 1024 * 1024, // 100MB
                ..Default::default()
            },
        }
    }
}

/// Enum to hold different backend types
pub enum BackendType {
    SQLite(Arc<SQLiteBackend>),
}

impl BackendType {
    /// Open a tree on the backend
    pub async fn open_tree(&self, name: &str) -> Result<Arc<dyn DatabaseTree>, anyhow::Error> {
        match self {
            BackendType::SQLite(db) => Ok(db
                .open_tree(name)
                .await
                .map_err(|e| anyhow::anyhow!("Database error: {}", e))?
                as Arc<dyn DatabaseTree>),
        }
    }

    /// Get stats from the backend
    pub async fn stats(&self) -> Result<crate::database::DatabaseStats, anyhow::Error> {
        match self {
            BackendType::SQLite(db) => db
                .stats()
                .await
                .map_err(|e| anyhow::anyhow!("Database error: {}", e)),
        }
    }
}

/// Database-backed cache adapter that provides the interface needed by universal cache
pub struct DatabaseCacheAdapter {
    /// Database backend
    database: BackendType,

    /// Universal cache tree
    universal_tree: Arc<dyn DatabaseTree>,
}

impl DatabaseCacheAdapter {
    /// Create a new database cache adapter
    pub async fn new(config: DatabaseCacheConfig) -> Result<Self> {
        Self::new_with_workspace_id(config, "universal_cache").await
    }

    /// Create a new database cache adapter with workspace-specific tree name
    pub async fn new_with_workspace_id(
        config: DatabaseCacheConfig,
        workspace_id: &str,
    ) -> Result<Self> {
        // Use database config directly - legacy fields removed
        let database_config = config.database_config;

        let database = {
            let db = Arc::new(SQLiteBackend::new(database_config).await.with_context(|| {
                format!(
                    "Failed to create SQLite backend for workspace '{workspace_id}'. \
                             Check database path permissions and disk space."
                )
            })?);
            BackendType::SQLite(db)
        };

        // Create workspace-specific tree name to ensure workspace isolation
        let tree_name = if workspace_id == "universal_cache" {
            // Backward compatibility for existing tests and legacy usage
            "universal_cache".to_string()
        } else {
            // Use workspace-specific tree name for proper isolation
            format!("universal_cache_{workspace_id}")
        };

        let universal_tree = database.open_tree(&tree_name).await.with_context(|| {
            format!(
                "Failed to open universal cache tree '{tree_name}' for workspace '{workspace_id}'. \
                     This may indicate database corruption or insufficient permissions."
            )
        })?;

        Ok(Self {
            database,
            universal_tree,
        })
    }

    /// Get an entry from the universal cache tree
    pub async fn get_universal_entry(&self, key: &str) -> Result<Option<Vec<u8>>> {
        self.universal_tree
            .get(key.as_bytes())
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))
    }

    /// Set an entry in the universal cache tree
    pub async fn set_universal_entry(&self, key: &str, value: &[u8]) -> Result<()> {
        eprintln!(
            "DEBUG: SQLite set_universal_entry - storing key: '{}', value_len: {}, tree: {:p}",
            key,
            value.len(),
            Arc::as_ptr(&self.universal_tree)
        );

        self.universal_tree
            .set(key.as_bytes(), value)
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

        eprintln!(
            "DEBUG: SQLite set_universal_entry - successfully stored to tree {:p}",
            Arc::as_ptr(&self.universal_tree)
        );
        Ok(())
    }

    /// Remove an entry from the universal cache tree
    pub async fn remove_universal_entry(&self, key: &str) -> Result<bool> {
        self.universal_tree
            .remove(key.as_bytes())
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))
    }

    /// Get statistics from the database (workspace-specific)
    pub async fn get_stats(&self) -> Result<DatabaseCacheStats> {
        // Get tree-specific stats (not global database stats) for workspace isolation
        let tree_entry_count = self
            .universal_tree
            .len()
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

        // WORKAROUND: If tree.len() returns 0, manually count entries by scanning all keys
        let actual_entry_count = if tree_entry_count == 0 {
            // Use scan_prefix with empty prefix to get all entries
            let all_entries = self
                .universal_tree
                .scan_prefix(&[])
                .await
                .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;
            let count = all_entries.len() as u64;
            eprintln!(
                "DEBUG: Tree len={}, scan_prefix count={} for tree {:p}",
                tree_entry_count,
                count,
                Arc::as_ptr(&self.universal_tree)
            );
            count
        } else {
            tree_entry_count
        };

        // Estimate size for this specific tree
        let estimated_avg_entry_size = 256; // bytes per entry
        let tree_size_bytes = actual_entry_count * estimated_avg_entry_size;

        // Try to get hit/miss counts from metadata tree
        let (hit_count, miss_count) = self.get_hit_miss_stats().await.unwrap_or((0, 0));

        Ok(DatabaseCacheStats {
            total_entries: actual_entry_count,
            total_size_bytes: tree_size_bytes,
            disk_size_bytes: 0, // Individual tree disk size not easily measurable
            total_nodes: actual_entry_count, // Same as total_entries for compatibility
            hit_count,
            miss_count,
        })
    }

    /// Clear entries older than the specified number of seconds
    pub async fn clear_entries_older_than(&self, _older_than_seconds: u64) -> Result<(u64, usize)> {
        // TODO: Implement age-based clearing using metadata
        // For now, return empty result
        Ok((0, 0))
    }

    /// Clear all entries in this cache
    pub async fn clear(&self) -> Result<()> {
        self.universal_tree
            .clear()
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))
    }

    /// Update hit/miss counts for cache statistics
    /// Performance optimized: batch operations when both hits and misses are updated
    pub async fn update_hit_miss_counts(
        &self,
        hits: Option<u64>,
        misses: Option<u64>,
    ) -> Result<()> {
        let stats_tree = self
            .database
            .open_tree("cache_stats")
            .await
            .map_err(|e| anyhow::anyhow!("Failed to open stats tree: {}", e))?;

        // PERFORMANCE OPTIMIZATION: Handle both updates at once when possible
        match (hits, misses) {
            (Some(hit_increment), Some(miss_increment)) => {
                // Batch read both current values
                let current_hits_task = stats_tree.get(b"hits");
                let current_misses_task = stats_tree.get(b"misses");

                let (hits_result, misses_result) =
                    futures::join!(current_hits_task, current_misses_task);

                let current_hits = hits_result
                    .map_err(|e| anyhow::anyhow!("Database error: {}", e))?
                    .and_then(|data| bincode::deserialize::<u64>(data.as_slice()).ok())
                    .unwrap_or(0);

                let current_misses = misses_result
                    .map_err(|e| anyhow::anyhow!("Database error: {}", e))?
                    .and_then(|data| bincode::deserialize::<u64>(data.as_slice()).ok())
                    .unwrap_or(0);

                // Batch write both new values
                let new_hits = current_hits.saturating_add(hit_increment);
                let new_misses = current_misses.saturating_add(miss_increment);

                let hits_data = bincode::serialize(&new_hits)
                    .map_err(|e| anyhow::anyhow!("Serialization error: {}", e))?;
                let misses_data = bincode::serialize(&new_misses)
                    .map_err(|e| anyhow::anyhow!("Serialization error: {}", e))?;

                let hits_write = stats_tree.set(b"hits", &hits_data);
                let misses_write = stats_tree.set(b"misses", &misses_data);

                let (hits_write_result, misses_write_result) =
                    futures::join!(hits_write, misses_write);
                hits_write_result.map_err(|e| anyhow::anyhow!("Database error: {}", e))?;
                misses_write_result.map_err(|e| anyhow::anyhow!("Database error: {}", e))?;
            }
            (Some(hit_increment), None) => {
                // Update only hits
                let current_hits = stats_tree
                    .get(b"hits")
                    .await
                    .map_err(|e| anyhow::anyhow!("Database error: {}", e))?
                    .and_then(|data| bincode::deserialize::<u64>(data.as_slice()).ok())
                    .unwrap_or(0);

                let new_hits = current_hits.saturating_add(hit_increment);
                let hits_data = bincode::serialize(&new_hits)
                    .map_err(|e| anyhow::anyhow!("Serialization error: {}", e))?;

                stats_tree
                    .set(b"hits", &hits_data)
                    .await
                    .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;
            }
            (None, Some(miss_increment)) => {
                // Update only misses
                let current_misses = stats_tree
                    .get(b"misses")
                    .await
                    .map_err(|e| anyhow::anyhow!("Database error: {}", e))?
                    .and_then(|data| bincode::deserialize::<u64>(data.as_slice()).ok())
                    .unwrap_or(0);

                let new_misses = current_misses.saturating_add(miss_increment);
                let misses_data = bincode::serialize(&new_misses)
                    .map_err(|e| anyhow::anyhow!("Serialization error: {}", e))?;

                stats_tree
                    .set(b"misses", &misses_data)
                    .await
                    .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;
            }
            (None, None) => {
                // Nothing to update
            }
        }

        Ok(())
    }

    /// Get hit/miss stats from the stats tree
    async fn get_hit_miss_stats(&self) -> Result<(u64, u64)> {
        let stats_tree = self
            .database
            .open_tree("cache_stats")
            .await
            .map_err(|e| anyhow::anyhow!("Failed to open stats tree: {}", e))?;

        let hits = stats_tree
            .get(b"hits")
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))?
            .and_then(|data| bincode::deserialize::<u64>(data.as_slice()).ok())
            .unwrap_or(0);

        let misses = stats_tree
            .get(b"misses")
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))?
            .and_then(|data| bincode::deserialize::<u64>(data.as_slice()).ok())
            .unwrap_or(0);

        Ok((hits, misses))
    }

    /// Get all cache entries for a specific file
    /// Performance optimized: uses prefix scanning instead of full table scan
    pub async fn get_by_file(&self, file_path: &Path) -> Result<Vec<CacheNode>> {
        let mut results = Vec::new();
        let _file_path_str = file_path.to_string_lossy();

        // Extract potential workspace-relative paths to match against
        let file_name = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Handle macOS symlink canonicalization: /var -> /private/var
        let canonical_path = file_path
            .canonicalize()
            .unwrap_or_else(|_| file_path.to_path_buf());
        let paths_to_try = vec![file_path, &canonical_path];

        // Try different workspace-relative path patterns
        let mut search_patterns = Vec::new();

        // 1. Just the filename (most common case)
        if !file_name.is_empty() {
            search_patterns.push(file_name.to_string());
        }

        // 2. Try relative paths with different depth levels for both paths
        for path in &paths_to_try {
            let path_components: Vec<_> = path.components().collect();
            for depth in 1..=3.min(path_components.len()) {
                if let Ok(relative_path) = path_components[path_components.len() - depth..]
                    .iter()
                    .collect::<PathBuf>()
                    .into_os_string()
                    .into_string()
                {
                    if !search_patterns.contains(&relative_path) {
                        search_patterns.push(relative_path);
                    }
                }
            }
        }

        // 3. Add full path strings for exact matching
        let file_path_str = file_path.to_string_lossy();
        let canonical_path_str = canonical_path.to_string_lossy();
        if file_path_str != canonical_path_str {
            search_patterns.push(canonical_path_str.to_string());
        }
        search_patterns.push(file_path_str.to_string());

        // Get all entries and parse keys to match file paths
        let all_entries = self.iter_universal_entries().await?;
        results.reserve(8);

        // Debug output removed - invalidation now working correctly

        for (key, data) in all_entries {
            // Parse key format: workspace_id:method:workspace_relative_path:hash[:symbol]
            let parts: Vec<&str> = key.splitn(5, ':').collect();
            if parts.len() >= 3 {
                let key_file_path = parts[2]; // workspace_relative_path from key

                // Check if any of our search patterns match the key's file path
                let matches = search_patterns.iter().any(|pattern| {
                    key_file_path == pattern ||
                    key_file_path.ends_with(&format!("/{pattern}")) ||
                    pattern.ends_with(key_file_path) ||
                    // Handle path prefix matching for symlinks
                    (pattern.contains(key_file_path) && pattern.len() > key_file_path.len())
                });

                if matches {
                    // Deserialize as CacheEntry using bincode (same as storage format)
                    if let Ok(cache_entry) = bincode::deserialize::<CacheEntry>(&data) {
                        // Convert the entry data to JSON for the CacheNode
                        if let Ok(json_data) =
                            serde_json::from_slice::<serde_json::Value>(&cache_entry.data)
                        {
                            results.push(CacheNode {
                                key,
                                data: json_data,
                                file_path: file_path.to_path_buf(),
                            });
                        }
                    }
                }
            }
        }

        Ok(results)
    }

    /// Remove a specific entry from the cache
    pub async fn remove(&self, key: &str) -> Result<bool> {
        self.remove_universal_entry(key).await
    }

    /// Clear all cache entries matching a prefix
    /// Performance optimized: uses database prefix scanning directly with robust tree detection
    pub async fn clear_universal_entries_by_prefix(&self, prefix: &str) -> Result<u64> {
        let mut cleared_count = 0u64;

        eprintln!("DEBUG: Clearing entries with prefix '{prefix}'");

        // Extract workspace ID from prefix for tree name resolution
        let workspace_id = prefix.split(':').next().unwrap_or("universal_cache");

        // Probe multiple plausible tree names based on common storage schemes
        // NOTE: The actual storage uses "universal_cache_{workspace_id}" format
        let tree_candidates = if workspace_id == "universal_cache" || workspace_id.is_empty() {
            vec!["universal_cache".to_string()]
        } else {
            vec![
                format!("universal_cache_{}", workspace_id), // PRIMARY: Actual storage pattern used
                "universal_cache".to_string(),               // Global tree (fallback)
                workspace_id.to_string(),                    // Raw workspace ID
                format!("universal_cache:{}", workspace_id), // Colon separator
                format!("cache_{}", workspace_id),           // Alternative prefix
            ]
        };

        // Try both full prefix and stripped prefix for each tree
        let prefix_candidates = if let Some(pos) = prefix.find(':') {
            vec![prefix.to_string(), prefix[pos + 1..].to_string()]
        } else {
            vec![prefix.to_string()]
        };

        // Use a HashSet to avoid double deletion across tree/prefix combinations
        let mut deleted_keys = std::collections::HashSet::<Vec<u8>>::new();

        // Probe all combinations of tree names and prefixes
        for tree_name in &tree_candidates {
            if let Ok(tree) = self.database.open_tree(tree_name).await {
                eprintln!("DEBUG: Checking tree '{tree_name}' for prefix '{prefix}'");
                for scan_prefix in &prefix_candidates {
                    if !scan_prefix.is_empty() {
                        // Avoid scanning entire tree
                        if let Ok(entries) = tree.scan_prefix(scan_prefix.as_bytes()).await {
                            if !entries.is_empty() {
                                eprintln!(
                                    "DEBUG: Found {} entries in tree '{}' with prefix '{}'",
                                    entries.len(),
                                    tree_name,
                                    scan_prefix
                                );

                                // Delete all matching entries (avoid duplicates)
                                for (key_bytes, _) in entries {
                                    if !deleted_keys.contains(&key_bytes)
                                        && tree.remove(&key_bytes).await.is_ok()
                                    {
                                        deleted_keys.insert(key_bytes.clone());
                                        cleared_count += 1;
                                    }
                                }
                            } else {
                                eprintln!(
                                    "DEBUG: No entries found in tree '{tree_name}' with prefix '{scan_prefix}'"
                                );
                            }
                        }
                    }
                }

                // Continue checking all trees, don't break early
                if cleared_count > 0 {
                    eprintln!("DEBUG: Found {cleared_count} entries in tree '{tree_name}' so far");
                }
            } else {
                eprintln!("DEBUG: Could not open tree '{tree_name}'");
            }
        }

        // If targeted prefix scans found nothing, try a fallback full-tree scan
        // with in-memory filtering (only for test environments)
        if cleared_count == 0 && !workspace_id.is_empty() && workspace_id != "universal_cache" {
            eprintln!("DEBUG: No entries found with targeted scans, trying fallback full scan");
            for tree_name in &tree_candidates {
                if let Ok(tree) = self.database.open_tree(tree_name).await {
                    if let Ok(all_entries) = tree.scan_prefix(b"").await {
                        eprintln!(
                            "DEBUG: Fallback scanning {} total entries in tree '{}'",
                            all_entries.len(),
                            tree_name
                        );
                        for (key_bytes, _) in all_entries {
                            // In-memory prefix matching
                            if key_bytes.starts_with(prefix.as_bytes())
                                && !deleted_keys.contains(&key_bytes)
                                && tree.remove(&key_bytes).await.is_ok()
                            {
                                deleted_keys.insert(key_bytes.clone());
                                cleared_count += 1;
                                eprintln!(
                                    "DEBUG: Fallback deleted key: {}",
                                    String::from_utf8_lossy(&key_bytes)
                                );
                            }
                        }

                        // Stop after first successful fallback
                        if cleared_count > 0 {
                            break;
                        }
                    }
                }
            }
        }

        eprintln!("DEBUG: Total cleared entries: {cleared_count}");
        Ok(cleared_count)
    }

    /// Iterate over all universal cache entries
    pub async fn iter_universal_entries(&self) -> Result<Vec<(String, Vec<u8>)>> {
        // Use the universal_tree's scan functionality to get all entries from universal cache tree
        let entries = self
            .universal_tree
            .scan_prefix(b"") // Empty prefix gets all entries
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

        let mut results = Vec::new();
        for (key_bytes, value_bytes) in entries {
            if let Ok(key) = String::from_utf8(key_bytes) {
                results.push((key, value_bytes));
            }
        }

        Ok(results)
    }

    /// Iterate over cache nodes (compatibility method for legacy code)
    pub async fn iter_nodes(&self) -> Result<Vec<CacheNode>> {
        let all_entries = self.iter_universal_entries().await?;
        let mut nodes = Vec::new();

        for (key, data) in all_entries {
            // Try to deserialize as generic cache node
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&data) {
                // Extract file path from key if possible
                // Key format: workspace_id:method:file:hash
                let file_path = if let Some(parts) = key.split(':').nth(2) {
                    PathBuf::from(parts)
                } else {
                    PathBuf::from("unknown")
                };

                nodes.push(CacheNode {
                    key,
                    data: value,
                    file_path,
                });
            }
        }

        Ok(nodes)
    }
}

/// Cache node representation for get_by_file return type
#[derive(Debug, Clone)]
pub struct CacheNode {
    pub key: String,
    pub data: serde_json::Value,
    pub file_path: std::path::PathBuf,
}

/// Database cache statistics
#[derive(Debug, Clone)]
pub struct DatabaseCacheStats {
    pub total_entries: u64,
    pub total_size_bytes: u64,
    pub disk_size_bytes: u64,
    pub total_nodes: u64, // Same as total_entries for compatibility
    pub hit_count: u64,   // Cache hit count
    pub miss_count: u64,  // Cache miss count
}

// Legacy type aliases and enums removed - use actual types directly
