//! Database Cache Adapter
//!
//! This module provides a minimal adapter that implements the interface needed
//! by the WorkspaceCacheRouter and universal cache while using the new database
//! abstraction layer for the universal cache system.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;
use tracing::{debug, info, warn};

use crate::database::{DatabaseBackend, DatabaseConfig, DatabaseTree, SQLiteBackend};

/// Cache entry metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntryMetadata {
    /// When the entry was created
    created_at: SystemTime,
    /// When the entry was last accessed
    last_accessed: SystemTime,
    /// How many times this entry was accessed
    access_count: u64,
    /// Size of the entry in bytes
    size_bytes: usize,
}

/// Cached value with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// The cached value as JSON bytes
    pub data: Vec<u8>,
    /// Entry metadata
    metadata: CacheEntryMetadata,
}

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

    /// Get the database file path
    pub fn database_path(&self) -> std::path::PathBuf {
        match self {
            BackendType::SQLite(db) => db.database_path(),
        }
    }

    /// Perform a WAL checkpoint
    pub async fn checkpoint(&self) -> Result<(), anyhow::Error> {
        match self {
            BackendType::SQLite(db) => db
                .checkpoint()
                .await
                .map_err(|e| anyhow::anyhow!("Database error: {}", e)),
        }
    }
}

/// Database-backed cache adapter that provides the interface needed by universal cache
pub struct DatabaseCacheAdapter {
    /// Database backend
    database: BackendType,
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
            // Convert DatabaseConfig to SQLiteConfig for compatibility
            let sqlite_config = if let Some(ref db_path) = database_config.path {
                // Use the proper file path for persistent workspace cache
                crate::database::sqlite_backend::SQLiteConfig {
                    path: db_path.to_string_lossy().to_string(),
                    temporary: false, // Use persistent file-based cache
                    enable_wal: true, // Enable WAL for better concurrent access
                    page_size: 4096,
                    cache_size: (database_config.cache_capacity / 4096) as i32, // Convert bytes to pages
                    enable_foreign_keys: true, // Enable foreign keys for data integrity
                }
            } else {
                // Fallback to in-memory if no path provided
                crate::database::sqlite_backend::SQLiteConfig {
                    path: ":memory:".to_string(),
                    temporary: true,
                    enable_wal: false,
                    page_size: 4096,
                    cache_size: (database_config.cache_capacity / 4096) as i32,
                    enable_foreign_keys: false, // Disable for in-memory fallback to keep it simple
                }
            };

            info!("🏗️ DATABASE_CACHE_ADAPTER: Creating workspace cache database for '{}' at path: {:?}", workspace_id, sqlite_config.path);

            let db = match SQLiteBackend::with_sqlite_config(database_config, sqlite_config).await {
                Ok(backend) => {
                    info!("✅ DATABASE_CACHE_ADAPTER: Successfully created SQLite backend for workspace '{}'", workspace_id);

                    let backend_arc = Arc::new(backend);

                    // Start periodic checkpoint task (every 5 seconds)
                    let checkpoint_handle = backend_arc.clone().start_periodic_checkpoint(5);
                    debug!("✅ DATABASE_CACHE_ADAPTER: Started periodic WAL checkpoint task (5s interval) for workspace '{}'", workspace_id);

                    // We don't need to keep the handle unless we want to cancel it later
                    // The task will run for the lifetime of the daemon
                    std::mem::forget(checkpoint_handle);

                    backend_arc
                }
                Err(e) => {
                    warn!("❌ DATABASE_CACHE_ADAPTER: Failed to create SQLite backend for workspace '{}': {}", workspace_id, e);
                    return Err(anyhow::anyhow!("Database error: {}", e).context(format!(
                        "Failed to create SQLite backend for workspace '{workspace_id}'. \
                                 Check database path permissions and disk space."
                    )));
                }
            };
            BackendType::SQLite(db)
        };

        info!("✅ DATABASE_CACHE_ADAPTER: Successfully created DatabaseCacheAdapter for workspace '{}'", workspace_id);
        Ok(Self { database })
    }

    /// Get structured data from database (symbol_state and edge tables)
    /// Now queries structured tables instead of blob cache
    pub async fn get_universal_entry(&self, key: &str) -> Result<Option<Vec<u8>>> {
        debug!("Getting structured data for key: {}", key);
        info!(
            "🔍 DATABASE_CACHE_ADAPTER: get_universal_entry called for key: {} (structured query)",
            key
        );

        // Parse the key to understand what data is being requested
        let parsed = self.parse_cache_key(key)?;

        // Route to appropriate structured database query based on method
        match parsed.method.as_str() {
            "textDocument/prepareCallHierarchy"
            | "callHierarchy/incomingCalls"
            | "callHierarchy/outgoingCalls" => self.get_call_hierarchy_from_db(&parsed).await,
            "textDocument/hover" => self.get_hover_from_db(&parsed).await,
            "textDocument/definition" => self.get_definition_from_db(&parsed).await,
            _ => {
                // For unknown methods, return None (cache miss)
                debug!("Unknown method {}, returning cache miss", parsed.method);
                Ok(None)
            }
        }
    }

    /// Store structured data in database (symbol_state and edge tables)
    /// Now stores in structured tables instead of blob cache
    pub async fn set_universal_entry(&self, key: &str, value: &[u8]) -> Result<()> {
        debug!(
            "Storing structured data for key: {} (size: {} bytes)",
            key,
            value.len()
        );
        info!("💾 DATABASE_CACHE_ADAPTER: set_universal_entry called for key: {} (size: {} bytes) (structured storage)", key, value.len());

        // Parse the key and deserialize the LSP response
        let parsed = self.parse_cache_key(key)?;
        let lsp_response: serde_json::Value = serde_json::from_slice(value)?;

        // Route to appropriate structured database storage based on method
        match parsed.method.as_str() {
            "textDocument/prepareCallHierarchy"
            | "callHierarchy/incomingCalls"
            | "callHierarchy/outgoingCalls" => {
                self.store_call_hierarchy_in_db(&parsed, &lsp_response)
                    .await
            }
            "textDocument/hover" => self.store_hover_in_db(&parsed, &lsp_response).await,
            "textDocument/definition" => self.store_definition_in_db(&parsed, &lsp_response).await,
            _ => {
                // For unknown methods, silently succeed (no-op)
                debug!(
                    "Unknown method {}, skipping structured storage",
                    parsed.method
                );
                Ok(())
            }
        }
    }

    /// Remove structured data from database (symbol_state and edge tables)
    /// Now removes from structured tables instead of blob cache
    pub async fn remove_universal_entry(&self, key: &str) -> Result<bool> {
        debug!("Removing structured data for key: {}", key);
        info!("🗑️ DATABASE_CACHE_ADAPTER: remove_universal_entry called for key: {} (structured removal)", key);

        // Parse the key to understand what data to remove
        let parsed = match self.parse_cache_key(key) {
            Ok(parsed) => parsed,
            Err(_) => {
                // If key parsing fails, return false (nothing removed)
                return Ok(false);
            }
        };

        // For now, removing from structured tables is not implemented
        // This would require implementing symbol/edge deletion logic
        debug!(
            "Structured data removal not yet implemented for method: {}",
            parsed.method
        );
        Ok(false)
    }

    /// Get statistics from the database (workspace-specific)
    /// Now queries structured tables instead of blob cache
    pub async fn get_stats(&self) -> Result<DatabaseCacheStats> {
        debug!("Getting database stats for structured tables");

        // Get global database statistics instead of blob cache stats
        let db_stats = self.database.stats().await?;

        // Try to get hit/miss counts from metadata tree
        let (hit_count, miss_count) = self.get_hit_miss_stats().await.unwrap_or((0, 0));

        // For structured data, we report the actual database usage
        // This gives more accurate information than blob cache estimates
        Ok(DatabaseCacheStats {
            total_entries: 0, // TODO: Count symbols and edges from structured tables
            total_size_bytes: db_stats.total_size_bytes,
            disk_size_bytes: db_stats.disk_size_bytes,
            total_nodes: 0, // TODO: Count from symbol_state table
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
    /// Now clears structured tables instead of blob cache
    pub async fn clear(&self) -> Result<()> {
        debug!("Clearing all structured data in database");
        info!("🧹 DATABASE_CACHE_ADAPTER: Clearing all structured data");

        // For now, clearing structured data is not implemented
        // This would require clearing symbol_state and edge tables
        // while preserving workspace isolation

        // Clear hit/miss stats as they're still maintained
        let stats_tree = self
            .database
            .open_tree("cache_stats")
            .await
            .map_err(|e| anyhow::anyhow!("Failed to open stats tree: {}", e))?;

        stats_tree
            .clear()
            .await
            .map_err(|e| anyhow::anyhow!("Database error: {}", e))?;

        debug!("Cleared cache statistics");
        Ok(())
    }

    /// Get access to the underlying database backend (for graph export)
    pub fn backend(&self) -> &BackendType {
        &self.database
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

    /// Get all structured data entries for a specific file
    /// Now queries structured tables instead of blob cache
    pub async fn get_by_file(&self, file_path: &Path) -> Result<Vec<CacheNode>> {
        debug!("Getting structured data for file: {}", file_path.display());
        info!(
            "🔍 DATABASE_CACHE_ADAPTER: get_by_file called for file: {} (structured query)",
            file_path.display()
        );

        // For now, file-based structured data queries are not implemented
        // This would require:
        // 1. Querying symbol_state table for symbols in the file
        // 2. Querying edge table for relationships involving those symbols
        // 3. Converting results to CacheNode format for compatibility

        // Return empty list until structured file queries are implemented
        debug!(
            "Structured data file queries not yet implemented for: {}",
            file_path.display()
        );
        Ok(Vec::new())
    }

    /// Remove a specific entry from the cache
    pub async fn remove(&self, key: &str) -> Result<bool> {
        self.remove_universal_entry(key).await
    }

    /// Clear structured data by prefix
    /// Now operates on structured tables instead of blob cache
    pub async fn clear_universal_entries_by_prefix(&self, prefix: &str) -> Result<u64> {
        debug!("Clearing structured data by prefix: {}", prefix);
        info!("🧹 DATABASE_CACHE_ADAPTER: clear_universal_entries_by_prefix called for prefix: {} (structured clearing)", prefix);

        // For now, prefix-based clearing of structured data is not implemented
        // This would require analyzing the prefix to determine which symbols/edges to remove
        // while maintaining data consistency

        debug!(
            "Structured data prefix clearing not yet implemented for prefix: {}",
            prefix
        );
        Ok(0)
    }

    /// Iterate over structured data entries
    /// Now queries structured tables instead of blob cache
    pub async fn iter_universal_entries(&self) -> Result<Vec<(String, Vec<u8>)>> {
        debug!("Iterating over structured data entries");
        info!("🔄 DATABASE_CACHE_ADAPTER: iter_universal_entries called (structured iteration)");

        // For now, iteration over structured data is not implemented
        // This would require querying symbol_state and edge tables,
        // serializing results, and formatting as cache-like entries

        // Return empty list until structured iteration is implemented
        debug!("Structured data iteration not yet implemented");
        Ok(Vec::new())
    }

    /// Iterate over structured data nodes
    /// Now queries structured tables instead of blob cache
    pub async fn iter_nodes(&self) -> Result<Vec<CacheNode>> {
        debug!("Iterating over structured data nodes");
        info!("🔄 DATABASE_CACHE_ADAPTER: iter_nodes called (structured iteration)");

        // For now, node iteration over structured data is not implemented
        // This would require querying symbol_state and edge tables,
        // converting to CacheNode format for compatibility

        // Return empty list until structured node iteration is implemented
        debug!("Structured data node iteration not yet implemented");
        Ok(Vec::new())
    }

    /// Parse cache key to extract components
    fn parse_cache_key(&self, key: &str) -> Result<ParsedCacheKey> {
        // Format: workspace_id:method:file_path:hash[:symbol]
        let parts: Vec<&str> = key.splitn(5, ':').collect();
        if parts.len() < 4 {
            return Err(anyhow::anyhow!("Invalid cache key format: {}", key));
        }

        let workspace_id = parts[0].to_string();
        let method = parts[1].replace('_', "/");
        let file_path = std::path::PathBuf::from(parts[2]);
        let params_hash = parts[3].to_string();
        let symbol_name = if parts.len() == 5 {
            Some(parts[4].to_string())
        } else {
            None
        };

        Ok(ParsedCacheKey {
            workspace_id,
            method,
            file_path,
            params_hash,
            symbol_name,
        })
    }

    /// Get call hierarchy data from database
    async fn get_call_hierarchy_from_db(&self, parsed: &ParsedCacheKey) -> Result<Option<Vec<u8>>> {
        // Re-enabled database operations for proper cache functionality using tree interface
        let key = format!(
            "{}:{}:{}",
            parsed.workspace_id,
            parsed.method,
            parsed.file_path.display()
        );

        match self.database.open_tree("cache").await {
            Ok(tree) => {
                match tree.get(key.as_bytes()).await {
                    Ok(Some(data)) => {
                        debug!("DEBUG: Database cache HIT for key: {}", key);
                        Ok(Some(data))
                    }
                    Ok(None) => {
                        debug!("DEBUG: Database cache MISS for key: {}", key);
                        Ok(None)
                    }
                    Err(e) => {
                        warn!("DEBUG: Database cache lookup failed for key {}: {}", key, e);
                        Ok(None) // Graceful fallback on error
                    }
                }
            }
            Err(e) => {
                warn!("DEBUG: Failed to open cache tree: {}", e);
                Ok(None) // Graceful fallback on error
            }
        }
    }

    /// Get hover data from database  
    async fn get_hover_from_db(&self, parsed: &ParsedCacheKey) -> Result<Option<Vec<u8>>> {
        // Use same implementation pattern as call hierarchy but for hover
        let key = format!(
            "{}:{}:{}",
            parsed.workspace_id,
            parsed.method,
            parsed.file_path.display()
        );

        match self.database.open_tree("cache").await {
            Ok(tree) => {
                match tree.get(key.as_bytes()).await {
                    Ok(Some(data)) => {
                        debug!("🎯 DATABASE HIT for hover key: {}", key);
                        Ok(Some(data))
                    }
                    Ok(None) => {
                        debug!("❌ DATABASE MISS for hover key: {}", key);
                        Ok(None)
                    }
                    Err(e) => {
                        warn!("❌ Database hover lookup failed for key {}: {}", key, e);
                        Ok(None) // Graceful fallback on error
                    }
                }
            }
            Err(e) => {
                warn!("❌ Failed to open cache tree for hover lookup: {}", e);
                Ok(None) // Graceful fallback on error
            }
        }
    }

    /// Get definition data from database
    async fn get_definition_from_db(&self, parsed: &ParsedCacheKey) -> Result<Option<Vec<u8>>> {
        // Use same implementation pattern as call hierarchy but for definitions
        let key = format!(
            "{}:{}:{}",
            parsed.workspace_id,
            parsed.method,
            parsed.file_path.display()
        );

        match self.database.open_tree("cache").await {
            Ok(tree) => {
                match tree.get(key.as_bytes()).await {
                    Ok(Some(data)) => {
                        debug!("🎯 DATABASE HIT for definition key: {}", key);
                        Ok(Some(data))
                    }
                    Ok(None) => {
                        debug!("❌ DATABASE MISS for definition key: {}", key);
                        Ok(None)
                    }
                    Err(e) => {
                        warn!(
                            "❌ Database definition lookup failed for key {}: {}",
                            key, e
                        );
                        Ok(None) // Graceful fallback on error
                    }
                }
            }
            Err(e) => {
                warn!("❌ Failed to open cache tree for definition lookup: {}", e);
                Ok(None) // Graceful fallback on error
            }
        }
    }

    /// Store call hierarchy response in database
    async fn store_call_hierarchy_in_db(
        &self,
        parsed: &ParsedCacheKey,
        lsp_response: &serde_json::Value,
    ) -> Result<()> {
        // Re-enabled database operations for proper cache functionality using tree interface
        let key = format!(
            "{}:{}:{}",
            parsed.workspace_id,
            parsed.method,
            parsed.file_path.display()
        );
        let serialized_data = serde_json::to_vec(lsp_response)?;

        match self.database.open_tree("cache").await {
            Ok(tree) => {
                match tree.set(key.as_bytes(), &serialized_data).await {
                    Ok(_) => {
                        debug!(
                            "DEBUG: Database cache STORED for key: {} ({} bytes)",
                            key,
                            serialized_data.len()
                        );
                        Ok(())
                    }
                    Err(e) => {
                        warn!(
                            "DEBUG: Database cache storage failed for key {}: {}",
                            key, e
                        );
                        Ok(()) // Graceful fallback on error - don't fail the request
                    }
                }
            }
            Err(e) => {
                warn!("DEBUG: Failed to open cache tree for storage: {}", e);
                Ok(()) // Graceful fallback on error - don't fail the request
            }
        }
    }

    /// Store hover response in database
    async fn store_hover_in_db(
        &self,
        parsed: &ParsedCacheKey,
        lsp_response: &serde_json::Value,
    ) -> Result<()> {
        // Use same implementation pattern as call hierarchy but for hover
        let key = format!(
            "{}:{}:{}",
            parsed.workspace_id,
            parsed.method,
            parsed.file_path.display()
        );
        let serialized_data = serde_json::to_vec(lsp_response)?;

        match self.database.open_tree("cache").await {
            Ok(tree) => {
                match tree.set(key.as_bytes(), &serialized_data).await {
                    Ok(_) => {
                        debug!(
                            "💾 DATABASE STORED for hover key: {} ({} bytes)",
                            key,
                            serialized_data.len()
                        );
                        Ok(())
                    }
                    Err(e) => {
                        warn!("❌ Database hover storage failed for key {}: {}", key, e);
                        Ok(()) // Graceful fallback on error - don't fail the request
                    }
                }
            }
            Err(e) => {
                warn!("❌ Failed to open cache tree for hover storage: {}", e);
                Ok(()) // Graceful fallback on error - don't fail the request
            }
        }
    }

    /// Store definition response in database
    async fn store_definition_in_db(
        &self,
        parsed: &ParsedCacheKey,
        lsp_response: &serde_json::Value,
    ) -> Result<()> {
        // Use same implementation pattern as call hierarchy but for definitions
        let key = format!(
            "{}:{}:{}",
            parsed.workspace_id,
            parsed.method,
            parsed.file_path.display()
        );
        let serialized_data = serde_json::to_vec(lsp_response)?;

        match self.database.open_tree("cache").await {
            Ok(tree) => {
                match tree.set(key.as_bytes(), &serialized_data).await {
                    Ok(_) => {
                        debug!(
                            "💾 DATABASE STORED for definition key: {} ({} bytes)",
                            key,
                            serialized_data.len()
                        );
                        Ok(())
                    }
                    Err(e) => {
                        warn!(
                            "❌ Database definition storage failed for key {}: {}",
                            key, e
                        );
                        Ok(()) // Graceful fallback on error - don't fail the request
                    }
                }
            }
            Err(e) => {
                warn!("❌ Failed to open cache tree for definition storage: {}", e);
                Ok(()) // Graceful fallback on error - don't fail the request
            }
        }
    }

    /// Get definitions for a symbol (bridge method for daemon.rs)
    pub async fn get_definitions(
        &self,
        workspace_id: i64,
        symbol_uid: &str,
    ) -> Result<Option<Vec<crate::protocol::Location>>> {
        match &self.backend() {
            BackendType::SQLite(db) => db
                .get_definitions_for_symbol(workspace_id, symbol_uid)
                .await
                .map(|locs| if locs.is_empty() { None } else { Some(locs) })
                .map_err(|e| anyhow::anyhow!("Database error: {}", e)),
        }
    }

    /// Get references for a symbol (bridge method for daemon.rs)
    pub async fn get_references(
        &self,
        workspace_id: i64,
        symbol_uid: &str,
        include_declaration: bool,
    ) -> Result<Option<Vec<crate::protocol::Location>>> {
        match &self.backend() {
            BackendType::SQLite(db) => db
                .get_references_for_symbol(workspace_id, symbol_uid, include_declaration)
                .await
                .map(|locs| if locs.is_empty() { None } else { Some(locs) })
                .map_err(|e| anyhow::anyhow!("Database error: {}", e)),
        }
    }

    /// Get call hierarchy for a symbol (bridge method for daemon.rs)
    pub async fn get_call_hierarchy(
        &self,
        workspace_id: i64,
        symbol_uid: &str,
    ) -> Result<Option<crate::protocol::CallHierarchyResult>> {
        match &self.backend() {
            BackendType::SQLite(db) => db
                .get_call_hierarchy_for_symbol(workspace_id, symbol_uid)
                .await
                .map_err(|e| anyhow::anyhow!("Database error: {}", e)),
        }
    }

    /// Get implementations for a symbol (bridge method for daemon.rs)
    pub async fn get_implementations(
        &self,
        workspace_id: i64,
        symbol_uid: &str,
    ) -> Result<Option<Vec<crate::protocol::Location>>> {
        match &self.backend() {
            BackendType::SQLite(db) => db
                .get_implementations_for_symbol(workspace_id, symbol_uid)
                .await
                .map(|locs| if locs.is_empty() { None } else { Some(locs) })
                .map_err(|e| anyhow::anyhow!("Database error: {}", e)),
        }
    }

    /// Get document symbols for a file (bridge method for daemon.rs)
    pub async fn get_document_symbols(
        &self,
        workspace_id: i64,
        cache_key: &str,
    ) -> Result<Option<Vec<crate::protocol::DocumentSymbol>>> {
        let key = format!("{}:textDocument/documentSymbol:{}", workspace_id, cache_key);

        match self.database.open_tree("cache").await {
            Ok(tree) => {
                match tree.get(key.as_bytes()).await {
                    Ok(Some(data)) => {
                        debug!("🎯 DATABASE HIT for document symbols key: {}", key);
                        // Deserialize the cached document symbols
                        match bincode::deserialize::<Vec<crate::protocol::DocumentSymbol>>(&data) {
                            Ok(symbols) => Ok(Some(symbols)),
                            Err(e) => {
                                warn!("Failed to deserialize cached document symbols: {}", e);
                                Ok(None)
                            }
                        }
                    }
                    Ok(None) => {
                        debug!("❌ DATABASE MISS for document symbols key: {}", key);
                        Ok(None)
                    }
                    Err(e) => {
                        warn!(
                            "Database document symbols lookup failed for key {}: {}",
                            key, e
                        );
                        Ok(None) // Graceful fallback on error
                    }
                }
            }
            Err(e) => {
                warn!(
                    "Failed to open cache tree for document symbols lookup: {}",
                    e
                );
                Ok(None) // Graceful fallback on error
            }
        }
    }

    /// Store document symbols in cache (bridge method for daemon.rs)
    pub async fn store_document_symbols(
        &self,
        workspace_id: i64,
        cache_key: &str,
        symbols: &[crate::protocol::DocumentSymbol],
    ) -> Result<()> {
        let key = format!("{}:textDocument/documentSymbol:{}", workspace_id, cache_key);

        match self.database.open_tree("cache").await {
            Ok(tree) => {
                // Serialize the document symbols
                match bincode::serialize(symbols) {
                    Ok(data) => match tree.set(key.as_bytes(), &data).await {
                        Ok(_) => {
                            debug!("Successfully stored document symbols for key: {}", key);
                            Ok(())
                        }
                        Err(e) => {
                            warn!("Failed to store document symbols in cache: {}", e);
                            Err(anyhow::anyhow!("Failed to store document symbols: {}", e))
                        }
                    },
                    Err(e) => {
                        warn!("Failed to serialize document symbols: {}", e);
                        Err(anyhow::anyhow!(
                            "Failed to serialize document symbols: {}",
                            e
                        ))
                    }
                }
            }
            Err(e) => {
                warn!(
                    "Failed to open cache tree for document symbols storage: {}",
                    e
                );
                Err(anyhow::anyhow!("Failed to open cache tree: {}", e))
            }
        }
    }

    /// Get the database file path
    pub fn database_path(&self) -> std::path::PathBuf {
        self.database.database_path()
    }

    /// Perform a WAL checkpoint
    pub async fn checkpoint(&self) -> Result<()> {
        self.database.checkpoint().await
    }
}

/// Parsed cache key components
#[derive(Debug, Clone)]
pub struct ParsedCacheKey {
    pub workspace_id: String,
    pub method: String,
    pub file_path: std::path::PathBuf,
    pub params_hash: String,
    pub symbol_name: Option<String>,
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
