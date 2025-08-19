use crate::cache_types::{CacheStats, CachedNode, CallHierarchyInfo, NodeId, NodeKey};
use anyhow::Result;
use dashmap::DashMap;
use std::collections::{HashSet, VecDeque};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex as AsyncMutex;
use tracing::{debug, info};

/// Position-based cache key for fast lookups
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PosKey {
    file: PathBuf,
    line: u32,
    column: u32,
    content_md5: String,
}

/// Configuration for the call graph cache
#[derive(Debug, Clone)]
pub struct CallGraphCacheConfig {
    /// Maximum number of nodes to cache
    pub capacity: usize,
    /// Time-to-live for cached entries
    pub ttl: Duration,
    /// How often to check for expired entries
    pub eviction_check_interval: Duration,
    /// Maximum depth for graph-based invalidation
    pub invalidation_depth: usize,
}

impl Default for CallGraphCacheConfig {
    fn default() -> Self {
        Self {
            capacity: 1000,
            ttl: Duration::from_secs(1800), // 30 minutes
            eviction_check_interval: Duration::from_secs(60),
            invalidation_depth: 2,
        }
    }
}

/// Thread-safe, content-addressed call graph cache with automatic invalidation
pub struct CallGraphCache {
    /// Main cache storage: NodeKey -> CachedNode
    nodes: DashMap<NodeKey, Arc<CachedNode>>,

    /// Index from NodeId to all versions (NodeKeys) of that node
    id_to_keys: DashMap<NodeId, HashSet<NodeKey>>,

    /// Graph edges for invalidation (NodeId -> NodeId)
    outgoing: DashMap<NodeId, HashSet<NodeId>>,
    incoming: DashMap<NodeId, HashSet<NodeId>>,

    /// File index for file-based invalidation
    file_index: DashMap<std::path::PathBuf, HashSet<NodeId>>,

    /// Position index for fast lookups: (file, line, column, md5) -> NodeKey
    pos_index: DashMap<PosKey, NodeKey>,

    /// Reverse index: NodeKey -> all position keys that should be removed with it
    key_to_positions: DashMap<NodeKey, HashSet<PosKey>>,

    /// In-flight deduplication
    inflight: DashMap<NodeKey, Arc<AsyncMutex<()>>>,

    /// Configuration
    config: CallGraphCacheConfig,

    /// Last eviction check time
    last_eviction: Arc<AsyncMutex<Instant>>,
}

impl CallGraphCache {
    pub fn new(config: CallGraphCacheConfig) -> Self {
        Self {
            nodes: DashMap::new(),
            id_to_keys: DashMap::new(),
            outgoing: DashMap::new(),
            incoming: DashMap::new(),
            file_index: DashMap::new(),
            pos_index: DashMap::new(),
            key_to_positions: DashMap::new(),
            inflight: DashMap::new(),
            config,
            last_eviction: Arc::new(AsyncMutex::new(Instant::now())),
        }
    }

    /// Get a cached node or compute it if not present
    pub async fn get_or_compute<F, Fut>(&self, key: NodeKey, compute: F) -> Result<Arc<CachedNode>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<CallHierarchyInfo>>,
    {
        // Check cache first
        if let Some(node) = self.get(&key) {
            return Ok(node);
        }

        // Deduplication: ensure only one computation per key
        let lock = self
            .inflight
            .entry(key.clone())
            .or_insert_with(|| Arc::new(AsyncMutex::new(())))
            .clone();

        let _guard = lock.lock().await;

        // Double-check after acquiring lock
        if let Some(node) = self.get(&key) {
            self.inflight.remove(&key);
            return Ok(node);
        }

        // Compute the value
        debug!(
            "Computing call hierarchy for {}:{} (md5: {})",
            key.file.display(),
            key.symbol,
            key.content_md5
        );

        let info = compute().await?;
        let node = Arc::new(CachedNode::new(key.clone(), info));

        // Insert into cache
        self.insert_node(node.clone());

        // Clean up in-flight tracker
        self.inflight.remove(&key);

        // Trigger eviction check if needed
        self.maybe_evict().await;

        Ok(node)
    }

    /// Get a cached node if present
    pub fn get(&self, key: &NodeKey) -> Option<Arc<CachedNode>> {
        self.nodes.get(key).map(|entry| {
            // Note: We can't update access time here without interior mutability
            // The cache eviction will use creation time for LRU
            entry.clone()
        })
    }

    /// Associate a position key (file, line, column, md5) with an existing NodeKey.
    /// Call this right after successfully caching a node for that position.
    pub fn index_position(
        &self,
        file: &Path,
        line: u32,
        column: u32,
        content_md5: &str,
        key: &NodeKey,
    ) {
        let pos = PosKey {
            file: file.to_path_buf(),
            line,
            column,
            content_md5: content_md5.to_string(),
        };
        self.pos_index.insert(pos.clone(), key.clone());
        self.key_to_positions
            .entry(key.clone())
            .or_default()
            .insert(pos);
    }

    /// Try to get a cached node using the current position.
    pub fn get_by_position(
        &self,
        file: &Path,
        line: u32,
        column: u32,
        content_md5: &str,
    ) -> Option<Arc<CachedNode>> {
        let pos = PosKey {
            file: file.to_path_buf(),
            line,
            column,
            content_md5: content_md5.to_string(),
        };
        self.pos_index.get(&pos).and_then(|entry| self.get(&entry))
    }

    /// Insert a node into the cache
    fn insert_node(&self, node: Arc<CachedNode>) {
        let key = node.key.clone();
        let id = key.to_node_id();

        // Add to main cache
        self.nodes.insert(key.clone(), node);

        // Update ID index
        self.id_to_keys.entry(id.clone()).or_default().insert(key);

        // Update file index
        self.file_index
            .entry(id.file.clone())
            .or_default()
            .insert(id);
    }

    /// Update graph edges for a node
    pub fn update_edges(&self, node_id: &NodeId, incoming: Vec<NodeId>, outgoing: Vec<NodeId>) {
        // Update outgoing edges
        if !outgoing.is_empty() {
            self.outgoing
                .insert(node_id.clone(), outgoing.iter().cloned().collect());
        } else {
            self.outgoing.remove(node_id);
        }

        // Update incoming edges
        if !incoming.is_empty() {
            self.incoming
                .insert(node_id.clone(), incoming.iter().cloned().collect());
        } else {
            self.incoming.remove(node_id);
        }

        // Ensure bidirectional consistency
        for target in &outgoing {
            self.incoming
                .entry(target.clone())
                .or_default()
                .insert(node_id.clone());
        }

        for source in &incoming {
            self.outgoing
                .entry(source.clone())
                .or_default()
                .insert(node_id.clone());
        }
    }

    /// Invalidate a specific node and optionally its connected nodes
    pub fn invalidate_node(&self, node_id: &NodeId, depth: usize) {
        let mut to_invalidate = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back((node_id.clone(), 0));

        // BFS to find all nodes to invalidate
        while let Some((current_id, current_depth)) = queue.pop_front() {
            if current_depth > depth || !to_invalidate.insert(current_id.clone()) {
                continue;
            }

            if current_depth < depth {
                // Add connected nodes
                if let Some(outgoing) = self.outgoing.get(&current_id) {
                    for target in outgoing.iter() {
                        queue.push_back((target.clone(), current_depth + 1));
                    }
                }
                if let Some(incoming) = self.incoming.get(&current_id) {
                    for source in incoming.iter() {
                        queue.push_back((source.clone(), current_depth + 1));
                    }
                }
            }
        }

        // Remove all invalidated nodes
        for id in &to_invalidate {
            if let Some(keys) = self.id_to_keys.remove(id) {
                for key in keys.1 {
                    // ensure secondary indexes are purged
                    self.remove_by_key(&key);
                }
            }

            // Clean up edges
            self.outgoing.remove(id);
            self.incoming.remove(id);
        }

        info!(
            "Invalidated {} nodes starting from {}:{}",
            to_invalidate.len(),
            node_id.file.display(),
            node_id.symbol
        );
    }

    /// Invalidate all nodes from a specific file
    pub fn invalidate_file(&self, file: &Path) {
        if let Some(node_ids) = self.file_index.remove(file) {
            let count = node_ids.1.len();
            for id in node_ids.1 {
                if let Some(keys) = self.id_to_keys.remove(&id) {
                    for key in keys.1 {
                        // ensure secondary indexes are purged
                        self.remove_by_key(&key);
                    }
                }
                self.outgoing.remove(&id);
                self.incoming.remove(&id);
            }
            info!("Invalidated {} nodes from file {}", count, file.display());
        }
    }

    /// Clear the entire cache
    pub fn clear(&self) {
        self.nodes.clear();
        self.id_to_keys.clear();
        self.pos_index.clear();
        self.key_to_positions.clear();
        self.outgoing.clear();
        self.incoming.clear();
        self.file_index.clear();
        self.inflight.clear();
        info!("Cache cleared");
    }

    /// Check and evict expired entries
    async fn maybe_evict(&self) {
        let mut last_check = self.last_eviction.lock().await;

        if last_check.elapsed() < self.config.eviction_check_interval {
            return;
        }

        *last_check = Instant::now();
        drop(last_check); // Release lock early

        let now = Instant::now();
        let mut expired_keys = Vec::new();
        let mut lru_candidates = Vec::new();

        // Find expired entries and collect LRU candidates
        for entry in self.nodes.iter() {
            let node = entry.value();
            if now.duration_since(node.created_at) > self.config.ttl {
                expired_keys.push(entry.key().clone());
            } else {
                lru_candidates.push((entry.key().clone(), node.last_accessed, node.access_count));
            }
        }

        // Remove expired entries
        for key in &expired_keys {
            self.remove_by_key(key);
        }

        // If over capacity, evict LRU entries
        if self.nodes.len() > self.config.capacity {
            // Sort by last accessed time (oldest first) and access count
            lru_candidates.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.2.cmp(&b.2)));

            let to_evict = self.nodes.len().saturating_sub(self.config.capacity);
            for (key, _, _) in lru_candidates.iter().take(to_evict) {
                self.remove_by_key(key);
            }

            debug!(
                "Evicted {} expired and {} LRU entries",
                expired_keys.len(),
                to_evict
            );
        }
    }

    /// Remove a node by its key
    fn remove_by_key(&self, key: &NodeKey) {
        if let Some((_, _node)) = self.nodes.remove(key) {
            let id = key.to_node_id();

            // Remove any position mappings referencing this key
            if let Some(pos_set) = self.key_to_positions.remove(key) {
                // pos_set.1 is the HashSet<PosKey>
                for pos in pos_set.1 {
                    self.pos_index.remove(&pos);
                }
            }

            // Update ID index
            if let Some(mut keys) = self.id_to_keys.get_mut(&id) {
                keys.remove(key);
                if keys.is_empty() {
                    drop(keys);
                    self.id_to_keys.remove(&id);
                }
            }

            // Update file index
            if let Some(mut ids) = self.file_index.get_mut(&id.file) {
                ids.remove(&id);
                if ids.is_empty() {
                    drop(ids);
                    self.file_index.remove(&id.file);
                }
            }
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            total_nodes: self.nodes.len(),
            total_ids: self.id_to_keys.len(),
            total_files: self.file_index.len(),
            total_edges: self.outgoing.len() + self.incoming.len(),
            inflight_computations: self.inflight.len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_basic_operations() {
        let cache = CallGraphCache::new(CallGraphCacheConfig::default());

        let key = NodeKey::new("test_func", "/test/file.rs", "abc123");

        // First call should compute
        let result = cache
            .get_or_compute(key.clone(), || async {
                Ok(CallHierarchyInfo {
                    incoming_calls: vec![],
                    outgoing_calls: vec![],
                })
            })
            .await
            .unwrap();

        assert_eq!(result.key.symbol, "test_func");

        // Second call should hit cache
        let cached = cache.get(&key);
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().key.symbol, "test_func");
    }

    #[tokio::test]
    async fn test_cache_invalidation() {
        let cache = CallGraphCache::new(CallGraphCacheConfig::default());

        let key1 = NodeKey::new("func1", "/test/file.rs", "hash1");
        let key2 = NodeKey::new("func2", "/test/file.rs", "hash2");

        // Add entries
        for key in [&key1, &key2] {
            cache
                .get_or_compute(key.clone(), || async {
                    Ok(CallHierarchyInfo {
                        incoming_calls: vec![],
                        outgoing_calls: vec![],
                    })
                })
                .await
                .unwrap();
        }

        // Both should be cached
        assert!(cache.get(&key1).is_some());
        assert!(cache.get(&key2).is_some());

        // Invalidate the file
        cache.invalidate_file(Path::new("/test/file.rs"));

        // Both should be gone
        assert!(cache.get(&key1).is_none());
        assert!(cache.get(&key2).is_none());
    }

    #[test]
    fn test_node_id_equality() {
        let id1 = NodeId::new("func", "/path/file.rs");
        let id2 = NodeId::new("func", "/path/file.rs");
        let id3 = NodeId::new("other", "/path/file.rs");

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[tokio::test]
    async fn test_position_index_lookup() {
        let cache = CallGraphCache::new(CallGraphCacheConfig::default());
        let file = Path::new("/test/file.rs");
        let md5 = "abc123";
        let key = NodeKey::new("func", file.to_string_lossy(), md5);

        // Insert and cache a dummy node
        cache
            .get_or_compute(key.clone(), || async {
                Ok(CallHierarchyInfo {
                    incoming_calls: vec![],
                    outgoing_calls: vec![],
                })
            })
            .await
            .unwrap();

        // Index a position for it
        cache.index_position(file, 10, 5, md5, &key);

        // Lookup by the same position should hit
        let node = cache.get_by_position(file, 10, 5, md5);
        assert!(node.is_some());
        assert_eq!(node.unwrap().key.symbol, "func");

        // Invalidate file should remove both node and position mapping
        cache.invalidate_file(file);
        assert!(cache.get_by_position(file, 10, 5, md5).is_none());
    }
}
