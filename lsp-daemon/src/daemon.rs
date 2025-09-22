use crate::cache_types::{CallHierarchyInfo, CallInfo, LspOperation, NodeId, NodeKey};
use crate::database_cache_adapter::BackendType;
use crate::database_cache_adapter::DatabaseCacheConfig;
use crate::hash_utils::md5_hex_file;
use crate::indexing::{IndexingConfig, IndexingManager};
use crate::ipc::{IpcListener, IpcStream};
use crate::language_detector::{Language, LanguageDetector};
use crate::logging::{LogBuffer, MemoryLogLayer};
use crate::lsp_database_adapter::LspDatabaseAdapter;
use crate::lsp_registry::LspRegistry;
use crate::path_safety::safe_canonicalize;
use crate::pid_lock::PidLock;
#[cfg(unix)]
use crate::process_group::ProcessGroup;
use crate::protocol::{
    parse_call_hierarchy_from_lsp, CallHierarchyItem, CallHierarchyResult, DaemonRequest,
    DaemonResponse, DaemonStatus, DocumentSymbol, HoverContent, LanguageInfo, Location,
    MessageCodec, PoolStatus, SymbolInformation,
};
use crate::server_manager::SingleServerManager;
use crate::socket_path::{get_default_socket_path, remove_socket_file};
use crate::symbol::{generate_version_aware_uid, SymbolUIDGenerator};
use crate::watchdog::{ProcessMonitor, Watchdog};
use crate::workspace_database_router::WorkspaceDatabaseRouter;
use crate::workspace_resolver::WorkspaceResolver;
// Position adjustment for different LSP servers
#[derive(Debug, Clone)]
enum PositionOffset {
    /// Use the start position of the identifier (column 0 of identifier)
    Start,
    /// Start position plus N characters
    StartPlusN(u32),
}

impl PositionOffset {
    /// Apply the offset to a base position, given the identifier length
    fn apply(&self, base_line: u32, base_column: u32, _identifier_len: u32) -> (u32, u32) {
        match self {
            PositionOffset::Start => (base_line, base_column),
            PositionOffset::StartPlusN(n) => (base_line, base_column + n),
        }
    }

    fn description(&self) -> &'static str {
        match self {
            PositionOffset::Start => "start of identifier",
            PositionOffset::StartPlusN(_) => "start + N characters",
        }
    }
}
use anyhow::Context;
use anyhow::{anyhow, Result};
use dashmap::DashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, RwLock, Semaphore};
use tokio::time::{timeout, Duration};

// Connection management constants
const MAX_CONCURRENT_CONNECTIONS: u32 = 64;
const READ_TIMEOUT: Duration = Duration::from_secs(5);
const WRITE_TIMEOUT: Duration = Duration::from_secs(5);
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const REQ_TIMEOUT: Duration = Duration::from_secs(25);
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes
use tracing::{debug, error, info, warn};
use tracing_subscriber::prelude::*;
use uuid::Uuid;

/// Database and cache metrics for monitoring (Step 30.3-30.4)
#[derive(Debug)]
pub struct DatabaseMetrics {
    // Database operation metrics
    pub database_errors: Arc<RwLock<std::collections::HashMap<String, u64>>>,
    pub database_operation_times: Arc<RwLock<Vec<(String, Duration)>>>, // Keep last 100 operations
    pub database_health_checks: Arc<RwLock<u64>>,
    pub database_connection_failures: Arc<RwLock<u64>>,

    // Cache hit/miss tracking per workspace
    pub cache_hits: Arc<RwLock<std::collections::HashMap<String, u64>>>,
    pub cache_misses: Arc<RwLock<std::collections::HashMap<String, u64>>>,
    pub cache_operations_total: Arc<RwLock<u64>>,

    // Symbol resolution metrics
    pub symbol_resolution_successes: Arc<RwLock<u64>>,
    pub symbol_resolution_fallbacks: Arc<RwLock<u64>>,
    pub symbol_resolution_failures: Arc<RwLock<u64>>,

    // Database integrity checks
    pub integrity_checks_passed: Arc<RwLock<u64>>,
    pub integrity_checks_failed: Arc<RwLock<u64>>,
}

impl DatabaseMetrics {
    pub fn new() -> Self {
        Self {
            database_errors: Arc::new(RwLock::new(std::collections::HashMap::new())),
            database_operation_times: Arc::new(RwLock::new(Vec::new())),
            database_health_checks: Arc::new(RwLock::new(0)),
            database_connection_failures: Arc::new(RwLock::new(0)),
            cache_hits: Arc::new(RwLock::new(std::collections::HashMap::new())),
            cache_misses: Arc::new(RwLock::new(std::collections::HashMap::new())),
            cache_operations_total: Arc::new(RwLock::new(0)),
            symbol_resolution_successes: Arc::new(RwLock::new(0)),
            symbol_resolution_fallbacks: Arc::new(RwLock::new(0)),
            symbol_resolution_failures: Arc::new(RwLock::new(0)),
            integrity_checks_passed: Arc::new(RwLock::new(0)),
            integrity_checks_failed: Arc::new(RwLock::new(0)),
        }
    }

    pub async fn increment_database_errors(&self, operation: &str) {
        let mut errors = self.database_errors.write().await;
        *errors.entry(operation.to_string()).or_insert(0) += 1;
    }

    pub async fn record_database_operation_time(&self, operation: &str, duration: Duration) {
        let mut times = self.database_operation_times.write().await;
        times.push((operation.to_string(), duration));
        // Keep only last 100 operations to prevent memory growth
        if times.len() > 100 {
            let excess = times.len() - 100;
            times.drain(0..excess);
        }
    }

    pub async fn increment_cache_hit(&self, workspace: &str) {
        let mut hits = self.cache_hits.write().await;
        *hits.entry(workspace.to_string()).or_insert(0) += 1;
        let mut total = self.cache_operations_total.write().await;
        *total += 1;
    }

    pub async fn increment_cache_miss(&self, workspace: &str) {
        let mut misses = self.cache_misses.write().await;
        *misses.entry(workspace.to_string()).or_insert(0) += 1;
        let mut total = self.cache_operations_total.write().await;
        *total += 1;
    }

    pub async fn get_cache_hit_rate(&self, workspace: &str) -> f64 {
        let hits = {
            let hits_map = self.cache_hits.read().await;
            *hits_map.get(workspace).unwrap_or(&0)
        };

        let misses = {
            let misses_map = self.cache_misses.read().await;
            *misses_map.get(workspace).unwrap_or(&0)
        };

        let total = hits + misses;
        if total == 0 {
            0.0
        } else {
            hits as f64 / total as f64 * 100.0
        }
    }
}

impl Default for DatabaseMetrics {
    fn default() -> Self {
        Self::new()
    }
}

/// Database health status tracking
#[derive(Debug, Clone)]
enum DatabaseHealth {
    Healthy,
    Degraded {
        error_count: u64,
        last_error: String,
    },
    Failed {
        error_message: String,
    },
}

// PID lock path is now handled directly by PidLock::new(socket_path)
// which creates socket_path.pid internally

pub struct LspDaemon {
    socket_path: String,
    registry: Arc<LspRegistry>,
    detector: Arc<LanguageDetector>,
    server_manager: Arc<SingleServerManager>,
    workspace_resolver: Arc<tokio::sync::Mutex<WorkspaceResolver>>,
    connections: Arc<DashMap<Uuid, Instant>>,
    connection_semaphore: Arc<Semaphore>, // Limit concurrent connections
    start_time: Instant,
    request_count: Arc<RwLock<u64>>,
    shutdown: Arc<RwLock<bool>>,
    log_buffer: LogBuffer,
    pid_lock: Option<PidLock>,
    #[cfg(unix)]
    process_group: ProcessGroup,
    child_processes: Arc<tokio::sync::Mutex<Vec<u32>>>, // Track all child PIDs
    // Performance metrics
    request_durations: Arc<RwLock<Vec<Duration>>>, // Keep last 100 request durations
    error_count: Arc<RwLock<usize>>,
    // Connection metrics
    total_connections_accepted: Arc<RwLock<usize>>,
    connections_cleaned_due_to_staleness: Arc<RwLock<usize>>,
    connections_rejected_due_to_limit: Arc<RwLock<usize>>,
    connection_durations: Arc<RwLock<Vec<Duration>>>, // Keep last 100 connection durations
    // Watchdog (disabled by default, enabled via --watchdog flag)
    watchdog: Arc<tokio::sync::Mutex<Option<Watchdog>>>,
    background_tasks: Arc<tokio::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    watchdog_enabled: Arc<AtomicBool>,
    watchdog_task: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
    process_monitor: Arc<ProcessMonitor>,
    child_first_seen: Arc<DashMap<u32, Instant>>,
    // UID generation
    uid_generator: Arc<SymbolUIDGenerator>,
    index_grace_secs: u64,
    // Workspace-aware cache router for multi-workspace environments
    workspace_cache_router: Arc<WorkspaceDatabaseRouter>,
    // Indexing configuration and manager
    indexing_config: Arc<RwLock<IndexingConfig>>,
    indexing_manager: Arc<tokio::sync::Mutex<Option<IndexingManager>>>,
    // Database and cache metrics for Step 30.3-30.4
    metrics: Arc<DatabaseMetrics>,
    // Database health tracking for Priority 4
    database_errors: Arc<AtomicU64>, // Count of database failures
    last_database_error: Arc<Mutex<Option<String>>>, // Last error message
    database_health_status: Arc<Mutex<DatabaseHealth>>, // Overall health
}

impl LspDaemon {
    pub fn new(socket_path: String) -> Result<Self> {
        Self::new_with_config(socket_path, None)
    }

    /// Generate a workspace ID compatible with the current i64 interface
    /// This converts the string workspace ID to a stable i64 hash
    fn generate_workspace_id_hash(&self, workspace_root: &Path) -> i64 {
        let workspace_id_str = self
            .workspace_cache_router
            .workspace_id_for(workspace_root)
            .unwrap_or_else(|_| "default_workspace".to_string());

        // Convert string to i64 hash for current i64 interface
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        workspace_id_str.hash(&mut hasher);
        hasher.finish() as i64
    }

    /// Get position offset for a language/LSP server combination based on known patterns
    fn get_position_offset(&self, language: &str, lsp_server: Option<&str>) -> PositionOffset {
        match (language, lsp_server) {
            // rust-analyzer works best with position at start of identifier
            ("rust", Some("rust-analyzer")) => PositionOffset::Start,
            // gopls works better with position slightly offset
            ("go", Some("gopls")) => PositionOffset::StartPlusN(1),
            // pylsp works with start position
            ("python", Some("pylsp")) => PositionOffset::Start,
            // typescript-language-server works with start position
            ("javascript" | "typescript", Some("typescript-language-server")) => {
                PositionOffset::Start
            }
            // Default to start position for unknown combinations
            _ => PositionOffset::Start,
        }
    }

    /// Create a new LSP daemon with async initialization for persistence
    pub async fn new_async(socket_path: String) -> Result<Self> {
        Self::new_with_config_async(socket_path, None).await
    }

    pub fn new_with_config(
        socket_path: String,
        allowed_roots: Option<Vec<PathBuf>>,
    ) -> Result<Self> {
        // Use the runtime to call the async version with persistence disabled
        let runtime = tokio::runtime::Handle::current();
        runtime.block_on(async {
            Self::new_with_config_and_cache_async(socket_path, allowed_roots).await
        })
    }

    /// Create a new LSP daemon with async initialization and custom cache config
    pub async fn new_with_config_async(
        socket_path: String,
        allowed_roots: Option<Vec<PathBuf>>,
    ) -> Result<Self> {
        Self::new_with_config_and_cache_async(socket_path, allowed_roots).await
    }

    async fn new_with_config_and_cache_async(
        socket_path: String,
        allowed_roots: Option<Vec<PathBuf>>,
    ) -> Result<Self> {
        // Log CI environment detection and persistence status
        if std::env::var("PROBE_CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok() {
            info!("CI environment detected - persistence disabled to prevent hanging");
        }
        info!("LSP daemon starting");

        let registry = Arc::new(LspRegistry::new()?);
        let detector = Arc::new(LanguageDetector::new());
        let child_processes = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let server_manager = Arc::new(SingleServerManager::new_with_tracker(
            registry.clone(),
            child_processes.clone(),
        ));
        let workspace_resolver = Arc::new(tokio::sync::Mutex::new(WorkspaceResolver::new(
            allowed_roots,
        )));

        // Create log buffer and set up tracing subscriber
        let log_buffer = LogBuffer::new();
        let memory_layer = MemoryLogLayer::new(log_buffer.clone());

        // Set up tracing subscriber with memory layer and optionally stderr
        use tracing_subscriber::EnvFilter;

        // Always use a filter to ensure INFO level is captured
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

        let subscriber = tracing_subscriber::registry()
            .with(memory_layer)
            .with(filter);

        // If PROBE_LOG_LEVEL is set to debug or trace, also add stderr logging
        let log_level = std::env::var("PROBE_LOG_LEVEL").unwrap_or_default();
        if log_level == "debug" || log_level == "trace" {
            use tracing_subscriber::fmt;

            let fmt_layer = fmt::layer().with_target(false).with_writer(std::io::stderr);

            if tracing::subscriber::set_global_default(subscriber.with(fmt_layer)).is_ok() {
                tracing::info!("Tracing initialized with memory and stderr logging");
            }
        } else {
            // Memory logging only
            if tracing::subscriber::set_global_default(subscriber).is_ok() {
                tracing::info!("Tracing initialized with memory logging layer");
            }
        }

        // Watchdog is disabled by default (can be enabled via --watchdog flag in lsp init)
        let process_monitor = Arc::new(ProcessMonitor::with_limits(80.0, 1024)); // 80% CPU, 1GB memory

        // Initialize indexing grace period from environment variable
        let index_grace_secs = std::env::var("PROBE_LSP_INDEX_GRACE_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(30); // Default 30 seconds for language server indexing

        // Initialize persistent cache store configuration
        let backend_type =
            std::env::var("PROBE_LSP_CACHE_BACKEND_TYPE").unwrap_or_else(|_| "sqlite".to_string());

        info!("LSP daemon using {} database backend", backend_type);

        let persistent_cache_config = DatabaseCacheConfig {
            backend_type,
            database_config: crate::database::DatabaseConfig {
                path: None,       // Will use default location
                temporary: false, // Persistent cache
                compression: true,
                cache_capacity: 1_000_000_000, // 1GB
                ..Default::default()
            },
        };

        // Initialize workspace cache router for universal cache
        let workspace_cache_router_config =
            crate::workspace_database_router::WorkspaceDatabaseRouterConfig {
                max_open_caches: std::env::var("PROBE_MAX_WORKSPACE_CACHES")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(8),
                max_parent_lookup_depth: 3,
                cache_config_template: persistent_cache_config.clone(),
                ..Default::default()
            };

        let workspace_cache_router =
            Arc::new(WorkspaceDatabaseRouter::new_with_workspace_resolver(
                workspace_cache_router_config,
                server_manager.clone(),
                Some(workspace_resolver.clone()),
            ));

        // Load indexing configuration with updated defaults
        let mut indexing_config = IndexingConfig::load().unwrap_or_else(|e| {
            warn!(
                "Failed to load indexing configuration: {}. Using defaults.",
                e
            );
            IndexingConfig::default()
        });

        // Override from environment if set - these take priority
        if let Ok(val) = std::env::var("PROBE_INDEXING_ENABLED") {
            indexing_config.enabled = val == "true" || val == "1";
        }
        if let Ok(val) = std::env::var("PROBE_INDEXING_AUTO_INDEX") {
            indexing_config.auto_index = val == "true" || val == "1";
        }
        if let Ok(val) = std::env::var("PROBE_INDEXING_WATCH_FILES") {
            indexing_config.watch_files = val == "true" || val == "1";
        }

        info!(
            "Loaded indexing configuration (enabled={}, auto_index={}, watch_files={})",
            indexing_config.enabled, indexing_config.auto_index, indexing_config.watch_files
        );

        let indexing_config = Arc::new(RwLock::new(indexing_config));

        info!("LSP daemon configured for direct database-first request handling");

        Ok(Self {
            socket_path,
            registry,
            detector,
            server_manager,
            workspace_resolver,
            connections: Arc::new(DashMap::new()),
            connection_semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_CONNECTIONS as usize)),
            start_time: Instant::now(),
            request_count: Arc::new(RwLock::new(0)),
            shutdown: Arc::new(RwLock::new(false)),
            log_buffer,
            pid_lock: None,
            #[cfg(unix)]
            process_group: ProcessGroup::new(),
            child_processes,
            request_durations: Arc::new(RwLock::new(Vec::with_capacity(100))),
            error_count: Arc::new(RwLock::new(0)),
            total_connections_accepted: Arc::new(RwLock::new(0)),
            connections_cleaned_due_to_staleness: Arc::new(RwLock::new(0)),
            connections_rejected_due_to_limit: Arc::new(RwLock::new(0)),
            connection_durations: Arc::new(RwLock::new(Vec::with_capacity(100))),
            watchdog: Arc::new(tokio::sync::Mutex::new(None)),
            background_tasks: Arc::new(tokio::sync::Mutex::new(Vec::new())),
            watchdog_enabled: Arc::new(AtomicBool::new(false)),
            watchdog_task: Arc::new(tokio::sync::Mutex::new(None)),
            process_monitor,
            child_first_seen: Arc::new(DashMap::new()),
            uid_generator: Arc::new(SymbolUIDGenerator::new()),
            index_grace_secs,
            workspace_cache_router,
            indexing_config,
            indexing_manager: Arc::new(tokio::sync::Mutex::new(None)),
            metrics: Arc::new(DatabaseMetrics::new()),
            // Initialize database health tracking
            database_errors: Arc::new(AtomicU64::new(0)),
            last_database_error: Arc::new(Mutex::new(None)),
            database_health_status: Arc::new(Mutex::new(DatabaseHealth::Healthy)),
        })
    }

    pub async fn run(mut self) -> Result<()> {
        // Acquire PID lock to ensure only one daemon runs
        // IMPORTANT: PidLock::new takes the socket path directly and creates the .pid file internally
        let mut pid_lock = PidLock::new(&self.socket_path);
        pid_lock
            .try_lock()
            .map_err(|e| anyhow!("Failed to acquire daemon lock: {}", e))?;
        self.pid_lock = Some(pid_lock);
        debug!("Acquired daemon PID lock for socket: {}", self.socket_path);

        // Set up process group for child management
        #[cfg(unix)]
        self.process_group.become_leader()?;

        // Clean up any existing socket
        remove_socket_file(&self.socket_path)?;

        // Migrate existing workspace caches to use git-based naming where possible
        if let Err(e) = self.workspace_cache_router.migrate_workspace_caches().await {
            warn!("Failed to migrate workspace caches: {}", e);
        }

        let listener = IpcListener::bind(&self.socket_path).await?;
        info!("LSP daemon listening on {}", self.socket_path);

        // Watchdog is started only when explicitly enabled via --watchdog flag
        // See enable_watchdog() method which is called from handle_init_workspaces

        // Set up signal handling for graceful shutdown
        #[cfg(unix)]
        {
            let daemon_for_signals = self.clone_refs();
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigterm = signal(SignalKind::terminate())?;
            let mut sigint = signal(SignalKind::interrupt())?;

            tokio::spawn(async move {
                tokio::select! {
                    _ = sigterm.recv() => {
                        info!("Received SIGTERM, shutting down gracefully");
                        *daemon_for_signals.shutdown.write().await = true;
                    }
                    _ = sigint.recv() => {
                        info!("Received SIGINT, shutting down gracefully");
                        *daemon_for_signals.shutdown.write().await = true;
                    }
                }
            });
        }

        // Start idle checker
        let daemon = self.clone_refs();
        let idle_handle = tokio::spawn(async move {
            daemon.idle_checker().await;
        });
        self.background_tasks.lock().await.push(idle_handle);

        // Start periodic cleanup task
        let daemon_for_cleanup = self.clone_refs();
        let cleanup_shutdown = self.shutdown.clone();
        let cleanup_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            loop {
                interval.tick().await;

                // Check if daemon is shutting down
                if *cleanup_shutdown.read().await {
                    debug!("Periodic cleanup task stopping due to shutdown");
                    break;
                }

                let cleaned = daemon_for_cleanup.cleanup_stale_connections();
                if cleaned > 0 {
                    debug!("Periodic cleanup removed {} stale connections", cleaned);
                }
            }
        });
        self.background_tasks.lock().await.push(cleanup_handle);

        // Health monitoring has been simplified and removed in favor of basic process monitoring

        // Start process monitoring task with grace period for indexing
        let process_monitor = self.process_monitor.clone();
        let child_processes_for_monitoring = self.child_processes.clone();
        let child_first_seen = self.child_first_seen.clone();
        let index_grace_secs = self.index_grace_secs;
        let shutdown_flag = self.shutdown.clone();
        let monitor_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30)); // Check every 30 seconds
            loop {
                interval.tick().await;
                if *shutdown_flag.read().await {
                    debug!("Process monitoring task stopping due to shutdown");
                    break;
                }

                let pids = {
                    let pids_guard = child_processes_for_monitoring.lock().await;
                    pids_guard.clone()
                };

                if !pids.is_empty() {
                    debug!("Monitoring {} child processes", pids.len());
                    let now = Instant::now();

                    // Track first seen time for new processes
                    for &pid in &pids {
                        child_first_seen.entry(pid).or_insert(now);
                    }

                    // Only monitor processes that are past the grace period
                    let pids_to_monitor: Vec<u32> = pids
                        .into_iter()
                        .filter(|&pid| {
                            if let Some(first_seen) = child_first_seen.get(&pid) {
                                let age = now.duration_since(*first_seen);
                                if age < Duration::from_secs(index_grace_secs) {
                                    debug!(
                                        "Process {} is in grace period (age: {:?}, grace: {}s)",
                                        pid, age, index_grace_secs
                                    );
                                    false
                                } else {
                                    true
                                }
                            } else {
                                // Should not happen since we just inserted it, but be safe
                                true
                            }
                        })
                        .collect();

                    if !pids_to_monitor.is_empty() {
                        let unhealthy_pids =
                            process_monitor.monitor_children(pids_to_monitor).await;

                        if !unhealthy_pids.is_empty() {
                            warn!(
                                "Found {} unhealthy child processes (past grace period): {:?}",
                                unhealthy_pids.len(),
                                unhealthy_pids
                            );

                            // Kill unhealthy processes and remove from tracking
                            #[cfg(unix)]
                            for pid in &unhealthy_pids {
                                child_first_seen.remove(pid);
                                unsafe {
                                    if libc::kill(*pid as i32, libc::SIGTERM) == 0 {
                                        warn!("Sent SIGTERM to unhealthy process {}", pid);
                                    } else {
                                        warn!("Failed to send SIGTERM to process {}", pid);
                                    }
                                }
                                // Also drop from the tracked pid list so we don't keep monitoring it.
                                {
                                    let mut guard = child_processes_for_monitoring.lock().await;
                                    guard.retain(|p| p != pid);
                                }
                            }
                        }
                    }

                    // Clean up tracking for processes that no longer exist
                    let current_pids: std::collections::HashSet<u32> = {
                        let guard = child_processes_for_monitoring.lock().await;
                        guard.iter().copied().collect()
                    };
                    child_first_seen.retain(|&pid, _| current_pids.contains(&pid));
                }
            }
        });
        self.background_tasks.lock().await.push(monitor_handle);

        // NOTE: Old CallGraph cache warming has been disabled.
        // The universal cache system handles its own cache persistence and loading.
        // self.start_cache_warming_task().await;

        // Trigger auto-indexing if enabled in configuration
        self.trigger_auto_indexing().await;

        loop {
            // Update watchdog heartbeat if enabled
            if self.watchdog_enabled.load(Ordering::Relaxed) {
                if let Some(ref watchdog) = *self.watchdog.lock().await {
                    watchdog.heartbeat();
                }
            }

            // Check shutdown flag
            if *self.shutdown.read().await {
                info!("Daemon shutting down...");
                break;
            }

            // Use select! to make accept interruptible by shutdown
            tokio::select! {
                    accept_result = listener.accept() => {
                        match accept_result {
                            Ok(stream) => {
                                // Acquire semaphore permit before spawning handler
                                let semaphore = self.connection_semaphore.clone();
                                match semaphore.try_acquire_owned() {
                                    Ok(permit) => {
                                        // Track accepted connection
                                        *self.total_connections_accepted.write().await += 1;

                                        let daemon = self.clone_refs();
                                        tokio::spawn(async move {
                                            // Hold permit for duration of connection
                                            let _permit = permit;
                                            if let Err(e) = daemon.handle_connection(stream).await {
                                                error!("Error handling connection: {}", e);
                                            }
                                        });
                                    }
                                    Err(_) => {
                                        // No permits available - reject connection
                                        *self.connections_rejected_due_to_limit.write().await += 1;
                                        warn!(
                                            "Connection limit reached ({} connections), rejecting new connection",
                                            MAX_CONCURRENT_CONNECTIONS
                                        );
                                        drop(stream); // Close connection immediately
                                    }
                                }
                            }
                            Err(e) => {
                                error!("Error accepting connection: {}", e);
                            }
                        }
                    }
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(100)) => {
                    // Periodic check for shutdown flag
                    if *self.shutdown.read().await {
                        info!("Daemon shutting down (periodic check)...");
                        break;
                    }
                }
            }
        }

        // Cleanup
        self.cleanup().await?;
        Ok(())
    }

    async fn handle_connection(&self, stream: IpcStream) -> Result<()> {
        let client_id = Uuid::new_v4();
        info!("New client connected: {}", client_id);

        let connection_start = Instant::now();
        let mut last_activity = Instant::now();

        // Store connection timestamp
        self.connections.insert(client_id, last_activity);

        // Split stream for concurrent read/write operations
        let (mut reader, mut writer) = stream.into_split();

        loop {
            // Check for idle timeout
            if last_activity.elapsed() > IDLE_TIMEOUT {
                warn!(
                    "Connection idle timeout for client {} - closing after {}s",
                    client_id,
                    IDLE_TIMEOUT.as_secs()
                );
                break;
            }

            // Check for overall connection timeout
            if connection_start.elapsed() > CONNECTION_TIMEOUT {
                warn!(
                    "Connection timeout for client {} - closing after {}s",
                    client_id,
                    CONNECTION_TIMEOUT.as_secs()
                );
                break;
            }

            // Check if shutdown was requested
            if *self.shutdown.read().await {
                info!(
                    "Daemon shutting down, closing client connection {}",
                    client_id
                );
                break;
            }

            // Read framed message with timeout
            let message_data = match MessageCodec::read_framed(&mut reader, READ_TIMEOUT).await {
                Ok(data) => data,
                Err(e) => {
                    let error_msg = e.to_string();
                    if error_msg.contains("Timeout") {
                        debug!("Read timeout from client {} - continuing", client_id);
                        continue; // Continue loop on timeout, don't close connection
                    } else if error_msg.contains("early eof") || error_msg.contains("UnexpectedEof")
                    {
                        // Client disconnected gracefully - this is normal
                        debug!("[{}] Client disconnected (early eof)", client_id);
                        break;
                    } else if error_msg.contains("Connection reset")
                        || error_msg.contains("Broken pipe")
                    {
                        // Client disconnected abruptly - also normal
                        debug!(
                            "[{}] Client disconnected abruptly: {}",
                            client_id, error_msg
                        );
                        break;
                    } else {
                        // Actual protocol or I/O error
                        error!("[{}] Failed to read message: {}", client_id, e);
                        break; // Close connection on actual errors
                    }
                }
            };

            // Decode request
            let request = match serde_json::from_slice::<DaemonRequest>(&message_data) {
                Ok(req) => req,
                Err(e) => {
                    error!("[{}] Failed to decode request: {}", client_id, e);
                    // Send error response for malformed requests
                    let error_response = DaemonResponse::Error {
                        request_id: Uuid::new_v4(),
                        error: format!("Malformed request: {e}"),
                    };

                    if let Err(write_err) = self.send_response(&mut writer, &error_response).await {
                        error!(
                            "[{}] Failed to send error response: {}",
                            client_id, write_err
                        );
                        break;
                    }
                    continue;
                }
            };

            // Update activity timestamp
            last_activity = Instant::now();
            self.connections.insert(client_id, last_activity);

            // Increment request count
            *self.request_count.write().await += 1;

            // Handle request with timeout
            let request_start = Instant::now();
            let response_result = timeout(REQ_TIMEOUT, self.handle_request(request)).await;
            let request_duration = request_start.elapsed();

            let response = match response_result {
                Ok(resp) => resp,
                Err(_) => {
                    warn!(
                        "[{}] Request processing timed out after {}s",
                        client_id,
                        REQ_TIMEOUT.as_secs()
                    );
                    DaemonResponse::Error {
                        request_id: Uuid::new_v4(),
                        error: format!("Request timed out after {}s", REQ_TIMEOUT.as_secs()),
                    }
                }
            };

            // Track request duration (keep only last 100)
            {
                let mut durations = self.request_durations.write().await;
                durations.push(request_duration);
                if durations.len() > 100 {
                    durations.remove(0);
                }
            }

            // Track errors
            if let DaemonResponse::Error { .. } = &response {
                *self.error_count.write().await += 1;
            }

            // Send response with timeout
            if let Err(e) = self.send_response(&mut writer, &response).await {
                error!("[{}] Failed to send response: {}", client_id, e);
                break; // Close connection on write errors
            }

            // Check if shutdown was requested
            if let DaemonResponse::Shutdown { .. } = response {
                *self.shutdown.write().await = true;
                break;
            }
        }

        // Calculate and log connection duration
        let connection_duration = connection_start.elapsed();

        // Track connection duration (keep only last 100)
        {
            let mut durations = self.connection_durations.write().await;
            durations.push(connection_duration);
            if durations.len() > 100 {
                durations.remove(0);
            }
        }

        // Remove connection
        self.connections.remove(&client_id);
        info!(
            "Client disconnected: {} (connected for {:?})",
            client_id, connection_duration
        );

        Ok(())
    }

    /// Helper method to send response with timeout
    async fn send_response(
        &self,
        writer: &mut crate::ipc::OwnedWriteHalf,
        response: &DaemonResponse,
    ) -> Result<()> {
        let json_data = serde_json::to_vec(response)?;
        MessageCodec::write_framed(writer, &json_data, WRITE_TIMEOUT).await
    }

    // Clean up connections that have been idle for too long
    fn cleanup_stale_connections(&self) -> usize {
        // Make MAX_IDLE_TIME configurable via environment variable
        let max_idle_secs = std::env::var("LSP_MAX_IDLE_TIME_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(300); // Default to 5 minutes
        let max_idle_time = Duration::from_secs(max_idle_secs);
        let now = Instant::now();

        let connections_before = self.connections.len();
        let mut cleaned_connections = Vec::new();

        self.connections.retain(|client_id, last_activity| {
            let idle_time = now.duration_since(*last_activity);
            if idle_time > max_idle_time {
                cleaned_connections.push((*client_id, idle_time));
                false
            } else {
                true
            }
        });

        let cleaned_count = cleaned_connections.len();
        if cleaned_count > 0 {
            // Update metrics (use blocking_write since this is not an async function)
            if let Ok(mut count) = self.connections_cleaned_due_to_staleness.try_write() {
                *count += cleaned_count;
            }

            info!(
                "Cleaned up {} stale connections (had {} total connections)",
                cleaned_count, connections_before
            );
            for (client_id, idle_time) in cleaned_connections {
                debug!(
                    "Removed stale connection {}: idle for {:?}",
                    client_id, idle_time
                );
            }
        }

        cleaned_count
    }

    /// Handle request with direct database-first approach
    async fn handle_request(&self, request: DaemonRequest) -> DaemonResponse {
        // Direct call to internal handler (database-first approach)
        self.handle_request_internal(request).await
    }

    /// Internal request handler (original implementation)
    async fn handle_request_internal(&self, request: DaemonRequest) -> DaemonResponse {
        // Reduced logging noise - only log interesting requests
        match &request {
            DaemonRequest::CallHierarchy { .. }
            | DaemonRequest::References { .. }
            | DaemonRequest::Definition { .. } => {
                debug!(
                    "Processing LSP request: {:?}",
                    std::mem::discriminant(&request)
                );
            }
            _ => {
                // Skip logging for routine requests like status checks
            }
        }

        // Document synchronization removed - using database-first approach

        // Clean up stale connections on every request to prevent accumulation
        self.cleanup_stale_connections();

        match request {
            DaemonRequest::Connect { client_id } => DaemonResponse::Connected {
                request_id: client_id,
                daemon_version: env!("CARGO_PKG_VERSION").to_string(),
            },

            DaemonRequest::InitializeWorkspace {
                request_id,
                workspace_root,
                language,
            } => {
                match self
                    .handle_initialize_workspace(workspace_root, language)
                    .await
                {
                    Ok((root, lang, server)) => DaemonResponse::WorkspaceInitialized {
                        request_id,
                        workspace_root: root,
                        language: lang,
                        lsp_server: server,
                    },
                    Err(e) => DaemonResponse::Error {
                        request_id,
                        error: e.to_string(),
                    },
                }
            }

            DaemonRequest::InitWorkspaces {
                request_id,
                workspace_root,
                languages,
                recursive,
                enable_watchdog,
            } => {
                // Enable watchdog if requested and not already running
                if enable_watchdog && !self.watchdog_enabled.load(Ordering::Relaxed) {
                    self.enable_watchdog().await;
                }

                match self
                    .handle_init_workspaces(workspace_root, languages, recursive)
                    .await
                {
                    Ok((initialized, errors)) => DaemonResponse::WorkspacesInitialized {
                        request_id,
                        initialized,
                        errors,
                    },
                    Err(e) => DaemonResponse::Error {
                        request_id,
                        error: e.to_string(),
                    },
                }
            }

            DaemonRequest::ListWorkspaces { request_id } => {
                let workspaces = self.server_manager.get_all_workspaces().await;
                DaemonResponse::WorkspaceList {
                    request_id,
                    workspaces,
                }
            }

            DaemonRequest::HealthCheck { request_id } => {
                // Calculate health metrics
                let uptime_seconds = self.start_time.elapsed().as_secs();
                let total_requests = *self.request_count.read().await as usize;
                let active_connections = self.connections.len();
                let active_servers = self.server_manager.get_active_server_count().await;

                // Get LSP server status information (simplified without health monitoring)
                let server_stats = self.server_manager.get_stats().await;

                let lsp_server_health: Vec<crate::protocol::LspServerHealthInfo> = server_stats
                    .into_iter()
                    .map(|s| {
                        crate::protocol::LspServerHealthInfo {
                            language: s.language,
                            is_healthy: s.initialized, // Simplified: healthy if initialized
                            consecutive_failures: 0,   // No failure tracking without health monitor
                            circuit_breaker_open: false, // No circuit breaker
                            last_check_ms: 0,          // No health check tracking
                            response_time_ms: 0,       // No response time tracking
                        }
                    })
                    .collect();

                // Calculate average request duration
                let avg_request_duration_ms = {
                    let durations = self.request_durations.read().await;
                    if durations.is_empty() {
                        0.0
                    } else {
                        let total: Duration = durations.iter().sum();
                        total.as_millis() as f64 / durations.len() as f64
                    }
                };

                // Get error count
                let errors = *self.error_count.read().await;
                let error_rate = if total_requests > 0 {
                    (errors as f64 / total_requests as f64) * 100.0
                } else {
                    0.0
                };

                // Get connection metrics
                let total_accepted = *self.total_connections_accepted.read().await;
                let total_cleaned = *self.connections_cleaned_due_to_staleness.read().await;
                let total_rejected = *self.connections_rejected_due_to_limit.read().await;

                // Estimate memory usage (simplified - in production you'd use a proper memory profiler)
                let memory_usage_mb = {
                    // This is a rough estimate - consider using a proper memory profiler
                    let rusage = std::mem::size_of_val(self) as f64 / 1_048_576.0;
                    rusage + (active_servers as f64 * 50.0) // Estimate 50MB per LSP server
                };

                // Universal cache statistics removed - using database-first approach
                // let cache_stats = None;

                // Health is considered good if:
                // - Not at connection limit (with some buffer)
                // - Reasonable memory usage
                // - Low error rate
                // - Reasonable response times
                // - Not rejecting too many connections
                let connection_rejection_rate = if total_accepted > 0 {
                    (total_rejected as f64 / total_accepted as f64) * 100.0
                } else {
                    0.0
                };

                let healthy = active_connections < 90
                    && memory_usage_mb < 1024.0
                    && error_rate < 5.0
                    && avg_request_duration_ms < 5000.0
                    && connection_rejection_rate < 10.0; // Less than 10% rejection rate

                // Calculate average connection duration
                let avg_connection_duration_ms = {
                    let durations = self.connection_durations.read().await;
                    if durations.is_empty() {
                        0.0
                    } else {
                        let total: Duration = durations.iter().sum();
                        total.as_millis() as f64 / durations.len() as f64
                    }
                };

                // Log basic health check information (cache stats removed)
                info!(
                    "Health check: connections={} (accepted={}, cleaned={}, rejected={}), memory={}MB, errors={}%, avg_req_duration={}ms, avg_conn_duration={}ms",
                    active_connections, total_accepted, total_cleaned, total_rejected, memory_usage_mb, error_rate, avg_request_duration_ms, avg_connection_duration_ms
                );

                DaemonResponse::HealthCheck {
                    request_id,
                    healthy,
                    uptime_seconds,
                    total_requests,
                    active_connections,
                    active_servers,
                    memory_usage_mb,
                    lsp_server_health,
                }
            }

            DaemonRequest::CallHierarchy {
                request_id,
                file_path,
                line,
                column,
                workspace_hint,
            } => {
                info!(
                    "Received DaemonRequest::CallHierarchy for {:?} at {}:{} (request_id: {})",
                    file_path, line, column, request_id
                );

                // Check if file should be excluded from LSP processing
                if should_exclude_from_lsp(&file_path) {
                    warn!(
                        "Ignoring CallHierarchy request for excluded file: {:?} (build artifact/generated code)",
                        file_path
                    );
                    return DaemonResponse::Error {
                        request_id,
                        error: "File is excluded from LSP processing (build artifact or generated code)".to_string(),
                    };
                }

                match self
                    .handle_call_hierarchy(&file_path, line, column, workspace_hint)
                    .await
                {
                    Ok(result) => DaemonResponse::CallHierarchy {
                        request_id,
                        result,
                        warnings: None,
                    },
                    Err(e) => DaemonResponse::Error {
                        request_id,
                        error: e.to_string(),
                    },
                }
            }

            DaemonRequest::Status { request_id } => {
                let server_stats = self.server_manager.get_stats().await;
                let all_readiness = self.server_manager.get_all_readiness_status().await;

                let pool_status: Vec<PoolStatus> = server_stats
                    .into_iter()
                    .map(|s| {
                        // Consider a server "ready" if it's initialized (simplified without health monitoring)
                        let is_ready = s.initialized;

                        // Find readiness information for this language
                        let readiness_info = all_readiness
                            .iter()
                            .find(|r| r.language == s.language)
                            .cloned();

                        PoolStatus {
                            language: s.language,
                            ready_servers: if is_ready { 1 } else { 0 },
                            busy_servers: 0, // No busy concept in single server model
                            total_servers: 1,
                            workspaces: s
                                .workspaces
                                .iter()
                                .map(|w| safe_canonicalize(w).to_string_lossy().to_string())
                                .collect(),
                            uptime_secs: s.uptime.as_secs(),
                            status: format!("{:?}", s.status),
                            health_status: if is_ready {
                                "healthy".to_string()
                            } else {
                                "initializing".to_string()
                            },
                            consecutive_failures: 0, // No failure tracking without health monitor
                            circuit_breaker_open: false, // No circuit breaker
                            readiness_info,
                        }
                    })
                    .collect();

                DaemonResponse::Status {
                    request_id,
                    status: DaemonStatus {
                        uptime_secs: self.start_time.elapsed().as_secs(),
                        pools: pool_status,
                        total_requests: *self.request_count.read().await,
                        active_connections: self.connections.len(),
                        version: env!("CARGO_PKG_VERSION").to_string(),
                        git_hash: env!("GIT_HASH").to_string(),
                        build_date: env!("BUILD_DATE").to_string(),
                        universal_cache_stats: None, // Universal cache layer removed
                        // Add database health information (Priority 4)
                        database_health: Some(self.get_database_health_summary().await),
                    },
                }
            }

            DaemonRequest::ListLanguages { request_id } => {
                let languages = self.registry.list_available_servers();
                let language_info: Vec<LanguageInfo> = languages
                    .into_iter()
                    .map(|(lang, available)| {
                        let config = self.registry.get(lang);
                        LanguageInfo {
                            language: lang,
                            lsp_server: config
                                .map(|c| c.command.clone())
                                .unwrap_or_else(|| "unknown".to_string()),
                            available,
                        }
                    })
                    .collect();

                DaemonResponse::LanguageList {
                    request_id,
                    languages: language_info,
                }
            }

            DaemonRequest::Shutdown { request_id } => {
                info!("Shutdown requested");
                DaemonResponse::Shutdown { request_id }
            }

            DaemonRequest::Ping { request_id } => DaemonResponse::Pong { request_id },

            DaemonRequest::GetLogs {
                request_id,
                lines,
                since_sequence,
            } => {
                let entries = if let Some(since) = since_sequence {
                    // Get logs since sequence
                    self.log_buffer.get_since_sequence(since, lines)
                } else {
                    // Backward compatibility: get last N logs
                    self.log_buffer.get_last(lines)
                };

                DaemonResponse::Logs {
                    request_id,
                    entries,
                }
            }

            DaemonRequest::CacheStats {
                request_id,
                detailed: _detailed,
                git: _git,
            } => {
                // Universal cache layer removed - return empty stats
                info!("Cache stats request (universal cache removed)");

                let legacy_stats = crate::protocol::CacheStatistics {
                    hit_rate: 0.0,
                    miss_rate: 1.0,
                    total_entries: 0,
                    total_size_bytes: 0,
                    disk_size_bytes: 0,
                    entries_per_file: std::collections::HashMap::new(),
                    entries_per_language: std::collections::HashMap::new(),
                    age_distribution: crate::protocol::AgeDistribution {
                        entries_last_hour: 0,
                        entries_last_day: 0,
                        entries_last_week: 0,
                        entries_last_month: 0,
                        entries_older: 0,
                    },
                    most_accessed: Vec::new(),
                    memory_usage: crate::protocol::MemoryUsage {
                        in_memory_cache_bytes: 0,
                        persistent_cache_bytes: 0,
                        metadata_bytes: 0,
                        index_bytes: 0,
                    },
                    per_workspace_stats: None,
                    per_operation_totals: None,
                };

                DaemonResponse::CacheStats {
                    request_id,
                    stats: legacy_stats,
                }
            }

            DaemonRequest::CacheClear {
                request_id,
                older_than_days: _older_than_days,
                file_path,
                commit_hash: _commit_hash,
                all,
            } => {
                // Universal cache clearing - different approach than legacy cache manager
                if all {
                    // Clear all workspace caches through the workspace router
                    match self
                        .workspace_cache_router
                        .clear_workspace_cache(None, None)
                        .await
                    {
                        Ok(result) => {
                            let legacy_result = crate::protocol::ClearResult {
                                entries_removed: result.total_files_removed as u64,
                                files_affected: result.total_files_removed as u64,
                                branches_affected: 0, // Not applicable to universal cache
                                commits_affected: 0,  // Not applicable to universal cache
                                bytes_reclaimed: result.total_size_freed_bytes,
                                duration_ms: 0, // Not tracked
                            };
                            DaemonResponse::CacheCleared {
                                request_id,
                                result: legacy_result,
                            }
                        }
                        Err(e) => DaemonResponse::Error {
                            request_id,
                            error: format!("Failed to clear all workspace caches: {e}"),
                        },
                    }
                } else if let Some(_file_path) = file_path {
                    // Clear cache for a specific file (universal cache removed)
                    // Return placeholder result since universal cache is removed
                    let legacy_result = crate::protocol::ClearResult {
                        entries_removed: 0,
                        files_affected: 1,
                        branches_affected: 0,
                        commits_affected: 0,
                        bytes_reclaimed: 0,
                        duration_ms: 0,
                    };
                    DaemonResponse::CacheCleared {
                        request_id,
                        result: legacy_result,
                    }
                } else {
                    // No specific clear target - universal cache removed
                    DaemonResponse::Error {
                        request_id,
                        error: "Cache clearing requires either 'all=true' or a specific file path"
                            .to_string(),
                    }
                }
            }

            DaemonRequest::CacheExport {
                request_id,
                output_path: _output_path,
                current_branch_only: _current_branch_only,
                compress: _compress,
            } => {
                // Universal cache export is not yet implemented
                DaemonResponse::Error {
                    request_id,
                    error: "Cache export is not yet supported in the universal cache system. Use workspace cache management instead.".to_string(),
                }
            }

            DaemonRequest::CacheImport {
                request_id,
                input_path: _input_path,
                merge: _merge,
            } => {
                // Universal cache import is not yet implemented
                DaemonResponse::Error {
                    request_id,
                    error: "Cache import is not yet supported in the universal cache system. Use workspace cache management instead.".to_string(),
                }
            }

            DaemonRequest::CacheCompact {
                request_id,
                target_size_mb: _target_size_mb,
            } => {
                // Universal cache compaction happens automatically at the workspace level
                DaemonResponse::Error {
                    request_id,
                    error: "Cache compaction is handled automatically by the universal cache system. Use workspace cache management for manual operations.".to_string(),
                }
            }

            // Indexing management requests
            DaemonRequest::StartIndexing {
                request_id,
                workspace_root,
                config,
            } => match self
                .handle_start_indexing(workspace_root.clone(), config)
                .await
            {
                Ok(session_id) => DaemonResponse::IndexingStarted {
                    request_id,
                    workspace_root,
                    session_id,
                },
                Err(e) => DaemonResponse::Error {
                    request_id,
                    error: e.to_string(),
                },
            },

            DaemonRequest::StopIndexing { request_id, force } => {
                match self.handle_stop_indexing(force).await {
                    Ok(was_running) => DaemonResponse::IndexingStopped {
                        request_id,
                        was_running,
                    },
                    Err(e) => DaemonResponse::Error {
                        request_id,
                        error: e.to_string(),
                    },
                }
            }

            DaemonRequest::IndexingStatus { request_id } => {
                match self.handle_indexing_status().await {
                    Ok(status) => DaemonResponse::IndexingStatusResponse { request_id, status },
                    Err(e) => DaemonResponse::Error {
                        request_id,
                        error: e.to_string(),
                    },
                }
            }

            DaemonRequest::IndexingConfig { request_id } => {
                let config = self.indexing_config.read().await;
                let protocol_config = self.convert_internal_to_protocol_config(&config);
                DaemonResponse::IndexingConfigResponse {
                    request_id,
                    config: protocol_config,
                }
            }

            DaemonRequest::SetIndexingConfig { request_id, config } => {
                match self.handle_set_indexing_config(config.clone()).await {
                    Ok(()) => DaemonResponse::IndexingConfigSet { request_id, config },
                    Err(e) => DaemonResponse::Error {
                        request_id,
                        error: e.to_string(),
                    },
                }
            }

            // Git-aware requests
            DaemonRequest::GetCallHierarchyAtCommit {
                request_id,
                file_path,
                symbol,
                line,
                column,
                commit_hash,
                workspace_hint,
            } => {
                match self
                    .handle_call_hierarchy_at_commit(
                        &file_path,
                        &symbol,
                        line,
                        column,
                        &commit_hash,
                        workspace_hint,
                    )
                    .await
                {
                    Ok((result, git_context)) => DaemonResponse::CallHierarchyAtCommit {
                        request_id,
                        result,
                        commit_hash,
                        git_context: Some(git_context),
                    },
                    Err(e) => DaemonResponse::Error {
                        request_id,
                        error: e.to_string(),
                    },
                }
            }

            DaemonRequest::GetCacheHistory {
                request_id,
                file_path,
                symbol,
                workspace_hint: _,
            } => match self.handle_get_cache_history(&file_path, &symbol).await {
                Ok(history) => DaemonResponse::CacheHistory {
                    request_id,
                    history,
                },
                Err(e) => DaemonResponse::Error {
                    request_id,
                    error: e.to_string(),
                },
            },

            DaemonRequest::GetCacheAtCommit {
                request_id,
                commit_hash: _,
                workspace_hint: _,
            } => DaemonResponse::Error {
                request_id,
                error: "GetCacheAtCommit operation is not supported in universal cache system"
                    .to_string(),
            },

            DaemonRequest::DiffCacheCommits {
                request_id,
                from_commit: _from_commit,
                to_commit: _to_commit,
                workspace_hint: _,
            } => {
                // Universal cache does not support commit-level diffing
                DaemonResponse::Error {
                    request_id,
                    error: "Cache commit diffing is not supported in the universal cache system. Use workspace cache management instead.".to_string(),
                }
            }

            // Workspace cache management requests
            DaemonRequest::WorkspaceCacheList { request_id } => {
                match self
                    .workspace_cache_router
                    .list_all_workspace_caches()
                    .await
                {
                    Ok(workspaces) => DaemonResponse::WorkspaceCacheList {
                        request_id,
                        workspaces,
                    },
                    Err(e) => DaemonResponse::Error {
                        request_id,
                        error: e.to_string(),
                    },
                }
            }

            DaemonRequest::WorkspaceCacheInfo {
                request_id,
                workspace_path,
            } => {
                match self
                    .workspace_cache_router
                    .get_workspace_cache_info(workspace_path.clone())
                    .await
                {
                    Ok(info_list) => {
                        if workspace_path.is_some() && !info_list.is_empty() {
                            // Return single workspace info
                            DaemonResponse::WorkspaceCacheInfo {
                                request_id,
                                workspace_info: Some(Box::new(
                                    info_list.into_iter().next().unwrap(),
                                )),
                                all_workspaces_info: None,
                            }
                        } else if workspace_path.is_none() && !info_list.is_empty() {
                            // Return all workspaces info
                            DaemonResponse::WorkspaceCacheInfo {
                                request_id,
                                workspace_info: None,
                                all_workspaces_info: Some(info_list),
                            }
                        } else {
                            // No info found
                            DaemonResponse::WorkspaceCacheInfo {
                                request_id,
                                workspace_info: None,
                                all_workspaces_info: None,
                            }
                        }
                    }
                    Err(e) => DaemonResponse::Error {
                        request_id,
                        error: e.to_string(),
                    },
                }
            }

            DaemonRequest::WorkspaceCacheClear {
                request_id,
                workspace_path,
                older_than_seconds,
            } => {
                if let Some(age_seconds) = older_than_seconds {
                    info!(
                        "Workspace cache clear requested for: {:?} (older than {} seconds)",
                        workspace_path
                            .as_deref()
                            .unwrap_or("all workspaces".as_ref()),
                        age_seconds
                    );
                } else {
                    info!(
                        "Workspace cache clear requested for: {:?}",
                        workspace_path
                            .as_deref()
                            .unwrap_or("all workspaces".as_ref())
                    );
                }

                match self
                    .workspace_cache_router
                    .clear_workspace_cache(workspace_path, older_than_seconds)
                    .await
                {
                    Ok(result) => {
                        info!(
                            "Workspace cache clear completed: {} workspaces cleared, {} bytes freed, {} files removed",
                            result.cleared_workspaces.len(),
                            result.total_size_freed_bytes,
                            result.total_files_removed
                        );

                        if !result.errors.is_empty() {
                            warn!(
                                "Workspace cache clear had {} errors: {:?}",
                                result.errors.len(),
                                result.errors
                            );
                        }

                        DaemonResponse::WorkspaceCacheCleared { request_id, result }
                    }
                    Err(e) => {
                        error!("Workspace cache clear failed: {}", e);
                        DaemonResponse::Error {
                            request_id,
                            error: e.to_string(),
                        }
                    }
                }
            }

            DaemonRequest::Definition {
                request_id,
                file_path,
                line,
                column,
                workspace_hint,
            } => {
                info!(
                    "Received DaemonRequest::Definition for {:?} at {}:{} (request_id: {})",
                    file_path, line, column, request_id
                );

                // Check if file should be excluded from LSP processing
                if should_exclude_from_lsp(&file_path) {
                    warn!(
                        "Ignoring Definition request for excluded file: {:?} (build artifact/generated code)",
                        file_path
                    );
                    return DaemonResponse::Error {
                        request_id,
                        error: "File is excluded from LSP processing (build artifact or generated code)".to_string(),
                    };
                }

                // Handle definition request directly (universal cache middleware handles caching)
                let absolute_file_path = safe_canonicalize(&file_path);

                let result = async {
                    let language = self.detector.detect(&absolute_file_path)?;
                    if language == Language::Unknown {
                        return Err(anyhow!(
                            "Unknown language for file: {:?}",
                            absolute_file_path
                        ));
                    }

                    let workspace_root = {
                        let mut resolver = self.workspace_resolver.lock().await;
                        resolver.resolve_workspace(&absolute_file_path, workspace_hint)?
                    };

                    // Read file content for symbol resolution
                    let content = fs::read_to_string(&absolute_file_path)?;

                    // PHASE 1: Try database first
                    if let Ok(symbol_name) = self.find_symbol_at_position(&absolute_file_path, &content, line, column) {
                        // Generate consistent symbol UID for database lookup
                        let symbol_uid = match self.generate_consistent_symbol_uid(&absolute_file_path, &symbol_name, line, column, language.as_str(), &workspace_root, &content).await {
                            Ok(uid) => uid,
                            Err(e) => {
                                debug!("[VERSION_AWARE_UID] Failed to generate version-aware UID, using fallback approach: {}", e);
                                // Fallback to version-aware UID with basic file content
                                match generate_version_aware_uid(&workspace_root, &absolute_file_path, &content, &symbol_name, line) {
                                    Ok(fallback_uid) => {
                                        debug!("[VERSION_AWARE_UID] Fallback UID generated: {}", fallback_uid);
                                        fallback_uid
                                    }
                                    Err(fallback_e) => {
                                        debug!("[VERSION_AWARE_UID] Even fallback failed: {}. Using emergency format", fallback_e);
                                        // Emergency fallback - should be very rare
                                        format!("EMERGENCY:{}:{}:{}:{}",
                                            absolute_file_path.file_name().unwrap_or_default().to_string_lossy(),
                                            symbol_name,
                                            line,
                                            column)
                                    }
                                }
                            }
                        };

                        if let Ok(workspace_cache) = self.workspace_cache_router.cache_for_workspace(&workspace_root).await {
                            // Generate workspace-specific ID from workspace_root
                            let workspace_id = self.generate_workspace_id_hash(&workspace_root);

                            match workspace_cache.get_definitions(workspace_id, &symbol_uid).await {
                                Ok(Some(locations)) => {
                                    info!("Database HIT for {} definitions at {}:{}:{}",
                                         symbol_name, absolute_file_path.display(), line, column);
                                    return Ok(locations);
                                }
                                Ok(None) => {
                                    debug!("Database MISS for {} definitions - calling LSP", symbol_name);
                                }
                                Err(e) => {
                                    warn!("Database query error: {} - falling back to LSP", e);
                                    // Track database error for health monitoring (Priority 4)
                                    self.record_database_error(&e).await;
                                }
                            }
                        }
                    } else {
                        debug!("Could not resolve symbol at position {}:{}:{} - skipping database query",
                               absolute_file_path.display(), line, column);
                    }

                    // PHASE 2: Database miss - proceed with LSP call
                    let server_instance = self
                        .server_manager
                        .ensure_workspace_registered(language, workspace_root.clone())
                        .await?;

                    // Make the definition request directly without explicit document lifecycle
                    // The LSP server manages its own document state
                    let response_json = {
                        let server = server_instance.lock().await;
                        server
                            .server
                            .definition(&absolute_file_path, line, column)
                            .await?
                    };

                    // Check if response is null vs empty array
                    let is_null_response = response_json.is_null();
                    let locations = Self::parse_definition_response(&response_json)?;

                    // MILESTONE 21: Store definitions data in the database
                    // Only store if we got a valid response (not null)
                    // Empty array [] is valid and should create "none" edges
                    if !is_null_response {
                        if let Err(e) = self.store_definitions_in_database(
                            &locations,
                            &absolute_file_path,
                            &workspace_root,
                            language.as_str(),
                            line,
                            column,
                        ).await {
                            error!(
                                "DATABASE_ERROR [definitions]: Failed to store {} definitions in database for {}:{}:{} - {} | cause: {:?} | context: language={}, workspace={:?}",
                                locations.len(),
                                absolute_file_path.display(),
                                line,
                                column,
                                e,
                                e.chain().collect::<Vec<_>>(),
                                format!("{:?}", language),
                                workspace_root
                            );
                            // Track database error metrics (Step 30.3) - TODO: Make async
                            // self.metrics.increment_database_errors("definitions").await;
                        }
                    } else {
                        info!("LSP returned null for definitions at {}:{}:{} - not caching (LSP server may not be ready)",
                              absolute_file_path.display(), line, column);
                    }

                    Ok(locations)
                }
                .await;

                match result {
                    Ok(locations) => DaemonResponse::Definition {
                        request_id,
                        locations,
                        warnings: None,
                    },
                    Err(e) => DaemonResponse::Error {
                        request_id,
                        error: e.to_string(),
                    },
                }
            }

            DaemonRequest::References {
                request_id,
                file_path,
                line,
                column,
                include_declaration,
                workspace_hint,
            } => {
                info!(
                    "Received DaemonRequest::References for {:?} at {}:{} include_decl={} (request_id: {})",
                    file_path, line, column, include_declaration, request_id
                );

                // Check if file should be excluded from LSP processing
                if should_exclude_from_lsp(&file_path) {
                    warn!(
                        "Ignoring References request for excluded file: {:?} (build artifact/generated code)",
                        file_path
                    );
                    return DaemonResponse::Error {
                        request_id,
                        error: "File is excluded from LSP processing (build artifact or generated code)".to_string(),
                    };
                }

                // Handle references request directly (universal cache middleware handles caching)
                let absolute_file_path = safe_canonicalize(&file_path);

                let result = async {
                    let language = self.detector.detect(&absolute_file_path)?;
                    if language == Language::Unknown {
                        return Err(anyhow!(
                            "Unknown language for file: {:?}",
                            absolute_file_path
                        ));
                    }

                    let workspace_root = {
                        let mut resolver = self.workspace_resolver.lock().await;
                        resolver.resolve_workspace(&absolute_file_path, workspace_hint)?
                    };

                    // Read file content for symbol resolution
                    let content = fs::read_to_string(&absolute_file_path)?;

                    // PHASE 1: Try database first
                    if let Ok(symbol_name) = self.find_symbol_at_position(&absolute_file_path, &content, line, column) {
                        // Generate consistent symbol UID for database lookup
                        let symbol_uid = match self.generate_consistent_symbol_uid(&absolute_file_path, &symbol_name, line, column, language.as_str(), &workspace_root, &content).await {
                            Ok(uid) => uid,
                            Err(e) => {
                                debug!("[VERSION_AWARE_UID] Failed to generate version-aware UID, using fallback approach: {}", e);
                                // Fallback to version-aware UID with basic file content
                                match generate_version_aware_uid(&workspace_root, &absolute_file_path, &content, &symbol_name, line) {
                                    Ok(fallback_uid) => {
                                        debug!("[VERSION_AWARE_UID] Fallback UID generated: {}", fallback_uid);
                                        fallback_uid
                                    }
                                    Err(fallback_e) => {
                                        debug!("[VERSION_AWARE_UID] Even fallback failed: {}. Using emergency format", fallback_e);
                                        // Emergency fallback - should be very rare
                                        format!("EMERGENCY:{}:{}:{}:{}",
                                            absolute_file_path.file_name().unwrap_or_default().to_string_lossy(),
                                            symbol_name,
                                            line,
                                            column)
                                    }
                                }
                            }
                        };

                        if let Ok(workspace_cache) = self.workspace_cache_router.cache_for_workspace(&workspace_root).await {
                            // Generate workspace-specific ID from workspace_root
                            let workspace_id = self.generate_workspace_id_hash(&workspace_root);

                            match workspace_cache.get_references(workspace_id, &symbol_uid, include_declaration).await {
                                Ok(Some(locations)) => {
                                    info!("Database HIT for {} references at {}:{}:{}",
                                         symbol_name, absolute_file_path.display(), line, column);
                                    return Ok(locations);
                                }
                                Ok(None) => {
                                    debug!("Database MISS for {} references - calling LSP", symbol_name);
                                }
                                Err(e) => {
                                    warn!("Database query error: {} - falling back to LSP", e);
                                    // Track database error for health monitoring (Priority 4)
                                    self.record_database_error(&e).await;
                                }
                            }
                        }
                    } else {
                        debug!("Could not resolve symbol at position {}:{}:{} - skipping database query",
                               absolute_file_path.display(), line, column);
                    }

                    // PHASE 2: Database miss - proceed with LSP call
                    let server_instance = self
                        .server_manager
                        .ensure_workspace_registered(language, workspace_root.clone())
                        .await?;

                    // Ensure document is opened and ready before querying references
                    // This is critical for many LSP servers (like phpactor) which require
                    // the document to be opened before they can provide references
                    let response_json = {
                        let server = server_instance.lock().await;

                        debug!(
                            "Opening document for references analysis: {:?}",
                            absolute_file_path
                        );

                        // Always open the document to ensure the LSP server has the latest content
                        // Many LSP servers need the file to be properly opened before references work
                        server
                            .server
                            .open_document(&absolute_file_path, &content)
                            .await?;

                        server
                            .server
                            .references(&absolute_file_path, line, column, include_declaration)
                            .await?
                    };

                    // Check if response is null vs empty array
                    let is_null_response = response_json.is_null();
                    let locations = Self::parse_references_response(&response_json)?;

                    // MILESTONE 21: Store references data in the database
                    // Only store if we got a valid response (not null)
                    // Empty array [] is valid and should create "none" edges
                    if !is_null_response {
                        if let Err(e) = self.store_references_in_database(
                            &locations,
                            &absolute_file_path,
                            &workspace_root,
                            language.as_str(),
                            line,
                            column,
                        ).await {
                            error!(
                                "DATABASE_ERROR [references]: Failed to store {} references in database for {}:{}:{} - {} | cause: {:?} | context: language={}, workspace={:?}",
                                locations.len(),
                                absolute_file_path.display(),
                                line,
                                column,
                                e,
                                e.chain().collect::<Vec<_>>(),
                                format!("{:?}", language),
                                workspace_root
                            );
                            // Track database error metrics (Step 30.3) - TODO: Make async
                            // self.metrics.increment_database_errors("references").await;
                        }
                    } else {
                        info!("LSP returned null for references at {}:{}:{} - not caching (LSP server may not be ready)",
                              absolute_file_path.display(), line, column);
                    }

                    Ok(locations)
                }
                .await;

                match result {
                    Ok(locations) => DaemonResponse::References {
                        request_id,
                        locations,
                        warnings: None,
                    },
                    Err(e) => DaemonResponse::Error {
                        request_id,
                        error: e.to_string(),
                    },
                }
            }

            DaemonRequest::Hover {
                request_id,
                file_path,
                line,
                column,
                workspace_hint,
            } => {
                info!(
                    "Received DaemonRequest::Hover for {:?} at {}:{} (request_id: {})",
                    file_path, line, column, request_id
                );

                // Check if file should be excluded from LSP processing
                if should_exclude_from_lsp(&file_path) {
                    warn!(
                        "Ignoring Hover request for excluded file: {:?} (build artifact/generated code)",
                        file_path
                    );
                    return DaemonResponse::Error {
                        request_id,
                        error: "File is excluded from LSP processing (build artifact or generated code)".to_string(),
                    };
                }

                // Handle hover request directly (universal cache middleware handles caching)
                let absolute_file_path = safe_canonicalize(&file_path);

                let result = async {
                    let language = self.detector.detect(&absolute_file_path)?;
                    if language == Language::Unknown {
                        return Err(anyhow!(
                            "Unknown language for file: {:?}",
                            absolute_file_path
                        ));
                    }

                    let workspace_root = {
                        let mut resolver = self.workspace_resolver.lock().await;
                        resolver.resolve_workspace(&absolute_file_path, workspace_hint)?
                    };

                    let server_instance = self
                        .server_manager
                        .ensure_workspace_registered(language, workspace_root)
                        .await?;

                    let server = server_instance.lock().await;
                    let response_json = server
                        .server
                        .hover(&absolute_file_path, line, column)
                        .await?;

                    let hover = Self::parse_hover_response(&response_json)?;
                    Ok(hover)
                }
                .await;

                match result {
                    Ok(content) => DaemonResponse::Hover {
                        request_id,
                        content,
                        warnings: None,
                    },
                    Err(e) => DaemonResponse::Error {
                        request_id,
                        error: e.to_string(),
                    },
                }
            }

            DaemonRequest::DocumentSymbols {
                request_id,
                file_path,
                workspace_hint,
            } => {
                info!(
                    "Received DaemonRequest::DocumentSymbols for {:?} (request_id: {})",
                    file_path, request_id
                );
                match self
                    .handle_document_symbols(&file_path, workspace_hint)
                    .await
                {
                    Ok(symbols) => DaemonResponse::DocumentSymbols {
                        request_id,
                        symbols,
                        warnings: None,
                    },
                    Err(e) => DaemonResponse::Error {
                        request_id,
                        error: e.to_string(),
                    },
                }
            }

            DaemonRequest::WorkspaceSymbols {
                request_id,
                query,
                workspace_hint,
            } => {
                info!(
                    "Received DaemonRequest::WorkspaceSymbols query='{}' (request_id: {})",
                    query, request_id
                );
                match self.handle_workspace_symbols(&query, workspace_hint).await {
                    Ok(symbols) => DaemonResponse::WorkspaceSymbols {
                        request_id,
                        symbols,
                        warnings: None,
                    },
                    Err(e) => DaemonResponse::Error {
                        request_id,
                        error: e.to_string(),
                    },
                }
            }

            DaemonRequest::Implementations {
                request_id,
                file_path,
                line,
                column,
                workspace_hint,
            } => {
                info!(
                    "Received DaemonRequest::Implementations for {:?} at {}:{} (request_id: {})",
                    file_path, line, column, request_id
                );
                match self
                    .handle_implementations(&file_path, line, column, workspace_hint)
                    .await
                {
                    Ok(locations) => DaemonResponse::Implementations {
                        request_id,
                        locations,
                        warnings: None,
                    },
                    Err(e) => DaemonResponse::Error {
                        request_id,
                        error: e.to_string(),
                    },
                }
            }

            DaemonRequest::TypeDefinition {
                request_id,
                file_path,
                line,
                column,
                workspace_hint,
            } => {
                info!(
                    "Received DaemonRequest::TypeDefinition for {:?} at {}:{} (request_id: {})",
                    file_path, line, column, request_id
                );
                match self
                    .handle_type_definition(&file_path, line, column, workspace_hint)
                    .await
                {
                    Ok(locations) => DaemonResponse::TypeDefinition {
                        request_id,
                        locations,
                        warnings: None,
                    },
                    Err(e) => DaemonResponse::Error {
                        request_id,
                        error: e.to_string(),
                    },
                }
            }

            // Symbol-specific cache clearing
            DaemonRequest::ClearSymbolCache {
                request_id,
                file_path,
                symbol_name,
                line,
                column,
                methods,
                all_positions,
            } => {
                info!(
                    "Received DaemonRequest::ClearSymbolCache for symbol '{}' in {:?} at {:?}:{:?}",
                    symbol_name, file_path, line, column
                );
                match self
                    .handle_clear_symbol_cache(
                        &file_path,
                        &symbol_name,
                        line,
                        column,
                        methods,
                        all_positions,
                    )
                    .await
                {
                    Ok(result) => DaemonResponse::SymbolCacheCleared { request_id, result },
                    Err(e) => DaemonResponse::Error {
                        request_id,
                        error: e.to_string(),
                    },
                }
            }

            // Explicit "not implemented" response for completion - not part of this implementation
            DaemonRequest::Completion { request_id, .. } => {
                warn!("Received unimplemented completion request, returning error with original request_id");
                DaemonResponse::Error {
                    request_id,
                    error: "Completion request type not implemented".to_string(),
                }
            }

            // Handle cache key listing
            DaemonRequest::CacheListKeys {
                request_id,
                workspace_path: _,
                operation_filter: _,
                file_pattern_filter: _,
                limit,
                offset,
                sort_by: _,
                sort_order: _,
                detailed: _,
            } => {
                // Universal cache layer removed - return empty keys list
                DaemonResponse::CacheListKeys {
                    request_id,
                    keys: Vec::new(),
                    total_count: 0,
                    offset,
                    limit,
                    has_more: false,
                }
            }

            DaemonRequest::IndexExport {
                request_id,
                workspace_path,
                output_path,
                checkpoint,
            } => {
                // Handle index export request
                self.handle_index_export(request_id, workspace_path, output_path, checkpoint)
                    .await
            }
        }
    }

    /// Handle index export request
    async fn handle_index_export(
        &self,
        request_id: Uuid,
        workspace_path: Option<PathBuf>,
        output_path: PathBuf,
        checkpoint: bool,
    ) -> DaemonResponse {
        use std::fs;

        // Determine which workspace to export from
        let workspace = match workspace_path {
            Some(path) => path,
            None => {
                // Use current working directory
                match std::env::current_dir() {
                    Ok(dir) => dir,
                    Err(e) => {
                        return DaemonResponse::Error {
                            request_id,
                            error: format!("Failed to get current directory: {}", e),
                        }
                    }
                }
            }
        };

        // Get the cache for this workspace
        let cache_adapter = match self
            .workspace_cache_router
            .cache_for_workspace(&workspace)
            .await
        {
            Ok(cache) => cache,
            Err(e) => {
                return DaemonResponse::Error {
                    request_id,
                    error: format!("Failed to get cache for workspace: {}", e),
                }
            }
        };

        // Get the database path from the cache adapter
        let db_path = cache_adapter.database_path();

        // If checkpoint is requested, perform WAL checkpoint
        if checkpoint {
            if let Err(e) = cache_adapter.checkpoint().await {
                error!("Failed to checkpoint database: {}", e);
                // Continue with export even if checkpoint fails
            }
        }

        // Copy the database file to the output path
        match fs::copy(&db_path, &output_path) {
            Ok(bytes_copied) => {
                info!(
                    "Exported database from {} to {} ({} bytes)",
                    db_path.display(),
                    output_path.display(),
                    bytes_copied
                );

                DaemonResponse::IndexExported {
                    request_id,
                    workspace_path: workspace,
                    output_path,
                    database_size_bytes: bytes_copied as usize,
                }
            }
            Err(e) => DaemonResponse::Error {
                request_id,
                error: format!("Failed to export database: {}", e),
            },
        }
    }

    /// Handle clearing cache for a specific symbol
    async fn handle_clear_symbol_cache(
        &self,
        file_path: &Path,
        symbol_name: &str,
        _line: Option<u32>,
        _column: Option<u32>,
        _methods: Option<Vec<String>>,
        _all_positions: bool,
    ) -> Result<crate::protocol::SymbolCacheClearResult> {
        let start_time = std::time::Instant::now();

        // Universal cache layer removed - no cache to clear
        let (entries_cleared, positions_cleared, methods_cleared, size_freed) =
            (0, Vec::new(), Vec::new(), 0);

        let duration_ms = start_time.elapsed().as_millis() as u64;

        Ok(crate::protocol::SymbolCacheClearResult {
            symbol_name: symbol_name.to_string(),
            file_path: file_path.to_path_buf(),
            entries_cleared,
            positions_cleared,
            methods_cleared,
            cache_size_freed_bytes: size_freed,
            duration_ms,
        })
    }

    async fn handle_call_hierarchy(
        &self,
        file_path: &Path,
        line: u32,
        column: u32,
        workspace_hint: Option<PathBuf>,
    ) -> Result<CallHierarchyResult> {
        // Use timeout to prevent hanging indefinitely
        let operation_timeout = tokio::time::Duration::from_secs(120); // 120 second timeout to accommodate rust-analyzer initialization

        tokio::time::timeout(
            operation_timeout,
            self.handle_call_hierarchy_inner(file_path, line, column, workspace_hint),
        )
        .await
        .map_err(|_| anyhow!("Call hierarchy operation timed out after 120 seconds"))?
    }

    async fn handle_call_hierarchy_inner(
        &self,
        file_path: &Path,
        line: u32,
        column: u32,
        workspace_hint: Option<PathBuf>,
    ) -> Result<CallHierarchyResult> {
        debug!(
            "handle_call_hierarchy_inner called for {:?} at {}:{}",
            file_path, line, column
        );

        // Convert relative path to absolute path
        // Be tolerant to transient canonicalize issues (e.g., symlinks/overlays in test fixtures).
        let absolute_file_path = match safe_canonicalize(file_path).as_path() {
            p if p.exists() => p.to_path_buf(),
            _ => {
                if file_path.is_absolute() {
                    file_path.to_path_buf()
                } else {
                    std::env::current_dir()
                        .unwrap_or_else(|_| PathBuf::from("/"))
                        .join(file_path)
                }
            }
        };

        // Compute MD5 hash for cache key
        let content_md5 = md5_hex_file(&absolute_file_path)?;

        // Detect language
        let language = self.detector.detect(file_path)?;

        if language == Language::Unknown {
            return Err(anyhow!("Unknown language for file: {:?}", file_path));
        }

        // Clone workspace_hint before it's moved to the resolver
        let _workspace_hint_for_cache = workspace_hint.clone();

        // Resolve workspace root
        let workspace_root = {
            let mut resolver = self.workspace_resolver.lock().await;
            resolver.resolve_workspace(file_path, workspace_hint)?
        };

        // Read file content
        let content = fs::read_to_string(&absolute_file_path)?;

        // PHASE 1: Try database first
        if let Ok(symbol_name) =
            self.find_symbol_at_position(&absolute_file_path, &content, line, column)
        {
            // Generate consistent symbol UID for database lookup
            let symbol_uid = match self
                .generate_consistent_symbol_uid(
                    &absolute_file_path,
                    &symbol_name,
                    line,
                    column,
                    language.as_str(),
                    &workspace_root,
                    &content,
                )
                .await
            {
                Ok(uid) => uid,
                Err(e) => {
                    debug!("[UID] Failed to generate consistent UID, falling back to simple format: {}", e);
                    // Fallback to simple format if UID generation fails
                    format!(
                        "{}:{}:{}:{}",
                        absolute_file_path.to_string_lossy(),
                        symbol_name,
                        line,
                        column
                    )
                }
            };

            if let Ok(workspace_cache) = self
                .workspace_cache_router
                .cache_for_workspace(&workspace_root)
                .await
            {
                // Generate workspace-specific ID from workspace_root
                let workspace_id = self.generate_workspace_id_hash(&workspace_root);

                match workspace_cache
                    .get_call_hierarchy(workspace_id, &symbol_uid)
                    .await
                {
                    Ok(Some(result)) => {
                        info!(
                            "Database HIT for {} at {}:{}:{}",
                            symbol_name,
                            absolute_file_path.display(),
                            line,
                            column
                        );
                        return Ok(result);
                    }
                    Ok(None) => {
                        debug!("Database MISS for {} - calling LSP", symbol_name);
                    }
                    Err(e) => {
                        warn!("Database query error: {} - falling back to LSP", e);
                    }
                }
            }
        } else {
            debug!(
                "Could not resolve symbol at position {}:{}:{} - skipping database query",
                absolute_file_path.display(),
                line,
                column
            );
        }

        // PHASE 2: Database miss - proceed with LSP call
        info!(
            "Cache miss for {}:{}:{} - proceeding to LSP server",
            absolute_file_path.display(),
            line,
            column
        );

        // Ensure workspace is registered with the server for this language
        let server_instance = self
            .server_manager
            .ensure_workspace_registered(language, workspace_root.clone())
            .await?;

        // Adaptive timing for Go/TypeScript in CI environments
        let is_ci = std::env::var("PROBE_CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok();
        // New strategy: probe immediately, then back off. This removes unconditional sleeps
        // and avoids blowing up test budgets, especially in "initialization timeout" paths.
        let (initial_wait, max_attempts, retry_delay) = match language {
            Language::Go | Language::TypeScript | Language::JavaScript if is_ci => {
                (5, 5, 3) // was (15,5,5): faster in CI; still allows warm-up
            }
            Language::Go | Language::TypeScript | Language::JavaScript => (0, 3, 2),
            _ => (0, 3, 1),
        };

        debug!(
            "Using adaptive timing for {:?}: initial_wait={}s, max_attempts={}, retry_delay={}s (CI={})",
            language, initial_wait, max_attempts, retry_delay, is_ci
        );

        // Ensure document is opened and ready before querying call hierarchy
        // This is critical for rust-analyzer which returns null if the document isn't properly opened
        {
            let server = server_instance.lock().await;

            debug!(
                "Opening document for LSP analysis: {:?}",
                absolute_file_path
            );

            // Always re-open the document to ensure rust-analyzer has the latest content
            // rust-analyzer needs the file to be properly opened and processed before call hierarchy works
            server
                .server
                .open_document(&absolute_file_path, &content)
                .await?;

            // For rust-analyzer, give it time to process the file and establish context
            if language == Language::Rust {
                debug!(
                    "Allowing rust-analyzer time to process and index document: {:?}",
                    absolute_file_path
                );
                // Wait for rust-analyzer to index the file content and establish symbol context
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
            }
        }

        // Additional initial wait for complex language servers in CI environments
        if initial_wait > 0 {
            tokio::time::sleep(tokio::time::Duration::from_secs(initial_wait)).await;
        }

        // PHASE 2.5: Apply position adjustment based on LSP server requirements
        let (adjusted_line, adjusted_column) = {
            // Try to find the symbol at the position for position adjustment
            if let Ok(symbol_name) =
                self.find_symbol_at_position(&absolute_file_path, &content, line, column)
            {
                debug!("Found symbol '{}' at position {}:{}, applying LSP server-specific position adjustment", symbol_name, line, column);

                // Get language string for pattern lookup
                let language_str = match language {
                    Language::Rust => "rust",
                    Language::Go => "go",
                    Language::Python => "python",
                    Language::JavaScript => "javascript",
                    Language::TypeScript => "typescript",
                    _ => "unknown",
                };

                // Determine LSP server name based on language
                let lsp_server = match language {
                    Language::Rust => Some("rust-analyzer"),
                    Language::Go => Some("gopls"),
                    Language::Python => Some("pylsp"),
                    Language::JavaScript | Language::TypeScript => {
                        Some("typescript-language-server")
                    }
                    _ => None,
                };

                // Get position adjustment for this language/server combination
                let offset = self.get_position_offset(language_str, lsp_server);
                let symbol_len = symbol_name.len() as u32;
                let (new_line, new_column) = offset.apply(line, column, symbol_len);

                debug!(
                    "Position adjustment for {}/{:?}: {}:{} -> {}:{} ({})",
                    language_str,
                    lsp_server,
                    line,
                    column,
                    new_line,
                    new_column,
                    offset.description()
                );

                (new_line, new_column)
            } else {
                debug!(
                    "Could not find symbol at position {}:{}, using original position",
                    line, column
                );
                (line, column)
            }
        };

        // Try call hierarchy with adaptive retry logic
        let mut attempt = 1;
        let mut result = None;

        while attempt <= max_attempts {
            debug!(
                "Call hierarchy attempt {} at {}:{} (adjusted from {}:{})",
                attempt, adjusted_line, adjusted_column, line, column
            );

            // Lock the server instance only for the call hierarchy request
            let call_result = {
                let server = server_instance.lock().await;
                server
                    .server
                    .call_hierarchy(&absolute_file_path, adjusted_line, adjusted_column)
                    .await
            };

            match call_result {
                Ok(response) => {
                    // Check the response from call_hierarchy method (which has already processed the LSP response)
                    debug!(
                        "Call hierarchy response received for attempt {}: {:?}",
                        attempt, response
                    );

                    // Check if we have a valid item
                    let has_valid_item = if let Some(item) = response.get("item") {
                        if let Some(name) = item.get("name").and_then(|n| n.as_str()) {
                            if name != "unknown" && !name.is_empty() {
                                debug!("Found valid symbol '{}' in call hierarchy response", name);
                                true
                            } else {
                                debug!("Item has invalid name: '{}'", name);
                                false
                            }
                        } else {
                            debug!("Item missing name field");
                            false
                        }
                    } else {
                        debug!("Response missing item field - this indicates rust-analyzer returned null");
                        false
                    };

                    // Check for any incoming/outgoing calls
                    let has_call_data = response
                        .get("incoming")
                        .and_then(|v| v.as_array())
                        .is_some_and(|arr| !arr.is_empty())
                        || response
                            .get("outgoing")
                            .and_then(|v| v.as_array())
                            .is_some_and(|arr| !arr.is_empty());

                    if has_call_data {
                        debug!("Found call hierarchy data (incoming/outgoing calls)");
                    }

                    // Accept the result if we have either a valid item or call data
                    if has_valid_item || has_call_data {
                        result = Some(response);
                        break;
                    }

                    // For rust-analyzer, if we get a null response (no item), retry
                    if language == Language::Rust && !has_valid_item && attempt < max_attempts {
                        debug!("rust-analyzer returned null response - document may not be fully indexed yet, retrying...");
                        // Give rust-analyzer more time to process
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    }

                    result = Some(response); // Keep the last response even if empty
                }
                Err(e) => {
                    warn!(
                        "Call hierarchy request failed on attempt {}: {}",
                        attempt, e
                    );
                    if attempt == max_attempts {
                        return Err(e);
                    }
                }
            }

            attempt += 1;
            if attempt <= max_attempts {
                // Adaptive retry delay
                tokio::time::sleep(tokio::time::Duration::from_secs(retry_delay)).await;
            }
        }

        let result = result.ok_or_else(|| {
            anyhow!(
                "Failed to get call hierarchy response after {} attempts",
                max_attempts
            )
        })?;

        // Close document - lock server instance briefly
        {
            let server = server_instance.lock().await;
            server.server.close_document(&absolute_file_path).await?;
        }

        // Convert the result to our protocol type and update cache edges
        let protocol_result = parse_call_hierarchy_from_lsp(&result)?;

        // Always store in database, even for empty results (to create "none" edges)
        // The empty check is now handled inside store_call_hierarchy_in_database_enhanced
        {
            // For empty results, try to use the symbol we found at the position
            let symbol_name =
                if protocol_result.item.name == "unknown" || protocol_result.item.name.is_empty() {
                    // Try to find the symbol at the position for better naming
                    self.find_symbol_at_position(&absolute_file_path, &content, line, column)
                        .unwrap_or_else(|_| "unknown".to_string())
                } else {
                    protocol_result.item.name.clone()
                };

            let _node_id = NodeId::new(&symbol_name, absolute_file_path.clone());

            info!(
                "Processing call hierarchy for {}:{} (md5: {}, item.name: '{}')",
                absolute_file_path.display(),
                symbol_name,
                content_md5,
                protocol_result.item.name
            );

            // Extract edges from the result
            let _incoming_ids: Vec<NodeId> = protocol_result
                .incoming
                .iter()
                .map(|call| {
                    let file_path = PathBuf::from(&call.from.uri.replace("file://", ""));
                    NodeId::new(&call.from.name, file_path)
                })
                .collect();

            let _outgoing_ids: Vec<NodeId> = protocol_result
                .outgoing
                .iter()
                .map(|call| {
                    let file_path = PathBuf::from(&call.from.uri.replace("file://", ""));
                    NodeId::new(&call.from.name, file_path)
                })
                .collect();

            // NOTE: Graph-based edge invalidation is handled by universal cache automatically

            // Create cache key and store the result
            let _cache_key = NodeKey::new(
                &symbol_name,
                absolute_file_path.clone(),
                content_md5.clone(),
            );
            let _cache_info = self.convert_to_cache_info(&protocol_result);

            // Capture request position for index
            let _pos_file_for_index = absolute_file_path.clone();
            let _pos_md5_for_index = content_md5.clone();
            let _pos_line_for_index = line;
            let _pos_col_for_index = column;

            // NOTE: In universal cache system, caching is handled automatically by the cache layer.
            // The call hierarchy results are cached transparently when the handler method returns.
            debug!("Call hierarchy result will be cached automatically by universal cache layer");

            // MILESTONE 21: Store call hierarchy data in the database
            if let Err(e) = self
                .store_call_hierarchy_in_database_enhanced(
                    &protocol_result,
                    &absolute_file_path,
                    &workspace_root,
                    language.as_str(),
                    &symbol_name,
                    line,
                    column,
                )
                .await
            {
                error!(
                    "DATABASE_ERROR [call_hierarchy]: Failed to store call hierarchy in database for {} - {} | cause: {:?} | context: language={}, workspace={:?}",
                    absolute_file_path.display(),
                    e,
                    e.chain().collect::<Vec<_>>(),
                    format!("{:?}", language),
                    workspace_root
                );
                // Track database error metrics (Step 30.3) - TODO: Make async
                // self.metrics.increment_database_errors("call_hierarchy").await;
            }
        }

        Ok(protocol_result)
    }

    /// Convert protocol CallHierarchyResult to cache CallHierarchyInfo
    fn convert_to_cache_info(&self, result: &CallHierarchyResult) -> CallHierarchyInfo {
        let incoming_calls = result
            .incoming
            .iter()
            .map(|call| CallInfo {
                name: call.from.name.clone(),
                file_path: call.from.uri.replace("file://", ""),
                line: call.from.range.start.line,
                column: call.from.range.start.character,
                symbol_kind: call.from.kind.clone(),
            })
            .collect();

        let outgoing_calls = result
            .outgoing
            .iter()
            .map(|call| CallInfo {
                name: call.from.name.clone(),
                file_path: call.from.uri.replace("file://", ""),
                line: call.from.range.start.line,
                column: call.from.range.start.character,
                symbol_kind: call.from.kind.clone(),
            })
            .collect();

        CallHierarchyInfo {
            incoming_calls,
            outgoing_calls,
        }
    }

    /// Convert cache CallHierarchyInfo to protocol CallHierarchyResult
    #[allow(dead_code)]
    fn convert_from_cache_info(
        &self,
        info: &CallHierarchyInfo,
        item: CallHierarchyItem,
    ) -> CallHierarchyResult {
        use crate::protocol::{CallHierarchyCall, Position, Range};

        let incoming = info
            .incoming_calls
            .iter()
            .map(|call| CallHierarchyCall {
                from: CallHierarchyItem {
                    name: call.name.clone(),
                    kind: call.symbol_kind.clone(),
                    uri: format!("file://{}", call.file_path),
                    range: Range {
                        start: Position {
                            line: call.line,
                            character: call.column,
                        },
                        end: Position {
                            line: call.line,
                            character: call.column,
                        },
                    },
                    selection_range: Range {
                        start: Position {
                            line: call.line,
                            character: call.column,
                        },
                        end: Position {
                            line: call.line,
                            character: call.column,
                        },
                    },
                },
                from_ranges: vec![],
            })
            .collect();

        let outgoing = info
            .outgoing_calls
            .iter()
            .map(|call| CallHierarchyCall {
                from: CallHierarchyItem {
                    name: call.name.clone(),
                    kind: call.symbol_kind.clone(),
                    uri: format!("file://{}", call.file_path),
                    range: Range {
                        start: Position {
                            line: call.line,
                            character: call.column,
                        },
                        end: Position {
                            line: call.line,
                            character: call.column,
                        },
                    },
                    selection_range: Range {
                        start: Position {
                            line: call.line,
                            character: call.column,
                        },
                        end: Position {
                            line: call.line,
                            character: call.column,
                        },
                    },
                },
                from_ranges: vec![],
            })
            .collect();

        CallHierarchyResult {
            item,
            incoming,
            outgoing,
        }
    }

    /// Convert cached CallHierarchyInfo back into an LSP-like JSON envelope
    /// so we can reuse `parse_call_hierarchy_from_lsp(...)` and return the same protocol type.
    #[allow(dead_code)]
    fn cache_to_lsp_json(
        &self,
        file: &Path,
        symbol: &str,
        cached: &CallHierarchyInfo,
    ) -> serde_json::Value {
        use serde_json::json;

        // The parser expects: { item: { name, uri }, incoming: [...], outgoing: [...] }
        let file_uri = format!("file://{}", file.display());

        let incoming = cached
            .incoming_calls
            .iter()
            .map(|c| {
                json!({
                    "from": {
                        "name": c.name,
                        "uri": format!("file://{}", c.file_path),
                        "kind": c.symbol_kind,
                        "range": {
                            "start": {"line": c.line, "character": c.column},
                            "end": {"line": c.line, "character": c.column}
                        },
                        "selectionRange": {
                            "start": {"line": c.line, "character": c.column},
                            "end": {"line": c.line, "character": c.column}
                        }
                    },
                    "fromRanges": []
                })
            })
            .collect::<Vec<_>>();

        let outgoing = cached
            .outgoing_calls
            .iter()
            .map(|c| {
                json!({
                    "from": {
                        "name": c.name,
                        "uri": format!("file://{}", c.file_path),
                        "kind": c.symbol_kind,
                        "range": {
                            "start": {"line": c.line, "character": c.column},
                            "end": {"line": c.line, "character": c.column}
                        },
                        "selectionRange": {
                            "start": {"line": c.line, "character": c.column},
                            "end": {"line": c.line, "character": c.column}
                        }
                    },
                    "fromRanges": []
                })
            })
            .collect::<Vec<_>>();

        json!({
            "item": {
                "name": symbol,
                "uri": file_uri,
                "kind": "12", // Function kind
                "range": {
                    "start": {"line": 0, "character": 0},
                    "end": {"line": 0, "character": 0}
                },
                "selectionRange": {
                    "start": {"line": 0, "character": 0},
                    "end": {"line": 0, "character": 0}
                }
            },
            "incoming": incoming,
            "outgoing": outgoing
        })
    }

    // ========================================================================================
    // LSP Response Parsing Helper Functions
    // ========================================================================================

    /// Parse LSP definition response (JSON) into Vec<Location>
    fn parse_definition_response(response: &serde_json::Value) -> Result<Vec<Location>> {
        if let Some(locations) = response.as_array() {
            let mut result = Vec::new();
            for loc_value in locations {
                if let Ok(location) = serde_json::from_value::<Location>(loc_value.clone()) {
                    result.push(location);
                }
            }
            Ok(result)
        } else if let Ok(location) = serde_json::from_value::<Location>(response.clone()) {
            Ok(vec![location])
        } else if response.is_null() {
            Ok(Vec::new())
        } else {
            Err(anyhow!("Invalid definition response format: {}", response))
        }
    }

    /// Parse LSP references response (JSON) into Vec<Location>
    fn parse_references_response(response: &serde_json::Value) -> Result<Vec<Location>> {
        if let Some(locations) = response.as_array() {
            let mut result = Vec::new();
            for loc_value in locations {
                if let Ok(location) = serde_json::from_value::<Location>(loc_value.clone()) {
                    result.push(location);
                }
            }
            Ok(result)
        } else if response.is_null() {
            Ok(Vec::new())
        } else {
            Err(anyhow!("Invalid references response format: {}", response))
        }
    }

    /// Parse LSP hover response (JSON) into Option<HoverContent>
    fn parse_hover_response(response: &serde_json::Value) -> Result<Option<HoverContent>> {
        if response.is_null() {
            return Ok(None);
        }

        if let Ok(hover) = serde_json::from_value::<HoverContent>(response.clone()) {
            Ok(Some(hover))
        } else {
            // Try to parse basic hover format
            if let Some(contents) = response.get("contents") {
                let contents_str = if contents.is_string() {
                    contents.as_str().unwrap_or("").to_string()
                } else if contents.is_array() {
                    // Handle array of markup content
                    contents
                        .as_array()
                        .unwrap_or(&vec![])
                        .iter()
                        .map(|v| v.as_str().unwrap_or(""))
                        .collect::<Vec<_>>()
                        .join("\n")
                } else {
                    contents.to_string()
                };

                let range = response
                    .get("range")
                    .and_then(|r| serde_json::from_value::<crate::protocol::Range>(r.clone()).ok());

                Ok(Some(HoverContent {
                    contents: contents_str,
                    range,
                }))
            } else {
                Err(anyhow!("Invalid hover response format: {}", response))
            }
        }
    }

    /// Parse LSP implementation response (JSON) into Vec<Location>
    fn parse_implementation_response(response: &serde_json::Value) -> Result<Vec<Location>> {
        if let Some(locations) = response.as_array() {
            let mut result = Vec::new();
            for loc_value in locations {
                let location: Location = serde_json::from_value(loc_value.clone())
                    .context("Failed to parse implementation location")?;
                result.push(location);
            }
            Ok(result)
        } else if response.is_null() {
            Ok(Vec::new())
        } else {
            Err(anyhow!(
                "Invalid implementation response format: {}",
                response
            ))
        }
    }

    // ========================================================================================
    // New LSP Operation Handler Methods
    // ========================================================================================

    // Old handler methods removed - LSP requests now go through universal cache layer via handle_request_internal

    async fn handle_document_symbols(
        &self,
        _file_path: &Path,
        _workspace_hint: Option<PathBuf>,
    ) -> Result<Vec<DocumentSymbol>> {
        // TODO: Implement document symbols support in LSP server
        Err(anyhow!("Document symbols operation is not yet implemented"))
    }

    async fn handle_workspace_symbols(
        &self,
        _query: &str,
        _workspace_hint: Option<PathBuf>,
    ) -> Result<Vec<SymbolInformation>> {
        // TODO: Implement workspace symbols support in LSP server
        Err(anyhow!(
            "Workspace symbols operation is not yet implemented"
        ))
    }

    async fn handle_implementations(
        &self,
        file_path: &Path,
        line: u32,
        column: u32,
        workspace_hint: Option<PathBuf>,
    ) -> Result<Vec<Location>> {
        debug!(
            "handle_implementations called for {:?} at {}:{}",
            file_path, line, column
        );

        // Check if file should be excluded from LSP processing
        if should_exclude_from_lsp(file_path) {
            warn!(
                "Ignoring implementations request for excluded file: {:?} (build artifact/generated code)",
                file_path
            );
            return Ok(Vec::new());
        }

        // Handle implementations request directly (universal cache middleware handles caching)
        let absolute_file_path = safe_canonicalize(file_path);

        let language = self.detector.detect(&absolute_file_path)?;
        if language == Language::Unknown {
            return Err(anyhow!(
                "Unknown language for file: {:?}",
                absolute_file_path
            ));
        }

        let workspace_root = {
            let mut resolver = self.workspace_resolver.lock().await;
            resolver.resolve_workspace(&absolute_file_path, workspace_hint)?
        };

        // Read file content for symbol resolution
        let content = fs::read_to_string(&absolute_file_path)?;

        // PHASE 1: Try database first
        if let Ok(symbol_name) =
            self.find_symbol_at_position(&absolute_file_path, &content, line, column)
        {
            // Generate consistent symbol UID for database lookup
            let symbol_uid = match self
                .generate_consistent_symbol_uid(
                    &absolute_file_path,
                    &symbol_name,
                    line,
                    column,
                    language.as_str(),
                    &workspace_root,
                    &content,
                )
                .await
            {
                Ok(uid) => uid,
                Err(e) => {
                    debug!("[UID] Failed to generate consistent UID, falling back to simple format: {}", e);
                    // Fallback to simple format if UID generation fails
                    format!(
                        "{}:{}:{}:{}",
                        absolute_file_path.to_string_lossy(),
                        symbol_name,
                        line,
                        column
                    )
                }
            };

            if let Ok(workspace_cache) = self
                .workspace_cache_router
                .cache_for_workspace(&workspace_root)
                .await
            {
                // Generate workspace-specific ID from workspace_root
                let workspace_id = self.generate_workspace_id_hash(&workspace_root);

                match workspace_cache
                    .get_implementations(workspace_id, &symbol_uid)
                    .await
                {
                    Ok(Some(locations)) => {
                        info!(
                            "Database HIT for {} implementations at {}:{}:{}",
                            symbol_name,
                            absolute_file_path.display(),
                            line,
                            column
                        );
                        return Ok(locations);
                    }
                    Ok(None) => {
                        debug!(
                            "Database MISS for {} implementations - calling LSP",
                            symbol_name
                        );
                    }
                    Err(e) => {
                        warn!("Database query error: {} - falling back to LSP", e);
                    }
                }
            }
        } else {
            debug!(
                "Could not resolve symbol at position {}:{}:{} - skipping database query",
                absolute_file_path.display(),
                line,
                column
            );
        }

        // PHASE 2: Database miss - proceed with LSP call
        let server_instance = self
            .server_manager
            .ensure_workspace_registered(language, workspace_root.clone())
            .await?;

        // Make the implementation request directly without explicit document lifecycle
        // The LSP server manages its own document state
        let response_json = {
            let server = server_instance.lock().await;
            server
                .server
                .implementation(&absolute_file_path, line, column)
                .await?
        };

        // Check if response is null vs empty array
        let is_null_response = response_json.is_null();
        let locations = Self::parse_implementation_response(&response_json)?;

        // MILESTONE 21: Store implementations data in the database
        // Only store if we got a valid response (not null)
        // Empty array [] is valid and should create "none" edges
        if !is_null_response {
            if let Err(e) = self
                .store_implementations_in_database(
                    &locations,
                    &absolute_file_path,
                    &workspace_root,
                    language.as_str(),
                    line,
                    column,
                )
                .await
            {
                error!(
                    "DATABASE_ERROR [implementations]: Failed to store {} implementations in database for {}:{}:{} - {} | cause: {:?} | context: language={}, workspace={:?}",
                    locations.len(),
                    absolute_file_path.display(),
                    line,
                    column,
                    e,
                    e.chain().collect::<Vec<_>>(),
                    format!("{:?}", language),
                    workspace_root
                );
                // Track database error metrics (Step 30.3) - TODO: Make async
                // self.metrics.increment_database_errors("implementations").await;
            }
        } else {
            info!("LSP returned null for implementations at {}:{}:{} - not caching (LSP server may not be ready)",
                  absolute_file_path.display(), line, column);
        }

        Ok(locations)
    }

    async fn handle_type_definition(
        &self,
        _file_path: &Path,
        _line: u32,
        _column: u32,
        _workspace_hint: Option<PathBuf>,
    ) -> Result<Vec<Location>> {
        // TODO: Implement type definition support in LSP server
        Err(anyhow!("Type definition operation is not yet implemented"))
    }

    // ========================================================================================
    // Database Storage Methods for LSP Responses (Milestone 21)
    // ========================================================================================

    /// Store call hierarchy data in the database
    async fn store_call_hierarchy_in_database(
        &self,
        result: &CallHierarchyResult,
        request_file_path: &Path,
        workspace_root: &Path,
        language: &str,
    ) -> Result<()> {
        debug!(
            "Storing call hierarchy data in database for file: {:?}",
            request_file_path
        );

        // Create database adapter
        let adapter = LspDatabaseAdapter::new();

        // Get workspace cache
        let workspace_cache = self
            .workspace_cache_router
            .cache_for_workspace(workspace_root)
            .await
            .with_context(|| format!("Failed to get workspace cache for {:?}", workspace_root))?;

        match workspace_cache.backend() {
            BackendType::SQLite(db) => {
                // Store in database with proper cleanup
                adapter
                    .store_call_hierarchy_with_cleanup(
                        &**db,
                        result,
                        request_file_path,
                        language,
                        1, // Default file_version_id for now
                        workspace_root,
                    )
                    .await
                    .with_context(|| {
                        "Failed to store call hierarchy data with cleanup in database"
                    })?;

                info!(
                    "Successfully stored call hierarchy data: {} symbols and {} edges",
                    result.incoming.len() + result.outgoing.len() + 1, // +1 for main symbol
                    result.incoming.len() + result.outgoing.len()
                );
            }
        }

        Ok(())
    }

    /// Enhanced store call hierarchy with empty detection and "none" edges
    /// This method detects when LSP returns empty call hierarchy and creates "none" edges
    async fn store_call_hierarchy_in_database_enhanced(
        &self,
        result: &CallHierarchyResult,
        request_file_path: &Path,
        workspace_root: &Path,
        language: &str,
        symbol_name: &str,
        line: u32,
        column: u32,
    ) -> Result<()> {
        debug!(
            "Enhanced storing call hierarchy data in database for file: {:?}, symbol: {}",
            request_file_path, symbol_name
        );

        // Create database adapter
        let adapter = LspDatabaseAdapter::new();

        // Get workspace cache
        let workspace_cache = self
            .workspace_cache_router
            .cache_for_workspace(workspace_root)
            .await
            .with_context(|| format!("Failed to get workspace cache for {:?}", workspace_root))?;

        match workspace_cache.backend() {
            BackendType::SQLite(db) => {
                // Convert LSP response to database format
                let (symbols, edges) = adapter.convert_call_hierarchy_to_database(
                    result,
                    request_file_path,
                    language,
                    1,
                    workspace_root,
                )?;

                info!("[DEBUG] store_call_hierarchy_in_database_enhanced: symbols.len()={}, edges.len()={}, incoming.len()={}, outgoing.len()={}, item.name='{}'",
                     symbols.len(), edges.len(), result.incoming.len(), result.outgoing.len(), result.item.name);

                // Detect empty call hierarchy and create "none" edges if needed
                let edges_to_store = if edges.is_empty()
                    && result.incoming.is_empty()
                    && result.outgoing.is_empty()
                {
                    // LSP returned empty call hierarchy {incoming: [], outgoing: []} - create "none" edges
                    info!("LSP returned empty call hierarchy for symbol '{}', creating 'none' edges to cache empty state", symbol_name);

                    // Generate consistent symbol UID using actual line and column
                    let content = std::fs::read_to_string(request_file_path)?;
                    let symbol_uid = match self
                        .generate_consistent_symbol_uid(
                            request_file_path,
                            symbol_name,
                            line,
                            column,
                            language,
                            workspace_root,
                            &content,
                        )
                        .await
                    {
                        Ok(uid) => uid,
                        Err(e) => {
                            debug!(
                                "[UID] Failed to generate consistent UID, using fallback: {}",
                                e
                            );
                            format!(
                                "{}:{}:{}:{}",
                                request_file_path.to_string_lossy(),
                                symbol_name,
                                line,
                                column
                            )
                        }
                    };

                    let none_edges = crate::database::create_none_call_hierarchy_edges(&symbol_uid);
                    info!(
                        "Created {} 'none' edges for symbol_uid '{}': {:?}",
                        none_edges.len(),
                        symbol_uid,
                        none_edges
                    );
                    none_edges
                } else {
                    info!(
                        "LSP returned {} real call hierarchy edges for symbol '{}'",
                        edges.len(),
                        symbol_name
                    );
                    edges
                };

                // Store symbols and edges (including "none" edges for empty results)
                adapter
                    .store_in_database(&**db, symbols, edges_to_store)
                    .await
                    .with_context(|| "Failed to store call hierarchy data in database")?;

                let edge_count = if result.incoming.is_empty() && result.outgoing.is_empty() {
                    2 // Two "none" edges for empty call hierarchy
                } else {
                    result.incoming.len() + result.outgoing.len()
                };

                info!(
                    "Successfully stored call hierarchy data: {} symbols and {} edges",
                    result.incoming.len() + result.outgoing.len() + 1, // +1 for main symbol
                    edge_count
                );
            }
        }

        Ok(())
    }

    /// Store references data in the database
    async fn store_references_in_database(
        &self,
        locations: &[Location],
        request_file_path: &Path,
        workspace_root: &Path,
        language: &str,
        line: u32,
        column: u32,
    ) -> Result<()> {
        debug!(
            "Storing references data in database for file: {:?}",
            request_file_path
        );

        // Create database adapter
        let adapter = LspDatabaseAdapter::new();

        // Get workspace cache
        let workspace_cache = self
            .workspace_cache_router
            .cache_for_workspace(workspace_root)
            .await
            .with_context(|| format!("Failed to get workspace cache for {:?}", workspace_root))?;

        match workspace_cache.backend() {
            BackendType::SQLite(db) => {
                // Convert to database format
                let edges = adapter
                    .convert_references_to_database(
                        locations,
                        request_file_path,
                        (line, column),
                        language,
                        1, // Default file_version_id for now
                        workspace_root,
                    )
                    .await?;

                // ✅ Handle empty references case
                let edges_to_store = if edges.is_empty() && locations.is_empty() {
                    // LSP returned empty references [] - create "none" edges
                    let content = std::fs::read_to_string(request_file_path)?;
                    let symbol_name =
                        self.find_symbol_at_position(request_file_path, &content, line, column)?;
                    info!("LSP returned empty references for symbol '{}', creating 'none' edges to cache empty state", symbol_name);

                    // Generate consistent symbol UID
                    let symbol_uid = match self
                        .generate_consistent_symbol_uid(
                            request_file_path,
                            &symbol_name,
                            line,
                            column,
                            language,
                            workspace_root,
                            &content,
                        )
                        .await
                    {
                        Ok(uid) => uid,
                        Err(e) => {
                            debug!(
                                "[UID] Failed to generate consistent UID, using fallback: {}",
                                e
                            );
                            format!(
                                "{}:{}:{}:{}",
                                request_file_path.to_string_lossy(),
                                symbol_name,
                                line,
                                column
                            )
                        }
                    };

                    crate::database::create_none_reference_edges(&symbol_uid)
                } else {
                    info!("LSP returned {} real reference edges", edges.len());
                    edges
                };

                // Store in database (references only create edges, no new symbols)
                adapter
                    .store_in_database(&**db, Vec::new(), edges_to_store)
                    .await
                    .with_context(|| "Failed to store references edges in database")?;

                let edge_count = if locations.is_empty() {
                    1
                } else {
                    locations.len()
                };
                info!("Successfully stored references data: {} edges", edge_count);
            }
        }

        Ok(())
    }

    /// Store definitions data in the database
    async fn store_definitions_in_database(
        &self,
        locations: &[Location],
        request_file_path: &Path,
        workspace_root: &Path,
        language: &str,
        line: u32,
        column: u32,
    ) -> Result<()> {
        debug!(
            "Storing definitions data in database for file: {:?}",
            request_file_path
        );

        // Create database adapter
        let adapter = LspDatabaseAdapter::new();

        // Get workspace cache
        let workspace_cache = self
            .workspace_cache_router
            .cache_for_workspace(workspace_root)
            .await
            .with_context(|| format!("Failed to get workspace cache for {:?}", workspace_root))?;

        match workspace_cache.backend() {
            BackendType::SQLite(db) => {
                // Convert to database format
                let edges = adapter.convert_definitions_to_database(
                    locations,
                    request_file_path,
                    (line, column),
                    language,
                    1, // Default file_version_id for now
                    workspace_root,
                )?;

                // ✅ Handle empty definitions case
                let edges_to_store = if edges.is_empty() && locations.is_empty() {
                    // LSP returned empty definitions [] - create "none" edges
                    let content = std::fs::read_to_string(request_file_path)?;
                    let symbol_name =
                        self.find_symbol_at_position(request_file_path, &content, line, column)?;
                    info!("LSP returned empty definitions for symbol '{}', creating 'none' edges to cache empty state", symbol_name);

                    // Generate consistent symbol UID
                    let symbol_uid = match self
                        .generate_consistent_symbol_uid(
                            request_file_path,
                            &symbol_name,
                            line,
                            column,
                            language,
                            workspace_root,
                            &content,
                        )
                        .await
                    {
                        Ok(uid) => uid,
                        Err(e) => {
                            debug!(
                                "[UID] Failed to generate consistent UID, using fallback: {}",
                                e
                            );
                            format!(
                                "{}:{}:{}:{}",
                                request_file_path.to_string_lossy(),
                                symbol_name,
                                line,
                                column
                            )
                        }
                    };

                    crate::database::create_none_definition_edges(&symbol_uid)
                } else {
                    info!("LSP returned {} real definition edges", edges.len());
                    edges
                };

                // Store in database (definitions only create edges, no new symbols)
                adapter
                    .store_in_database(&**db, Vec::new(), edges_to_store)
                    .await
                    .with_context(|| "Failed to store definitions edges in database")?;

                let edge_count = if locations.is_empty() {
                    1
                } else {
                    locations.len()
                };
                info!("Successfully stored definitions data: {} edges", edge_count);
            }
        }

        Ok(())
    }

    /// Store implementations data in the database
    async fn store_implementations_in_database(
        &self,
        locations: &[Location],
        request_file_path: &Path,
        workspace_root: &Path,
        language: &str,
        line: u32,
        column: u32,
    ) -> Result<()> {
        debug!(
            "Storing implementations data in database for file: {:?}",
            request_file_path
        );

        // Create database adapter
        let adapter = LspDatabaseAdapter::new();

        // Get workspace cache
        let workspace_cache = self
            .workspace_cache_router
            .cache_for_workspace(workspace_root)
            .await
            .with_context(|| format!("Failed to get workspace cache for {:?}", workspace_root))?;

        match workspace_cache.backend() {
            BackendType::SQLite(db) => {
                // Convert to database format
                let edges = adapter.convert_implementations_to_database(
                    locations,
                    request_file_path,
                    (line, column),
                    language,
                    1, // Default file_version_id for now
                    workspace_root,
                )?;

                // ✅ Handle empty implementations case
                let edges_to_store = if edges.is_empty() && locations.is_empty() {
                    // LSP returned empty implementations [] - create "none" edges
                    let content = std::fs::read_to_string(request_file_path)?;
                    let symbol_name =
                        self.find_symbol_at_position(request_file_path, &content, line, column)?;
                    info!("LSP returned empty implementations for symbol '{}', creating 'none' edges to cache empty state", symbol_name);

                    // Generate consistent symbol UID
                    let symbol_uid = match self
                        .generate_consistent_symbol_uid(
                            request_file_path,
                            &symbol_name,
                            line,
                            column,
                            language,
                            workspace_root,
                            &content,
                        )
                        .await
                    {
                        Ok(uid) => uid,
                        Err(e) => {
                            debug!(
                                "[UID] Failed to generate consistent UID, using fallback: {}",
                                e
                            );
                            format!(
                                "{}:{}:{}:{}",
                                request_file_path.to_string_lossy(),
                                symbol_name,
                                line,
                                column
                            )
                        }
                    };

                    crate::database::create_none_implementation_edges(&symbol_uid)
                } else {
                    info!("LSP returned {} real implementation edges", edges.len());
                    edges
                };

                // Store in database (implementations only create edges, no new symbols)
                adapter
                    .store_in_database(&**db, Vec::new(), edges_to_store)
                    .await
                    .with_context(|| "Failed to store implementations edges in database")?;

                let edge_count = if locations.is_empty() {
                    1
                } else {
                    locations.len()
                };
                info!(
                    "Successfully stored implementations data: {} edges",
                    edge_count
                );
            }
        }

        Ok(())
    }

    // ========================================================================================
    // End of New LSP Operation Handler Methods
    // ========================================================================================

    async fn handle_initialize_workspace(
        &self,
        workspace_root: PathBuf,
        language_hint: Option<Language>,
    ) -> Result<(PathBuf, Language, String)> {
        // Validate workspace root exists
        if !workspace_root.exists() {
            return Err(anyhow!(
                "Workspace root does not exist: {:?}",
                workspace_root
            ));
        }

        // Canonicalize the workspace root to ensure it's an absolute path
        let canonical_root = safe_canonicalize(&workspace_root);

        // Check if workspace is allowed
        {
            let resolver = self.workspace_resolver.lock().await;
            if !resolver.is_path_allowed(&canonical_root) {
                return Err(anyhow!(
                    "Workspace {:?} not in allowed roots",
                    canonical_root
                ));
            }
        }

        // Determine language - use hint if provided, otherwise detect from workspace
        let language = if let Some(lang) = language_hint {
            lang
        } else {
            // Try to detect language from common files in workspace
            self.detect_workspace_language(&canonical_root)?
        };

        // Get LSP server config
        let config = self
            .registry
            .get(language)
            .ok_or_else(|| anyhow!("No LSP server configured for {:?}", language))?
            .clone();

        // Ensure workspace is registered with the server
        let _server_instance = self
            .server_manager
            .ensure_workspace_registered(language, canonical_root.clone())
            .await?;

        Ok((canonical_root, language, config.command))
    }

    async fn enable_watchdog(&self) {
        if self.watchdog_enabled.load(Ordering::Relaxed) {
            info!("Watchdog already enabled");
            return;
        }

        info!("Enabling watchdog monitoring");

        // Create and start the watchdog
        let watchdog = Watchdog::new(60);
        let shutdown_for_watchdog = self.shutdown.clone();

        // Set recovery callback
        watchdog
            .set_recovery_callback(move || {
                // Set shutdown flag when watchdog detects unresponsive daemon
                if let Ok(mut shutdown) = shutdown_for_watchdog.try_write() {
                    *shutdown = true;
                    error!("Watchdog triggered daemon shutdown due to unresponsiveness");
                }
            })
            .await;

        // Start watchdog monitoring
        let watchdog_task = watchdog.start();

        // Store the watchdog in the struct
        let mut watchdog_guard = self.watchdog.lock().await;
        *watchdog_guard = Some(watchdog);

        // Mark as enabled
        self.watchdog_enabled.store(true, Ordering::Relaxed);

        // Store the task handle
        let mut task_guard = self.watchdog_task.lock().await;
        *task_guard = Some(watchdog_task);

        info!("Watchdog monitoring enabled");
    }

    async fn handle_init_workspaces(
        &self,
        workspace_root: PathBuf,
        languages: Option<Vec<Language>>,
        recursive: bool,
    ) -> Result<(Vec<crate::protocol::InitializedWorkspace>, Vec<String>)> {
        use crate::protocol::InitializedWorkspace;

        // Validate workspace root exists
        if !workspace_root.exists() {
            return Err(anyhow!(
                "Workspace root does not exist: {:?}",
                workspace_root
            ));
        }

        // Canonicalize the workspace root to ensure it's an absolute path
        let canonical_root = safe_canonicalize(&workspace_root);

        // Discover workspaces - use WorkspaceResolver for single authoritative workspace
        // instead of recursive discovery which creates multiple separate workspaces
        let discovered_workspaces = if recursive {
            // Only use recursive discovery when explicitly requested
            let detector = crate::language_detector::LanguageDetector::new();
            detector.discover_workspaces(&canonical_root, recursive)?
        } else {
            // For non-recursive mode, check if current directory is a workspace root first
            let workspace_root = if crate::workspace_utils::is_workspace_root(&canonical_root) {
                tracing::info!(
                    "Current directory is workspace root: {}",
                    canonical_root.display()
                );
                canonical_root.clone()
            } else {
                // Create a dummy file path in the directory to use with find_workspace_root_with_fallback
                let dummy_file = canonical_root.join("dummy");
                let found_root =
                    crate::workspace_utils::find_workspace_root_with_fallback(&dummy_file)?;
                tracing::info!("Found workspace root: {}", found_root.display());
                found_root
            };

            let detector = crate::language_detector::LanguageDetector::new();

            // First try to detect workspace languages from markers (Cargo.toml, package.json, etc)
            let detected_languages = if let Some(languages) =
                detector.detect_workspace_languages(&workspace_root)?
            {
                tracing::info!("Detected workspace languages from markers: {:?}", languages);
                languages
            } else if let Some(languages) = detector.detect_languages_from_files(&workspace_root)? {
                tracing::info!("Detected languages from files: {:?}", languages);
                // Fall back to file extension detection if no workspace markers found
                languages
            } else {
                tracing::warn!("No languages detected from workspace markers or files");
                // No languages detected
                std::collections::HashSet::new()
            };

            if !detected_languages.is_empty() {
                tracing::info!(
                    "Creating workspace entry for {} with languages {:?}",
                    workspace_root.display(),
                    detected_languages
                );
                let mut result = std::collections::HashMap::new();
                result.insert(workspace_root, detected_languages);
                result
            } else {
                tracing::warn!("No detected languages, returning empty workspace map");
                std::collections::HashMap::new()
            }
        };

        if discovered_workspaces.is_empty() {
            return Ok((vec![], vec!["No workspaces found".to_string()]));
        }

        let mut initialized = Vec::new();
        let mut errors = Vec::new();

        // Filter by requested languages if specified
        for (workspace_path, detected_languages) in discovered_workspaces {
            // Canonicalize each workspace path to ensure it's absolute
            let canonical_workspace = safe_canonicalize(&workspace_path);

            let languages_to_init = if let Some(ref requested_languages) = languages {
                // Only initialize requested languages that were detected
                detected_languages
                    .intersection(&requested_languages.iter().copied().collect())
                    .copied()
                    .collect::<Vec<_>>()
            } else {
                // Initialize all detected languages
                detected_languages.into_iter().collect()
            };

            for language in languages_to_init {
                // Skip unknown language
                if language == Language::Unknown {
                    continue;
                }

                // Get LSP server config
                let config = match self.registry.get(language) {
                    Some(cfg) => cfg,
                    None => {
                        errors.push(format!(
                            "No LSP server configured for {language:?} in {canonical_workspace:?}"
                        ));
                        continue;
                    }
                };

                // Try to initialize the workspace
                match self
                    .server_manager
                    .ensure_workspace_registered(language, canonical_workspace.clone())
                    .await
                {
                    Ok(_) => {
                        // Ensure the workspace path is absolute before returning
                        let absolute_workspace = if canonical_workspace.is_absolute() {
                            canonical_workspace.clone()
                        } else {
                            let joined_path = std::env::current_dir()
                                .unwrap_or_else(|_| PathBuf::from("/"))
                                .join(&canonical_workspace);
                            safe_canonicalize(&joined_path)
                        };

                        initialized.push(InitializedWorkspace {
                            workspace_root: absolute_workspace,
                            language,
                            lsp_server: config.command.clone(),
                            status: "Ready".to_string(),
                        });
                        info!(
                            "Initialized {:?} for workspace {:?}",
                            language, canonical_workspace
                        );
                    }
                    Err(e) => {
                        errors.push(format!(
                            "Failed to initialize {language:?} for {canonical_workspace:?}: {e}"
                        ));
                    }
                }
            }
        }

        Ok((initialized, errors))
    }

    fn detect_workspace_language(&self, workspace_root: &Path) -> Result<Language> {
        // Look for common language markers in the workspace
        let markers = [
            ("go.mod", Language::Go),
            ("Cargo.toml", Language::Rust),
            ("package.json", Language::JavaScript),
            ("pyproject.toml", Language::Python),
            ("setup.py", Language::Python),
            ("pom.xml", Language::Java),
            ("build.gradle", Language::Java),
        ];

        for (marker, language) in &markers {
            if workspace_root.join(marker).exists() {
                return Ok(*language);
            }
        }

        Err(anyhow!(
            "Could not detect language for workspace: {:?}",
            workspace_root
        ))
    }

    async fn idle_checker(&self) {
        let idle_timeout = std::time::Duration::from_secs(86400); // 24 hours

        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;

            // Check if we should shutdown due to inactivity
            let now = Instant::now();
            let mut all_idle = true;

            for entry in self.connections.iter() {
                let last_activity = *entry.value();
                if now.duration_since(last_activity) < idle_timeout {
                    all_idle = false;
                    break;
                }
            }

            if all_idle && self.connections.is_empty() && self.start_time.elapsed() > idle_timeout {
                info!("Daemon idle for too long, shutting down");
                *self.shutdown.write().await = true;
                break;
            }
        }
    }

    async fn cleanup(&mut self) -> Result<()> {
        info!("Cleaning up daemon resources");

        // Abort/await background tasks to stop loops quickly.
        {
            let mut guard = self.background_tasks.lock().await;
            // Abort all in reverse order to stop dependents first
            while let Some(handle) = guard.pop() {
                handle.abort();
                // It's okay if awaiting returns an error due to abort
                let _ = handle.await;
            }
        }

        // Stop the watchdog if it was enabled
        if self.watchdog_enabled.load(Ordering::Relaxed) {
            info!("Stopping watchdog");
            if let Some(ref watchdog) = *self.watchdog.lock().await {
                watchdog.stop();
            }
        }

        // Shutdown all servers gracefully first, but don't block forever
        match tokio::time::timeout(Duration::from_secs(5), self.server_manager.shutdown_all()).await
        {
            Ok(_) => {
                debug!("Language servers shut down cleanly");
            }
            Err(_) => {
                warn!(
                    "Timed out waiting for language servers to shutdown; proceeding with forced cleanup"
                );
            }
        }

        // Small grace period
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Kill any remaining child processes directly
        let child_pids = self.child_processes.lock().await;
        #[cfg(unix)]
        for &pid in child_pids.iter() {
            unsafe {
                let _ = libc::kill(pid as i32, libc::SIGTERM);
                debug!("Sent SIGTERM to child process {}", pid);
            }
        }
        #[cfg(not(unix))]
        for &_pid in child_pids.iter() {
            // Windows: process cleanup handled differently
        }
        drop(child_pids);

        // Wait for children to go away; escalate if needed.
        #[cfg(unix)]
        {
            use std::time::Instant as StdInstant;
            fn pid_still_exists(pid: u32) -> bool {
                // kill(pid, 0) returns 0 if the process exists and we can send signals,
                // -1 with ESRCH if it doesn't exist.
                unsafe {
                    let res = libc::kill(pid as i32, 0);
                    if res == 0 {
                        true
                    } else {
                        #[cfg(target_os = "linux")]
                        let err = *libc::__errno_location();
                        #[cfg(target_os = "macos")]
                        let err = *libc::__error();
                        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
                        let err = 0;
                        err != libc::ESRCH
                    }
                }
            }

            let start = StdInstant::now();
            let soft_deadline = Duration::from_secs(2);
            let hard_deadline = Duration::from_secs(5);

            // soft wait
            loop {
                let pids_snapshot: Vec<u32> = {
                    let guard = self.child_processes.lock().await;
                    guard.clone()
                };
                let alive: Vec<u32> = pids_snapshot
                    .into_iter()
                    .filter(|&p| pid_still_exists(p))
                    .collect();
                if alive.is_empty() || start.elapsed() >= soft_deadline {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            // escalate to SIGKILL if anything is still alive
            let pids_snapshot: Vec<u32> = {
                let guard = self.child_processes.lock().await;
                guard.clone()
            };
            for pid in pids_snapshot.into_iter().filter(|&p| pid_still_exists(p)) {
                unsafe {
                    let _ = libc::kill(pid as i32, libc::SIGKILL);
                    warn!("Escalated to SIGKILL for stubborn child process {}", pid);
                }
            }

            // hard wait
            let hard_start = StdInstant::now();
            while hard_start.elapsed() < hard_deadline {
                let guard = self.child_processes.lock().await;
                if guard.iter().all(|&pid| !pid_still_exists(pid)) {
                    break;
                }
                drop(guard);
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }

        // Force kill any remaining processes in our process group
        #[cfg(unix)]
        self.process_group.kill_all();

        // Release PID lock
        if let Some(mut lock) = self.pid_lock.take() {
            lock.unlock()?;
        }

        // Remove socket file (Unix only)
        remove_socket_file(&self.socket_path)?;

        // Final cleanup of pid list
        *self.child_processes.lock().await = Vec::new();

        Ok(())
    }

    // Document synchronization methods removed - using database-first approach

    fn clone_refs(&self) -> Self {
        Self {
            socket_path: self.socket_path.clone(),
            registry: self.registry.clone(),
            detector: self.detector.clone(),
            server_manager: self.server_manager.clone(),
            workspace_resolver: self.workspace_resolver.clone(),
            connections: self.connections.clone(),
            connection_semaphore: self.connection_semaphore.clone(), // Share semaphore
            start_time: self.start_time,
            request_count: self.request_count.clone(),
            shutdown: self.shutdown.clone(),
            log_buffer: self.log_buffer.clone(),
            pid_lock: None, // Don't clone the PID lock
            #[cfg(unix)]
            process_group: ProcessGroup::new(), // Create new for cloned instance
            child_processes: self.child_processes.clone(), // Share child process tracking
            request_durations: self.request_durations.clone(),
            error_count: self.error_count.clone(),
            total_connections_accepted: self.total_connections_accepted.clone(),
            connections_cleaned_due_to_staleness: self.connections_cleaned_due_to_staleness.clone(),
            connections_rejected_due_to_limit: self.connections_rejected_due_to_limit.clone(),
            connection_durations: self.connection_durations.clone(),
            watchdog: self.watchdog.clone(),
            background_tasks: self.background_tasks.clone(),
            watchdog_enabled: self.watchdog_enabled.clone(),
            watchdog_task: self.watchdog_task.clone(),
            process_monitor: self.process_monitor.clone(),
            child_first_seen: self.child_first_seen.clone(),
            uid_generator: self.uid_generator.clone(),
            index_grace_secs: self.index_grace_secs,
            workspace_cache_router: self.workspace_cache_router.clone(),
            indexing_config: self.indexing_config.clone(),
            indexing_manager: self.indexing_manager.clone(),
            metrics: self.metrics.clone(),
            // Clone database health tracking fields
            database_errors: self.database_errors.clone(),
            last_database_error: self.last_database_error.clone(),
            database_health_status: self.database_health_status.clone(),
        }
    }

    // Note: Cache management is now handled by CacheManager

    /// Handle cache clear request
    #[allow(dead_code)]
    async fn handle_cache_clear(
        &self,
        operation: Option<LspOperation>,
    ) -> Result<(Vec<LspOperation>, usize)> {
        let mut operations_cleared = Vec::new();
        let mut total_entries_removed = 0;

        match operation {
            Some(op) => {
                // Clear specific cache
                match op {
                    LspOperation::CallHierarchy => {
                        // NOTE: Universal cache system handles clearing automatically
                        // Cache clearing is now done through workspace cache management
                        warn!("Individual cache clearing not supported in universal cache system. Use workspace cache management instead.");
                        operations_cleared.push(LspOperation::CallHierarchy);
                    }
                    LspOperation::Definition => {
                        // NOTE: Universal cache system handles clearing automatically
                        warn!("Individual cache clearing not supported in universal cache system. Use workspace cache management instead.");
                        operations_cleared.push(LspOperation::Definition);
                    }
                    LspOperation::References => {
                        // NOTE: Universal cache system handles clearing automatically
                        warn!("Individual cache clearing not supported in universal cache system. Use workspace cache management instead.");
                        operations_cleared.push(LspOperation::References);
                    }
                    LspOperation::Hover => {
                        // NOTE: Universal cache system handles clearing automatically
                        warn!("Individual cache clearing not supported in universal cache system. Use workspace cache management instead.");
                        operations_cleared.push(LspOperation::Hover);
                    }
                    LspOperation::DocumentSymbols => {
                        // Not implemented yet
                        return Err(anyhow!("DocumentSymbols cache not implemented"));
                    }
                }
            }
            None => {
                // Clear all caches - in universal cache system, this is handled by workspace clearing
                warn!("Global cache clearing not supported in universal cache system. Use workspace cache management instead.");

                // Instead, we can clear the universal cache layer (if needed)
                // self.universal_cache_layer.invalidate_all().await;

                // No entries actually removed in universal cache system
                total_entries_removed = 0;

                operations_cleared = vec![
                    LspOperation::CallHierarchy,
                    LspOperation::Definition,
                    LspOperation::References,
                    LspOperation::Hover,
                ];
            }
        }

        info!(
            "Cleared {} cache entries for operations: {:?}",
            total_entries_removed, operations_cleared
        );
        Ok((operations_cleared, total_entries_removed))
    }

    /// Handle cache export request
    #[allow(dead_code)]
    async fn handle_cache_export(&self, operation: Option<LspOperation>) -> Result<String> {
        match operation {
            Some(op) => {
                // Export specific cache
                match op {
                    LspOperation::CallHierarchy => {
                        Err(anyhow!("Cache export not supported in universal cache system. Use workspace cache management instead."))
                    }
                    LspOperation::Definition => {
                        Err(anyhow!("Cache export not supported in universal cache system. Use workspace cache management instead."))
                    }
                    LspOperation::References => {
                        Err(anyhow!("Cache export not supported in universal cache system. Use workspace cache management instead."))
                    }
                    LspOperation::Hover => {
                        Err(anyhow!("Cache export not supported in universal cache system. Use workspace cache management instead."))
                    }
                    LspOperation::DocumentSymbols => {
                        Err(anyhow!("DocumentSymbols cache not implemented"))
                    }
                }
            }
            None => {
                // Export all caches - not supported in universal cache system
                Err(anyhow!("Global cache export not supported in universal cache system. Use workspace cache management instead."))
            }
        }
    }

    // Indexing management methods
    async fn handle_start_indexing(
        &self,
        workspace_root: PathBuf,
        config: crate::protocol::IndexingConfig,
    ) -> Result<String> {
        use crate::indexing::manager::ManagerConfig;

        // Convert protocol config to internal manager config
        let manager_config = ManagerConfig {
            max_workers: config.max_workers.unwrap_or_else(|| num_cpus::get().max(2)),
            max_queue_size: 10000,
            exclude_patterns: config.exclude_patterns,
            include_patterns: config.include_patterns,
            max_file_size_bytes: config
                .max_file_size_mb
                .map(|mb| mb * 1024 * 1024)
                .unwrap_or(10 * 1024 * 1024),
            enabled_languages: config.languages,
            incremental_mode: config.incremental.unwrap_or(true),
            discovery_batch_size: 100,
            status_update_interval_secs: 5,
        };

        // Check if indexing manager is already running
        {
            let manager_guard = self.indexing_manager.lock().await;
            if manager_guard.is_some() {
                return Err(anyhow!("Indexing is already running"));
            }
        }

        // Create indexing manager using universal cache system
        // The IndexingManager will be adapted to work with the universal cache layer
        // by routing LSP operations through the universal_cache_layer.handle_request method
        info!(
            "Creating IndexingManager with universal cache integration for workspace: {:?}",
            workspace_root
        );

        // Create definition cache for IndexingManager
        let definition_cache = Arc::new(
            crate::lsp_cache::LspCache::new(
                crate::cache_types::LspOperation::Definition,
                crate::lsp_cache::LspCacheConfig::default(),
            )
            .await
            .map_err(|e| anyhow!("Failed to create definition cache: {}", e))?,
        );

        // Create the IndexingManager
        let indexing_manager = IndexingManager::new(
            manager_config,
            self.detector.clone(),
            self.server_manager.clone(),
            definition_cache,
            self.workspace_cache_router.clone(),
        );

        let session_id = uuid::Uuid::new_v4().to_string();

        // Store the indexing manager
        {
            let mut manager_guard = self.indexing_manager.lock().await;
            *manager_guard = Some(indexing_manager);
        }

        // Start indexing in background
        let indexing_manager_clone = self.indexing_manager.clone();
        let workspace_root_clone = workspace_root.clone();
        let session_id_clone = session_id.clone();

        tokio::spawn(async move {
            info!(
                "Starting background indexing for workspace: {:?} with session: {}",
                workspace_root_clone, session_id_clone
            );

            // Get the indexing manager and start indexing
            let manager_guard = indexing_manager_clone.lock().await;
            if let Some(manager) = manager_guard.as_ref() {
                info!(
                    "Starting file discovery and indexing for workspace: {:?}",
                    workspace_root_clone
                );

                // Actually start the indexing process!
                if let Err(e) = manager.start_indexing(workspace_root_clone.clone()).await {
                    error!(
                        "Failed to start indexing for workspace {:?}: {}",
                        workspace_root_clone, e
                    );
                } else {
                    info!(
                        "IndexingManager successfully started indexing for workspace: {:?}",
                        workspace_root_clone
                    );

                    // The indexing will work by:
                    // 1. Discovering files in the workspace
                    // 2. Using the existing server_manager to make LSP requests
                    // 3. These requests go through universal_cache_layer.handle_request
                    // 4. Results are automatically cached in the universal cache system
                    // This provides the same functionality as the original indexing design
                }
            } else {
                warn!("Failed to retrieve indexing manager for background task");
            }
        });

        info!(
            "Indexing started for workspace: {:?} with session ID: {}",
            workspace_root, session_id
        );
        Ok(session_id)
    }

    async fn handle_stop_indexing(&self, force: bool) -> Result<bool> {
        let mut manager_guard = self.indexing_manager.lock().await;
        if let Some(manager) = manager_guard.as_ref() {
            manager.stop_indexing().await?;
            // Always clear the manager when stopping, regardless of force flag
            // This allows starting a new indexing session
            *manager_guard = None;
            info!("Stopped indexing (force: {})", force);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn handle_indexing_status(&self) -> Result<crate::protocol::IndexingStatusInfo> {
        use crate::protocol::{IndexingProgressInfo, IndexingQueueInfo};

        let manager_guard = self.indexing_manager.lock().await;
        if let Some(manager) = manager_guard.as_ref() {
            let status = manager.get_status().await;
            let progress = manager.get_progress().await;

            let status_info = crate::protocol::IndexingStatusInfo {
                manager_status: format!("{status:?}"),
                progress: IndexingProgressInfo {
                    total_files: progress.total_files,
                    processed_files: progress.processed_files,
                    failed_files: progress.failed_files,
                    active_files: progress.active_files,
                    skipped_files: progress.skipped_files,
                    processed_bytes: progress.processed_bytes,
                    symbols_extracted: progress.symbols_extracted,
                    progress_ratio: if progress.total_files > 0 {
                        (progress.processed_files + progress.failed_files + progress.skipped_files)
                            as f64
                            / progress.total_files as f64
                    } else {
                        0.0
                    },
                    files_per_second: if progress.elapsed_seconds > 0 {
                        progress.processed_files as f64 / progress.elapsed_seconds as f64
                    } else {
                        0.0
                    },
                    bytes_per_second: if progress.elapsed_seconds > 0 {
                        progress.processed_bytes as f64 / progress.elapsed_seconds as f64
                    } else {
                        0.0
                    },
                },
                queue: IndexingQueueInfo {
                    total_items: 0,   // TODO: Get from queue
                    pending_items: 0, // TODO: Get from queue
                    high_priority_items: 0,
                    medium_priority_items: 0,
                    low_priority_items: 0,
                    is_paused: false,
                    memory_pressure: false,
                },
                workers: vec![], // TODO: Get worker info
                session_id: Some("current".to_string()),
                started_at: Some(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs()
                        .saturating_sub(progress.elapsed_seconds),
                ),
                elapsed_seconds: progress.elapsed_seconds,
                lsp_enrichment: manager.get_lsp_enrichment_info().await,
            };

            Ok(status_info)
        } else {
            // No indexing manager active
            let status_info = crate::protocol::IndexingStatusInfo {
                manager_status: "Idle".to_string(),
                progress: IndexingProgressInfo {
                    total_files: 0,
                    processed_files: 0,
                    failed_files: 0,
                    active_files: 0,
                    skipped_files: 0,
                    processed_bytes: 0,
                    symbols_extracted: 0,
                    progress_ratio: 0.0,
                    files_per_second: 0.0,
                    bytes_per_second: 0.0,
                },
                queue: IndexingQueueInfo {
                    total_items: 0,
                    pending_items: 0,
                    high_priority_items: 0,
                    medium_priority_items: 0,
                    low_priority_items: 0,
                    is_paused: false,
                    memory_pressure: false,
                },
                workers: vec![],
                session_id: None,
                started_at: None,
                elapsed_seconds: 0,
                lsp_enrichment: None,
            };

            Ok(status_info)
        }
    }

    async fn handle_set_indexing_config(
        &self,
        config: crate::protocol::IndexingConfig,
    ) -> Result<()> {
        // Convert protocol config to internal config using the proper conversion function
        let internal_config = crate::indexing::IndexingConfig::from_protocol_config(&config);

        // Update stored config
        *self.indexing_config.write().await = internal_config;

        info!("Updated indexing configuration");
        Ok(())
    }

    fn convert_internal_to_protocol_config(
        &self,
        config: &crate::indexing::IndexingConfig,
    ) -> crate::protocol::IndexingConfig {
        // Use the proper conversion function
        config.to_protocol_config()
    }

    /// Trigger auto-indexing for current workspace if enabled in configuration
    async fn trigger_auto_indexing(&self) {
        let config = self.indexing_config.read().await;

        // Check if auto_index is enabled
        if !config.enabled || !config.auto_index {
            debug!(
                "Auto-indexing is disabled (enabled: {}, auto_index: {})",
                config.enabled, config.auto_index
            );
            return;
        }

        // Find the current working directory or workspace root to index
        let workspace_root = match std::env::current_dir() {
            Ok(cwd) => {
                debug!("Using current directory as workspace root: {:?}", cwd);
                cwd
            }
            Err(e) => {
                warn!(
                    "Could not determine current directory for auto-indexing: {}",
                    e
                );
                return;
            }
        };

        // Check if there's already an indexing manager running
        {
            let manager_guard = self.indexing_manager.lock().await;
            if manager_guard.is_some() {
                debug!("Indexing manager already exists, skipping auto-indexing");
                return;
            }
        }

        info!("Starting auto-indexing for workspace: {:?}", workspace_root);

        // Convert internal config to protocol config for the indexing manager
        let protocol_config = config.to_protocol_config();

        // Start indexing in the background
        let daemon_ref = self.clone_refs();
        let workspace_path = workspace_root.clone();

        tokio::spawn(async move {
            if let Err(e) = daemon_ref
                .handle_start_indexing(workspace_path, protocol_config)
                .await
            {
                warn!("Auto-indexing failed: {}", e);
            } else {
                info!("Auto-indexing started successfully");
            }
        });
    }

    /// Start cache warming task in background
    #[allow(dead_code)]
    async fn start_cache_warming_task(&self) {
        // Check if cache warming is enabled
        let cache_warming_enabled = std::env::var("PROBE_CACHE_WARMING_ENABLED")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(true); // Default to enabled

        if !cache_warming_enabled {
            debug!("Cache warming is disabled via PROBE_CACHE_WARMING_ENABLED=false");
            return;
        }

        let concurrency = std::env::var("PROBE_CACHE_WARMING_CONCURRENCY")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(4); // Default to 4 concurrent operations

        info!("Starting cache warming task (concurrency: {})", concurrency);

        let daemon_ref = self.clone_refs();
        let cache_warming_handle = tokio::spawn(async move {
            daemon_ref
                .warm_cache_from_persistent_storage(concurrency)
                .await
        });

        // Add to background tasks for proper cleanup
        self.background_tasks
            .lock()
            .await
            .push(cache_warming_handle);
    }

    /// Warm the cache by loading previously cached entries from persistent storage
    /// No-op since universal cache layer was removed
    #[allow(dead_code)]
    async fn warm_cache_from_persistent_storage(&self, _concurrency: usize) {
        // No-op: Universal cache layer was removed, cache warming is no longer needed
        debug!("Cache warming skipped - universal cache layer removed");
    }

    /// Handle call hierarchy at commit request (stub - git functionality removed)
    async fn handle_call_hierarchy_at_commit(
        &self,
        file_path: &Path,
        _symbol: &str,
        line: u32,
        column: u32,
        _commit_hash: &str,
        workspace_hint: Option<PathBuf>,
    ) -> Result<(
        crate::protocol::CallHierarchyResult,
        crate::protocol::GitContext,
    )> {
        // Git functionality has been removed - fall back to current call hierarchy
        let result = self
            .handle_call_hierarchy(file_path, line, column, workspace_hint)
            .await?;

        // Return a stub git context
        let git_context = crate::protocol::GitContext {
            commit_hash: "unknown".to_string(),
            branch: "unknown".to_string(),
            is_dirty: false,
            remote_url: None,
            repo_root: std::env::current_dir().unwrap_or_default(),
        };

        Ok((result, git_context))
    }

    /// Handle get cache history request
    async fn handle_get_cache_history(
        &self,
        _file_path: &Path,
        _symbol: &str,
    ) -> Result<Vec<crate::protocol::CacheHistoryEntry>> {
        // NOTE: Cache history is not supported in universal cache system
        // The universal cache tracks statistics but not individual entry history
        warn!("Cache history not supported in universal cache system");
        Ok(Vec::new()) // Return empty history
    }

    // Database health tracking methods for Priority 4

    /// Record a database error and update health status
    async fn record_database_error(&self, error: &anyhow::Error) {
        let error_count = self.database_errors.fetch_add(1, Ordering::Relaxed) + 1;
        let error_msg = format!("{:#}", error);

        // Update last error
        *self.last_database_error.lock().await = Some(error_msg.clone());

        // Update health status
        *self.database_health_status.lock().await = DatabaseHealth::Degraded {
            error_count,
            last_error: error_msg.clone(),
        };

        // Log with structured data for monitoring
        error!(
            database_error_count = error_count,
            error_type = error.to_string(),
            "Database operation failed"
        );

        // Also increment metrics for backward compatibility
        self.metrics
            .increment_database_errors("database_operation")
            .await;
    }

    /// Get database health summary string for status responses
    async fn get_database_health_summary(&self) -> String {
        let health = self.database_health_status.lock().await;
        match &*health {
            DatabaseHealth::Healthy => "✅ Database operational".to_string(),
            DatabaseHealth::Degraded {
                error_count,
                last_error,
            } => {
                format!(
                    "⚠️ Database degraded ({} errors) - Last: {}",
                    error_count, last_error
                )
            }
            DatabaseHealth::Failed { error_message } => {
                format!("❌ Database failed - {}", error_message)
            }
        }
    }

    /// Check if there have been recent database errors
    async fn has_recent_database_errors(&self) -> bool {
        let error_count = self.database_errors.load(Ordering::Relaxed);
        error_count > 0
    }

    /// Mark database as completely failed (for critical errors)
    async fn mark_database_failed(&self, error_message: String) {
        *self.database_health_status.lock().await = DatabaseHealth::Failed {
            error_message: error_message.clone(),
        };

        error!(
            database_status = "failed",
            error_message = error_message,
            "Database marked as failed"
        );
    }

    /// Find what symbol is at a specific line/column position in a file
    /// This is used for persistent cache fallback when position index is empty after restart
    #[allow(dead_code)]
    fn find_symbol_at_position(
        &self,
        file_path: &Path,
        content: &str,
        line: u32,
        column: u32,
    ) -> Result<String> {
        debug!(
            "Looking for symbol at {}:{} in file: {:?}",
            line, column, file_path
        );

        // Use tree-sitter to find the actual symbol at the position
        let extension = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        // Try tree-sitter parsing for supported languages
        if let Some(tree) = self.parse_with_tree_sitter(content, extension) {
            // Find the symbol at the exact position using tree-sitter
            if let Some(symbol_name) = self.find_symbol_at_position_tree_sitter(
                tree.root_node(),
                content.as_bytes(),
                line,
                column,
            ) {
                debug!(
                    "Found symbol '{}' at position {}:{} using tree-sitter",
                    symbol_name, line, column
                );
                return Ok(symbol_name);
            }

            debug!(
                "No symbol found at position {}:{} using tree-sitter, falling back to regex",
                line, column
            );
        } else {
            debug!(
                "Tree-sitter parsing not available for extension '{}', using regex fallback",
                extension
            );
        }

        // Fallback to regex-based approach
        self.find_symbol_at_position_fallback(file_path, content, line, column)
    }

    /// Parse file with tree-sitter if supported language
    fn parse_with_tree_sitter(&self, content: &str, extension: &str) -> Option<tree_sitter::Tree> {
        use tree_sitter::Parser;

        let mut parser = Parser::new();

        let _language = match extension {
            "rs" => {
                parser
                    .set_language(&tree_sitter_rust::LANGUAGE.into())
                    .ok()?;
                Some(())
            }
            "ts" | "tsx" => {
                parser
                    .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
                    .ok()?;
                Some(())
            }
            "js" | "jsx" => {
                parser
                    .set_language(&tree_sitter_javascript::LANGUAGE.into())
                    .ok()?;
                Some(())
            }
            "py" => {
                parser
                    .set_language(&tree_sitter_python::LANGUAGE.into())
                    .ok()?;
                Some(())
            }
            "go" => {
                parser.set_language(&tree_sitter_go::LANGUAGE.into()).ok()?;
                Some(())
            }
            "java" => {
                parser
                    .set_language(&tree_sitter_java::LANGUAGE.into())
                    .ok()?;
                Some(())
            }
            "c" | "h" => {
                parser.set_language(&tree_sitter_c::LANGUAGE.into()).ok()?;
                Some(())
            }
            "cpp" | "cc" | "cxx" | "hpp" => {
                parser
                    .set_language(&tree_sitter_cpp::LANGUAGE.into())
                    .ok()?;
                Some(())
            }
            _ => None,
        }?;

        parser.parse(content.as_bytes(), None)
    }

    /// Find any symbol at the given position using tree-sitter (helper function)
    /// Simplified to let the LSP server handle all symbol semantics
    fn find_symbol_at_position_tree_sitter(
        &self,
        node: tree_sitter::Node,
        content: &[u8],
        target_line: u32,
        target_column: u32,
    ) -> Option<String> {
        // Check if this node contains the target position
        let start_pos = node.start_position();
        let end_pos = node.end_position();

        if target_line < start_pos.row as u32 || target_line > end_pos.row as u32 {
            return None;
        }

        if target_line == start_pos.row as u32 && target_column < start_pos.column as u32 {
            return None;
        }

        if target_line == end_pos.row as u32 && target_column > end_pos.column as u32 {
            return None;
        }

        // Check if this is any symbol node (function, struct, variable, etc.)
        let node_kind = node.kind();
        let is_symbol = match node_kind {
            // Rust
            "function_item" | "struct_item" | "enum_item" | "trait_item" | "union_item" => true,
            // JavaScript/TypeScript
            "function_declaration"
            | "method_definition"
            | "method_signature"
            | "arrow_function"
            | "function_expression"
            | "class_declaration"
            | "interface_declaration"
            | "type_alias_declaration" => true,
            // Python
            "function_definition" | "class_definition" | "method" => true,
            // Go
            "func_declaration" | "method_declaration" | "type_declaration" | "struct_type"
            | "interface_type" => true,
            // Java
            "constructor_declaration" | "enum_declaration" => true,
            _ => false,
        };

        if is_symbol {
            // Extract the symbol name from this node
            if let Some(name) = self.extract_symbol_name_from_node(node, content) {
                debug!(
                    "Found symbol '{}' of type '{}' at {}:{}-{}:{}",
                    name,
                    node_kind,
                    start_pos.row + 1,
                    start_pos.column + 1,
                    end_pos.row + 1,
                    end_pos.column + 1
                );
                return Some(name);
            }
        }

        // Recursively search child nodes
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(result) =
                self.find_symbol_at_position_tree_sitter(child, content, target_line, target_column)
            {
                return Some(result);
            }
        }

        None
    }

    /// Extract the name of any symbol from a tree-sitter node
    fn extract_symbol_name_from_node(
        &self,
        node: tree_sitter::Node,
        content: &[u8],
    ) -> Option<String> {
        // Look for identifier nodes within this callable node
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "identifier"
                | "field_identifier"
                | "type_identifier"
                | "property_identifier"
                | "function_declarator" => {
                    let name = child.utf8_text(content).unwrap_or("");
                    if !name.is_empty() {
                        return Some(name.to_string());
                    }
                }
                _ => {}
            }
        }

        None
    }

    /// Fallback regex-based symbol finding (original implementation)
    fn find_symbol_at_position_fallback(
        &self,
        file_path: &Path,
        content: &str,
        line: u32,
        column: u32,
    ) -> Result<String> {
        // Convert to 1-based line numbers for line lookup
        let target_line_1based = line + 1;
        let lines: Vec<&str> = content.lines().collect();

        if target_line_1based as usize > lines.len() {
            return Err(anyhow::anyhow!(
                "Line {} is beyond file length {} in {:?}",
                target_line_1based,
                lines.len(),
                file_path
            ));
        }

        // Get the line at the target position (convert back to 0-based)
        let target_line_content = lines[line as usize];

        debug!(
            "Looking for symbol at {}:{} in line: '{}' (fallback mode)",
            line, column, target_line_content
        );

        // Try to extract a symbol name from this line or nearby lines
        // Look for function definitions, method definitions, etc.

        // First, check if the current line or nearby lines contain function-like patterns
        let start_search = (line as usize).saturating_sub(5);
        let end_search = ((line as usize) + 5).min(lines.len());

        for (i, line) in lines.iter().enumerate().take(end_search).skip(start_search) {
            let line_content = line.trim();

            // Skip empty lines and comments
            if line_content.is_empty()
                || line_content.starts_with("//")
                || line_content.starts_with("///")
            {
                continue;
            }

            // Look for function/method/struct definitions
            if let Some(symbol) = self.extract_symbol_from_line(line_content, file_path) {
                debug!(
                    "Found symbol '{}' from line {}: '{}' (fallback mode)",
                    symbol,
                    i + 1,
                    line_content
                );
                return Ok(symbol);
            }
        }

        // Fallback: try to extract any identifier from the target line at the given position
        if let Some(symbol) = self.extract_identifier_at_position(target_line_content, column) {
            debug!(
                "Found identifier '{}' at position {}:{} in '{}' (fallback mode)",
                symbol, line, column, target_line_content
            );
            return Ok(symbol);
        }

        Err(anyhow::anyhow!(
            "Could not determine symbol at position {}:{} in {:?}",
            line,
            column,
            file_path
        ))
    }

    /// Extract a symbol name from a line of code (function, method, struct, etc.)
    #[allow(dead_code)]
    fn extract_symbol_from_line(&self, line: &str, file_path: &Path) -> Option<String> {
        let trimmed = line.trim();

        // Detect file extension for language-specific patterns
        let extension = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        match extension {
            "rs" => {
                // Rust patterns
                if let Some(caps) =
                    regex::Regex::new(r"\b(?:pub\s+)?(?:async\s+)?fn\s+([a-zA-Z_][a-zA-Z0-9_]*)")
                        .ok()?
                        .captures(trimmed)
                {
                    return caps.get(1).map(|m| m.as_str().to_string());
                }
                if let Some(caps) =
                    regex::Regex::new(r"\b(?:pub\s+)?struct\s+([a-zA-Z_][a-zA-Z0-9_]*)")
                        .ok()?
                        .captures(trimmed)
                {
                    return caps.get(1).map(|m| m.as_str().to_string());
                }
                if let Some(caps) =
                    regex::Regex::new(r"\bimpl\s+(?:.*\s+for\s+)?([a-zA-Z_][a-zA-Z0-9_]*)")
                        .ok()?
                        .captures(trimmed)
                {
                    return caps.get(1).map(|m| m.as_str().to_string());
                }
            }
            "js" | "ts" | "jsx" | "tsx" => {
                // JavaScript/TypeScript patterns
                if let Some(caps) = regex::Regex::new(r"\bfunction\s+([a-zA-Z_$][a-zA-Z0-9_$]*)")
                    .ok()?
                    .captures(trimmed)
                {
                    return caps.get(1).map(|m| m.as_str().to_string());
                }
                if let Some(caps) = regex::Regex::new(
                    r"\b(?:const|let|var)\s+([a-zA-Z_$][a-zA-Z0-9_$]*)\s*=\s*(?:function|async)",
                )
                .ok()?
                .captures(trimmed)
                {
                    return caps.get(1).map(|m| m.as_str().to_string());
                }
                if let Some(caps) = regex::Regex::new(r"\bclass\s+([a-zA-Z_$][a-zA-Z0-9_$]*)")
                    .ok()?
                    .captures(trimmed)
                {
                    return caps.get(1).map(|m| m.as_str().to_string());
                }
            }
            "py" => {
                // Python patterns
                if let Some(caps) = regex::Regex::new(r"\bdef\s+([a-zA-Z_][a-zA-Z0-9_]*)")
                    .ok()?
                    .captures(trimmed)
                {
                    return caps.get(1).map(|m| m.as_str().to_string());
                }
                if let Some(caps) = regex::Regex::new(r"\bclass\s+([a-zA-Z_][a-zA-Z0-9_]*)")
                    .ok()?
                    .captures(trimmed)
                {
                    return caps.get(1).map(|m| m.as_str().to_string());
                }
            }
            "go" => {
                // Go patterns
                if let Some(caps) = regex::Regex::new(r"\bfunc\s+([a-zA-Z_][a-zA-Z0-9_]*)")
                    .ok()?
                    .captures(trimmed)
                {
                    return caps.get(1).map(|m| m.as_str().to_string());
                }
            }
            _ => {
                // Generic patterns for other languages
                if let Some(caps) =
                    regex::Regex::new(r"\b(?:function|func|fn|def)\s+([a-zA-Z_][a-zA-Z0-9_]*)")
                        .ok()?
                        .captures(trimmed)
                {
                    return caps.get(1).map(|m| m.as_str().to_string());
                }
            }
        }

        None
    }

    /// Extract any identifier at a specific column position in a line
    #[allow(dead_code)]
    fn extract_identifier_at_position(&self, line: &str, column: u32) -> Option<String> {
        let chars: Vec<char> = line.chars().collect();
        let col_idx = column as usize;

        if col_idx >= chars.len() {
            return None;
        }

        // Find the start of the identifier (go backwards)
        let mut start = col_idx;
        while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
            start -= 1;
        }

        // Find the end of the identifier (go forwards)
        let mut end = col_idx;
        while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
            end += 1;
        }

        if start == end {
            return None;
        }

        let identifier: String = chars[start..end].iter().collect();

        // Only return valid identifiers (not empty, not just underscores, not all numbers)
        if !identifier.is_empty()
            && !identifier.chars().all(|c| c == '_')
            && !identifier.chars().all(|c| c.is_numeric())
            && (identifier.chars().next().unwrap().is_alphabetic() || identifier.starts_with('_'))
        {
            Some(identifier)
        } else {
            None
        }
    }

    /// Read database stats for cache stats (DEPRECATED - sled support removed)
    async fn read_sled_db_stats_for_cache_stats(
        &self,
        db_path: &std::path::Path,
    ) -> Result<(u64, u64, u64)> {
        // Calculate directory size
        let disk_size_bytes = self.calculate_directory_size_for_cache_stats(db_path).await;

        // Sled database reading is no longer supported
        warn!(
            "Sled database reading is deprecated. Database at {} cannot be read.",
            db_path.display()
        );

        // Return minimal stats based on file size
        Ok((
            if disk_size_bytes > 0 { 1 } else { 0 },
            disk_size_bytes,
            disk_size_bytes,
        ))
    }

    /// Calculate directory size for cache stats
    async fn calculate_directory_size_for_cache_stats(&self, dir_path: &std::path::Path) -> u64 {
        let mut total_size = 0u64;
        let mut dirs_to_process = vec![dir_path.to_path_buf()];

        while let Some(current_dir) = dirs_to_process.pop() {
            if let Ok(mut entries) = tokio::fs::read_dir(&current_dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    if let Ok(metadata) = entry.metadata().await {
                        if metadata.is_file() {
                            total_size += metadata.len();
                        } else if metadata.is_dir() {
                            dirs_to_process.push(entry.path());
                        }
                    }
                }
            }
        }

        total_size
    }

    /// Generate comprehensive cache statistics (universal cache removed - returns empty)
    async fn generate_comprehensive_cache_stats(
        &self,
    ) -> Result<(
        Vec<crate::protocol::WorkspaceCacheStats>,
        Vec<crate::protocol::OperationCacheStats>,
    )> {
        // Universal cache layer removed - return empty statistics
        info!("Cache statistics unavailable - universal cache layer removed");
        Ok((Vec::new(), Vec::new()))
    }

    /// Generate enhanced cache statistics by reading directly from disk
    /// This is a fallback when the universal cache list_keys functionality fails
    async fn generate_enhanced_disk_stats(
        &self,
    ) -> Result<(
        Vec<crate::protocol::WorkspaceCacheStats>,
        Vec<crate::protocol::OperationCacheStats>,
    )> {
        info!("Generating enhanced cache statistics by reading directly from disk");

        let mut global_operation_counts: std::collections::HashMap<String, (u64, u64)> =
            std::collections::HashMap::new();
        let mut workspace_stats: Vec<crate::protocol::WorkspaceCacheStats> = Vec::new();

        // Check workspace cache directories
        let base_cache_dir = if let Ok(cache_dir) = std::env::var("PROBE_LSP_WORKSPACE_CACHE_DIR") {
            std::path::PathBuf::from(cache_dir)
        } else {
            // Use default cache directory
            let home_dir = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
            std::path::PathBuf::from(home_dir).join("Library/Caches/probe/lsp/workspaces")
        };

        if !base_cache_dir.exists() {
            info!(
                "Workspace cache directory does not exist: {:?}",
                base_cache_dir
            );
            return Ok((Vec::new(), Vec::new()));
        }

        // Iterate through workspace cache directories
        if let Ok(entries) = std::fs::read_dir(&base_cache_dir) {
            for entry in entries.flatten() {
                let workspace_dir = entry.path();
                if !workspace_dir.is_dir() {
                    continue;
                }

                let workspace_name = workspace_dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();

                info!("Processing workspace directory: {:?}", workspace_dir);

                // Try to find the cache database
                let cache_db_path = workspace_dir.join("cache.db");
                if !cache_db_path.exists() {
                    info!("No cache.db found in workspace: {:?}", workspace_dir);
                    continue;
                }

                // Try to get basic stats from workspace router, but always try direct access for operation details
                let (entries, size, _disk_size) = match self
                    .read_stats_through_workspace_router(&workspace_name)
                    .await
                {
                    Ok((entries, size, disk_size, _per_op_stats)) => {
                        info!(
                            "Workspace {} (via router): {} entries, {} bytes",
                            workspace_name, entries, size
                        );
                        (entries, size, disk_size)
                    }
                    Err(_) => {
                        info!(
                            "Workspace {} not found in router, will use direct access only",
                            workspace_name
                        );
                        (0, 0, 0) // Will be overridden by direct access
                    }
                };

                // Always try direct database access for per-operation breakdown
                match self
                    .read_sled_db_stats_with_operations(&cache_db_path)
                    .await
                {
                    Ok((direct_entries, direct_size, _disk_size, per_op_stats)) => {
                        info!(
                            "Workspace {} (direct): {} entries, {} bytes, {} operations",
                            workspace_name,
                            direct_entries,
                            direct_size,
                            per_op_stats.len()
                        );

                        // Use router stats if available and higher, otherwise use direct stats
                        let final_entries = if entries > 0 { entries } else { direct_entries };
                        let final_size = if size > 0 { size } else { direct_size };

                        // Extract workspace path from workspace_id
                        let workspace_path = if let Some(underscore_pos) = workspace_name.find('_')
                        {
                            std::path::PathBuf::from(&workspace_name[underscore_pos + 1..])
                        } else {
                            std::path::PathBuf::from(&workspace_name)
                        };

                        // Convert operation stats to workspace format
                        let workspace_op_stats: Vec<crate::protocol::OperationCacheStats> =
                            per_op_stats
                                .iter()
                                .map(|op| {
                                    // Update global operation counts
                                    let global_entry = global_operation_counts
                                        .entry(op.operation.clone())
                                        .or_insert((0, 0));
                                    global_entry.0 += op.entries;
                                    global_entry.1 += op.size_bytes;

                                    crate::protocol::OperationCacheStats {
                                        operation: op.operation.clone(),
                                        entries: op.entries,
                                        size_bytes: op.size_bytes,
                                        hit_rate: op.hit_rate,
                                        miss_rate: op.miss_rate,
                                        avg_response_time_ms: op.avg_response_time_ms,
                                    }
                                })
                                .collect();

                        workspace_stats.push(crate::protocol::WorkspaceCacheStats {
                            workspace_id: workspace_name,
                            workspace_path,
                            entries: final_entries,
                            size_bytes: final_size,
                            hit_rate: 0.0, // Will be updated if we have hit/miss data
                            miss_rate: 0.0,
                            per_operation_stats: workspace_op_stats,
                        });
                    }
                    Err(e) => {
                        warn!("Failed to read cache stats from {:?}: {}", cache_db_path, e);

                        // If direct access failed but router succeeded, still create entry without per-operation stats
                        if entries > 0 {
                            let workspace_path =
                                if let Some(underscore_pos) = workspace_name.find('_') {
                                    std::path::PathBuf::from(&workspace_name[underscore_pos + 1..])
                                } else {
                                    std::path::PathBuf::from(&workspace_name)
                                };

                            workspace_stats.push(crate::protocol::WorkspaceCacheStats {
                                workspace_id: workspace_name,
                                workspace_path,
                                entries,
                                size_bytes: size,
                                hit_rate: 0.0,
                                miss_rate: 0.0,
                                per_operation_stats: Vec::new(),
                            });
                        }
                    }
                }
            }
        }

        // Generate global operation totals
        let per_operation_totals: Vec<crate::protocol::OperationCacheStats> =
            global_operation_counts
                .into_iter()
                .map(
                    |(operation, (entries, size_bytes))| crate::protocol::OperationCacheStats {
                        operation,
                        entries,
                        size_bytes,
                        hit_rate: 0.0, // Could be enhanced with actual hit/miss data
                        miss_rate: 0.0,
                        avg_response_time_ms: None,
                    },
                )
                .collect();

        info!(
            "Enhanced disk stats generated: {} workspaces, {} operations",
            workspace_stats.len(),
            per_operation_totals.len()
        );

        Ok((workspace_stats, per_operation_totals))
    }

    /// Read stats through workspace router to avoid lock conflicts
    async fn read_stats_through_workspace_router(
        &self,
        workspace_id: &str,
    ) -> Result<(u64, u64, u64, Vec<crate::protocol::OperationCacheStats>)> {
        // For now, let's try to extract workspace path from workspace_id and use direct access
        // This method could be enhanced to use the workspace router's existing connection
        let _workspace_path = if let Some(underscore_pos) = workspace_id.find('_') {
            std::path::PathBuf::from(&workspace_id[underscore_pos + 1..])
        } else {
            std::path::PathBuf::from(workspace_id)
        };

        // Try to get stats from workspace router
        let router_stats = self.workspace_cache_router.get_stats().await;

        // Find matching workspace in router stats
        for ws_stat in router_stats.workspace_stats {
            if ws_stat.workspace_id == workspace_id {
                if let Some(cache_stats) = ws_stat.cache_stats {
                    // Convert database cache stats to our expected format
                    return Ok((
                        cache_stats.total_entries,
                        cache_stats.total_size_bytes,
                        cache_stats.disk_size_bytes,
                        Vec::new(), // No per-operation breakdown available from router
                    ));
                }
            }
        }

        Err(anyhow::anyhow!("Workspace not found in router stats"))
    }

    /// Read database stats with per-operation breakdown (DEPRECATED - sled support removed)
    /// This is adapted from the client-side implementation
    async fn read_sled_db_stats_with_operations(
        &self,
        db_path: &std::path::Path,
    ) -> Result<(u64, u64, u64, Vec<crate::protocol::OperationCacheStats>)> {
        let disk_size_bytes = self.calculate_directory_size_for_cache_stats(db_path).await;

        warn!(
            "Sled database reading is deprecated. Database at {} cannot be read.",
            db_path.display()
        );

        Ok((0, disk_size_bytes, disk_size_bytes, Vec::new()))
    }

    /// Extract operation type from cache key
    #[allow(dead_code)]
    fn extract_operation_from_key(&self, key: &str) -> String {
        // Universal cache key format: workspace_id:operation:file:hash
        if key.contains(':') {
            let parts: Vec<&str> = key.split(':').collect();
            if parts.len() >= 2 {
                let op_part = parts[1];
                if op_part.starts_with("textDocument_") {
                    return op_part
                        .strip_prefix("textDocument_")
                        .unwrap_or(op_part)
                        .replace('_', " ");
                } else if op_part.starts_with("textDocument/") {
                    return op_part
                        .strip_prefix("textDocument/")
                        .unwrap_or(op_part)
                        .replace('/', " ");
                }
                return op_part.to_string();
            }
        }

        // Fallback patterns
        let operations = [
            ("prepareCallHierarchy", "call hierarchy"),
            ("call_hierarchy", "call hierarchy"),
            ("hover", "hover"),
            ("definition", "definition"),
            ("references", "references"),
            ("type_definition", "type definition"),
            ("implementations", "implementations"),
            ("document_symbols", "document symbols"),
            ("workspace_symbols", "workspace symbols"),
            ("completion", "completion"),
        ];

        for (pattern, name) in operations {
            if key.contains(pattern) {
                return name.to_string();
            }
        }

        "unknown".to_string()
    }

    /// Generate consistent UID for a symbol using SymbolUIDGenerator
    /// This ensures storage and retrieval use identical UIDs
    async fn generate_consistent_symbol_uid(
        &self,
        file_path: &Path,
        symbol_name: &str,
        line: u32,
        column: u32,
        _language: &str,
        workspace_root: &Path,
        file_content: &str,
    ) -> Result<String> {
        debug!(
            "[VERSION_AWARE_UID] Generating consistent UID for symbol '{}' at {}:{}:{}",
            symbol_name,
            file_path.display(),
            line,
            column
        );

        // Generate version-aware UID using the same helper as storage path
        let uid = generate_version_aware_uid(
            workspace_root,
            file_path,
            file_content,
            symbol_name,
            line, // LSP lines are already 1-indexed for definitions
        )
        .with_context(|| {
            format!(
                "Failed to generate version-aware UID for symbol: {}",
                symbol_name
            )
        })?;

        debug!(
            "[VERSION_AWARE_UID] Generated consistent UID for '{}': {}",
            symbol_name, uid
        );
        Ok(uid)
    }
}

fn find_daemon_binary() -> Result<PathBuf> {
    use crate::socket_path::normalize_executable;

    // Try to find lsp-daemon binary in various locations
    let daemon_name = normalize_executable("lsp-daemon");

    // 1. Check if it's in PATH
    if let Ok(path) = which::which(&daemon_name) {
        debug!("Found daemon in PATH: {:?}", path);
        return Ok(path);
    }

    // 2. Check in the same directory as current executable
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(parent) = current_exe.parent() {
            let daemon_path = parent.join(&daemon_name);
            if daemon_path.exists() {
                debug!("Found daemon in same directory: {:?}", daemon_path);
                return Ok(daemon_path);
            }
        }
    }

    // 3. Check target/debug directory (for development/testing)
    if let Ok(current_exe) = std::env::current_exe() {
        // Go up directories to find the workspace root and check target/debug
        let mut check_path = current_exe.parent();
        while let Some(path) = check_path {
            let target_debug = path.join("target").join("debug").join(&daemon_name);
            if target_debug.exists() {
                debug!("Found daemon in target/debug: {:?}", target_debug);
                return Ok(target_debug);
            }
            check_path = path.parent();
        }
    }

    // 4. Check common installation directories
    let common_paths = [
        "/usr/local/bin",
        "/usr/bin",
        "/opt/local/bin",
        "~/.cargo/bin",
    ];

    for path_str in &common_paths {
        let path = if path_str.starts_with("~") {
            if let Some(home) = dirs::home_dir() {
                home.join(&path_str[2..]).join(&daemon_name)
            } else {
                continue;
            }
        } else {
            PathBuf::from(path_str).join(&daemon_name)
        };

        if path.exists() {
            debug!("Found daemon in {}: {:?}", path_str, path);
            return Ok(path);
        }
    }

    Err(anyhow!(
        "Could not find lsp-daemon binary. Please ensure it's installed and in your PATH"
    ))
}

pub async fn start_daemon_background() -> Result<()> {
    // Allow tests or callers to override the socket explicitly
    let socket_path =
        std::env::var("PROBE_LSP_SOCKET_PATH").unwrap_or_else(|_| get_default_socket_path());

    // Check if daemon is already running by trying to connect
    if (crate::ipc::IpcStream::connect(&socket_path).await).is_ok() {
        debug!("Daemon already running");
        return Ok(());
    }

    // Clean up any stale socket
    remove_socket_file(&socket_path)?;

    // Fork daemon process - try multiple locations
    let daemon_binary = find_daemon_binary()?;

    debug!("Starting daemon binary: {:?}", daemon_binary);

    std::process::Command::new(&daemon_binary)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| anyhow!("Failed to spawn daemon: {}", e))?;

    info!("Started daemon in background");
    Ok(())
}

/// Check if a file path should be excluded from LSP processing
///
/// This filters out build artifacts, generated code, and temporary files that
/// shouldn't be processed by language servers as they can cause performance issues
/// and provide unhelpful results to users.
fn should_exclude_from_lsp(file_path: &Path) -> bool {
    let path_str = file_path.to_string_lossy().to_lowercase();

    // Exclude common build and generated code directories
    let excluded_patterns = [
        // Rust build artifacts
        "/target/debug/build/",
        "/target/release/build/",
        "/target/debug/deps/",
        "/target/release/deps/",
        // Generated binding files
        "bindgen.rs",
        "build.rs", // Build scripts themselves are fine, but their generated output isn't
        // Temporary and cache files
        "/.git/",
        "/tmp/",
        "/temp/",
        "/.cache/",
        // Node.js build artifacts
        "/node_modules/",
        "/dist/",
        "/.next/",
        // Other common build directories
        "/build/",
        "/out/",
        "/.output/",
        // IDE and editor files
        "/.vscode/",
        "/.idea/",
        "*.tmp",
        "*.bak",
        "*~",
    ];

    for pattern in &excluded_patterns {
        if path_str.contains(pattern) {
            return true;
        }
    }

    false
}
