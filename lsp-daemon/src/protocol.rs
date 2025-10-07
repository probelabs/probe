use crate::language_detector::Language;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{timeout, Duration};
use uuid::Uuid;

/// Shared limit for length-prefixed messages (also used by daemon).
pub const MAX_MESSAGE_SIZE: usize = 10 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DaemonRequest {
    Connect {
        client_id: Uuid,
    },
    // Workspace management
    InitializeWorkspace {
        request_id: Uuid,
        workspace_root: PathBuf,
        language: Option<Language>,
    },
    InitWorkspaces {
        request_id: Uuid,
        workspace_root: PathBuf,
        languages: Option<Vec<Language>>,
        recursive: bool,
        enable_watchdog: bool,
    },
    ListWorkspaces {
        request_id: Uuid,
    },
    // Health check endpoint for monitoring
    HealthCheck {
        request_id: Uuid,
    },
    // Analysis requests with optional workspace hints
    CallHierarchy {
        request_id: Uuid,
        file_path: PathBuf,
        line: u32,
        column: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace_hint: Option<PathBuf>,
    },
    Definition {
        request_id: Uuid,
        file_path: PathBuf,
        line: u32,
        column: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace_hint: Option<PathBuf>,
    },
    References {
        request_id: Uuid,
        file_path: PathBuf,
        line: u32,
        column: u32,
        include_declaration: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace_hint: Option<PathBuf>,
    },
    Hover {
        request_id: Uuid,
        file_path: PathBuf,
        line: u32,
        column: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace_hint: Option<PathBuf>,
    },
    Completion {
        request_id: Uuid,
        file_path: PathBuf,
        line: u32,
        column: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace_hint: Option<PathBuf>,
    },
    DocumentSymbols {
        request_id: Uuid,
        file_path: PathBuf,
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace_hint: Option<PathBuf>,
    },
    WorkspaceSymbols {
        request_id: Uuid,
        query: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace_hint: Option<PathBuf>,
    },
    Implementations {
        request_id: Uuid,
        file_path: PathBuf,
        line: u32,
        column: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace_hint: Option<PathBuf>,
    },
    TypeDefinition {
        request_id: Uuid,
        file_path: PathBuf,
        line: u32,
        column: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace_hint: Option<PathBuf>,
    },
    // System requests
    Status {
        request_id: Uuid,
    },
    /// Lightweight version info (no DB, no server stats). Safe for early boot.
    Version {
        request_id: Uuid,
    },
    ListLanguages {
        request_id: Uuid,
    },
    Shutdown {
        request_id: Uuid,
    },
    Ping {
        request_id: Uuid,
    },
    GetLogs {
        request_id: Uuid,
        lines: usize,
        #[serde(default)]
        since_sequence: Option<u64>, // New optional field for sequence-based retrieval
        #[serde(skip_serializing_if = "Option::is_none")]
        #[serde(default)]
        min_level: Option<LogLevel>, // Optional minimum log level filter
    },
    /// Lightweight database writer/lock snapshot for diagnostics
    DbLockSnapshot {
        request_id: Uuid,
    },
    // Indexing management requests
    StartIndexing {
        request_id: Uuid,
        workspace_root: PathBuf,
        config: IndexingConfig,
    },
    StopIndexing {
        request_id: Uuid,
        force: bool,
    },
    IndexingStatus {
        request_id: Uuid,
    },
    IndexingConfig {
        request_id: Uuid,
    },
    SetIndexingConfig {
        request_id: Uuid,
        config: IndexingConfig,
    },
    // Cache management requests
    CacheStats {
        request_id: Uuid,
        detailed: bool,
        git: bool,
    },
    CacheClear {
        request_id: Uuid,
        older_than_days: Option<u64>,
        file_path: Option<PathBuf>,
        commit_hash: Option<String>,
        all: bool,
    },
    CacheExport {
        request_id: Uuid,
        output_path: PathBuf,
        current_branch_only: bool,
        compress: bool,
    },
    CacheImport {
        request_id: Uuid,
        input_path: PathBuf,
        merge: bool,
    },
    CacheCompact {
        request_id: Uuid,
        target_size_mb: Option<usize>,
    },

    // Workspace cache management requests
    WorkspaceCacheList {
        request_id: Uuid,
    },
    WorkspaceCacheInfo {
        request_id: Uuid,
        workspace_path: Option<PathBuf>,
    },
    WorkspaceCacheClear {
        request_id: Uuid,
        workspace_path: Option<PathBuf>,
        older_than_seconds: Option<u64>,
    },

    // Symbol-specific cache clearing
    ClearSymbolCache {
        request_id: Uuid,
        file_path: PathBuf,
        symbol_name: String,
        line: Option<u32>,
        column: Option<u32>,
        methods: Option<Vec<String>>,
        all_positions: bool,
    },

    // Git-aware requests
    GetCallHierarchyAtCommit {
        request_id: Uuid,
        file_path: PathBuf,
        symbol: String,
        line: u32,
        column: u32,
        commit_hash: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace_hint: Option<PathBuf>,
    },
    GetCacheHistory {
        request_id: Uuid,
        file_path: PathBuf,
        symbol: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace_hint: Option<PathBuf>,
    },
    GetCacheAtCommit {
        request_id: Uuid,
        commit_hash: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace_hint: Option<PathBuf>,
    },
    DiffCacheCommits {
        request_id: Uuid,
        from_commit: String,
        to_commit: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace_hint: Option<PathBuf>,
    },

    // Cache key listing
    CacheListKeys {
        request_id: Uuid,
        workspace_path: Option<PathBuf>,
        operation_filter: Option<String>,
        file_pattern_filter: Option<String>,
        limit: usize,
        offset: usize,
        sort_by: String,
        sort_order: String,
        detailed: bool,
    },

    /// Get workspace database file path (used by CLI for offline operations)
    WorkspaceDbPath {
        request_id: Uuid,
        workspace_path: Option<PathBuf>,
    },

    // Index export request
    IndexExport {
        request_id: Uuid,
        workspace_path: Option<PathBuf>,
        output_path: PathBuf,
        checkpoint: bool,
    },
    /// Force WAL checkpoint and wait for exclusive access if needed
    WalSync {
        request_id: Uuid,
        /// Maximum seconds to wait (0 = wait indefinitely)
        timeout_secs: u64,
        /// Quiesce readers in this process before checkpoint (blocks new reads)
        #[serde(default)]
        quiesce: bool,
        /// Checkpoint mode to use: "auto" (default behavior), or one of
        /// "passive", "full", "restart", "truncate".
        #[serde(default)]
        mode: String,
        /// Use engine-direct checkpoint API (turso connection.checkpoint) instead of PRAGMA path.
        /// Defaults to false for backwards compatibility.
        #[serde(default)]
        direct: bool,
    },
    /// Cancel a long-running request (e.g., WAL sync) by its request ID
    Cancel {
        request_id: Uuid,
        cancel_request_id: Uuid,
    },
    /// Run an on-demand edge audit scan over the current workspace DB
    EdgeAuditScan {
        request_id: Uuid,
        #[serde(skip_serializing_if = "Option::is_none")]
        workspace_path: Option<PathBuf>,
        #[serde(default)]
        samples: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
#[allow(clippy::large_enum_variant)]
pub enum DaemonResponse {
    Connected {
        request_id: Uuid,
        daemon_version: String,
    },
    // Workspace responses
    WorkspaceInitialized {
        request_id: Uuid,
        workspace_root: PathBuf,
        language: Language,
        lsp_server: String,
    },
    WorkspacesInitialized {
        request_id: Uuid,
        initialized: Vec<InitializedWorkspace>,
        errors: Vec<String>,
    },
    WorkspaceList {
        request_id: Uuid,
        workspaces: Vec<WorkspaceInfo>,
    },
    // Analysis responses
    CallHierarchy {
        request_id: Uuid,
        result: CallHierarchyResult,
        #[serde(skip_serializing_if = "Option::is_none")]
        warnings: Option<Vec<String>>,
    },
    Definition {
        request_id: Uuid,
        locations: Vec<Location>,
        #[serde(skip_serializing_if = "Option::is_none")]
        warnings: Option<Vec<String>>,
    },
    References {
        request_id: Uuid,
        locations: Vec<Location>,
        #[serde(skip_serializing_if = "Option::is_none")]
        warnings: Option<Vec<String>>,
    },
    Hover {
        request_id: Uuid,
        content: Option<HoverContent>,
        #[serde(skip_serializing_if = "Option::is_none")]
        warnings: Option<Vec<String>>,
    },
    Completion {
        request_id: Uuid,
        items: Vec<CompletionItem>,
        #[serde(skip_serializing_if = "Option::is_none")]
        warnings: Option<Vec<String>>,
    },
    DocumentSymbols {
        request_id: Uuid,
        symbols: Vec<DocumentSymbol>,
        #[serde(skip_serializing_if = "Option::is_none")]
        warnings: Option<Vec<String>>,
    },
    WorkspaceSymbols {
        request_id: Uuid,
        symbols: Vec<SymbolInformation>,
        #[serde(skip_serializing_if = "Option::is_none")]
        warnings: Option<Vec<String>>,
    },
    Implementations {
        request_id: Uuid,
        locations: Vec<Location>,
        #[serde(skip_serializing_if = "Option::is_none")]
        warnings: Option<Vec<String>>,
    },
    TypeDefinition {
        request_id: Uuid,
        locations: Vec<Location>,
        #[serde(skip_serializing_if = "Option::is_none")]
        warnings: Option<Vec<String>>,
    },
    // System responses
    Status {
        request_id: Uuid,
        status: DaemonStatus,
    },
    /// Lightweight version info
    VersionInfo {
        request_id: Uuid,
        version: String,
        git_hash: String,
        build_date: String,
    },
    LanguageList {
        request_id: Uuid,
        languages: Vec<LanguageInfo>,
    },
    Shutdown {
        request_id: Uuid,
    },
    Pong {
        request_id: Uuid,
    },
    HealthCheck {
        request_id: Uuid,
        healthy: bool,
        uptime_seconds: u64,
        total_requests: usize,
        active_connections: usize,
        active_servers: usize,
        memory_usage_mb: f64,
        #[serde(default)]
        lsp_server_health: Vec<LspServerHealthInfo>,
    },
    Logs {
        request_id: Uuid,
        entries: Vec<LogEntry>,
    },
    /// Lightweight database writer/lock snapshot response
    DbLockSnapshotResponse {
        request_id: Uuid,
        busy: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        gate_owner_op: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        gate_owner_ms: Option<u128>,
        #[serde(skip_serializing_if = "Option::is_none")]
        section_label: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        section_ms: Option<u128>,
        #[serde(skip_serializing_if = "Option::is_none")]
        active_ms: Option<u128>,
    },
    // Indexing management responses
    IndexingStarted {
        request_id: Uuid,
        workspace_root: PathBuf,
        session_id: String,
    },
    IndexingStopped {
        request_id: Uuid,
        was_running: bool,
    },
    IndexingStatusResponse {
        request_id: Uuid,
        status: IndexingStatusInfo,
    },
    IndexingConfigResponse {
        request_id: Uuid,
        config: IndexingConfig,
    },
    IndexingConfigSet {
        request_id: Uuid,
        config: IndexingConfig,
    },
    // Cache management responses
    CacheStats {
        request_id: Uuid,
        stats: CacheStatistics,
    },
    CacheCleared {
        request_id: Uuid,
        result: ClearResult,
    },
    CacheExported {
        request_id: Uuid,
        output_path: PathBuf,
        entries_exported: usize,
        compressed: bool,
    },
    CacheImported {
        request_id: Uuid,
        result: ImportResult,
    },
    CacheCompacted {
        request_id: Uuid,
        result: CompactResult,
    },

    // Workspace cache management responses
    WorkspaceCacheList {
        request_id: Uuid,
        workspaces: Vec<WorkspaceCacheEntry>,
    },
    WorkspaceCacheInfo {
        request_id: Uuid,
        workspace_info: Option<Box<WorkspaceCacheInfo>>,
        all_workspaces_info: Option<Vec<WorkspaceCacheInfo>>,
    },
    WorkspaceCacheCleared {
        request_id: Uuid,
        result: WorkspaceClearResult,
    },

    // Symbol cache clearing response
    SymbolCacheCleared {
        request_id: Uuid,
        result: SymbolCacheClearResult,
    },

    // Git-aware responses
    CacheHistory {
        request_id: Uuid,
        history: Vec<CacheHistoryEntry>,
    },
    CacheAtCommit {
        request_id: Uuid,
        commit_hash: String,
        snapshot: CacheSnapshot,
    },
    CacheCommitDiff {
        request_id: Uuid,
        from_commit: String,
        to_commit: String,
        diff: CacheDiff,
    },
    CallHierarchyAtCommit {
        request_id: Uuid,
        result: CallHierarchyResult,
        commit_hash: String,
        git_context: Option<GitContext>,
    },

    // Cache key listing response
    CacheListKeys {
        request_id: Uuid,
        keys: Vec<CacheKeyInfo>,
        total_count: usize,
        offset: usize,
        limit: usize,
        has_more: bool,
    },

    WorkspaceDbPath {
        request_id: Uuid,
        workspace_path: PathBuf,
        db_path: PathBuf,
    },

    // Index export response
    IndexExported {
        request_id: Uuid,
        workspace_path: PathBuf,
        output_path: PathBuf,
        database_size_bytes: usize,
    },
    /// Response for WAL sync request
    WalSynced {
        request_id: Uuid,
        waited_ms: u64,
        iterations: u32,
        details: Option<String>,
    },
    /// Edge audit report for on-demand scan
    EdgeAuditReport {
        request_id: Uuid,
        counts: EdgeAuditInfo,
        samples: Vec<String>,
    },

    Error {
        request_id: Uuid,
        error: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallHierarchyResult {
    pub item: CallHierarchyItem,
    pub incoming: Vec<CallHierarchyCall>,
    pub outgoing: Vec<CallHierarchyCall>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallHierarchyItem {
    pub name: String,
    pub kind: String,
    pub uri: String,
    pub range: Range,
    pub selection_range: Range,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallHierarchyCall {
    pub from: CallHierarchyItem,
    pub from_ranges: Vec<Range>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub uri: String,
    pub range: Range,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Range {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoverContent {
    pub contents: String,
    pub range: Option<Range>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionItem {
    pub label: String,
    pub kind: Option<CompletionItemKind>,
    pub detail: Option<String>,
    pub documentation: Option<String>,
    pub insert_text: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CompletionItemKind {
    Text = 1,
    Method = 2,
    Function = 3,
    Constructor = 4,
    Field = 5,
    Variable = 6,
    Class = 7,
    Interface = 8,
    Module = 9,
    Property = 10,
    Unit = 11,
    Value = 12,
    Enum = 13,
    Keyword = 14,
    Snippet = 15,
    Color = 16,
    File = 17,
    Reference = 18,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSymbol {
    pub name: String,
    pub detail: Option<String>,
    pub kind: SymbolKind,
    pub deprecated: Option<bool>,
    pub range: Range,
    pub selection_range: Range,
    pub children: Option<Vec<DocumentSymbol>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInformation {
    pub name: String,
    pub kind: SymbolKind,
    pub deprecated: Option<bool>,
    pub location: Location,
    pub container_name: Option<String>,
    pub tags: Option<Vec<SymbolTag>>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(try_from = "u32")]
pub enum SymbolTag {
    Deprecated = 1,
}

impl TryFrom<u32> for SymbolTag {
    type Error = String;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(SymbolTag::Deprecated),
            _ => Err(format!("Unknown SymbolTag value: {}", value)),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
pub enum SymbolKind {
    File = 1,
    Module = 2,
    Namespace = 3,
    Package = 4,
    Class = 5,
    Method = 6,
    Property = 7,
    Field = 8,
    Constructor = 9,
    Enum = 10,
    Interface = 11,
    Function = 12,
    Variable = 13,
    Constant = 14,
    String = 15,
    Number = 16,
    Boolean = 17,
    Array = 18,
    Object = 19,
    Key = 20,
    Null = 21,
    EnumMember = 22,
    Struct = 23,
    Event = 24,
    Operator = 25,
    TypeParameter = 26,
}

impl TryFrom<u32> for SymbolKind {
    type Error = String;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(SymbolKind::File),
            2 => Ok(SymbolKind::Module),
            3 => Ok(SymbolKind::Namespace),
            4 => Ok(SymbolKind::Package),
            5 => Ok(SymbolKind::Class),
            6 => Ok(SymbolKind::Method),
            7 => Ok(SymbolKind::Property),
            8 => Ok(SymbolKind::Field),
            9 => Ok(SymbolKind::Constructor),
            10 => Ok(SymbolKind::Enum),
            11 => Ok(SymbolKind::Interface),
            12 => Ok(SymbolKind::Function),
            13 => Ok(SymbolKind::Variable),
            14 => Ok(SymbolKind::Constant),
            15 => Ok(SymbolKind::String),
            16 => Ok(SymbolKind::Number),
            17 => Ok(SymbolKind::Boolean),
            18 => Ok(SymbolKind::Array),
            19 => Ok(SymbolKind::Object),
            20 => Ok(SymbolKind::Key),
            21 => Ok(SymbolKind::Null),
            22 => Ok(SymbolKind::EnumMember),
            23 => Ok(SymbolKind::Struct),
            24 => Ok(SymbolKind::Event),
            25 => Ok(SymbolKind::Operator),
            26 => Ok(SymbolKind::TypeParameter),
            _ => Err(format!("Unknown SymbolKind value: {}", value)),
        }
    }
}

impl std::str::FromStr for SymbolKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "File" => Ok(SymbolKind::File),
            "Module" => Ok(SymbolKind::Module),
            "Namespace" => Ok(SymbolKind::Namespace),
            "Package" => Ok(SymbolKind::Package),
            "Class" => Ok(SymbolKind::Class),
            "Method" => Ok(SymbolKind::Method),
            "Property" => Ok(SymbolKind::Property),
            "Field" => Ok(SymbolKind::Field),
            "Constructor" => Ok(SymbolKind::Constructor),
            "Enum" => Ok(SymbolKind::Enum),
            "Interface" => Ok(SymbolKind::Interface),
            "Function" => Ok(SymbolKind::Function),
            "Variable" => Ok(SymbolKind::Variable),
            "Constant" => Ok(SymbolKind::Constant),
            "String" => Ok(SymbolKind::String),
            "Number" => Ok(SymbolKind::Number),
            "Boolean" => Ok(SymbolKind::Boolean),
            "Array" => Ok(SymbolKind::Array),
            "Object" => Ok(SymbolKind::Object),
            "Key" => Ok(SymbolKind::Key),
            "Null" => Ok(SymbolKind::Null),
            "EnumMember" => Ok(SymbolKind::EnumMember),
            "Struct" => Ok(SymbolKind::Struct),
            "Event" => Ok(SymbolKind::Event),
            "Operator" => Ok(SymbolKind::Operator),
            "TypeParameter" => Ok(SymbolKind::TypeParameter),
            _ => Err(format!("Unknown SymbolKind string: {}", s)),
        }
    }
}

impl<'de> serde::Deserialize<'de> for SymbolKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{self, Unexpected, Visitor};
        use std::fmt;

        struct SymbolKindVisitor;

        impl<'de> Visitor<'de> for SymbolKindVisitor {
            type Value = SymbolKind;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a SymbolKind integer or string")
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                SymbolKind::try_from(value as u32)
                    .map_err(|e| de::Error::invalid_value(Unexpected::Unsigned(value), &e.as_str()))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value < 0 {
                    return Err(de::Error::invalid_value(
                        Unexpected::Signed(value),
                        &"a positive integer",
                    ));
                }
                self.visit_u64(value as u64)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                use std::str::FromStr;
                SymbolKind::from_str(value)
                    .map_err(|e| de::Error::invalid_value(Unexpected::Str(value), &e.as_str()))
            }
        }

        deserializer.deserialize_any(SymbolKindVisitor)
    }
}

// Indexing configuration and status types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingConfig {
    #[serde(default)]
    pub max_workers: Option<usize>,
    #[serde(default)]
    pub memory_budget_mb: Option<u64>,
    #[serde(default)]
    pub exclude_patterns: Vec<String>,
    #[serde(default)]
    pub include_patterns: Vec<String>,
    #[serde(default)]
    pub specific_files: Vec<String>,
    #[serde(default)]
    pub max_file_size_mb: Option<u64>,
    #[serde(default)]
    pub incremental: Option<bool>,
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub recursive: bool,

    // LSP Caching Configuration
    #[serde(default)]
    pub lsp_indexing_enabled: Option<bool>,
    #[serde(default)]
    pub cache_call_hierarchy: Option<bool>,
    #[serde(default)]
    pub cache_definitions: Option<bool>,
    #[serde(default)]
    pub cache_references: Option<bool>,
    #[serde(default)]
    pub cache_hover: Option<bool>,
    #[serde(default)]
    pub cache_document_symbols: Option<bool>,
    // cache_during_indexing removed - indexing ALWAYS caches LSP data
    #[serde(default)]
    pub preload_common_symbols: Option<bool>,
    #[serde(default)]
    pub max_cache_entries_per_operation: Option<usize>,
    #[serde(default)]
    pub lsp_operation_timeout_ms: Option<u64>,
    #[serde(default)]
    pub lsp_priority_operations: Vec<String>,
    #[serde(default)]
    pub lsp_disabled_operations: Vec<String>,
}

impl Default for IndexingConfig {
    fn default() -> Self {
        Self {
            max_workers: None,
            memory_budget_mb: None,            // Removed - not used anymore
            lsp_indexing_enabled: Some(false), // Disabled by default for structural analysis focus
            exclude_patterns: vec![
                "*.git/*".to_string(),
                "*/node_modules/*".to_string(),
                "*/target/*".to_string(),
                "*/build/*".to_string(),
                "*/dist/*".to_string(),
            ],
            include_patterns: vec![],
            specific_files: vec![],
            max_file_size_mb: Some(10),
            incremental: Some(true),
            languages: vec![],
            recursive: true,

            // LSP Caching defaults (None means use system defaults)
            cache_call_hierarchy: None,
            cache_definitions: None,
            cache_references: None,
            cache_hover: None,
            cache_document_symbols: None,
            // cache_during_indexing removed - always enabled
            preload_common_symbols: None,
            max_cache_entries_per_operation: None,
            lsp_operation_timeout_ms: None,
            lsp_priority_operations: vec![],
            lsp_disabled_operations: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingStatusInfo {
    pub manager_status: String, // "Idle", "Discovering", "Indexing", "Paused", "Shutdown", etc.
    pub progress: IndexingProgressInfo,
    pub queue: IndexingQueueInfo,
    pub workers: Vec<IndexingWorkerInfo>,
    pub session_id: Option<String>,
    pub started_at: Option<u64>, // Unix timestamp
    pub elapsed_seconds: u64,
    pub lsp_enrichment: Option<LspEnrichmentInfo>, // LSP enrichment progress
    pub database: Option<DatabaseInfo>,            // Actual persisted database counts
    #[serde(default)]
    pub lsp_indexing: Option<LspIndexingInfo>, // LSP indexing (prewarm) aggregated stats
    /// Optional sync status, populated when the workspace backend is available
    #[serde(default)]
    pub sync: Option<SyncStatusInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingProgressInfo {
    pub total_files: u64,
    pub processed_files: u64,
    pub failed_files: u64,
    pub active_files: u64,
    pub skipped_files: u64,
    pub processed_bytes: u64,
    pub symbols_extracted: u64,
    pub progress_ratio: f64,
    pub files_per_second: f64,
    pub bytes_per_second: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingQueueInfo {
    pub total_items: usize,
    pub pending_items: usize,
    pub high_priority_items: usize,
    pub medium_priority_items: usize,
    pub low_priority_items: usize,
    pub is_paused: bool,
    pub memory_pressure: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingWorkerInfo {
    pub worker_id: usize,
    pub is_active: bool,
    pub current_file: Option<PathBuf>,
    pub files_processed: u64,
    pub bytes_processed: u64,
    pub symbols_extracted: u64,
    pub errors_encountered: u64,
    pub last_activity: Option<u64>, // Unix timestamp
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspEnrichmentInfo {
    pub is_enabled: bool,
    pub active_workers: u64,
    pub symbols_processed: u64,
    pub symbols_enriched: u64,
    pub symbols_failed: u64,
    pub queue_stats: LspEnrichmentQueueInfo,
    /// Current in-memory queue size (items pending in RAM)
    #[serde(default)]
    pub in_memory_queue_items: usize,
    /// Current in-memory total operations (refs+impls+calls) pending in RAM
    #[serde(default)]
    pub in_memory_queue_operations: usize,
    /// In-memory priority breakdown
    #[serde(default)]
    pub in_memory_high_priority_items: usize,
    #[serde(default)]
    pub in_memory_medium_priority_items: usize,
    #[serde(default)]
    pub in_memory_low_priority_items: usize,
    /// In-memory per-operation breakdown
    #[serde(default)]
    pub in_memory_references_operations: usize,
    #[serde(default)]
    pub in_memory_implementations_operations: usize,
    #[serde(default)]
    pub in_memory_call_hierarchy_operations: usize,
    // DB writer status (snapshot)
    #[serde(default)]
    pub writer_busy: bool,
    #[serde(default)]
    pub writer_active_ms: u64,
    #[serde(default)]
    pub writer_last_ms: u64,
    #[serde(default)]
    pub writer_last_symbols: u64,
    #[serde(default)]
    pub writer_last_edges: u64,
    // New: DB writer gate owner and section details
    #[serde(default)]
    pub writer_gate_owner_op: String,
    #[serde(default)]
    pub writer_gate_owner_ms: u64,
    #[serde(default)]
    pub writer_section_label: String,
    #[serde(default)]
    pub writer_section_ms: u64,
    // DB readers status
    #[serde(default)]
    pub reader_active: u64,
    #[serde(default)]
    pub reader_last_label: String,
    #[serde(default)]
    pub reader_last_ms: u64,
    /// Total call hierarchy edges persisted by workers
    pub edges_created: u64,
    /// Total reference edges persisted by workers
    #[serde(default)]
    pub reference_edges_created: u64,
    /// Total implementation edges persisted by workers
    #[serde(default)]
    pub implementation_edges_created: u64,
    /// Positions adjusted (snapped to identifier)
    #[serde(default)]
    pub positions_adjusted: u64,
    /// Successful call hierarchy operations
    #[serde(default)]
    pub call_hierarchy_success: u64,
    /// Total references found across symbols
    #[serde(default)]
    pub references_found: u64,
    /// Total implementations found across symbols
    #[serde(default)]
    pub implementations_found: u64,
    /// Reference operations attempted (including empty results)
    #[serde(default)]
    pub references_attempted: u64,
    /// Implementation operations attempted (including empty results)
    #[serde(default)]
    pub implementations_attempted: u64,
    /// Call hierarchy operations attempted (including empty results)
    #[serde(default)]
    pub call_hierarchy_attempted: u64,
    pub success_rate: f64, // Percentage of successfully enriched symbols
    /// Implementation ops skipped by core-trait/builtin heuristic (total)
    #[serde(default)]
    pub impls_skipped_core_total: u64,
    /// Implementation ops skipped (Rust core traits)
    #[serde(default)]
    pub impls_skipped_core_rust: u64,
    /// Implementation ops skipped (JS/TS builtins)
    #[serde(default)]
    pub impls_skipped_core_js_ts: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspEnrichmentQueueInfo {
    pub total_items: usize,
    pub high_priority_items: usize,
    pub medium_priority_items: usize,
    pub low_priority_items: usize,
    #[serde(default)]
    pub total_operations: usize,
    #[serde(default)]
    pub references_operations: usize,
    #[serde(default)]
    pub implementations_operations: usize,
    #[serde(default)]
    pub call_hierarchy_operations: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspIndexingInfo {
    pub positions_adjusted: u64,
    pub call_hierarchy_success: u64,
    pub symbols_persisted: u64,
    pub edges_persisted: u64,
    pub references_found: u64,
    pub reference_edges_persisted: u64,
    pub lsp_calls: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseInfo {
    pub total_symbols: u64,           // Count from symbol_state table
    pub total_edges: u64,             // Count from edge table
    pub total_files: u64,             // Count from file table
    pub workspace_id: Option<String>, // Current workspace ID
    #[serde(default)]
    pub counts_locked: bool, // True if counts could not be fetched due to a DB lock
    #[serde(default)]
    pub db_quiesced: bool, // True if counts skipped due to quiesce
    // Reader/writer gate status: write-held indicates quiesce write lock is currently held
    #[serde(default)]
    pub rw_gate_write_held: bool,
    // Number of active readers
    #[serde(default)]
    pub reader_active: u64,
    // Last reader label and duration
    #[serde(default)]
    pub reader_last_label: String,
    #[serde(default)]
    pub reader_last_ms: u64,
    // Writer snapshot (for quick lock visibility in index-status)
    #[serde(default)]
    pub writer_busy: bool,
    #[serde(default)]
    pub writer_active_ms: u64,
    #[serde(default)]
    pub writer_last_ms: u64,
    #[serde(default)]
    pub writer_last_symbols: u64,
    #[serde(default)]
    pub writer_last_edges: u64,
    #[serde(default)]
    pub writer_gate_owner_op: String,
    #[serde(default)]
    pub writer_gate_owner_ms: u64,
    #[serde(default)]
    pub writer_section_label: String,
    #[serde(default)]
    pub writer_section_ms: u64,
    // Whether MVCC was enabled for this database
    #[serde(default)]
    pub mvcc_enabled: bool,
    #[serde(default)]
    pub edge_audit: Option<EdgeAuditInfo>,
}

/// Edge audit counters (lightweight summary of malformed IDs detected)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EdgeAuditInfo {
    pub eid001_abs_path: u64,
    pub eid002_uid_path_mismatch: u64,
    pub eid003_malformed_uid: u64,
    pub eid004_zero_line: u64,
    pub eid009_non_relative_file_path: u64,
    pub eid010_self_loop: u64,
    pub eid011_orphan_source: u64,
    pub eid012_orphan_target: u64,
    pub eid013_line_mismatch: u64,
}

/// Synchronization status snapshot for the current workspace database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatusInfo {
    /// A stable client identifier used for sync; source of truth is the backend KV.
    #[serde(default)]
    pub client_id: String,
    /// Unix time of last successful pull (seconds since epoch).
    #[serde(default)]
    pub last_pull_unix_time: Option<i64>,
    /// Unix time of last successful push (seconds since epoch).
    #[serde(default)]
    pub last_push_unix_time: Option<i64>,
    /// Hint fields mirroring Turso engine conventions (if present in KV).
    #[serde(default)]
    pub last_pull_generation: Option<i64>,
    #[serde(default)]
    pub last_change_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonStatus {
    pub uptime_secs: u64,
    pub pools: Vec<PoolStatus>,
    pub total_requests: u64,
    pub active_connections: usize,
    #[serde(default)]
    pub lsp_inflight_current: u64,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub git_hash: String,
    #[serde(default)]
    pub build_date: String,
    /// Universal cache statistics (if enabled)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub universal_cache_stats: Option<UniversalCacheStats>,
    /// Database health information (Priority 4)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database_health: Option<String>,
}

/// Universal cache statistics for monitoring and observability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniversalCacheStats {
    /// Whether universal cache is enabled
    pub enabled: bool,
    /// Total number of cache entries across all workspaces
    pub total_entries: u64,
    /// Total cache size in bytes across all workspaces
    pub total_size_bytes: u64,
    /// Number of active workspaces with caches
    pub active_workspaces: usize,
    /// Overall hit rate (0.0 - 1.0)
    pub hit_rate: f64,
    /// Overall miss rate (0.0 - 1.0)
    pub miss_rate: f64,
    /// Total cache hits
    pub total_hits: u64,
    /// Total cache misses
    pub total_misses: u64,
    /// Cache statistics per LSP method
    pub method_stats: std::collections::HashMap<String, UniversalCacheMethodStats>,
    /// Cache performance overview
    pub cache_enabled: bool,
    /// Workspace-specific cache summaries
    pub workspace_summaries: Vec<UniversalCacheWorkspaceSummary>,
    /// Cache configuration summary
    pub config_summary: UniversalCacheConfigSummary,
}

/// Statistics for a specific LSP method in universal cache
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniversalCacheMethodStats {
    /// LSP method name (e.g., "textDocument/definition")
    pub method: String,
    /// Whether caching is enabled for this method
    pub enabled: bool,
    /// Number of entries for this method
    pub entries: u64,
    /// Size in bytes for this method
    pub size_bytes: u64,
    /// Hit count for this method
    pub hits: u64,
    /// Miss count for this method
    pub misses: u64,
    /// Hit rate for this method (0.0 - 1.0)
    pub hit_rate: f64,
    /// Average response time from cache (microseconds)
    pub avg_cache_response_time_us: u64,
    /// Average response time from LSP server (microseconds)
    pub avg_lsp_response_time_us: u64,
}

/// Workspace-specific cache summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniversalCacheWorkspaceSummary {
    /// Workspace identifier
    pub workspace_id: String,
    /// Workspace root path
    pub workspace_root: std::path::PathBuf,
    /// Number of cache entries for this workspace
    pub entries: u64,
    /// Cache size for this workspace in bytes
    pub size_bytes: u64,
    /// Hit count for this workspace
    pub hits: u64,
    /// Miss count for this workspace
    pub misses: u64,
    /// Hit rate for this workspace (0.0 - 1.0)
    pub hit_rate: f64,
    /// Last accessed timestamp
    pub last_accessed: String,
    /// Languages with cached data in this workspace
    pub languages: Vec<String>,
}

/// Configuration summary for universal cache
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniversalCacheConfigSummary {
    /// Whether caching is enabled
    pub enabled: bool,
    /// Maximum cache size in MB (if configured)
    pub max_size_mb: Option<usize>,
    /// Number of methods with custom configuration
    pub custom_method_configs: usize,
    /// Whether compression is enabled
    pub compression_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServerHealthInfo {
    pub language: Language,
    pub is_healthy: bool,
    pub consecutive_failures: u32,
    pub circuit_breaker_open: bool,
    pub last_check_ms: u64,
    pub response_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolStatus {
    pub language: Language,
    pub ready_servers: usize,
    pub busy_servers: usize,
    pub total_servers: usize,
    #[serde(default)]
    pub workspaces: Vec<String>,
    #[serde(default)]
    pub uptime_secs: u64,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub health_status: String,
    #[serde(default)]
    pub consecutive_failures: u32,
    #[serde(default)]
    pub circuit_breaker_open: bool,
    /// Readiness information for the language server
    #[serde(default)]
    pub readiness_info: Option<ServerReadinessInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageInfo {
    pub language: Language,
    pub lsp_server: String,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    pub root: PathBuf,
    pub language: Language,
    pub server_status: ServerStatus,
    pub file_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitializedWorkspace {
    pub workspace_root: PathBuf,
    pub language: Language,
    pub lsp_server: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerStatus {
    Initializing,
    Ready,
    Busy,
    Error(String),
}

/// Information about a server's readiness status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerReadinessInfo {
    pub workspace_root: PathBuf,
    pub language: Language,
    pub server_type: String,
    pub is_initialized: bool,
    pub is_ready: bool,
    pub elapsed_secs: f64,
    pub active_progress_count: usize,
    pub recent_messages: Vec<String>,
    pub queued_requests: usize,
    pub expected_timeout_secs: f64,
    pub status_description: String,
    pub is_stalled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    #[serde(default)] // For backward compatibility
    pub sequence: u64,
    pub timestamp: String,
    pub level: LogLevel,
    pub target: String,
    pub message: String,
    pub file: Option<String>,
    pub line: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "TRACE"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
        }
    }
}

// Workspace cache management types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceCacheEntry {
    pub workspace_id: String,
    pub workspace_root: PathBuf,
    pub cache_path: PathBuf,
    pub size_bytes: u64,
    pub file_count: usize,
    pub last_accessed: String, // ISO 8601 timestamp
    pub created_at: String,    // ISO 8601 timestamp
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceCacheInfo {
    pub workspace_id: String,
    pub workspace_root: PathBuf,
    pub cache_path: PathBuf,
    pub size_bytes: u64,
    pub file_count: usize,
    pub last_accessed: String,
    pub created_at: String,
    // Additional fields for compatibility with management.rs
    pub disk_size_bytes: u64,
    pub files_indexed: u64,
    pub languages: Vec<String>,
    // Router statistics
    pub router_stats: Option<WorkspaceCacheRouterStats>,
    // Cache statistics from the persistent cache
    pub cache_stats: Option<CacheStatistics>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceCacheRouterStats {
    pub max_open_caches: usize,
    pub current_open_caches: usize,
    pub total_workspaces_seen: usize,
    pub access_count: u64,
    pub hit_rate: f64,
    pub miss_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceClearResult {
    pub cleared_workspaces: Vec<WorkspaceClearEntry>,
    pub total_size_freed_bytes: u64,
    pub total_files_removed: usize,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceClearEntry {
    pub workspace_id: String,
    pub workspace_root: PathBuf,
    pub success: bool,
    pub size_freed_bytes: u64,
    pub files_removed: usize,
    pub error: Option<String>,
}

// Cache statistics for workspace caches
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStatistics {
    pub total_size_bytes: u64,
    pub disk_size_bytes: u64,
    pub total_entries: u64,
    pub entries_per_file: std::collections::HashMap<PathBuf, u64>,
    pub entries_per_language: std::collections::HashMap<String, u64>,
    pub hit_rate: f64,
    pub miss_rate: f64,
    pub age_distribution: AgeDistribution,
    pub most_accessed: Vec<HotSpot>,
    pub memory_usage: MemoryUsage,
    // New hierarchical statistics
    pub per_workspace_stats: Option<Vec<WorkspaceCacheStats>>,
    pub per_operation_totals: Option<Vec<OperationCacheStats>>, // Global operation totals
}

/// Cache statistics for a specific workspace with per-operation breakdown
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceCacheStats {
    pub workspace_id: String,
    pub workspace_path: PathBuf,
    pub entries: u64,
    pub size_bytes: u64,
    pub hit_rate: f64,
    pub miss_rate: f64,
    // Per-operation breakdown within this workspace
    pub per_operation_stats: Vec<OperationCacheStats>,
}

/// Cache statistics for a specific operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationCacheStats {
    pub operation: String, // "hover", "definition", "references", "call_hierarchy", etc.
    pub entries: u64,
    pub size_bytes: u64,
    pub hit_rate: f64,
    pub miss_rate: f64,
    pub avg_response_time_ms: Option<f64>,
}

// Symbol cache clear result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolCacheClearResult {
    pub symbol_name: String,
    pub file_path: PathBuf,
    pub entries_cleared: usize,
    pub positions_cleared: Vec<(u32, u32)>, // (line, column) pairs
    pub methods_cleared: Vec<String>,
    pub cache_size_freed_bytes: u64,
    pub duration_ms: u64,
}

// Generic cache operation results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearResult {
    pub entries_removed: u64,
    pub files_affected: u64,
    pub branches_affected: u64,
    pub commits_affected: u64,
    pub bytes_reclaimed: u64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub entries_imported: u64,
    pub entries_merged: u64,
    pub entries_replaced: u64,
    pub validation_errors: Vec<String>,
    pub bytes_imported: u64,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactResult {
    pub size_based_entries_removed: u64,
    pub size_before_bytes: u64,
    pub size_after_bytes: u64,
    pub bytes_reclaimed: u64,
    pub fragmentation_reduced: f64,
    pub duration_ms: u64,
}

// Cache key information for listing operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheKeyInfo {
    /// The cache key identifier
    pub key: String,
    /// Workspace relative file path
    pub file_path: String,
    /// LSP operation type
    pub operation: String,
    /// Position in file (line:column)
    pub position: String,
    /// Symbol name if available
    pub symbol_name: Option<String>,
    /// Size of cached data in bytes
    pub size_bytes: usize,
    /// Number of times this key has been accessed
    pub access_count: u64,
    /// Last accessed time (ISO 8601 timestamp)
    pub last_accessed: String,
    /// Creation time (ISO 8601 timestamp)
    pub created_at: String,
    /// Content hash for cache invalidation
    pub content_hash: String,
    /// Workspace identifier
    pub workspace_id: String,
    /// Whether the entry has expired
    pub is_expired: bool,
}

pub struct MessageCodec;

impl MessageCodec {
    pub fn encode(msg: &DaemonRequest) -> Result<Vec<u8>> {
        let json = serde_json::to_string(msg)?;
        let bytes = json.as_bytes();

        // Validate message size before encoding
        if bytes.len() > MAX_MESSAGE_SIZE {
            return Err(anyhow::anyhow!(
                "Message size {} exceeds maximum allowed size of {} bytes",
                bytes.len(),
                MAX_MESSAGE_SIZE
            ));
        }

        // Simple length-prefixed encoding
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        encoded.extend_from_slice(bytes);

        Ok(encoded)
    }

    pub fn encode_response(msg: &DaemonResponse) -> Result<Vec<u8>> {
        let json = serde_json::to_string(msg)?;
        let bytes = json.as_bytes();

        // Validate message size before encoding
        if bytes.len() > MAX_MESSAGE_SIZE {
            return Err(anyhow::anyhow!(
                "Message size {} exceeds maximum allowed size of {} bytes",
                bytes.len(),
                MAX_MESSAGE_SIZE
            ));
        }

        let mut encoded = Vec::new();
        encoded.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        encoded.extend_from_slice(bytes);

        Ok(encoded)
    }

    pub fn decode_request(bytes: &[u8]) -> Result<DaemonRequest> {
        // Maximum message size is shared with the daemon (see MAX_MESSAGE_SIZE).

        if bytes.len() < 4 {
            return Err(anyhow::anyhow!("Message too short"));
        }

        let len = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;

        // Validate message size to prevent excessive memory allocation
        if len > MAX_MESSAGE_SIZE {
            return Err(anyhow::anyhow!(
                "Message size {} exceeds maximum allowed size of {} bytes",
                len,
                MAX_MESSAGE_SIZE
            ));
        }

        if bytes.len() < 4 + len {
            return Err(anyhow::anyhow!("Incomplete message"));
        }

        let json_bytes = &bytes[4..4 + len];
        let request = serde_json::from_slice(json_bytes)?;

        Ok(request)
    }

    pub fn decode_response(bytes: &[u8]) -> Result<DaemonResponse> {
        // Maximum message size is shared with the daemon (see MAX_MESSAGE_SIZE).

        if bytes.len() < 4 {
            return Err(anyhow::anyhow!("Message too short"));
        }

        let len = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;

        // Validate message size to prevent excessive memory allocation
        if len > MAX_MESSAGE_SIZE {
            return Err(anyhow::anyhow!(
                "Message size {} exceeds maximum allowed size of {} bytes",
                len,
                MAX_MESSAGE_SIZE
            ));
        }

        if bytes.len() < 4 + len {
            return Err(anyhow::anyhow!("Incomplete message"));
        }

        let json_bytes = &bytes[4..4 + len];
        let response = serde_json::from_slice(json_bytes)?;

        Ok(response)
    }

    /// Decode a framed message with size validation
    pub fn decode_framed(bytes: &[u8]) -> Result<(usize, Vec<u8>)> {
        if bytes.len() < 4 {
            return Err(anyhow::anyhow!("Message too short for framing"));
        }

        let len = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;

        // Validate message size to prevent excessive memory allocation
        if len > MAX_MESSAGE_SIZE {
            return Err(anyhow::anyhow!(
                "Message size {} exceeds maximum allowed size of {} bytes",
                len,
                MAX_MESSAGE_SIZE
            ));
        }

        if bytes.len() < 4 + len {
            return Err(anyhow::anyhow!("Incomplete message"));
        }

        Ok((4 + len, bytes[4..4 + len].to_vec()))
    }

    /// Async method to read a framed message with timeout
    pub async fn read_framed<R>(reader: &mut R, read_timeout: Duration) -> Result<Vec<u8>>
    where
        R: AsyncReadExt + Unpin,
    {
        // Read length prefix with timeout
        let mut length_buf = [0u8; 4];
        timeout(read_timeout, reader.read_exact(&mut length_buf))
            .await
            .map_err(|_| anyhow::anyhow!("Timeout reading message length"))?
            .map_err(|e| anyhow::anyhow!("Failed to read message length: {}", e))?;

        let message_len = u32::from_be_bytes(length_buf) as usize;

        // Validate message size
        if message_len > MAX_MESSAGE_SIZE {
            return Err(anyhow::anyhow!(
                "Message size {} exceeds maximum allowed size of {} bytes",
                message_len,
                MAX_MESSAGE_SIZE
            ));
        }

        // Read message body with timeout
        let mut message_buf = vec![0u8; message_len];
        timeout(read_timeout, reader.read_exact(&mut message_buf))
            .await
            .map_err(|_| anyhow::anyhow!("Timeout reading message body"))?
            .map_err(|e| anyhow::anyhow!("Failed to read message body: {}", e))?;

        Ok(message_buf)
    }

    /// Async method to write a framed message with timeout
    pub async fn write_framed<W>(writer: &mut W, data: &[u8], write_timeout: Duration) -> Result<()>
    where
        W: AsyncWriteExt + Unpin,
    {
        // Validate message size
        if data.len() > MAX_MESSAGE_SIZE {
            return Err(anyhow::anyhow!(
                "Message size {} exceeds maximum allowed size of {} bytes",
                data.len(),
                MAX_MESSAGE_SIZE
            ));
        }

        // Write length prefix and data with timeout
        let length_bytes = (data.len() as u32).to_be_bytes();
        let mut frame = Vec::with_capacity(4 + data.len());
        frame.extend_from_slice(&length_bytes);
        frame.extend_from_slice(data);

        timeout(write_timeout, writer.write_all(&frame))
            .await
            .map_err(|_| anyhow::anyhow!("Timeout writing message"))?
            .map_err(|e| anyhow::anyhow!("Failed to write message: {}", e))?;

        timeout(write_timeout, writer.flush())
            .await
            .map_err(|_| anyhow::anyhow!("Timeout flushing message"))?
            .map_err(|e| anyhow::anyhow!("Failed to flush message: {}", e))?;

        Ok(())
    }
}

// Small helper to build a default/empty CallHierarchyItem
fn default_call_hierarchy_item() -> CallHierarchyItem {
    CallHierarchyItem {
        name: "unknown".to_string(),
        kind: "unknown".to_string(),
        uri: "".to_string(),
        range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 0,
            },
        },
        selection_range: Range {
            start: Position {
                line: 0,
                character: 0,
            },
            end: Position {
                line: 0,
                character: 0,
            },
        },
    }
}

// Helper function to convert from serde_json::Value to our types
pub fn parse_call_hierarchy_from_lsp(value: &Value) -> Result<CallHierarchyResult> {
    // Accept alternative shapes: when LSP returns an array (prepare call result),
    // take the first element as the root item and leave incoming/outgoing empty.
    if let Some(arr) = value.as_array() {
        if let Some(first) = arr.first() {
            return Ok(CallHierarchyResult {
                item: parse_call_hierarchy_item(first)?,
                incoming: vec![],
                outgoing: vec![],
            });
        } else {
            return Ok(CallHierarchyResult {
                item: default_call_hierarchy_item(),
                incoming: vec![],
                outgoing: vec![],
            });
        }
    }
    // Handle case where rust-analyzer returns empty call hierarchy (no item)
    let item = match value.get("item") {
        Some(item) => item,
        None => {
            // Return empty call hierarchy result
            return Ok(CallHierarchyResult {
                item: default_call_hierarchy_item(),
                incoming: vec![],
                outgoing: vec![],
            });
        }
    };

    let incoming = value
        .get("incoming")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| parse_call_hierarchy_call(v).ok())
                .collect()
        })
        .unwrap_or_default();

    let outgoing = value
        .get("outgoing")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| parse_call_hierarchy_call(v).ok())
                .collect()
        })
        .unwrap_or_default();

    Ok(CallHierarchyResult {
        item: parse_call_hierarchy_item(item)?,
        incoming,
        outgoing,
    })
}

fn parse_call_hierarchy_item(value: &Value) -> Result<CallHierarchyItem> {
    Ok(CallHierarchyItem {
        name: value
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string(),
        // Accept numeric or string kinds
        kind: match value.get("kind") {
            Some(kv) => {
                if let Some(num) = kv.as_u64() {
                    num.to_string()
                } else {
                    kv.as_str().unwrap_or("unknown").to_string()
                }
            }
            None => "unknown".to_string(),
        },
        // Accept targetUri as a fallback
        uri: value
            .get("uri")
            .or_else(|| value.get("targetUri"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        range: parse_range(value.get("range").unwrap_or(&json!({})))?,
        selection_range: parse_range(
            value
                .get("selectionRange")
                .or_else(|| value.get("range"))
                .unwrap_or(&json!({})),
        )?,
    })
}

fn parse_call_hierarchy_call(value: &Value) -> Result<CallHierarchyCall> {
    // For incoming calls, use "from" field
    // For outgoing calls, use "to" field (rename it to "from" for consistency)
    let from = value
        .get("from")
        .or_else(|| value.get("to"))
        .ok_or_else(|| anyhow::anyhow!("Missing 'from' or 'to' in call"))?;

    let from_ranges = value
        .get("fromRanges")
        .or_else(|| value.get("toRanges"))
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|r| parse_range(r).ok()).collect())
        .unwrap_or_default();

    Ok(CallHierarchyCall {
        from: parse_call_hierarchy_item(from)?,
        from_ranges,
    })
}

fn parse_range(value: &Value) -> Result<Range> {
    let default_pos = json!({});
    let start = value.get("start").unwrap_or(&default_pos);
    let end = value.get("end").unwrap_or(&default_pos);

    Ok(Range {
        start: Position {
            line: start.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            character: start.get("character").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        },
        end: Position {
            line: end.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            character: end.get("character").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
        },
    })
}

/// Git-aware cache history entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheHistoryEntry {
    pub commit_hash: String,
    pub branch: String,
    pub timestamp: u64, // Unix timestamp
    pub cache_entry: CachedCallHierarchy,
}

/// Cached call hierarchy information with git metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedCallHierarchy {
    pub file_path: PathBuf,
    pub symbol: String,
    pub line: u32,
    pub column: u32,
    pub result: CallHierarchyResult,
    pub cached_at: u64, // Unix timestamp
}

/// Complete cache snapshot at a specific commit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheSnapshot {
    pub commit_hash: String,
    pub timestamp: u64,
    pub entries: Vec<CachedCallHierarchy>,
    pub total_entries: usize,
}

/// Difference between cache states at two commits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheDiff {
    pub from_commit: String,
    pub to_commit: String,
    pub added_entries: Vec<CachedCallHierarchy>,
    pub removed_entries: Vec<CachedCallHierarchy>,
    pub modified_entries: Vec<CacheModification>,
    pub unchanged_entries: usize,
}

/// Represents a modification to a cache entry between commits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheModification {
    pub file_path: PathBuf,
    pub symbol: String,
    pub old_entry: CachedCallHierarchy,
    pub new_entry: CachedCallHierarchy,
    pub change_type: CacheChangeType,
}

/// Type of change detected in cache entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheChangeType {
    /// Call hierarchy structure changed
    StructureChanged,
    /// File content changed (different MD5)
    ContentChanged,
    /// Symbol position moved
    PositionChanged,
    /// Context updated but structure preserved
    ContextUpdated,
}

/// Git-aware cache statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitCacheStats {
    /// Statistics per branch
    pub branch_stats: std::collections::HashMap<String, BranchCacheStats>,
    /// Statistics per commit (recent commits only)
    pub commit_stats: std::collections::HashMap<String, CommitCacheStats>,
    /// Hot spots across commits (most frequently accessed symbols)
    pub hot_spots: Vec<HotSpot>,
}

/// Cache statistics for a specific branch
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchCacheStats {
    pub branch_name: String,
    pub total_entries: usize,
    pub hit_rate: f64,
    pub last_active: u64, // Unix timestamp
    pub commits_tracked: usize,
}

/// Cache statistics for a specific commit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitCacheStats {
    pub commit_hash: String,
    pub branch: String,
    pub cache_size: usize,
    pub hit_rate: f64,
    pub created_at: u64,    // Unix timestamp
    pub last_accessed: u64, // Unix timestamp
}

/// Hot spot analysis across git history
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotSpot {
    pub file_path: PathBuf,
    pub symbol: String,
    pub access_count: usize,
    pub hit_rate: f64,
    pub branches_seen: Vec<String>,
    pub commits_seen: usize,
    pub first_seen: u64,    // Unix timestamp
    pub last_accessed: u64, // Unix timestamp
}

use serde_json::json;

// Additional cache management types needed by cache_management.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearFilter {
    pub older_than_days: Option<u64>,
    pub file_path: Option<PathBuf>,
    pub commit_hash: Option<String>,
    pub all: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportOptions {
    pub current_branch_only: bool,
    pub compress: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompactOptions {
    pub target_size_mb: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgeDistribution {
    pub entries_last_hour: u64,
    pub entries_last_day: u64,
    pub entries_last_week: u64,
    pub entries_last_month: u64,
    pub entries_older: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryUsage {
    pub in_memory_cache_bytes: u64,
    pub persistent_cache_bytes: u64,
    pub metadata_bytes: u64,
    pub index_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitContext {
    pub commit_hash: String,
    pub branch: String,
    pub is_dirty: bool,
    pub remote_url: Option<String>,
    pub repo_root: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_message_codec_large_response() {
        // Create a large response with many log entries
        let mut large_log_entries = Vec::new();
        for i in 0..100 {
            large_log_entries.push(LogEntry {
                sequence: i as u64,
                timestamp: format!("2024-01-01 12:00:{:02}.000 UTC", i % 60),
                level: LogLevel::Info,
                target: "test".to_string(),
                message: format!("Large message {i} with lots of content that makes the overall response quite big"),
                file: Some("test.rs".to_string()),
                line: Some(i),
            });
        }

        let response = DaemonResponse::Logs {
            request_id: Uuid::new_v4(),
            entries: large_log_entries,
        };

        // Encode the response
        let encoded =
            MessageCodec::encode_response(&response).expect("Failed to encode large response");

        // Ensure it's properly encoded with length prefix
        assert!(encoded.len() >= 4);
        let expected_len = encoded.len() - 4;
        let actual_len =
            u32::from_be_bytes([encoded[0], encoded[1], encoded[2], encoded[3]]) as usize;
        assert_eq!(actual_len, expected_len);

        // Decode it back
        let decoded =
            MessageCodec::decode_response(&encoded).expect("Failed to decode large response");

        match decoded {
            DaemonResponse::Logs { entries, .. } => {
                assert_eq!(entries.len(), 100);
                assert_eq!(entries[0].message, "Large message 0 with lots of content that makes the overall response quite big");
            }
            _ => panic!("Expected Logs response"),
        }
    }

    #[test]
    fn test_incomplete_message_detection() {
        // Create a normal response
        let response = DaemonResponse::Pong {
            request_id: Uuid::new_v4(),
        };

        let encoded = MessageCodec::encode_response(&response).expect("Failed to encode");

        // Test with truncated message (missing some bytes)
        let truncated = &encoded[..encoded.len() - 5];
        let result = MessageCodec::decode_response(truncated);

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Incomplete message"));
    }

    #[test]
    fn test_message_too_short() {
        // Test with message shorter than 4 bytes
        let short_message = vec![1, 2];
        let result = MessageCodec::decode_response(&short_message);

        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Message too short"));
    }

    #[test]
    fn test_message_codec_large_request() {
        // Create a large request (GetLogs), encode and decode it
        let request = DaemonRequest::GetLogs {
            request_id: Uuid::new_v4(),
            lines: 1000,
            since_sequence: None,
            min_level: None,
        };
        let encoded = MessageCodec::encode(&request).expect("encode");
        let decoded = MessageCodec::decode_request(&encoded).expect("decode");
        match decoded {
            DaemonRequest::GetLogs {
                lines,
                since_sequence,
                ..
            } => {
                assert_eq!(lines, 1000);
                assert_eq!(since_sequence, None);
            }
            _ => panic!("expected GetLogs"),
        }
    }

    #[test]
    fn test_get_logs_request_with_sequence() {
        // Test GetLogs request with sequence parameter
        let request = DaemonRequest::GetLogs {
            request_id: Uuid::new_v4(),
            lines: 50,
            since_sequence: Some(123),
            min_level: None,
        };
        let encoded = MessageCodec::encode(&request).expect("encode");
        let decoded = MessageCodec::decode_request(&encoded).expect("decode");
        match decoded {
            DaemonRequest::GetLogs {
                lines,
                since_sequence,
                ..
            } => {
                assert_eq!(lines, 50);
                assert_eq!(since_sequence, Some(123));
            }
            _ => panic!("expected GetLogs"),
        }
    }

    #[test]
    fn test_log_entry_sequence_serialization() {
        // Test LogEntry with sequence number serializes correctly
        let entry = LogEntry {
            sequence: 42,
            timestamp: "2024-01-01 12:00:00.000 UTC".to_string(),
            level: LogLevel::Info,
            target: "test".to_string(),
            message: "Test message".to_string(),
            file: Some("test.rs".to_string()),
            line: Some(10),
        };

        let serialized = serde_json::to_string(&entry).expect("serialize");
        let deserialized: LogEntry = serde_json::from_str(&serialized).expect("deserialize");

        assert_eq!(deserialized.sequence, 42);
        assert_eq!(deserialized.timestamp, entry.timestamp);
        assert_eq!(deserialized.message, entry.message);
    }

    #[test]
    fn test_log_entry_backward_compatibility() {
        // Test that LogEntry without sequence field can be deserialized (backward compatibility)
        let json_without_sequence = r#"{
            "timestamp": "2024-01-01 12:00:00.000 UTC",
            "level": "Info",
            "target": "test",
            "message": "Test message",
            "file": "test.rs",
            "line": 10
        }"#;

        let deserialized: LogEntry =
            serde_json::from_str(json_without_sequence).expect("deserialize");

        assert_eq!(deserialized.sequence, 0); // Default value
        assert_eq!(deserialized.timestamp, "2024-01-01 12:00:00.000 UTC");
        assert_eq!(deserialized.message, "Test message");
    }

    #[test]
    fn test_parse_call_hierarchy_accepts_string_kind_and_to_ranges() {
        let v = serde_json::json!({
            "item": {
                "name": "root",
                "kind": "Function",
                "uri": "file:///root.rs",
                "range": { "start": {"line":1, "character":2}, "end": {"line":1, "character":10} },
                "selectionRange": { "start": {"line":1, "character":2}, "end": {"line":1, "character":10} }
            },
            "incoming": [{
                "from": {
                    "name": "caller",
                    "kind": "Method",
                    "uri": "file:///caller.rs",
                    "range": { "start": {"line":0, "character":0}, "end": {"line":0, "character":1} },
                    "selectionRange": { "start": {"line":0, "character":0}, "end": {"line":0, "character":1} }
                },
                "fromRanges": [ { "start": {"line":0, "character":0}, "end": {"line":0, "character":1} } ]
            }],
            "outgoing": [{
                "to": {
                    "name": "callee",
                    "kind": 12,
                    "targetUri": "file:///callee.rs",
                    "range": { "start": {"line":2, "character":0}, "end": {"line":2, "character":1} },
                    "selectionRange": { "start": {"line":2, "character":0}, "end": {"line":2, "character":1} }
                },
                "toRanges": [ { "start": {"line":2, "character":0}, "end": {"line":2, "character":1} } ]
            }]
        });
        let result = parse_call_hierarchy_from_lsp(&v).expect("parse ok");
        assert_eq!(result.item.kind, "Function");
        assert_eq!(result.incoming.len(), 1);
        assert_eq!(result.outgoing.len(), 1);
        assert_eq!(result.outgoing[0].from.kind, "12");
        assert_eq!(result.outgoing[0].from.uri, "file:///callee.rs");
        assert_eq!(result.outgoing[0].from_ranges.len(), 1);
    }

    #[test]
    fn test_parse_call_hierarchy_array_item_defaults() {
        let v = serde_json::json!([{
            "name": "root",
            "kind": 3,
            "uri": "file:///root.rs",
            "range": { "start": {"line":3, "character":0}, "end": {"line":3, "character":5} }
        }]);
        let result = parse_call_hierarchy_from_lsp(&v).expect("parse");
        assert_eq!(result.item.name, "root");
        assert!(result.incoming.is_empty());
        assert!(result.outgoing.is_empty());
    }
}
