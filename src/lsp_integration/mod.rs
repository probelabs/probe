pub mod call_graph_cache;
pub mod client;
pub mod management;
pub mod position_analyzer;
pub mod readiness;
pub mod stdlib_filter;
pub mod symbol_resolver;
pub mod types;

pub use client::LspClient;
pub use management::LspManager;
pub use position_analyzer::{LspOperation, PositionAnalyzer, PositionOffset, PositionPattern};
pub use readiness::{
    check_lsp_readiness_for_file, wait_for_lsp_readiness, ReadinessCheckResult, ReadinessConfig,
};
pub use stdlib_filter::{is_stdlib_path, is_stdlib_path_cached};
pub use symbol_resolver::{resolve_location, ResolvedLocation};
pub use types::*;

use clap::{Subcommand, ValueEnum};

#[derive(ValueEnum, Debug, Clone)]
pub enum OutputFormat {
    Terminal,
    Json,
}

#[derive(Subcommand, Debug, Clone)]
pub enum LspSubcommands {
    /// Show LSP daemon status, uptime, and server pool information
    Status {
        /// Use daemon mode (auto-start if not running)
        #[clap(long = "daemon", default_value = "true")]
        daemon: bool,

        /// Workspace hint for LSP server initialization
        #[clap(long = "workspace-hint")]
        workspace_hint: Option<String>,

        /// Output format (terminal, json)
        #[clap(long, value_enum, default_value = "terminal")]
        format: OutputFormat,
    },

    /// List available LSP servers and their installation status
    Languages,

    /// Health check and connectivity test for LSP daemon
    Ping {
        /// Use daemon mode (auto-start if not running)
        #[clap(long = "daemon", default_value = "true")]
        daemon: bool,

        /// Workspace hint for LSP server initialization
        #[clap(long = "workspace-hint")]
        workspace_hint: Option<String>,
    },

    /// Gracefully shutdown the LSP daemon
    Shutdown,

    /// Restart the LSP daemon (shutdown + auto-start)
    Restart {
        /// Workspace hint for LSP server initialization
        #[clap(long = "workspace-hint")]
        workspace_hint: Option<String>,
    },

    /// Show version information with git hash and build date
    Version,

    /// View LSP daemon logs
    Logs {
        /// Follow the log output (like tail -f)
        #[clap(short = 'f', long = "follow")]
        follow: bool,

        /// Number of lines to show from the end of the log
        #[clap(short = 'n', long = "lines", default_value = "50")]
        lines: usize,

        /// Clear the log file
        #[clap(long = "clear")]
        clear: bool,
    },

    /// View LSP daemon crash logs with stack traces
    CrashLogs {
        /// Number of lines to show from the end of the crash log
        #[clap(short = 'n', long = "lines", default_value = "100")]
        lines: usize,

        /// Clear the crash log file
        #[clap(long = "clear")]
        clear: bool,

        /// Show the crash log file path
        #[clap(long = "path")]
        show_path: bool,
    },

    /// Start the LSP daemon (embedded mode)
    Start {
        /// Path to the IPC endpoint (Unix socket or Windows named pipe)
        #[clap(short, long)]
        socket: Option<String>,

        /// Log level (trace, debug, info, warn, error)
        #[clap(short, long, default_value = "info")]
        log_level: String,

        /// Run in foreground (don't daemonize)
        #[clap(short, long)]
        foreground: bool,
    },

    /// Initialize language servers for workspaces
    Init {
        /// Workspace path to initialize (defaults to current directory)
        #[clap(short = 'w', long = "workspace")]
        workspace: Option<String>,

        /// Specific languages to initialize (comma-separated, e.g., "rust,typescript")
        #[clap(short = 'l', long = "languages")]
        languages: Option<String>,

        /// Recursively search for and initialize nested workspaces
        #[clap(short = 'r', long = "recursive")]
        recursive: bool,

        /// Use daemon mode (auto-start if not running)
        #[clap(long = "daemon", default_value = "true")]
        daemon: bool,

        /// Enable watchdog monitoring for daemon health and resource usage
        #[clap(long = "watchdog")]
        watchdog: bool,
    },

    /// Call LSP methods directly
    Call {
        #[clap(subcommand)]
        command: LspCallCommands,
    },

    /// Cache management subcommands
    Cache {
        #[clap(subcommand)]
        cache_command: CacheSubcommands,
    },

    /// Start indexing a workspace
    Index {
        /// Workspace path to index (defaults to current directory)
        #[clap(short = 'w', long = "workspace")]
        workspace: Option<String>,

        /// Specific file(s) to index (can be specified multiple times)
        #[clap(short = 'f', long = "file")]
        files: Vec<String>,

        /// Specific languages to index (comma-separated, e.g., "rust,typescript")
        #[clap(short = 'l', long = "languages")]
        languages: Option<String>,

        /// Recursively index nested workspaces
        #[clap(short = 'r', long = "recursive")]
        recursive: bool,

        /// Maximum number of worker threads (default: CPU count)
        #[clap(long = "max-workers")]
        max_workers: Option<usize>,

        /// Memory budget in MB (default: 512MB)
        #[clap(long = "memory-budget")]
        memory_budget: Option<u64>,

        /// Output format (terminal, json)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json"])]
        format: String,

        /// Show progress bar (disable for scripting)
        #[clap(long = "progress", default_value = "true")]
        progress: bool,

        /// Wait for indexing to complete before returning
        #[clap(long = "wait")]
        wait: bool,
    },

    /// Show detailed indexing status
    IndexStatus {
        /// Output format (terminal, json)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json"])]
        format: String,

        /// Show per-file progress details
        #[clap(long = "detailed")]
        detailed: bool,

        /// Follow indexing progress (like tail -f)
        #[clap(short = 'f', long = "follow")]
        follow: bool,

        /// Update interval for follow mode (seconds)
        #[clap(long = "interval", default_value = "1")]
        interval: u64,
    },

    /// Stop ongoing indexing
    IndexStop {
        /// Force stop even if indexing is in progress
        #[clap(short = 'f', long = "force")]
        force: bool,

        /// Output format (terminal, json)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json"])]
        format: String,
    },

    /// Configure indexing settings
    IndexConfig {
        #[clap(subcommand)]
        config_command: IndexConfigSubcommands,
    },

    /// Export the index database to a file
    IndexExport {
        /// Workspace path to export from (defaults to current directory)
        #[clap(short = 'w', long = "workspace")]
        workspace: Option<std::path::PathBuf>,

        /// Output file path (required - where to save the database export)
        #[clap(short = 'o', long = "output", required = true)]
        output: std::path::PathBuf,

        /// Force WAL checkpoint before export
        #[clap(long = "checkpoint", default_value = "true")]
        checkpoint: bool,

        /// Use daemon mode (auto-start if not running)
        #[clap(long = "daemon", default_value = "true")]
        daemon: bool,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum IndexConfigSubcommands {
    /// Show current indexing configuration
    Show {
        /// Output format (terminal, json)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json"])]
        format: String,
    },

    /// Set indexing configuration
    Set {
        /// Maximum number of worker threads
        #[clap(long = "max-workers")]
        max_workers: Option<usize>,

        /// Memory budget in MB
        #[clap(long = "memory-budget")]
        memory_budget: Option<u64>,

        /// File patterns to exclude (comma-separated)
        #[clap(long = "exclude")]
        exclude_patterns: Option<String>,

        /// File patterns to include (comma-separated, empty=all)
        #[clap(long = "include")]
        include_patterns: Option<String>,

        /// Maximum file size to index (MB)
        #[clap(long = "max-file-size")]
        max_file_size: Option<u64>,

        /// Enable incremental indexing mode
        #[clap(long = "incremental")]
        incremental: Option<bool>,

        // LSP Caching Configuration Options
        /// Enable caching of call hierarchy operations during indexing
        #[clap(long = "cache-call-hierarchy")]
        cache_call_hierarchy: Option<bool>,

        /// Enable caching of definition lookups during indexing
        #[clap(long = "cache-definitions")]
        cache_definitions: Option<bool>,

        /// Enable caching of reference lookups during indexing
        #[clap(long = "cache-references")]
        cache_references: Option<bool>,

        /// Enable caching of hover information during indexing
        #[clap(long = "cache-hover")]
        cache_hover: Option<bool>,

        /// Enable caching of document symbols during indexing
        #[clap(long = "cache-document-symbols")]
        cache_document_symbols: Option<bool>,

        /// Perform LSP operations during indexing (vs only on-demand)
        #[clap(long = "cache-during-indexing")]
        cache_during_indexing: Option<bool>,

        /// Preload cache with common operations after indexing
        #[clap(long = "preload-common-symbols")]
        preload_common_symbols: Option<bool>,

        /// Maximum LSP operations to cache per operation type during indexing
        #[clap(long = "max-cache-entries-per-operation")]
        max_cache_entries_per_operation: Option<usize>,

        /// Timeout for LSP operations during indexing (milliseconds)
        #[clap(long = "lsp-operation-timeout-ms")]
        lsp_operation_timeout_ms: Option<u64>,

        /// Priority operations during indexing (comma-separated: call_hierarchy,definition,references,hover,document_symbols)
        #[clap(long = "lsp-priority-operations")]
        lsp_priority_operations: Option<String>,

        /// Operations to skip during indexing (comma-separated: call_hierarchy,definition,references,hover,document_symbols)
        #[clap(long = "lsp-disabled-operations")]
        lsp_disabled_operations: Option<String>,

        /// Output format (terminal, json)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json"])]
        format: String,
    },

    /// Reset indexing configuration to defaults
    Reset {
        /// Output format (terminal, json)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json"])]
        format: String,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum CacheSubcommands {
    /// Show cache statistics and configuration
    Stats {
        /// Show detailed per-method statistics
        #[clap(long)]
        detailed: bool,

        /// Show per-workspace breakdown
        #[clap(long)]
        per_workspace: bool,

        /// Show layer-specific statistics (memory, disk, server)
        #[clap(long)]
        layers: bool,

        /// Output format (terminal, json)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json"])]
        format: String,
    },

    /// Clear cache entries
    Clear {
        /// Clear entries for specific LSP method
        #[clap(long)]
        method: Option<String>,

        /// Clear entries for specific workspace
        #[clap(long)]
        workspace: Option<std::path::PathBuf>,

        /// Clear entries for specific file
        #[clap(long)]
        file: Option<std::path::PathBuf>,

        /// Clear entries older than N seconds
        #[clap(long)]
        older_than: Option<u64>,

        /// Clear all entries (requires confirmation)
        #[clap(long)]
        all: bool,

        /// Force clear without confirmation
        #[clap(short = 'f', long = "force")]
        force: bool,

        /// Output format (terminal, json)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json"])]
        format: String,
    },

    /// Compact the cache database
    Compact {
        /// Target size in MB (removes oldest entries)
        #[clap(long)]
        target_size_mb: Option<usize>,
    },

    /// List all workspace caches
    List {
        /// Show detailed information for each workspace cache
        #[clap(long)]
        detailed: bool,

        /// Output format (terminal, json)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json"])]
        format: String,
    },

    /// Show detailed information about workspace caches
    Info {
        /// Workspace path to get info for (optional, shows all if not specified)
        workspace: Option<std::path::PathBuf>,

        /// Output format (terminal, json)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json"])]
        format: String,
    },

    /// Clear workspace caches
    ClearWorkspace {
        /// Workspace path to clear (optional, clears all if not specified)
        workspace: Option<std::path::PathBuf>,

        /// Only clear entries older than specified duration (e.g., "1h", "30m", "7d")
        #[clap(long = "older-than")]
        older_than: Option<String>,

        /// Skip confirmation prompt (force clear without confirmation)
        #[clap(short = 'f', long = "force")]
        force: bool,

        /// Skip confirmation prompt (same as --force for convenience)
        #[clap(short = 'y', long = "yes")]
        yes: bool,

        /// Output format (terminal, json)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json"])]
        format: String,
    },

    /// Clear cache for a specific symbol
    ClearSymbol {
        /// File path containing the symbol, or file#symbol format
        #[clap(required = true)]
        file: String,

        /// Symbol name to clear cache for (optional if using file#symbol format)
        #[clap(required = false)]
        symbol: Option<String>,

        /// Clear for specific LSP methods only (comma-separated: CallHierarchy,References,Hover,Definition)
        #[clap(long = "methods")]
        methods: Option<String>,

        /// Include all positions in multi-position fallback (for rust-analyzer)
        #[clap(long = "all-positions")]
        all_positions: bool,

        /// Force clear without confirmation
        #[clap(short = 'f', long = "force")]
        force: bool,

        /// Output format (terminal, json)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json"])]
        format: String,
    },

    /// Configure cache settings
    Config {
        #[clap(subcommand)]
        config_command: CacheConfigSubcommands,
    },

    /// Test cache functionality
    Test {
        /// Workspace path to test (defaults to current directory)
        #[clap(short = 'w', long = "workspace")]
        workspace: Option<std::path::PathBuf>,

        /// Specific LSP methods to test (comma-separated)
        #[clap(long)]
        methods: Option<String>,

        /// Number of test operations to perform
        #[clap(long, default_value = "10")]
        operations: usize,

        /// Test only cache performance (no LSP server interaction)
        #[clap(long)]
        cache_only: bool,

        /// Output format (terminal, json)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json"])]
        format: String,
    },

    /// Validate cache integrity
    Validate {
        /// Workspace path to validate (optional, validates all if not specified)
        #[clap(short = 'w', long = "workspace")]
        workspace: Option<std::path::PathBuf>,

        /// Fix validation errors automatically
        #[clap(long)]
        fix: bool,

        /// Show detailed validation results
        #[clap(long)]
        detailed: bool,

        /// Output format (terminal, json)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json"])]
        format: String,
    },

    /// List all cache keys with pagination and filtering support
    ListKeys {
        /// Workspace path to list keys for (optional, lists all if not specified)
        #[clap(short = 'w', long = "workspace")]
        workspace: Option<std::path::PathBuf>,

        /// Filter by LSP operation type (hover, references, definition, etc.)
        #[clap(long)]
        operation: Option<String>,

        /// Filter by file path pattern
        #[clap(long)]
        file_pattern: Option<String>,

        /// Number of results to return per page
        #[clap(long, default_value = "20")]
        limit: usize,

        /// Number of results to skip (for pagination)
        #[clap(long, default_value = "0")]
        offset: usize,

        /// Sort order (size, access-time, hit-count, created-time)
        #[clap(long, default_value = "access-time", value_parser = ["size", "access-time", "hit-count", "created-time"])]
        sort_by: String,

        /// Sort direction (asc, desc)
        #[clap(long, default_value = "desc", value_parser = ["asc", "desc"])]
        sort_order: String,

        /// Show detailed information for each key
        #[clap(long)]
        detailed: bool,

        /// Output format (terminal, json)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json"])]
        format: String,
    },
}

// UniversalCacheSubcommands removed - merged into CacheSubcommands

#[derive(Subcommand, Debug, Clone)]
pub enum CacheConfigSubcommands {
    /// Show current cache configuration
    Show {
        /// Show only configuration for specific method
        #[clap(long)]
        method: Option<String>,

        /// Show configuration for specific layer (memory, disk, server)
        #[clap(long, value_parser = ["memory", "disk", "server"])]
        layer: Option<String>,

        /// Output format (terminal, json)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json"])]
        format: String,
    },

    /// Enable cache globally
    Enable {
        /// Enable for specific methods only (comma-separated)
        #[clap(long)]
        methods: Option<String>,

        /// Enable specific layers only (comma-separated: memory,disk,server)
        #[clap(long)]
        layers: Option<String>,

        /// Output format (terminal, json)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json"])]
        format: String,
    },

    /// Disable cache globally or selectively
    Disable {
        /// Disable for specific methods only (comma-separated)
        #[clap(long)]
        methods: Option<String>,

        /// Disable specific layers only (comma-separated: memory,disk,server)
        #[clap(long)]
        layers: Option<String>,

        /// Output format (terminal, json)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json"])]
        format: String,
    },
}

// Removed UniversalCacheConfigSubcommands - merged into CacheConfigSubcommands

#[derive(Subcommand, Debug, Clone)]
pub enum LspCacheConfigSubcommands {
    /// Show current universal cache configuration
    Show {
        /// Show only configuration for specific method
        #[clap(long)]
        method: Option<String>,

        /// Show configuration for specific layer (memory, disk, server)
        #[clap(long, value_parser = ["memory", "disk", "server"])]
        layer: Option<String>,

        /// Output format (terminal, json)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json"])]
        format: String,
    },

    /// Enable universal cache globally
    Enable {
        /// Enable for specific methods only (comma-separated)
        #[clap(long)]
        methods: Option<String>,

        /// Enable specific layers only (comma-separated: memory,disk,server)
        #[clap(long)]
        layers: Option<String>,

        /// Output format (terminal, json)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json"])]
        format: String,
    },

    /// Disable universal cache globally or selectively
    Disable {
        /// Disable for specific methods only (comma-separated)
        #[clap(long)]
        methods: Option<String>,

        /// Disable specific layers only (comma-separated: memory,disk,server)
        #[clap(long)]
        layers: Option<String>,

        /// Output format (terminal, json)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json"])]
        format: String,
    },

    /// Configure cache layer settings
    SetLayer {
        /// Cache layer to configure (memory, disk, server)
        #[clap(value_parser = ["memory", "disk", "server"])]
        layer: String,

        /// Maximum size in MB for the layer
        #[clap(long)]
        max_size_mb: Option<u64>,

        /// Maximum number of entries for the layer
        #[clap(long)]
        max_entries: Option<u64>,

        /// Enable/disable the layer
        #[clap(long)]
        enabled: Option<bool>,

        /// Output format (terminal, json)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json"])]
        format: String,
    },

    /// Reset universal cache configuration to defaults
    Reset {
        /// Reset only specific method configuration
        #[clap(long)]
        method: Option<String>,

        /// Reset only specific layer configuration
        #[clap(long, value_parser = ["memory", "disk", "server"])]
        layer: Option<String>,

        /// Force reset without confirmation
        #[clap(short = 'f', long = "force")]
        force: bool,

        /// Output format (terminal, json)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json"])]
        format: String,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum LspCallCommands {
    /// Go to definition of a symbol
    Definition {
        /// Location in format 'file.rs:42:10' (line:column) or 'file.rs#symbol_name'
        location: String,

        /// Output format (terminal, json, plain)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json", "plain"])]
        format: String,
    },

    /// Find all references to a symbol
    References {
        /// Location in format 'file.rs:42:10' (line:column) or 'file.rs#symbol_name'
        location: String,

        /// Include the declaration/definition in results
        #[clap(long = "include-declaration")]
        include_declaration: bool,

        /// Output format (terminal, json, plain)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json", "plain"])]
        format: String,
    },

    /// Get hover information for a symbol
    Hover {
        /// Location in format 'file.rs:42:10' (line:column) or 'file.rs#symbol_name'
        location: String,

        /// Output format (terminal, json, plain)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json", "plain"])]
        format: String,
    },

    /// List all symbols in a document
    DocumentSymbols {
        /// File path to get symbols from
        file: std::path::PathBuf,

        /// Output format (terminal, json, plain)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json", "plain"])]
        format: String,
    },

    /// Search for symbols in the workspace
    WorkspaceSymbols {
        /// Query string to search for
        query: String,

        /// Maximum number of results to return
        #[clap(long = "max-results")]
        max_results: Option<usize>,

        /// Output format (terminal, json, plain)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json", "plain"])]
        format: String,
    },

    /// Get call hierarchy information for a symbol
    CallHierarchy {
        /// Location in format 'file.rs:42:10' (line:column) or 'file.rs#symbol_name'
        location: String,

        /// Output format (terminal, json, plain)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json", "plain"])]
        format: String,
    },

    /// Find implementations of a symbol (interfaces, traits)
    Implementations {
        /// Location in format 'file.rs:42:10' (line:column) or 'file.rs#symbol_name'
        location: String,

        /// Output format (terminal, json, plain)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json", "plain"])]
        format: String,
    },

    /// Go to type definition of a symbol
    TypeDefinition {
        /// Location in format 'file.rs:42:10' (line:column) or 'file.rs#symbol_name'
        location: String,

        /// Output format (terminal, json, plain)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json", "plain"])]
        format: String,
    },

    /// Get fully qualified name (FQN) for a symbol
    Fqn {
        /// Location in format 'file.rs:42:10' (line:column) or 'file.rs#symbol_name'
        location: String,

        /// Output format (terminal, json, plain)
        #[clap(short = 'o', long = "format", default_value = "terminal", value_parser = ["terminal", "json", "plain"])]
        format: String,
    },
}

use anyhow::Result;

/// Initialize LSP integration system
pub fn init_lsp() -> Result<()> {
    // Initialize any global LSP state if needed
    Ok(())
}
