use serde::{Deserialize, Serialize};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::time::Instant;

/// Unique identifier for a node in the call graph (logical identity)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId {
    pub symbol: String,
    pub file: PathBuf,
}

impl NodeId {
    pub fn new(symbol: impl Into<String>, file: impl Into<PathBuf>) -> Self {
        Self {
            symbol: symbol.into(),
            file: file.into(),
        }
    }
}

/// Content-addressed key for cache lookups (includes MD5 hash)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeKey {
    pub symbol: String,
    pub file: PathBuf,
    pub content_md5: String,
}

impl NodeKey {
    pub fn new(
        symbol: impl Into<String>,
        file: impl Into<PathBuf>,
        content_md5: impl Into<String>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            file: file.into(),
            content_md5: content_md5.into(),
        }
    }

    pub fn to_node_id(&self) -> NodeId {
        NodeId::new(&self.symbol, &self.file)
    }
}

impl PartialEq for NodeKey {
    fn eq(&self, other: &Self) -> bool {
        self.symbol == other.symbol
            && self.file == other.file
            && self.content_md5 == other.content_md5
    }
}

impl Eq for NodeKey {}

impl Hash for NodeKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.symbol.hash(state);
        self.file.hash(state);
        self.content_md5.hash(state);
    }
}

/// Call hierarchy information returned from LSP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallHierarchyInfo {
    pub incoming_calls: Vec<CallInfo>,
    pub outgoing_calls: Vec<CallInfo>,
}

/// Information about a single call relationship
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallInfo {
    pub name: String,
    pub file_path: String,
    pub line: u32,
    pub column: u32,
    pub symbol_kind: String,
}

/// A cached node in the call graph
#[derive(Debug, Clone)]
pub struct CachedNode {
    pub key: NodeKey,
    pub info: CallHierarchyInfo,
    pub created_at: Instant,
    pub last_accessed: Instant,
    pub access_count: usize,
}

impl CachedNode {
    pub fn new(key: NodeKey, info: CallHierarchyInfo) -> Self {
        let now = Instant::now();
        Self {
            key,
            info,
            created_at: now,
            last_accessed: now,
            access_count: 1,
        }
    }

    pub fn touch(&mut self) {
        self.last_accessed = Instant::now();
        self.access_count += 1;
    }
}

/// Statistics about the cache
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    pub total_nodes: usize,
    pub total_ids: usize,
    pub total_files: usize,
    pub total_edges: usize,
    pub inflight_computations: usize,
}

/// Generic cache key for LSP operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspCacheKey {
    pub file: PathBuf,
    pub line: u32,
    pub column: u32,
    pub content_md5: String,
    pub operation: LspOperation,
    pub extra_params: Option<String>, // For operation-specific parameters (e.g., include_declaration for references)
}

impl LspCacheKey {
    pub fn new(
        file: impl Into<PathBuf>,
        line: u32,
        column: u32,
        content_md5: impl Into<String>,
        operation: LspOperation,
        extra_params: Option<String>,
    ) -> Self {
        Self {
            file: file.into(),
            line,
            column,
            content_md5: content_md5.into(),
            operation,
            extra_params,
        }
    }
}

impl PartialEq for LspCacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.file == other.file
            && self.line == other.line
            && self.column == other.column
            && self.content_md5 == other.content_md5
            && self.operation == other.operation
            && self.extra_params == other.extra_params
    }
}

impl Eq for LspCacheKey {}

impl Hash for LspCacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.file.hash(state);
        self.line.hash(state);
        self.column.hash(state);
        self.content_md5.hash(state);
        self.operation.hash(state);
        self.extra_params.hash(state);
    }
}

/// LSP operation types for caching
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LspOperation {
    CallHierarchy,
    Definition,
    References,
    Hover,
    DocumentSymbols,
}

/// Generic cached node for LSP operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedLspNode<T> {
    pub key: LspCacheKey,
    pub data: T,
    #[serde(with = "instant_serialization")]
    pub created_at: Instant,
    #[serde(with = "instant_serialization")]
    pub last_accessed: Instant,
    pub access_count: usize,
}

mod instant_serialization {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    pub fn serialize<S>(_instant: &Instant, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Convert Instant to duration since Unix epoch for serialization
        // This is an approximation since Instant doesn't have a fixed epoch
        let duration_since_unix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        duration_since_unix.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Instant, D::Error>
    where
        D: Deserializer<'de>,
    {
        let duration = Duration::deserialize(deserializer)?;
        // Convert back to Instant (this is approximate)
        // For cache purposes, we'll use current time minus the stored duration
        let now = Instant::now();
        Ok(now - duration.min(now.elapsed()))
    }
}

impl<T> CachedLspNode<T> {
    pub fn new(key: LspCacheKey, data: T) -> Self {
        let now = Instant::now();
        Self {
            key,
            data,
            created_at: now,
            last_accessed: now,
            access_count: 1,
        }
    }

    pub fn touch(&mut self) {
        self.last_accessed = Instant::now();
        self.access_count += 1;
    }
}

/// Definition locations for caching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefinitionInfo {
    pub locations: Vec<LocationInfo>,
}

/// References information for caching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferencesInfo {
    pub locations: Vec<LocationInfo>,
    pub include_declaration: bool,
}

/// Hover information for caching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoverInfo {
    pub contents: Option<String>,
    pub range: Option<RangeInfo>,
}

/// Document symbols information for caching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSymbolsInfo {
    pub symbols: Vec<DocumentSymbolInfo>,
}

/// Location information for caching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationInfo {
    pub uri: String,
    pub range: RangeInfo,
}

/// Range information for caching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RangeInfo {
    pub start_line: u32,
    pub start_character: u32,
    pub end_line: u32,
    pub end_character: u32,
}

/// Document symbol information for caching
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSymbolInfo {
    pub name: String,
    pub kind: String,
    pub range: RangeInfo,
    pub selection_range: RangeInfo,
    pub children: Option<Vec<DocumentSymbolInfo>>,
}

/// Generic cache statistics for different operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspCacheStats {
    pub operation: LspOperation,
    pub total_entries: usize,
    pub hit_count: u64,
    pub miss_count: u64,
    pub eviction_count: u64,
    pub inflight_count: usize,
    pub memory_usage_estimate: usize,
}

/// Combined cache statistics for all operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllCacheStats {
    pub per_operation: Vec<LspCacheStats>,
    pub total_memory_usage: usize,
    pub cache_directory: Option<String>,
    pub persistent_cache_enabled: bool,
}
