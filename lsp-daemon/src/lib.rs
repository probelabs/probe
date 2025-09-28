// LSP Daemon Library
// Exports public interfaces for client implementations
#![allow(dead_code, clippy::all)]

// Core modules
pub mod fqn;
pub mod git_service;
#[cfg(test)]
mod git_service_test;
pub mod ipc;
pub mod language_detector;
pub mod logging;
pub mod path_resolver;
pub mod path_safety;
pub mod pid_lock;
pub mod process_group;
pub mod protocol;
pub mod socket_path;

// Cache modules
pub mod cache_types;
pub mod database;
pub mod database_cache_adapter;
// database_cache_adapter_tests removed - universal cache no longer used
pub mod hash_utils;
pub mod lsp_cache;
pub mod lsp_database_adapter;
pub mod position;
// pub mod universal_cache; // Removed - using database-first approach

// Handler modules removed

// Internal modules - exposed for direct client use
pub mod lsp_registry;
pub mod lsp_server;
pub mod readiness_tracker;

// Internal modules - exposed for embedded daemon use
pub mod daemon;
mod pool; // Keep for now but mark as deprecated
pub mod server_manager;
pub mod watchdog;
pub mod workspace_cache_router;
pub mod workspace_database_router;
pub mod workspace_resolver;
pub mod workspace_utils;

// Indexing subsystem
pub mod indexing;

// File watching subsystem
pub mod file_watcher;

// Workspace management subsystem
pub mod workspace;

// Symbol UID generation subsystem
pub mod symbol;

// Multi-language analyzer framework
pub mod analyzer;
pub mod relationship;

// Graph export functionality
pub mod graph_exporter;

// Re-export commonly used types
pub use protocol::{
    parse_call_hierarchy_from_lsp,
    AgeDistribution,
    // Cache management types
    CacheStatistics,
    CallHierarchyItem,
    CallHierarchyResult,
    ClearResult,
    CompactResult,
    DaemonRequest,
    DaemonResponse,
    DaemonStatus,
    HotSpot,
    ImportResult,
    LanguageInfo,
    LogEntry,
    LogLevel,
    LspServerHealthInfo,
    MemoryUsage,
    MessageCodec,
    PoolStatus,
    ServerStatus,
    WorkspaceInfo,
};

pub use ipc::{IpcListener, IpcStream};
pub use language_detector::{Language, LanguageDetector};
pub use logging::{LogBuffer, MemoryLogLayer};
pub use socket_path::{get_default_socket_path, normalize_executable, remove_socket_file};

// Re-export daemon for binary and embedded use
pub use daemon::{start_daemon_background, LspDaemon};
pub use lsp_registry::LspRegistry;
pub use watchdog::{ProcessHealth, ProcessMonitor, ProcessStats, Watchdog};

pub use git_service::GitService;
pub use path_resolver::PathResolver;
pub use workspace_utils::{
    find_workspace_root, find_workspace_root_with_fallback, is_workspace_root,
};

// Re-export indexing types for external use
pub use indexing::{
    CacheStrategy, EffectiveConfig, IndexingConfig, IndexingFeatures, IndexingManager,
    IndexingPipeline, IndexingProgress, IndexingQueue, LanguageIndexConfig, LanguagePipeline,
    ManagerConfig, ManagerStatus, PipelineConfig, PipelineResult, Priority, ProgressMetrics,
    ProgressSnapshot, QueueItem, QueueMetrics, QueueSnapshot, WorkerStats,
};

// Re-export file watcher types for external use
pub use file_watcher::{
    FileEvent, FileEventType, FileWatcher, FileWatcherConfig, FileWatcherStats,
};

// Re-export workspace cache router types for external use
pub use workspace_cache_router::{
    WorkspaceCacheRouter, WorkspaceCacheRouterConfig, WorkspaceCacheRouterStats, WorkspaceStats,
};

// Re-export workspace database router types for external use
pub use workspace_database_router::{WorkspaceDatabaseRouter, WorkspaceDatabaseRouterConfig};

// Universal cache types removed - using database-first approach
// pub use universal_cache::{};

// Re-export database types for external use
pub use database::{
    DatabaseBackend, DatabaseBackendExt, DatabaseConfig, DatabaseError, DatabaseStats,
    DatabaseTree, DatabaseTreeExt,
};

// Re-export pipeline-specific types
pub use indexing::pipelines::SymbolInfo as IndexingSymbolInfo;

// Re-export workspace management types for external use
pub use workspace::{
    BranchSwitchResult, ComprehensiveBranchSwitchResult, FileChange, FileChangeType,
    IndexingResult, WorkspaceConfig, WorkspaceError, WorkspaceEvent, WorkspaceIndexingResult,
    WorkspaceManagementError, WorkspaceManager, WorkspaceMetrics,
};

// Re-export symbol UID generation types for external use
pub use symbol::{
    HashAlgorithm, LanguageRules, LanguageRulesFactory, Normalizer, SignatureNormalization,
    SymbolContext, SymbolInfo as UIDSymbolInfo, SymbolKind, SymbolLocation, SymbolUIDGenerator,
    UIDError, UIDResult, Visibility,
};

// Re-export analyzer framework types for external use
pub use analyzer::{
    AnalysisContext, AnalysisError, AnalysisResult, AnalyzerCapabilities, AnalyzerConfig,
    AnalyzerManager, CodeAnalyzer, ExtractedRelationship, ExtractedSymbol, GenericAnalyzer,
    HybridAnalyzer, LanguageAnalyzerConfig, LanguageSpecificAnalyzer, LspAnalyzer, PythonAnalyzer,
    RelationType, RustAnalyzer, TreeSitterAnalyzer, TypeScriptAnalyzer,
};
