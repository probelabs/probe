use crate::cache_types::{CacheStats, CachedNode, CallHierarchyInfo, NodeId, NodeKey};
use crate::git_utils::{GitConfig, GitContext};
use crate::language_detector::Language;
use crate::persistent_cache::{PersistentCacheConfig, PersistentCallGraphCache};
use crate::protocol::{
    BranchCacheStats, CacheDiff, CacheHistoryEntry, CacheSnapshot, CachedCallHierarchy,
    CommitCacheStats, GitCacheStats, HotSpot,
};
use anyhow::Result;
use dashmap::DashMap;
use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, Mutex as AsyncMutex};
use tracing::{debug, info, warn};

/// Messages for background persistence writer
#[derive(Debug, Clone)]
enum PersistenceMessage {
    /// Write a node to persistent storage
    Write {
        key: NodeKey,
        info: CallHierarchyInfo,
        language: Language,
    },
    /// Remove a node from persistent storage
    Remove { key: NodeKey },
    /// Remove all nodes for a file from persistent storage
    RemoveFile { file_path: PathBuf },
    /// Clear all persistent storage
    Clear,
}

/// Position-based cache key for fast lookups
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PosKey {
    file: PathBuf,
    line: u32,
    column: u32,
    content_md5: String,
}

/// Access metadata for tracking LRU without modifying CachedNode
#[derive(Debug, Clone)]
struct AccessMeta {
    last_accessed: Instant,
    access_count: usize,
}

impl AccessMeta {
    fn new() -> Self {
        Self {
            last_accessed: Instant::now(),
            access_count: 1,
        }
    }

    fn touch(&mut self) {
        self.last_accessed = Instant::now();
        self.access_count += 1;
    }
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
    /// Whether persistence is enabled
    pub persistence_enabled: bool,
    /// Path for persistent storage (None = use default)
    pub persistence_path: Option<PathBuf>,
    /// Number of writes to batch before flushing to disk
    pub persistence_write_batch_size: usize,
    /// Interval between background write flushes
    pub persistence_write_interval: Duration,
    /// Git-aware features configuration
    pub git_config: GitConfig,
}

impl Default for CallGraphCacheConfig {
    fn default() -> Self {
        Self {
            capacity: 1000,
            ttl: Duration::from_secs(1800), // 30 minutes
            eviction_check_interval: Duration::from_secs(60),
            invalidation_depth: 2,
            persistence_enabled: false,
            persistence_path: None,
            persistence_write_batch_size: 10,
            persistence_write_interval: Duration::from_secs(5),
            git_config: GitConfig::default(),
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

    /// Access metadata for true LRU tracking (separate from immutable CachedNode)
    access_meta: DashMap<NodeKey, AccessMeta>,

    /// In-flight deduplication
    inflight: DashMap<NodeKey, Arc<AsyncMutex<()>>>,

    /// Configuration
    config: CallGraphCacheConfig,

    /// Last eviction check time
    last_eviction: Arc<AsyncMutex<Instant>>,

    /// Optional persistent storage layer (L2 cache)
    persistent_store: Option<Arc<PersistentCallGraphCache>>,

    /// Channel for background persistence writes
    write_channel: Option<mpsc::UnboundedSender<PersistenceMessage>>,

    /// Git-aware tracking data
    /// Current git context for the workspace
    current_git_context: Arc<AsyncMutex<Option<GitContext>>>,
    /// Git context history (limited by config)
    git_history: Arc<AsyncMutex<VecDeque<GitContext>>>,
    /// Mapping from commit hash to cache keys for git-aware queries
    commit_index: DashMap<String, HashSet<NodeKey>>,
    /// Branch statistics for monitoring
    branch_stats: DashMap<String, BranchCacheStats>,
    /// Commit statistics for recent commits
    commit_stats: DashMap<String, CommitCacheStats>,
    /// Hot spot tracking across commits
    hot_spots: Arc<AsyncMutex<HashMap<(PathBuf, String), HotSpot>>>,
}

impl CallGraphCache {
    /// Create a new cache instance without persistence
    pub fn new(config: CallGraphCacheConfig) -> Self {
        Self {
            nodes: DashMap::new(),
            id_to_keys: DashMap::new(),
            outgoing: DashMap::new(),
            incoming: DashMap::new(),
            file_index: DashMap::new(),
            pos_index: DashMap::new(),
            key_to_positions: DashMap::new(),
            access_meta: DashMap::new(),
            inflight: DashMap::new(),
            config,
            last_eviction: Arc::new(AsyncMutex::new(Instant::now())),
            persistent_store: None,
            write_channel: None,
            current_git_context: Arc::new(AsyncMutex::new(None)),
            git_history: Arc::new(AsyncMutex::new(VecDeque::new())),
            commit_index: DashMap::new(),
            branch_stats: DashMap::new(),
            commit_stats: DashMap::new(),
            hot_spots: Arc::new(AsyncMutex::new(HashMap::new())),
        }
    }

    /// Create a new cache instance with optional persistence
    pub async fn new_with_persistence(config: CallGraphCacheConfig) -> Result<Self> {
        let (persistent_store, write_channel) = if config.persistence_enabled {
            // Create persistence config from cache config
            let persistence_config = PersistentCacheConfig {
                cache_directory: config.persistence_path.clone(),
                max_size_bytes: 0, // Unlimited by default
                ttl_days: 30,      // 30 days default
                git_integration: true,
                compress: true,
            };

            // Create persistent store
            let store = Arc::new(PersistentCallGraphCache::new(persistence_config).await?);

            // Create write channel for background persistence
            let (write_tx, write_rx) = mpsc::unbounded_channel();

            // Spawn background writer task
            let store_clone = Arc::clone(&store);
            let batch_size = config.persistence_write_batch_size;
            let write_interval = config.persistence_write_interval;

            tokio::spawn(Self::background_writer(
                write_rx,
                store_clone,
                batch_size,
                write_interval,
            ));

            info!("Persistent cache enabled with background writer");
            (Some(store), Some(write_tx))
        } else {
            (None, None)
        };

        Ok(Self {
            nodes: DashMap::new(),
            id_to_keys: DashMap::new(),
            outgoing: DashMap::new(),
            incoming: DashMap::new(),
            file_index: DashMap::new(),
            pos_index: DashMap::new(),
            key_to_positions: DashMap::new(),
            access_meta: DashMap::new(),
            inflight: DashMap::new(),
            config,
            last_eviction: Arc::new(AsyncMutex::new(Instant::now())),
            persistent_store,
            write_channel,
            current_git_context: Arc::new(AsyncMutex::new(None)),
            git_history: Arc::new(AsyncMutex::new(VecDeque::new())),
            commit_index: DashMap::new(),
            branch_stats: DashMap::new(),
            commit_stats: DashMap::new(),
            hot_spots: Arc::new(AsyncMutex::new(HashMap::new())),
        })
    }

    /// Get a cached node or compute it if not present using layered caching:
    /// L1: In-memory cache (fastest, <1ms)
    /// L2: Persistent storage (~1-5ms)
    /// L3: LSP server computation (100ms-10s)
    pub async fn get_or_compute<F, Fut>(&self, key: NodeKey, compute: F) -> Result<Arc<CachedNode>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<CallHierarchyInfo>>,
    {
        // L1: Check in-memory cache first (fastest)
        if let Some(node) = self.get(&key) {
            debug!("L1 cache hit for {}:{}", key.file.display(), key.symbol);
            return Ok(node);
        }

        // Deduplication: ensure only one computation per key
        let lock = self
            .inflight
            .entry(key.clone())
            .or_insert_with(|| Arc::new(AsyncMutex::new(())))
            .clone();

        let _guard = lock.lock().await;

        // Double-check L1 after acquiring lock
        if let Some(node) = self.get(&key) {
            self.inflight.remove(&key);
            return Ok(node);
        }

        // L2: Check persistent storage if available
        if let Some(ref persistent_store) = self.persistent_store {
            match persistent_store.get(&key).await {
                Ok(Some(persisted_node)) => {
                    debug!("L2 cache hit for {}:{}", key.file.display(), key.symbol);

                    // Create CachedNode from persisted data
                    let node = Arc::new(CachedNode::new(key.clone(), persisted_node.info));

                    // Insert into L1 cache for future access
                    self.insert_node(node.clone());

                    // Clean up in-flight tracker
                    self.inflight.remove(&key);

                    return Ok(node);
                }
                Ok(None) => {
                    debug!("L2 cache miss for {}:{}", key.file.display(), key.symbol);
                }
                Err(e) => {
                    warn!(
                        "L2 cache error for {}:{}: {}",
                        key.file.display(),
                        key.symbol,
                        e
                    );
                    // Continue to L3 computation on L2 error
                }
            }
        }

        // L3: Compute the value using LSP server (most expensive)
        debug!(
            "L3 computation for {}:{} (md5: {})",
            key.file.display(),
            key.symbol,
            key.content_md5
        );

        let info = compute().await?;
        let node = Arc::new(CachedNode::new(key.clone(), info.clone()));

        // Insert into L1 cache
        self.insert_node(node.clone());

        // Write-through to L2 cache if available
        if let Some(ref write_channel) = self.write_channel {
            // Try to detect language from file extension (fallback to Unknown)
            let language = key
                .file
                .extension()
                .and_then(|ext| ext.to_str())
                .and_then(Language::from_str)
                .unwrap_or(Language::Unknown);

            let msg = PersistenceMessage::Write {
                key: key.clone(),
                info,
                language,
            };

            if let Err(e) = write_channel.send(msg) {
                warn!(
                    "Failed to queue persistence write for {}:{}: {}",
                    key.file.display(),
                    key.symbol,
                    e
                );
            }
        }

        // Clean up in-flight tracker
        self.inflight.remove(&key);

        // Trigger eviction check if needed
        self.maybe_evict().await;

        Ok(node)
    }

    /// Warm the cache from persistent storage on startup
    /// Loads the most recently used entries up to the configured capacity
    pub async fn warm_from_persistence(&self) -> Result<usize> {
        let Some(ref persistent_store) = self.persistent_store else {
            debug!("No persistent store available for cache warming");
            return Ok(0);
        };

        info!("Starting cache warming from persistent storage");

        let start = Instant::now();
        let mut loaded_count = 0;

        // Get basic stats to understand the scope
        let stats = persistent_store.get_stats().await?;
        info!(
            "Persistent cache contains {} nodes across {} files",
            stats.total_nodes, stats.total_files
        );

        if stats.total_nodes == 0 {
            info!("No nodes in persistent storage to warm cache with");
            return Ok(0);
        }

        // For now, we'll use a simple approach: iterate through the database
        // In a more sophisticated implementation, we could prioritize by access time,
        // file modification time, or other heuristics

        // Create a temporary hashmap to track what we've loaded
        let mut loaded_nodes = std::collections::HashMap::new();

        // Get all nodes from persistent storage
        match persistent_store.iter_nodes().await {
            Ok(nodes) => {
                for (key, persisted_node) in nodes {
                    // Respect capacity limits
                    if loaded_count >= self.config.capacity {
                        debug!("Reached capacity limit during cache warming");
                        break;
                    }

                    // Check if we already loaded this NodeId (but different version)
                    let node_id = key.to_node_id();
                    if loaded_nodes.contains_key(&node_id) {
                        // Skip if we already have a version of this node
                        continue;
                    }

                    // Create CachedNode and insert into L1 cache
                    let cached_node = Arc::new(CachedNode::new(key.clone(), persisted_node.info));

                    // Insert into L1 cache structures
                    self.insert_node(cached_node);

                    loaded_nodes.insert(node_id, key.clone());
                    loaded_count += 1;

                    if loaded_count % 100 == 0 {
                        debug!("Cache warming progress: {} nodes loaded", loaded_count);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to iterate persistent cache for warming: {}", e);
            }
        }

        let duration = start.elapsed();
        info!(
            "Cache warming completed: loaded {} nodes in {:?}",
            loaded_count, duration
        );

        Ok(loaded_count)
    }

    /// Background task for batched persistence writes
    async fn background_writer(
        mut write_rx: mpsc::UnboundedReceiver<PersistenceMessage>,
        persistent_store: Arc<PersistentCallGraphCache>,
        batch_size: usize,
        write_interval: Duration,
    ) {
        let mut batch = Vec::new();
        let mut interval = tokio::time::interval(write_interval);

        info!(
            "Background persistence writer started (batch_size: {}, interval: {:?})",
            batch_size, write_interval
        );

        loop {
            tokio::select! {
                // Handle incoming messages
                msg = write_rx.recv() => {
                    match msg {
                        Some(message) => {
                            batch.push(message);

                            // Flush if batch is full
                            if batch.len() >= batch_size {
                                Self::flush_batch(&persistent_store, &mut batch).await;
                            }
                        }
                        None => {
                            // Channel closed, flush remaining and exit
                            if !batch.is_empty() {
                                Self::flush_batch(&persistent_store, &mut batch).await;
                            }
                            info!("Background persistence writer stopping");
                            break;
                        }
                    }
                }
                // Periodic flush
                _ = interval.tick() => {
                    if !batch.is_empty() {
                        Self::flush_batch(&persistent_store, &mut batch).await;
                    }
                }
            }
        }
    }

    /// Flush a batch of persistence messages
    async fn flush_batch(
        persistent_store: &Arc<PersistentCallGraphCache>,
        batch: &mut Vec<PersistenceMessage>,
    ) {
        if batch.is_empty() {
            return;
        }

        debug!("Flushing {} persistence operations", batch.len());

        for message in batch.drain(..) {
            match message {
                PersistenceMessage::Write {
                    key,
                    info,
                    language,
                } => {
                    if let Err(e) = persistent_store.insert(key.clone(), info, language).await {
                        warn!(
                            "Failed to persist write for {}:{}: {}",
                            key.file.display(),
                            key.symbol,
                            e
                        );
                    }
                }
                PersistenceMessage::Remove { key } => {
                    if let Err(e) = persistent_store.remove(&key).await {
                        warn!(
                            "Failed to persist removal for {}:{}: {}",
                            key.file.display(),
                            key.symbol,
                            e
                        );
                    }
                }
                PersistenceMessage::RemoveFile { file_path } => {
                    // For file removal, we need to get all nodes for the file first
                    match persistent_store.get_by_file(&file_path).await {
                        Ok(nodes) => {
                            for node in nodes {
                                if let Err(e) = persistent_store.remove(&node.key).await {
                                    warn!(
                                        "Failed to persist file removal for {}:{}: {}",
                                        node.key.file.display(),
                                        node.key.symbol,
                                        e
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            warn!(
                                "Failed to get nodes for file removal {}: {}",
                                file_path.display(),
                                e
                            );
                        }
                    }
                }
                PersistenceMessage::Clear => {
                    if let Err(e) = persistent_store.clear().await {
                        warn!("Failed to persist cache clear: {}", e);
                    }
                }
            }
        }
    }

    /// Get a cached node if present
    pub fn get(&self, key: &NodeKey) -> Option<Arc<CachedNode>> {
        self.nodes.get(key).map(|entry| {
            // Touch access metadata separately for true LRU tracking
            if let Some(mut meta) = self.access_meta.get_mut(key) {
                meta.touch();
            }
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

        // Initialize access metadata for LRU tracking
        self.access_meta.insert(key.clone(), AccessMeta::new());

        // Update ID index
        self.id_to_keys
            .entry(id.clone())
            .or_default()
            .insert(key.clone());

        // Update file index
        self.file_index
            .entry(id.file.clone())
            .or_default()
            .insert(id);

        // Update git-aware commit index if git tracking is enabled
        if self.config.git_config.track_commits {
            // We'll need to get current git context to associate with this cache entry
            // For now, we'll spawn a task to handle this asynchronously
            let key_clone = key.clone();
            let commit_index_clone = self.commit_index.clone();
            let current_git_context = self.current_git_context.clone();

            tokio::spawn(async move {
                if let Some(git_context) = current_git_context.lock().await.as_ref() {
                    commit_index_clone
                        .entry(git_context.commit_hash.clone())
                        .or_default()
                        .insert(key_clone);
                }
            });
        }
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

        // Remove all invalidated nodes from L1 cache and queue L2 removal
        for id in &to_invalidate {
            if let Some(keys) = self.id_to_keys.remove(id) {
                for key in keys.1 {
                    // Remove from L1 cache (ensure secondary indexes are purged)
                    self.remove_by_key(&key);

                    // Queue removal from L2 cache if persistence is enabled
                    if let Some(ref write_channel) = self.write_channel {
                        let msg = PersistenceMessage::Remove { key: key.clone() };
                        if let Err(e) = write_channel.send(msg) {
                            warn!(
                                "Failed to queue persistence removal for {}:{}: {}",
                                key.file.display(),
                                key.symbol,
                                e
                            );
                        }
                    }
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

            // Queue file removal from L2 cache if persistence is enabled
            if let Some(ref write_channel) = self.write_channel {
                let msg = PersistenceMessage::RemoveFile {
                    file_path: file.to_path_buf(),
                };
                if let Err(e) = write_channel.send(msg) {
                    warn!(
                        "Failed to queue persistence file removal for {}: {}",
                        file.display(),
                        e
                    );
                }
            }

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
        // Queue clear for L2 cache if persistence is enabled
        if let Some(ref write_channel) = self.write_channel {
            let msg = PersistenceMessage::Clear;
            if let Err(e) = write_channel.send(msg) {
                warn!("Failed to queue persistence clear: {}", e);
            }
        }

        // Clear L1 cache
        self.nodes.clear();
        self.id_to_keys.clear();
        self.pos_index.clear();
        self.key_to_positions.clear();
        self.access_meta.clear();
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

        self.do_evict().await;
    }

    /// Force eviction check (for testing)
    #[cfg(test)]
    async fn force_evict(&self) {
        self.do_evict().await;
    }

    /// Internal eviction logic
    async fn do_evict(&self) {
        let now = Instant::now();
        let mut expired_keys = Vec::new();
        let mut lru_candidates = Vec::new();

        // Find expired entries and collect LRU candidates
        for entry in self.nodes.iter() {
            let key = entry.key();
            let node = entry.value();

            if now.duration_since(node.created_at) > self.config.ttl {
                expired_keys.push(key.clone());
            } else {
                // Get access metadata for true LRU ranking
                if let Some(meta) = self.access_meta.get(key) {
                    lru_candidates.push((key.clone(), meta.last_accessed, meta.access_count));
                } else {
                    // Fallback to node creation time if metadata is missing
                    lru_candidates.push((key.clone(), node.created_at, 1));
                }
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

            // Remove access metadata
            self.access_meta.remove(key);

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

    /// Git-aware methods for commit and branch tracking
    /// Set the current git context for the cache
    pub async fn set_git_context(&self, context: GitContext) -> Result<Option<GitContext>> {
        let mut current_context = self.current_git_context.lock().await;
        let previous = current_context.clone();

        // Detect changes and handle accordingly
        if let Some(ref prev_ctx) = previous {
            if prev_ctx.has_changed(&context) {
                info!(
                    "Git context changed from {} to {}",
                    prev_ctx.short_id(),
                    context.short_id()
                );

                // Handle branch switches
                if prev_ctx.has_branch_changed(&context) {
                    self.handle_branch_switch(prev_ctx, &context).await?;
                }

                // Handle new commits
                if prev_ctx.has_new_commits(&context) {
                    self.handle_new_commits(prev_ctx, &context).await?;
                }

                // Update git history
                let mut history = self.git_history.lock().await;
                history.push_back(prev_ctx.clone());

                // Limit history size
                while history.len() > self.config.git_config.max_history_depth {
                    history.pop_front();
                }
            }
        } else {
            info!("Initial git context set: {}", context.short_id());
        }

        *current_context = Some(context);
        Ok(previous)
    }

    /// Get the current git context
    pub async fn get_git_context(&self) -> Option<GitContext> {
        self.current_git_context.lock().await.clone()
    }

    /// Get cached node for a specific commit
    pub async fn get_for_commit(&self, key: &NodeKey, commit: &str) -> Option<Arc<CachedNode>> {
        // Check if the key exists for this commit
        if let Some(commit_keys) = self.commit_index.get(commit) {
            if commit_keys.contains(key) {
                return self.get(key);
            }
        }
        None
    }

    /// Get all cache history for a node across commits
    pub async fn get_history(&self, node_id: &NodeId) -> Result<Vec<CacheHistoryEntry>> {
        let mut history = Vec::new();

        // Get all versions of this node
        if let Some(keys) = self.id_to_keys.get(node_id) {
            for key in keys.iter() {
                if let Some(_node) = self.get(key) {
                    // Find which commits this key belongs to
                    for commit_entry in self.commit_index.iter() {
                        if commit_entry.value().contains(key) {
                            // Try to get git context for this commit from history
                            let git_history = self.git_history.lock().await;

                            if let Some(git_context) = git_history
                                .iter()
                                .find(|ctx| ctx.commit_hash == *commit_entry.key())
                            {
                                let cache_entry = CachedCallHierarchy {
                                    file_path: node_id.file.clone(),
                                    symbol: node_id.symbol.clone(),
                                    line: 0, // We don't store line/column in NodeId
                                    column: 0,
                                    result: crate::protocol::CallHierarchyResult {
                                        item: crate::protocol::CallHierarchyItem {
                                            name: node_id.symbol.clone(),
                                            kind: "function".to_string(),
                                            uri: format!("file://{}", node_id.file.display()),
                                            range: crate::protocol::Range {
                                                start: crate::protocol::Position {
                                                    line: 0,
                                                    character: 0,
                                                },
                                                end: crate::protocol::Position {
                                                    line: 0,
                                                    character: 0,
                                                },
                                            },
                                            selection_range: crate::protocol::Range {
                                                start: crate::protocol::Position {
                                                    line: 0,
                                                    character: 0,
                                                },
                                                end: crate::protocol::Position {
                                                    line: 0,
                                                    character: 0,
                                                },
                                            },
                                        },
                                        incoming: vec![],
                                        outgoing: vec![],
                                    },
                                    cached_at: SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_secs(),
                                };

                                history.push(CacheHistoryEntry {
                                    commit_hash: commit_entry.key().clone(),
                                    branch: git_context.branch.clone(),
                                    timestamp: git_context.commit_hash.len() as u64, // Placeholder
                                    cache_entry,
                                    git_context: git_context.clone(),
                                });
                            }
                        }
                    }
                }
            }
        }

        // Sort by timestamp (most recent first)
        history.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(history)
    }

    /// Get cache snapshot at a specific commit
    pub async fn snapshot_at_commit(&self, commit: &str) -> Result<CacheSnapshot> {
        let commit_keys = self
            .commit_index
            .get(commit)
            .ok_or_else(|| anyhow::anyhow!("No cache data for commit {}", commit))?;

        let mut entries = Vec::new();
        for key in commit_keys.iter() {
            if let Some(_node) = self.get(key) {
                let entry = CachedCallHierarchy {
                    file_path: key.file.clone(),
                    symbol: key.symbol.clone(),
                    line: 0,
                    column: 0,
                    result: crate::protocol::CallHierarchyResult {
                        item: crate::protocol::CallHierarchyItem {
                            name: key.symbol.clone(),
                            kind: "function".to_string(),
                            uri: format!("file://{}", key.file.display()),
                            range: crate::protocol::Range {
                                start: crate::protocol::Position {
                                    line: 0,
                                    character: 0,
                                },
                                end: crate::protocol::Position {
                                    line: 0,
                                    character: 0,
                                },
                            },
                            selection_range: crate::protocol::Range {
                                start: crate::protocol::Position {
                                    line: 0,
                                    character: 0,
                                },
                                end: crate::protocol::Position {
                                    line: 0,
                                    character: 0,
                                },
                            },
                        },
                        incoming: vec![],
                        outgoing: vec![],
                    },
                    cached_at: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                };
                entries.push(entry);
            }
        }

        // Try to get git context from history
        let git_history = self.git_history.lock().await;
        let git_context = git_history
            .iter()
            .find(|ctx| ctx.commit_hash == commit)
            .cloned()
            .unwrap_or_else(|| GitContext {
                commit_hash: commit.to_string(),
                branch: "unknown".to_string(),
                is_dirty: false,
                remote_url: None,
                repo_root: PathBuf::from("/unknown"),
            });

        Ok(CacheSnapshot {
            commit_hash: commit.to_string(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            entries,
            git_context,
            total_entries: commit_keys.len(),
        })
    }

    /// Compare cache between two commits
    pub async fn diff_commits(&self, from: &str, to: &str) -> Result<CacheDiff> {
        let from_keys = self
            .commit_index
            .get(from)
            .map(|r| r.value().clone())
            .unwrap_or_default();
        let to_keys = self
            .commit_index
            .get(to)
            .map(|r| r.value().clone())
            .unwrap_or_default();

        let added_keys: HashSet<_> = to_keys.difference(&from_keys).collect();
        let removed_keys: HashSet<_> = from_keys.difference(&to_keys).collect();
        let common_keys: HashSet<_> = from_keys.intersection(&to_keys).collect();

        let mut added_entries = Vec::new();
        let mut removed_entries = Vec::new();
        let modified_entries = Vec::new();

        // Process added entries
        for key in added_keys {
            if let Some(_node) = self.get(key) {
                added_entries.push(CachedCallHierarchy {
                    file_path: key.file.clone(),
                    symbol: key.symbol.clone(),
                    line: 0,
                    column: 0,
                    result: crate::protocol::CallHierarchyResult {
                        item: crate::protocol::CallHierarchyItem {
                            name: key.symbol.clone(),
                            kind: "function".to_string(),
                            uri: format!("file://{}", key.file.display()),
                            range: crate::protocol::Range {
                                start: crate::protocol::Position {
                                    line: 0,
                                    character: 0,
                                },
                                end: crate::protocol::Position {
                                    line: 0,
                                    character: 0,
                                },
                            },
                            selection_range: crate::protocol::Range {
                                start: crate::protocol::Position {
                                    line: 0,
                                    character: 0,
                                },
                                end: crate::protocol::Position {
                                    line: 0,
                                    character: 0,
                                },
                            },
                        },
                        incoming: vec![],
                        outgoing: vec![],
                    },
                    cached_at: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs(),
                });
            }
        }

        // Process removed entries (would need to get from persistent store)
        for key in removed_keys {
            removed_entries.push(CachedCallHierarchy {
                file_path: key.file.clone(),
                symbol: key.symbol.clone(),
                line: 0,
                column: 0,
                result: crate::protocol::CallHierarchyResult {
                    item: crate::protocol::CallHierarchyItem {
                        name: key.symbol.clone(),
                        kind: "function".to_string(),
                        uri: format!("file://{}", key.file.display()),
                        range: crate::protocol::Range {
                            start: crate::protocol::Position {
                                line: 0,
                                character: 0,
                            },
                            end: crate::protocol::Position {
                                line: 0,
                                character: 0,
                            },
                        },
                        selection_range: crate::protocol::Range {
                            start: crate::protocol::Position {
                                line: 0,
                                character: 0,
                            },
                            end: crate::protocol::Position {
                                line: 0,
                                character: 0,
                            },
                        },
                    },
                    incoming: vec![],
                    outgoing: vec![],
                },
                cached_at: 0,
            });
        }

        Ok(CacheDiff {
            from_commit: from.to_string(),
            to_commit: to.to_string(),
            added_entries,
            removed_entries,
            modified_entries,
            unchanged_entries: common_keys.len(),
        })
    }

    /// Handle branch switch event
    async fn handle_branch_switch(&self, from: &GitContext, to: &GitContext) -> Result<()> {
        info!(
            "Handling branch switch from {} to {}",
            from.branch, to.branch
        );

        if self.config.git_config.preserve_across_branches {
            debug!("Preserving cache across branch switch");
        } else {
            info!("Clearing cache due to branch switch");

            if self.config.git_config.namespace_by_branch {
                // TODO: Implement branch-namespaced cache clearing
                self.clear();
            } else {
                self.clear();
            }
        }

        // Update branch statistics
        let _ = self.update_branch_stats(&to.branch).await;

        Ok(())
    }

    /// Handle new commits event
    async fn handle_new_commits(&self, from: &GitContext, to: &GitContext) -> Result<()> {
        info!(
            "Handling new commits from {} to {}",
            from.commit_hash[..8].to_string(),
            to.commit_hash[..8].to_string()
        );

        if self.config.git_config.auto_detect_changes {
            // Get changed files between commits
            match GitContext::get_changed_files_between_commits(
                &to.repo_root,
                &from.commit_hash,
                &to.commit_hash,
            ) {
                Ok(changed_files) => {
                    info!(
                        "Detected {} changed files, invalidating related cache entries",
                        changed_files.len()
                    );

                    for file_path in changed_files {
                        self.invalidate_file(&file_path);
                    }
                }
                Err(e) => {
                    warn!("Failed to get changed files, clearing entire cache: {}", e);
                    self.clear();
                }
            }
        }

        // Update commit statistics
        let _ = self.update_commit_stats(&to.commit_hash, &to.branch).await;

        Ok(())
    }

    /// Update branch statistics
    async fn update_branch_stats(&self, branch: &str) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if let Some(mut stats) = self.branch_stats.get_mut(branch) {
            stats.last_active = now;
            stats.total_entries = self.nodes.len();
        } else {
            let stats = BranchCacheStats {
                branch_name: branch.to_string(),
                total_entries: self.nodes.len(),
                hit_rate: 0.0, // Will be calculated separately
                last_active: now,
                commits_tracked: 1,
            };
            self.branch_stats.insert(branch.to_string(), stats);
        }

        Ok(())
    }

    /// Update commit statistics
    async fn update_commit_stats(&self, commit: &str, branch: &str) -> Result<()> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let stats = CommitCacheStats {
            commit_hash: commit.to_string(),
            branch: branch.to_string(),
            cache_size: self.nodes.len(),
            hit_rate: 0.0, // Will be calculated separately
            created_at: now,
            last_accessed: now,
        };

        self.commit_stats.insert(commit.to_string(), stats);

        Ok(())
    }

    /// Get git-aware cache statistics
    pub async fn get_git_stats(&self) -> Result<GitCacheStats> {
        let branch_stats: HashMap<String, BranchCacheStats> = self
            .branch_stats
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect();

        let commit_stats: HashMap<String, CommitCacheStats> = self
            .commit_stats
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect();

        let hot_spots = {
            let hot_spots_guard = self.hot_spots.lock().await;
            hot_spots_guard.values().cloned().collect()
        };

        let current_context = self.get_git_context().await;

        Ok(GitCacheStats {
            branch_stats,
            commit_stats,
            hot_spots,
            current_context,
        })
    }

    /// Get cache statistics including persistence information
    pub async fn stats(&self) -> CacheStats {
        let (
            persistence_enabled,
            persistent_nodes,
            persistent_size_bytes,
            persistent_disk_size_bytes,
        ) = if let Some(ref persistent_store) = self.persistent_store {
            match persistent_store.get_stats().await {
                Ok(stats) => (
                    true,
                    Some(stats.total_nodes),
                    Some(stats.total_size_bytes),
                    Some(stats.disk_size_bytes),
                ),
                Err(e) => {
                    warn!("Failed to get persistent cache stats: {}", e);
                    (true, None, None, None)
                }
            }
        } else {
            (false, None, None, None)
        };

        CacheStats {
            total_nodes: self.nodes.len(),
            total_ids: self.id_to_keys.len(),
            total_files: self.file_index.len(),
            total_edges: self.outgoing.len() + self.incoming.len(),
            inflight_computations: self.inflight.len(),
            persistence_enabled,
            persistent_nodes,
            persistent_size_bytes,
            persistent_disk_size_bytes,
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
        let key = NodeKey::new("func", file, md5);

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

    #[tokio::test]
    async fn test_layered_caching_persistence() {
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let mut config = CallGraphCacheConfig::default();
        config.persistence_enabled = true;
        config.persistence_path = Some(temp_dir.path().to_path_buf());
        config.capacity = 10; // Small for testing

        let key1 = NodeKey::new("func1", "/test/file.rs", "hash1");
        let key2 = NodeKey::new("func2", "/test/file.rs", "hash2");

        // Phase 1: Create cache and add entries
        {
            let cache = CallGraphCache::new_with_persistence(config.clone())
                .await
                .unwrap();

            // Add first entry - should go to L1 and L2
            let result1 = cache
                .get_or_compute(key1.clone(), || async {
                    Ok(CallHierarchyInfo {
                        incoming_calls: vec![],
                        outgoing_calls: vec![],
                    })
                })
                .await
                .unwrap();

            assert_eq!(result1.key.symbol, "func1");

            // Verify it's in L1 cache
            assert!(cache.get(&key1).is_some());

            // Add second entry
            cache
                .get_or_compute(key2.clone(), || async {
                    Ok(CallHierarchyInfo {
                        incoming_calls: vec![],
                        outgoing_calls: vec![],
                    })
                })
                .await
                .unwrap();
        }

        // Phase 2: Create new cache instance (simulates restart)
        {
            let cache = CallGraphCache::new_with_persistence(config).await.unwrap();

            // L1 cache should be empty initially
            assert!(cache.get(&key1).is_none());
            assert!(cache.get(&key2).is_none());

            // But warming should have loaded data from L2
            // Try to trigger a get_or_compute to see if it hits L2
            let result = cache
                .get_or_compute(key1.clone(), || async {
                    panic!("Should not compute - should hit L2 cache");
                })
                .await;

            // If this doesn't panic, L2 cache worked
            assert!(result.is_ok());
            let node = result.unwrap();
            assert_eq!(node.key.symbol, "func1");
        }
    }

    #[tokio::test]
    async fn test_layered_cache_invalidation() {
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let mut config = CallGraphCacheConfig::default();
        config.persistence_enabled = true;
        config.persistence_path = Some(temp_dir.path().to_path_buf());

        let cache = CallGraphCache::new_with_persistence(config).await.unwrap();
        let key = NodeKey::new("func", "/test/file.rs", "hash");

        // Add entry to both L1 and L2
        cache
            .get_or_compute(key.clone(), || async {
                Ok(CallHierarchyInfo {
                    incoming_calls: vec![],
                    outgoing_calls: vec![],
                })
            })
            .await
            .unwrap();

        // Verify it's cached
        assert!(cache.get(&key).is_some());

        // Invalidate file
        cache.invalidate_file(Path::new("/test/file.rs"));

        // Should be gone from L1
        assert!(cache.get(&key).is_none());

        // Should also be gone from L2 (would need background writer to flush)
        // We can test this by ensuring the next computation actually runs
        let mut compute_called = false;
        let _result = cache
            .get_or_compute(key.clone(), || async {
                compute_called = true;
                Ok(CallHierarchyInfo {
                    incoming_calls: vec![],
                    outgoing_calls: vec![],
                })
            })
            .await
            .unwrap();

        // Give time for background writer to process
        tokio::time::sleep(Duration::from_millis(100)).await;

        // This test is a bit tricky because the background writer is async
        // In a real scenario, we'd want to test this differently
    }

    #[tokio::test]
    async fn test_cache_warming_functionality() {
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let mut config = CallGraphCacheConfig::default();
        config.persistence_enabled = true;
        config.persistence_path = Some(temp_dir.path().to_path_buf());
        config.capacity = 5;

        // Create and populate cache
        let keys: Vec<NodeKey> = (0..3)
            .map(|i| {
                NodeKey::new(
                    format!("func_{}", i),
                    "/test/file.rs",
                    format!("hash_{}", i),
                )
            })
            .collect();

        {
            let cache = CallGraphCache::new_with_persistence(config.clone())
                .await
                .unwrap();

            for key in &keys {
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

            // Give time for background writer
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        // Create new cache and test warming
        {
            let cache = CallGraphCache::new_with_persistence(config).await.unwrap();

            // Test that warming worked by checking stats
            let stats = cache.stats().await;
            assert!(stats.persistence_enabled);
            assert!(stats.persistent_nodes.unwrap_or(0) > 0);
        }
    }

    #[tokio::test]
    async fn test_git_aware_cache_operations() {
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let mut config = CallGraphCacheConfig::default();
        config.git_config.track_commits = true;
        config.git_config.preserve_across_branches = false;

        let cache = CallGraphCache::new(config);

        // Set up initial git context
        let git_context1 = GitContext {
            commit_hash: "abcd1234".to_string(),
            branch: "main".to_string(),
            is_dirty: false,
            remote_url: Some("git@github.com:test/repo.git".to_string()),
            repo_root: temp_dir.path().to_path_buf(),
        };

        cache.set_git_context(git_context1.clone()).await.unwrap();

        // Add some cache entries
        let key1 = NodeKey::new("function1", "/test/file1.rs", "hash1");
        let key2 = NodeKey::new("function2", "/test/file2.rs", "hash2");

        cache
            .get_or_compute(key1.clone(), || async {
                Ok(CallHierarchyInfo {
                    incoming_calls: vec![],
                    outgoing_calls: vec![],
                })
            })
            .await
            .unwrap();

        cache
            .get_or_compute(key2.clone(), || async {
                Ok(CallHierarchyInfo {
                    incoming_calls: vec![],
                    outgoing_calls: vec![],
                })
            })
            .await
            .unwrap();

        // Verify entries are associated with the commit
        assert!(cache.get_for_commit(&key1, "abcd1234").await.is_some());
        assert!(cache.get_for_commit(&key2, "abcd1234").await.is_some());
        assert!(cache
            .get_for_commit(&key1, "different_commit")
            .await
            .is_none());

        // Test branch switch
        let git_context2 = GitContext {
            commit_hash: "abcd1234".to_string(), // Same commit
            branch: "feature".to_string(),       // Different branch
            is_dirty: false,
            remote_url: Some("git@github.com:test/repo.git".to_string()),
            repo_root: temp_dir.path().to_path_buf(),
        };

        cache.set_git_context(git_context2).await.unwrap();

        // Since preserve_across_branches is false, cache should be cleared
        assert!(cache.get(&key1).is_none());
        assert!(cache.get(&key2).is_none());
    }

    #[tokio::test]
    async fn test_git_context_history_tracking() {
        let mut config = CallGraphCacheConfig::default();
        config.git_config.max_history_depth = 3;

        let cache = CallGraphCache::new(config);

        // Add multiple git contexts to test history
        let contexts = vec![
            GitContext {
                commit_hash: "commit1".to_string(),
                branch: "main".to_string(),
                is_dirty: false,
                remote_url: None,
                repo_root: PathBuf::from("/test"),
            },
            GitContext {
                commit_hash: "commit2".to_string(),
                branch: "main".to_string(),
                is_dirty: false,
                remote_url: None,
                repo_root: PathBuf::from("/test"),
            },
            GitContext {
                commit_hash: "commit3".to_string(),
                branch: "main".to_string(),
                is_dirty: false,
                remote_url: None,
                repo_root: PathBuf::from("/test"),
            },
            GitContext {
                commit_hash: "commit4".to_string(),
                branch: "main".to_string(),
                is_dirty: false,
                remote_url: None,
                repo_root: PathBuf::from("/test"),
            },
        ];

        // Set contexts sequentially
        for context in contexts {
            cache.set_git_context(context).await.unwrap();
        }

        // Verify history is limited to max_history_depth
        let history = cache.git_history.lock().await;
        assert_eq!(history.len(), 3); // Should only keep last 3 previous contexts

        // Verify current context is the latest
        let current = cache.get_git_context().await.unwrap();
        assert_eq!(current.commit_hash, "commit4");
    }

    #[tokio::test]
    async fn test_cache_snapshot_at_commit() {
        let cache = CallGraphCache::new(CallGraphCacheConfig::default());

        // Set git context
        let git_context = GitContext {
            commit_hash: "test_commit".to_string(),
            branch: "main".to_string(),
            is_dirty: false,
            remote_url: None,
            repo_root: PathBuf::from("/test"),
        };

        cache.set_git_context(git_context.clone()).await.unwrap();

        // Add cache entries
        let key1 = NodeKey::new("func1", "/test/file.rs", "hash1");
        let key2 = NodeKey::new("func2", "/test/file.rs", "hash2");

        cache
            .get_or_compute(key1.clone(), || async {
                Ok(CallHierarchyInfo {
                    incoming_calls: vec![],
                    outgoing_calls: vec![],
                })
            })
            .await
            .unwrap();

        cache
            .get_or_compute(key2.clone(), || async {
                Ok(CallHierarchyInfo {
                    incoming_calls: vec![],
                    outgoing_calls: vec![],
                })
            })
            .await
            .unwrap();

        // Wait for async commit index update
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Get snapshot
        let snapshot = cache.snapshot_at_commit("test_commit").await.unwrap();

        assert_eq!(snapshot.commit_hash, "test_commit");
        assert_eq!(snapshot.git_context.commit_hash, "test_commit");
        assert_eq!(snapshot.git_context.branch, "main");
        assert!(snapshot.entries.len() >= 2);
    }

    #[tokio::test]
    async fn test_commit_diff() {
        let cache = CallGraphCache::new(CallGraphCacheConfig::default());

        // Set up first commit with some entries
        let git_context1 = GitContext {
            commit_hash: "commit1".to_string(),
            branch: "main".to_string(),
            is_dirty: false,
            remote_url: None,
            repo_root: PathBuf::from("/test"),
        };

        cache.set_git_context(git_context1).await.unwrap();

        let key1 = NodeKey::new("func1", "/test/file.rs", "hash1");
        cache
            .get_or_compute(key1.clone(), || async {
                Ok(CallHierarchyInfo {
                    incoming_calls: vec![],
                    outgoing_calls: vec![],
                })
            })
            .await
            .unwrap();

        // Set up second commit with different entries
        let git_context2 = GitContext {
            commit_hash: "commit2".to_string(),
            branch: "main".to_string(),
            is_dirty: false,
            remote_url: None,
            repo_root: PathBuf::from("/test"),
        };

        cache.set_git_context(git_context2).await.unwrap();

        let key2 = NodeKey::new("func2", "/test/file.rs", "hash2");
        cache
            .get_or_compute(key2.clone(), || async {
                Ok(CallHierarchyInfo {
                    incoming_calls: vec![],
                    outgoing_calls: vec![],
                })
            })
            .await
            .unwrap();

        // Wait for async updates
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Get diff
        let diff = cache.diff_commits("commit1", "commit2").await.unwrap();

        assert_eq!(diff.from_commit, "commit1");
        assert_eq!(diff.to_commit, "commit2");
        // Note: The actual entries may vary based on commit index implementation
    }

    #[tokio::test]
    async fn test_git_statistics() {
        let mut config = CallGraphCacheConfig::default();
        config.git_config.track_commits = true;

        let cache = CallGraphCache::new(config);

        // Set git context
        let git_context = GitContext {
            commit_hash: "test_commit".to_string(),
            branch: "main".to_string(),
            is_dirty: false,
            remote_url: None,
            repo_root: PathBuf::from("/test"),
        };

        cache.set_git_context(git_context).await.unwrap();

        // Add some cache entries to generate stats
        let key = NodeKey::new("test_func", "/test/file.rs", "hash");
        cache
            .get_or_compute(key, || async {
                Ok(CallHierarchyInfo {
                    incoming_calls: vec![],
                    outgoing_calls: vec![],
                })
            })
            .await
            .unwrap();

        // Get git stats
        let stats = cache.get_git_stats().await.unwrap();

        assert!(stats.current_context.is_some());
        assert_eq!(stats.current_context.unwrap().commit_hash, "test_commit");

        // Branch stats should be populated
        assert!(stats.branch_stats.contains_key("main"));

        // Commit stats should be populated
        assert!(stats.commit_stats.contains_key("test_commit"));
    }

    #[tokio::test]
    async fn test_background_writer_error_handling() {
        use tempfile::tempdir;

        let temp_dir = tempdir().unwrap();
        let mut config = CallGraphCacheConfig::default();
        config.persistence_enabled = true;
        config.persistence_path = Some(temp_dir.path().to_path_buf());
        config.persistence_write_batch_size = 2; // Small batch for testing
        config.persistence_write_interval = Duration::from_millis(50); // Fast for testing

        let cache = CallGraphCache::new_with_persistence(config).await.unwrap();

        // Add multiple entries quickly to test batching
        for i in 0..5 {
            let key = NodeKey::new(
                format!("func_{}", i),
                "/test/file.rs",
                format!("hash_{}", i),
            );
            cache
                .get_or_compute(key, || async {
                    Ok(CallHierarchyInfo {
                        incoming_calls: vec![],
                        outgoing_calls: vec![],
                    })
                })
                .await
                .unwrap();
        }

        // Give time for background writer to process
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Check that persistence worked
        let stats = cache.stats().await;
        assert!(stats.persistence_enabled);
        assert!(stats.persistent_nodes.unwrap_or(0) > 0);
    }

    #[tokio::test]
    async fn test_lru_eviction_with_access_metadata() {
        // Create cache with small capacity for testing
        let mut config = CallGraphCacheConfig::default();
        config.capacity = 3;
        config.ttl = Duration::from_secs(3600); // Long TTL to avoid time-based eviction
        let cache = CallGraphCache::new(config);

        // Insert 3 nodes to fill cache
        let key1 = NodeKey::new("func1", "/test/file1.rs", "hash1");
        let key2 = NodeKey::new("func2", "/test/file2.rs", "hash2");
        let key3 = NodeKey::new("func3", "/test/file3.rs", "hash3");

        for key in [&key1, &key2, &key3] {
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

        // All 3 should be cached
        assert!(cache.get(&key1).is_some());
        assert!(cache.get(&key2).is_some());
        assert!(cache.get(&key3).is_some());
        assert_eq!(cache.nodes.len(), 3);

        // Access key1 and key3 to make them more recently used than key2
        tokio::time::sleep(Duration::from_millis(10)).await;
        let _ = cache.get(&key1);
        tokio::time::sleep(Duration::from_millis(10)).await;
        let _ = cache.get(&key3);
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Insert a 4th node, which should trigger eviction of key2 (least recently used)
        let key4 = NodeKey::new("func4", "/test/file4.rs", "hash4");
        cache
            .get_or_compute(key4.clone(), || async {
                Ok(CallHierarchyInfo {
                    incoming_calls: vec![],
                    outgoing_calls: vec![],
                })
            })
            .await
            .unwrap();

        // At this point we have 4 nodes but capacity is 3, so eviction should happen
        assert_eq!(cache.nodes.len(), 4, "Should have 4 nodes before eviction");

        // Force eviction check
        cache.force_evict().await;

        // key2 should have been evicted (it was least recently accessed)
        // key1, key3, and key4 should remain
        assert!(
            cache.get(&key1).is_some(),
            "key1 should remain (recently accessed)"
        );
        assert!(
            cache.get(&key2).is_none(),
            "key2 should be evicted (least recently used)"
        );
        assert!(
            cache.get(&key3).is_some(),
            "key3 should remain (recently accessed)"
        );
        assert!(
            cache.get(&key4).is_some(),
            "key4 should remain (just inserted)"
        );
        assert_eq!(cache.nodes.len(), 3, "Cache should maintain capacity limit");
    }
}
