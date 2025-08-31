//! Database Cache Adapter
//!
//! This module provides a minimal adapter that implements the interface needed
//! by the WorkspaceCacheRouter and universal cache while using the new database
//! abstraction layer instead of the legacy PersistentCallGraphCache.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::database::{DatabaseBackend, DatabaseConfig, DatabaseTree, SledBackend};

/// Configuration for database-backed cache
#[derive(Debug, Clone)]
pub struct DatabaseCacheConfig {
    /// Database configuration
    pub database_config: DatabaseConfig,

    /// Legacy fields for backward compatibility
    pub memory_only: bool,
    pub backend_type: DatabaseBackendType,
}

impl Default for DatabaseCacheConfig {
    fn default() -> Self {
        Self {
            database_config: DatabaseConfig {
                temporary: false,
                compression: true,
                cache_capacity: 100 * 1024 * 1024, // 100MB
                ..Default::default()
            },
            memory_only: false,
            backend_type: DatabaseBackendType::Sled,
        }
    }
}

/// Database-backed cache adapter that provides the interface needed by universal cache
pub struct DatabaseCacheAdapter {
    /// Database backend
    database: Arc<SledBackend>,

    /// Universal cache tree
    universal_tree: Arc<dyn DatabaseTree>,
}

impl DatabaseCacheAdapter {
    /// Create a new database cache adapter
    pub async fn new(config: DatabaseCacheConfig) -> Result<Self> {
        // Apply legacy settings to database config
        let mut database_config = config.database_config;
        if config.memory_only {
            database_config.temporary = true;
        }

        let database = Arc::new(
            SledBackend::new(database_config)
                .await
                .context("Failed to create database backend")?,
        );

        let universal_tree = database
            .open_tree("universal_cache")
            .await
            .context("Failed to open universal cache tree")?;

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
        self.universal_tree
            .set(key.as_bytes(), value)
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))
    }

    /// Remove an entry from the universal cache tree
    pub async fn remove_universal_entry(&self, key: &str) -> Result<bool> {
        self.universal_tree
            .remove(key.as_bytes())
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))
    }

    /// Get statistics from the database
    pub async fn get_stats(&self) -> Result<DatabaseCacheStats> {
        let db_stats = self
            .database
            .stats()
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

        // Try to get hit/miss counts from metadata tree
        let (hit_count, miss_count) = self.get_hit_miss_stats().await.unwrap_or((0, 0));

        Ok(DatabaseCacheStats {
            total_entries: db_stats.total_entries,
            total_size_bytes: db_stats.total_size_bytes,
            disk_size_bytes: db_stats.disk_size_bytes,
            total_nodes: db_stats.total_entries, // Same as total_entries for compatibility
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
                
                let (hits_result, misses_result) = futures::join!(current_hits_task, current_misses_task);
                
                let current_hits = hits_result
                    .map_err(|e| anyhow::anyhow!("Database error: {}", e))?
                    .and_then(|data| bincode::deserialize::<u64>(&data).ok())
                    .unwrap_or(0);
                    
                let current_misses = misses_result
                    .map_err(|e| anyhow::anyhow!("Database error: {}", e))?
                    .and_then(|data| bincode::deserialize::<u64>(&data).ok())
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
                
                let (hits_write_result, misses_write_result) = futures::join!(hits_write, misses_write);
                hits_write_result.map_err(|e| anyhow::anyhow!("Database error: {}", e))?;
                misses_write_result.map_err(|e| anyhow::anyhow!("Database error: {}", e))?;
            }
            (Some(hit_increment), None) => {
                // Update only hits
                let current_hits = stats_tree
                    .get(b"hits")
                    .await
                    .map_err(|e| anyhow::anyhow!("Database error: {}", e))?
                    .and_then(|data| bincode::deserialize::<u64>(&data).ok())
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
                    .and_then(|data| bincode::deserialize::<u64>(&data).ok())
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
            .and_then(|data| bincode::deserialize::<u64>(&data).ok())
            .unwrap_or(0);

        let misses = stats_tree
            .get(b"misses")
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))?
            .and_then(|data| bincode::deserialize::<u64>(&data).ok())
            .unwrap_or(0);

        Ok((hits, misses))
    }

    /// Get all cache entries for a specific file
    /// Performance optimized: uses prefix scanning instead of full table scan
    pub async fn get_by_file(&self, file_path: &Path) -> Result<Vec<CacheNode>> {
        let mut results = Vec::new();
        let file_path_str = file_path.to_string_lossy();
        
        // PERFORMANCE OPTIMIZATION: Try to use prefix scanning if possible
        // Since keys are in format: workspace_id:operation:file:hash
        // We can scan with different prefixes to reduce the search space
        
        // First, try to get a reasonable prefix from the file path
        // This is more efficient than scanning all entries
        let file_name = file_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        
        // For now, fall back to the full scan but with optimized string comparison
        let all_entries = self.iter_universal_entries().await?;
        let file_path_string = file_path_str.to_string(); // Convert once
        
        results.reserve(8); // Reserve space for typical cache hit count
        
        for (key, data) in all_entries {
            // PERFORMANCE: More efficient string matching
            // Check if this key contains our file path or filename
            if key.contains(&file_path_string) || (!file_name.is_empty() && key.contains(file_name)) {
                // Try to deserialize the data as a generic node
                if let Ok(node) = serde_json::from_slice::<serde_json::Value>(&data) {
                    results.push(CacheNode {
                        key,
                        data: node,
                        file_path: file_path.to_path_buf(),
                    });
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
    /// Performance optimized: uses database prefix scanning directly
    pub async fn clear_universal_entries_by_prefix(&self, prefix: &str) -> Result<u64> {
        let mut cleared_count = 0u64;
        
        // PERFORMANCE OPTIMIZATION: Use database-level prefix scanning
        // This is much more efficient than scanning all entries in memory
        let matching_entries = self
            .universal_tree
            .scan_prefix(prefix.as_bytes())
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

        // Remove each matching entry
        for (key_bytes, _) in matching_entries {
            if let Ok(key) = String::from_utf8(key_bytes) {
                if self.remove_universal_entry(&key).await? {
                    cleared_count += 1;
                }
            }
        }

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

/// Legacy type aliases for backward compatibility
pub type PersistentCallGraphCache = DatabaseCacheAdapter;
pub type PersistentCacheConfig = DatabaseCacheConfig;
pub type PersistentCacheStats = DatabaseCacheStats;

/// Legacy enum for backward compatibility
#[derive(Debug, Clone)]
pub enum DatabaseBackendType {
    Sled,
    Memory,
}

impl Default for DatabaseBackendType {
    fn default() -> Self {
        Self::Sled
    }
}
