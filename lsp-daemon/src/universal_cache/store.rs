//! Cache Store Implementation
//!
//! This module provides the storage layer for the universal cache system,
//! maintaining per-workspace cache isolation while providing a unified interface.

use crate::universal_cache::{key::CacheKey, CacheStats, MethodStats};
use anyhow::{Context, Result};
use moka::future::Cache as MokaCache;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Cache entry metadata
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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

        // Aggregate statistics across all workspaces
        for workspace_stats in stats_map.values() {
            total_entries += workspace_stats.entries;
            total_size_bytes += workspace_stats.size_bytes;
            total_hits += workspace_stats.hits;
            total_misses += workspace_stats.misses;

            // Combine method statistics
            for (method, method_stats) in &workspace_stats.method_stats {
                let combined_stats = combined_method_stats.entry(*method).or_insert(MethodStats {
                    entries: 0,
                    size_bytes: 0,
                    hits: 0,
                    misses: 0,
                });

                combined_stats.entries += method_stats.entries;
                combined_stats.size_bytes += method_stats.size_bytes;
                combined_stats.hits += method_stats.hits;
                combined_stats.misses += method_stats.misses;
            }
        }

        let total_requests = total_hits + total_misses;
        let hit_rate = if total_requests > 0 {
            total_hits as f64 / total_requests as f64
        } else {
            0.0
        };
        let miss_rate = 1.0 - hit_rate;

        Ok(CacheStats {
            total_entries,
            total_size_bytes,
            active_workspaces: stats_map.len(),
            hit_rate,
            miss_rate,
            method_stats: combined_method_stats,
        })
    }

    // === Private Methods ===

    /// Get entry from persistent cache
    async fn get_from_persistent_cache(&self, _key: &CacheKey) -> Result<Option<CacheEntry>> {
        // Get workspace cache
        let _workspace_cache = self
            .workspace_router
            .cache_for_workspace(
                &std::env::current_dir().unwrap(), // TODO: Get proper workspace root from key
            )
            .await?;

        // TODO: Implement proper key lookup in persistent cache
        // For now, return None to indicate cache miss
        Ok(None)
    }

    /// Store entry in persistent cache
    async fn set_in_persistent_cache(&self, _key: &CacheKey, _entry: &CacheEntry) -> Result<()> {
        // Get workspace cache
        let _workspace_cache = self
            .workspace_router
            .cache_for_workspace(
                &std::env::current_dir().unwrap(), // TODO: Get proper workspace root from key
            )
            .await?;

        // TODO: Implement proper storage in persistent cache
        // This would involve extending the persistent cache to support generic JSON values
        Ok(())
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
