use crate::cache_types::{CallHierarchyInfo, CallInfo, LspOperation, NodeId, NodeKey};
use crate::hash_utils::md5_hex_file;
use crate::indexing::{IndexingConfig, IndexingManager};
use crate::ipc::{IpcListener, IpcStream};
use crate::language_detector::{Language, LanguageDetector};
use crate::logging::{LogBuffer, MemoryLogLayer};
use crate::lsp_registry::LspRegistry;
use crate::path_safety::safe_canonicalize;
use crate::persistent_cache::{DatabaseBackendType, PersistentCacheConfig};
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
use crate::watchdog::{ProcessMonitor, Watchdog};
use crate::workspace_cache_router::WorkspaceCacheRouter;
use crate::workspace_resolver::WorkspaceResolver;
use anyhow::Context;
use anyhow::{anyhow, Result};
use dashmap::DashMap;
use futures;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{RwLock, Semaphore};
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

// Keep the PID lock file distinct from the Unix socket path to avoid collisions with stale sockets
// or removing the lock when cleaning up the socket file.
#[inline]
fn pid_lock_path(socket_path: &str) -> String {
    format!("{socket_path}.lock")
}

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
    index_grace_secs: u64,
    // Workspace-aware cache router for multi-workspace environments
    workspace_cache_router: Arc<WorkspaceCacheRouter>,
    // Indexing configuration and manager
    indexing_config: Arc<RwLock<IndexingConfig>>,
    indexing_manager: Arc<tokio::sync::Mutex<Option<IndexingManager>>>,
    // Universal cache layer for transparent LSP request caching
    universal_cache_layer: Arc<crate::universal_cache::CacheLayer>,
    // Document provider for tracking unsaved changes
    document_provider: Arc<crate::universal_cache::DaemonDocumentProvider>,
}

impl LspDaemon {
    pub fn new(socket_path: String) -> Result<Self> {
        Self::new_with_config(socket_path, None)
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
        let persistent_cache_config = PersistentCacheConfig {
            cache_directory: None,         // Will use default location
            max_size_bytes: 1_000_000_000, // 1GB
            ttl_days: 30,
            compress: true,
            memory_only: false,                      // Default to persistent
            backend_type: DatabaseBackendType::Sled, // Default to Sled backend
        };

        // Initialize workspace cache router for universal cache
        let workspace_cache_router_config =
            crate::workspace_cache_router::WorkspaceCacheRouterConfig {
                max_open_caches: std::env::var("PROBE_MAX_WORKSPACE_CACHES")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(8),
                max_parent_lookup_depth: 3,
                cache_config_template: persistent_cache_config.clone(),
                ..Default::default()
            };

        let workspace_cache_router = Arc::new(WorkspaceCacheRouter::new_with_workspace_resolver(
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

        // Create document provider for tracking unsaved changes
        let document_provider = Arc::new(crate::universal_cache::DaemonDocumentProvider::new(
            Some(workspace_resolver.clone()),
        ));

        // Create universal cache using the workspace router with shared workspace resolver
        let universal_cache = Arc::new(
            crate::universal_cache::UniversalCache::new_with_workspace_resolver(
                workspace_cache_router.clone(),
                Some(workspace_resolver.clone()),
            )
                .await
                .context("Failed to initialize universal cache")?,
        );

        // Configure cache layer with intelligent defaults
        let cache_layer_config = crate::universal_cache::CacheLayerConfig {
            cache_warming_enabled: std::env::var("PROBE_CACHE_WARMING_ENABLED")
                .map(|v| v == "true")
                .unwrap_or(true),
            cache_warming_concurrency: std::env::var("PROBE_CACHE_WARMING_CONCURRENCY")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(4),
            singleflight_timeout: Duration::from_secs(
                std::env::var("PROBE_SINGLEFLIGHT_TIMEOUT_SECS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(30),
            ),
            detailed_metrics: true,
            workspace_revision_ttl: Duration::from_secs(60),
        };

        // Create universal cache layer
        let universal_cache_layer = Arc::new(crate::universal_cache::CacheLayer::new(
            universal_cache,
            Some(document_provider.clone()),
            Some(cache_layer_config),
        ));

        info!("Universal cache layer initialized with intelligent caching middleware");

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
            index_grace_secs,
            workspace_cache_router,
            indexing_config,
            indexing_manager: Arc::new(tokio::sync::Mutex::new(None)),
            universal_cache_layer,
            document_provider,
        })
    }

    pub async fn run(mut self) -> Result<()> {
        // Acquire PID lock to ensure only one daemon runs
        // IMPORTANT: use a separate file from the Unix socket to avoid collisions with stale sockets.
        let lock_path = pid_lock_path(&self.socket_path);
        let mut pid_lock = PidLock::new(&lock_path);
        pid_lock
            .try_lock()
            .map_err(|e| anyhow!("Failed to acquire daemon lock: {}", e))?;
        self.pid_lock = Some(pid_lock);
        debug!(
            "Acquired daemon PID lock at {} (socket: {})",
            lock_path, self.socket_path
        );

        // Set up process group for child management
        #[cfg(unix)]
        self.process_group.become_leader()?;

        // Clean up any existing socket
        remove_socket_file(&self.socket_path)?;

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

    /// Handle request with universal cache layer middleware
    async fn handle_request(&self, request: DaemonRequest) -> DaemonResponse {
        // Clone self reference for the async closure
        let daemon_self = self.clone_refs();

        // Use the universal cache layer to handle the request transparently
        match self
            .universal_cache_layer
            .handle_request(request.clone(), move |req| async move {
                daemon_self.handle_request_internal(req).await
            })
            .await
        {
            Ok(response) => response,
            Err(e) => {
                error!("Cache layer error: {}", e);
                // Fall back to original implementation
                self.handle_request_internal(request).await
            }
        }
    }

    /// Internal request handler (original implementation)
    async fn handle_request_internal(&self, request: DaemonRequest) -> DaemonResponse {
        debug!(
            "Received daemon request: {:?}",
            std::mem::discriminant(&request)
        );

        // Track document-related operations for future document synchronization
        match &request {
            DaemonRequest::Definition { file_path, .. }
            | DaemonRequest::References { file_path, .. }
            | DaemonRequest::Hover { file_path, .. }
            | DaemonRequest::DocumentSymbols { file_path, .. } => {
                // Prepare file URI for document provider
                let uri = format!("file://{}", file_path.to_string_lossy());

                // Check if document is already tracked (for future document sync)
                let is_tracked = self.document_provider.is_document_open(&uri).await;
                debug!("Document {} tracking status: {}", uri, is_tracked);

                // TODO: In future, this is where we'd sync document content
                // if the file has unsaved changes in the editor
            }
            _ => {} // Non-document operations
        }

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

                // Get universal cache statistics
                let cache_stats = match self.universal_cache_layer.get_stats().await {
                    Ok(stats) => {
                        debug!("Universal cache stats: {:?} hit rate, {} active workspaces, warming enabled: {}", 
                               stats.cache_stats.hit_rate, stats.active_workspaces, stats.cache_warming_enabled);
                        Some(stats)
                    }
                    Err(e) => {
                        warn!("Failed to get universal cache statistics: {}", e);
                        None
                    }
                };

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

                // Log enhanced health check information including cache statistics
                if let Some(ref stats) = cache_stats {
                    info!(
                        "Health check: connections={} (accepted={}, cleaned={}, rejected={}), memory={}MB, errors={}%, avg_req_duration={}ms, avg_conn_duration={}ms, cache_hit_rate={:.1}%, active_workspaces={}, warming={}",
                        active_connections, total_accepted, total_cleaned, total_rejected, memory_usage_mb, error_rate, avg_request_duration_ms, avg_connection_duration_ms,
                        stats.cache_stats.hit_rate * 100.0, stats.active_workspaces, stats.cache_warming_enabled
                    );
                } else {
                    info!(
                        "Health check: connections={} (accepted={}, cleaned={}, rejected={}), memory={}MB, errors={}%, avg_req_duration={}ms, avg_conn_duration={}ms, cache_stats=unavailable",
                        active_connections, total_accepted, total_cleaned, total_rejected, memory_usage_mb, error_rate, avg_request_duration_ms, avg_connection_duration_ms
                    );
                }

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
                match self
                    .handle_call_hierarchy(&file_path, line, column, workspace_hint)
                    .await
                {
                    Ok(result) => DaemonResponse::CallHierarchy { request_id, result },
                    Err(e) => DaemonResponse::Error {
                        request_id,
                        error: e.to_string(),
                    },
                }
            }

            DaemonRequest::Status { request_id } => {
                let server_stats = self.server_manager.get_stats().await;

                let pool_status: Vec<PoolStatus> = server_stats
                    .into_iter()
                    .map(|s| {
                        // Consider a server "ready" if it's initialized (simplified without health monitoring)
                        let is_ready = s.initialized;

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
                        universal_cache_stats: {
                            // Get universal cache stats for the status response
                            match self.universal_cache_layer.get_stats().await {
                                Ok(layer_stats) => {
                                    // Convert cache layer stats to universal cache stats format
                                    match self
                                        .convert_cache_layer_stats_to_universal_cache_stats(
                                            layer_stats,
                                        )
                                        .await
                                    {
                                        Ok(universal_stats) => Some(universal_stats),
                                        Err(e) => {
                                            warn!("Failed to convert cache layer stats to universal cache stats: {}", e);
                                            Some(crate::universal_cache::monitoring::get_disabled_cache_stats())
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!("Failed to get universal cache stats for status: {}", e);
                                    Some(crate::universal_cache::monitoring::get_disabled_cache_stats())
                                }
                            }
                        },
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
                // Get workspace cache router stats from open caches first
                info!("Getting cache stats from workspace cache router (open caches)");
                let router_stats = self.workspace_cache_router.get_stats().await;
                info!(
                    "Workspace cache router stats: {} workspaces seen, {} current open caches",
                    router_stats.total_workspaces_seen, router_stats.current_open_caches
                );

                // Also scan the filesystem for all available workspace caches
                info!("Scanning filesystem for all workspace caches");
                let all_workspace_caches = match self
                    .workspace_cache_router
                    .list_all_workspace_caches()
                    .await
                {
                    Ok(caches) => {
                        info!("Found {} workspace caches on disk", caches.len());
                        caches
                    }
                    Err(e) => {
                        warn!("Failed to list workspace caches: {}", e);
                        Vec::new()
                    }
                };

                // Aggregate stats from all workspace caches (open + on-disk)
                let mut total_entries = 0u64;
                let mut total_size_bytes = 0u64;
                let mut total_disk_size_bytes = 0u64;
                let mut total_hits = 0u64;
                let mut total_misses = 0u64;

                // First, include stats from currently open caches
                for workspace_stats in &router_stats.workspace_stats {
                    info!(
                        "Processing open workspace: {}",
                        workspace_stats.workspace_id
                    );
                    if let Some(cache_stats) = &workspace_stats.cache_stats {
                        info!(
                            "Cache stats for open workspace {}: {} nodes, {} bytes disk",
                            workspace_stats.workspace_id,
                            cache_stats.total_nodes,
                            cache_stats.disk_size_bytes
                        );
                        total_entries += cache_stats.total_nodes;
                        total_size_bytes += cache_stats.total_size_bytes;
                        total_disk_size_bytes += cache_stats.disk_size_bytes;
                        total_hits += cache_stats.hit_count;
                        total_misses += cache_stats.miss_count;
                    }
                }

                // For on-disk caches that aren't currently open, read stats directly from sled
                for cache_entry in &all_workspace_caches {
                    // Check if this cache is already included in open caches
                    let already_counted = router_stats
                        .workspace_stats
                        .iter()
                        .any(|ws| ws.workspace_id == cache_entry.workspace_id);

                    if !already_counted {
                        info!("Processing disk workspace: {}", cache_entry.workspace_id);
                        let call_graph_db_path = cache_entry.cache_path.join("call_graph.db");

                        if call_graph_db_path.exists() && call_graph_db_path.is_dir() {
                            match self
                                .read_sled_db_stats_for_cache_stats(&call_graph_db_path)
                                .await
                            {
                                Ok((entries, size_bytes, disk_bytes)) => {
                                    info!("Disk cache stats for workspace {}: {} entries, {} bytes disk", cache_entry.workspace_id, entries, disk_bytes);
                                    total_entries += entries;
                                    total_size_bytes += size_bytes;
                                    total_disk_size_bytes += disk_bytes;
                                    // Note: disk-only caches don't have hit/miss stats in memory
                                }
                                Err(e) => {
                                    warn!(
                                        "Failed to read stats for disk cache {}: {}",
                                        cache_entry.workspace_id, e
                                    );
                                }
                            }
                        }
                    }
                }

                info!(
                    "Aggregated stats: entries={}, size_bytes={}, disk_bytes={}",
                    total_entries, total_size_bytes, total_disk_size_bytes
                );

                // Calculate hit/miss rates
                let total_requests = total_hits + total_misses;
                let hit_rate = if total_requests > 0 {
                    total_hits as f64 / total_requests as f64
                } else {
                    0.0
                };
                let miss_rate = 1.0 - hit_rate;

                let legacy_stats = crate::protocol::CacheStatistics {
                    hit_rate,
                    miss_rate,
                    total_entries,
                    total_size_bytes,
                    disk_size_bytes: total_disk_size_bytes,
                    entries_per_file: std::collections::HashMap::new(), // TODO: Could be populated from workspace summaries
                    entries_per_language: std::collections::HashMap::new(), // TODO: Could be populated from method stats
                    age_distribution: crate::protocol::AgeDistribution {
                        entries_last_hour: 0, // TODO: Would need timestamp tracking
                        entries_last_day: 0,
                        entries_last_week: 0,
                        entries_last_month: total_entries, // Assume all entries are recent
                        entries_older: 0,
                    },
                    most_accessed: Vec::new(), // TODO: Could be populated from access counts
                    memory_usage: crate::protocol::MemoryUsage {
                        in_memory_cache_bytes: 0, // Workspace cache router doesn't track in-memory size
                        persistent_cache_bytes: total_disk_size_bytes,
                        metadata_bytes: total_disk_size_bytes / 100, // Rough estimate (1% metadata)
                        index_bytes: total_disk_size_bytes / 50,     // Rough estimate (2% index)
                    },
                };

                info!(
                    "Returning legacy stats: entries={}, disk_size={}",
                    legacy_stats.total_entries, legacy_stats.disk_size_bytes
                );

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
                        .clear_workspace_cache(None)
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
                } else if let Some(file_path) = file_path {
                    // Clear cache for a specific file
                    match self.universal_cache_layer.invalidate_file(&file_path).await {
                        Ok(entries_removed) => {
                            let legacy_result = crate::protocol::ClearResult {
                                entries_removed: entries_removed as u64,
                                files_affected: 1,
                                branches_affected: 0, // Not applicable to universal cache
                                commits_affected: 0,  // Not applicable to universal cache
                                bytes_reclaimed: 0,   // Size not tracked at this level
                                duration_ms: 0,       // Not tracked
                            };
                            DaemonResponse::CacheCleared {
                                request_id,
                                result: legacy_result,
                            }
                        }
                        Err(e) => DaemonResponse::Error {
                            request_id,
                            error: format!("Failed to clear cache for file {file_path:?}: {e}"),
                        },
                    }
                } else {
                    // No specific clear target - not supported in universal cache
                    DaemonResponse::Error {
                        request_id,
                        error: "Universal cache system requires either 'all=true' or a specific file path to clear".to_string(),
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
                clean_expired: _clean_expired,
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
            } => {
                info!(
                    "Workspace cache clear requested for: {:?}",
                    workspace_path
                        .as_deref()
                        .unwrap_or("all workspaces".as_ref())
                );

                match self
                    .workspace_cache_router
                    .clear_workspace_cache(workspace_path)
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
                match self
                    .handle_definition(&file_path, line, column, workspace_hint)
                    .await
                {
                    Ok(locations) => DaemonResponse::Definition {
                        request_id,
                        locations,
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
                match self
                    .handle_references(
                        &file_path,
                        line,
                        column,
                        include_declaration,
                        workspace_hint,
                    )
                    .await
                {
                    Ok(locations) => DaemonResponse::References {
                        request_id,
                        locations,
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
                match self
                    .handle_hover(&file_path, line, column, workspace_hint)
                    .await
                {
                    Ok(content) => DaemonResponse::Hover {
                        request_id,
                        content,
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
        }
    }

    /// Handle clearing cache for a specific symbol
    async fn handle_clear_symbol_cache(
        &self,
        file_path: &Path,
        symbol_name: &str,
        line: Option<u32>,
        column: Option<u32>,
        methods: Option<Vec<String>>,
        all_positions: bool,
    ) -> Result<crate::protocol::SymbolCacheClearResult> {
        let start_time = std::time::Instant::now();

        // Clear the symbol cache through the universal cache layer
        let (entries_cleared, positions_cleared, methods_cleared, size_freed) = self
            .universal_cache_layer
            .clear_symbol(
                file_path,
                symbol_name,
                line,
                column,
                methods.clone(),
                all_positions,
            )
            .await?;

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

    /// Convert cache layer stats to universal cache stats format
    async fn convert_cache_layer_stats_to_universal_cache_stats(
        &self,
        layer_stats: crate::universal_cache::CacheLayerStats,
    ) -> Result<crate::protocol::UniversalCacheStats> {
        // Get base cache stats from the cache layer
        let cache_stats = &layer_stats.cache_stats;

        // Convert method stats to protocol format
        let method_stats: std::collections::HashMap<
            String,
            crate::protocol::UniversalCacheMethodStats,
        > = cache_stats
            .method_stats
            .iter()
            .map(|(method, stats)| {
                let total_ops = stats.hits + stats.misses;
                let hit_rate = if total_ops > 0 {
                    stats.hits as f64 / total_ops as f64
                } else {
                    0.0
                };

                let protocol_stats = crate::protocol::UniversalCacheMethodStats {
                    method: method.as_str().to_string(),
                    enabled: true, // Would check actual policy
                    entries: stats.entries,
                    size_bytes: stats.size_bytes,
                    hits: stats.hits,
                    misses: stats.misses,
                    hit_rate,
                    avg_cache_response_time_us: 100, // Placeholder - would track actual timing
                    avg_lsp_response_time_us: 5000,  // Placeholder - would track actual timing
                    ttl_seconds: Some(3600),         // Placeholder - would get from policy
                };

                (method.as_str().to_string(), protocol_stats)
            })
            .collect();

        // Create layer stats with realistic data
        let layer_stats_protocol = crate::protocol::UniversalCacheLayerStats {
            memory: crate::protocol::CacheLayerStat {
                enabled: true,
                entries: cache_stats.total_entries / 10, // Assume 10% is in memory
                size_bytes: cache_stats.total_size_bytes / 10,
                hits: cache_stats
                    .method_stats
                    .values()
                    .map(|s| s.hits)
                    .sum::<u64>()
                    * 9
                    / 10,
                misses: cache_stats
                    .method_stats
                    .values()
                    .map(|s| s.misses)
                    .sum::<u64>()
                    / 10,
                hit_rate: 0.9,            // Memory layer should have higher hit rate
                avg_response_time_us: 10, // Memory is fast
                max_capacity: Some(10 * 1024 * 1024), // 10MB memory cache
                capacity_utilization: (cache_stats.total_size_bytes / 10) as f64
                    / (10.0 * 1024.0 * 1024.0),
            },
            disk: crate::protocol::CacheLayerStat {
                enabled: true,
                entries: cache_stats.total_entries * 9 / 10, // Most entries are on disk
                size_bytes: cache_stats.total_size_bytes * 9 / 10,
                hits: cache_stats
                    .method_stats
                    .values()
                    .map(|s| s.hits)
                    .sum::<u64>()
                    / 10,
                misses: cache_stats
                    .method_stats
                    .values()
                    .map(|s| s.misses)
                    .sum::<u64>()
                    * 9
                    / 10,
                hit_rate: 0.1, // Disk layer has lower hit rate (most misses are disk misses)
                avg_response_time_us: 1000, // Disk is slower (1ms)
                max_capacity: Some(1024 * 1024 * 1024), // 1GB disk cache
                capacity_utilization: (cache_stats.total_size_bytes * 9 / 10) as f64
                    / (1024.0 * 1024.0 * 1024.0),
            },
            server: None, // No remote server layer yet
        };

        // Get workspace summaries from the workspace router
        let workspace_summaries = if let Ok(workspace_info_list) = self
            .workspace_cache_router
            .get_workspace_cache_info(None)
            .await
        {
            workspace_info_list
                .into_iter()
                .map(|info| {
                    crate::protocol::UniversalCacheWorkspaceSummary {
                        workspace_id: info.workspace_id,
                        workspace_root: info.workspace_root,
                        entries: info.files_indexed,
                        size_bytes: info.size_bytes,
                        hits: 100, // Would need to get from actual stats
                        misses: 10,
                        hit_rate: 0.91,
                        last_accessed: info.last_accessed,
                        languages: info.languages,
                    }
                })
                .collect()
        } else {
            Vec::new()
        };

        // Calculate totals
        let total_operations = cache_stats
            .method_stats
            .values()
            .map(|s| s.hits + s.misses)
            .sum::<u64>();

        let total_hits = cache_stats
            .method_stats
            .values()
            .map(|s| s.hits)
            .sum::<u64>();

        let total_misses = cache_stats
            .method_stats
            .values()
            .map(|s| s.misses)
            .sum::<u64>();

        let hit_rate = if total_operations > 0 {
            total_hits as f64 / total_operations as f64
        } else {
            0.0
        };

        let miss_rate = if total_operations > 0 {
            total_misses as f64 / total_operations as f64
        } else {
            0.0
        };

        // Configuration summary
        let config_summary = crate::protocol::UniversalCacheConfigSummary {
            gradual_migration_enabled: false,
            rollback_enabled: false,
            memory_config: crate::protocol::CacheLayerConfigSummary {
                enabled: true,
                max_size_mb: Some(10),
                max_entries: Some(10000),
                eviction_policy: Some("lru".to_string()),
                compression: Some(false),
            },
            disk_config: crate::protocol::CacheLayerConfigSummary {
                enabled: true,
                max_size_mb: Some(1024),
                max_entries: Some(1000000),
                eviction_policy: Some("lru".to_string()),
                compression: Some(true),
            },
            server_config: None,
            custom_method_configs: method_stats.len(),
        };

        Ok(crate::protocol::UniversalCacheStats {
            enabled: true,
            total_entries: cache_stats.total_entries,
            total_size_bytes: cache_stats.total_size_bytes,
            active_workspaces: layer_stats.active_workspaces,
            hit_rate,
            miss_rate,
            total_hits,
            total_misses,
            method_stats,
            layer_stats: layer_stats_protocol,
            workspace_summaries,
            config_summary,
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

        // NOTE: Position-based cache lookup is now handled transparently by universal cache layer

        // NOTE: Old CallGraph cache fallback has been removed.
        // The universal cache system handles all caching through the cache layer middleware.
        // If we reach this point, it means the cache truly missed and we need to query the LSP server.

        info!(
            "Cache miss for {}:{}:{} - proceeding to LSP server",
            absolute_file_path.display(),
            line,
            column
        );

        // The following old fallback logic has been intentionally removed to ensure
        // only the universal cache system is used:
        // - Symbol discovery at position
        // - NodeKey creation for CallGraph cache
        // - Workspace persistent cache lookup
        // All caching is now handled by the universal cache layer.

        {
            info!(
                "Could not determine symbol at position {}:{}:{} for persistent cache fallback",
                absolute_file_path.display(),
                line,
                column
            );
        }

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

        // Ensure workspace is registered with the server for this language
        let server_instance = self
            .server_manager
            .ensure_workspace_registered(language, workspace_root.clone())
            .await?;

        // Read file content
        let content = fs::read_to_string(&absolute_file_path)?;

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

        // Try call hierarchy with adaptive retry logic
        let mut attempt = 1;
        let mut result = None;

        while attempt <= max_attempts {
            debug!("Call hierarchy attempt {} at {}:{}", attempt, line, column);

            // Lock the server instance only for the call hierarchy request
            let call_result = {
                let server = server_instance.lock().await;
                server
                    .server
                    .call_hierarchy(&absolute_file_path, line, column)
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

        // Now that we have the result, extract the symbol name and cache it
        if !protocol_result.item.name.is_empty() && protocol_result.item.name != "unknown" {
            let symbol_name = protocol_result.item.name.clone();
            let _node_id = NodeId::new(&symbol_name, absolute_file_path.clone());

            info!(
                "Caching call hierarchy for {}:{} (md5: {})",
                absolute_file_path.display(),
                symbol_name,
                content_md5
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

    // ========================================================================================
    // New LSP Operation Handler Methods
    // ========================================================================================

    async fn handle_definition(
        &self,
        file_path: &Path,
        line: u32,
        column: u32,
        workspace_hint: Option<PathBuf>,
    ) -> Result<Vec<Location>> {
        let absolute_file_path = safe_canonicalize(file_path);

        // Call LSP server directly (caching handled by universal cache middleware)
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
            .definition(&absolute_file_path, line, column)
            .await?;

        let locations = Self::parse_definition_response(&response_json)?;
        Ok(locations)
    }

    async fn handle_references(
        &self,
        file_path: &Path,
        line: u32,
        column: u32,
        include_declaration: bool,
        workspace_hint: Option<PathBuf>,
    ) -> Result<Vec<Location>> {
        let absolute_file_path = safe_canonicalize(file_path);

        // Call LSP server directly (caching handled by universal cache middleware)
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
            .references(&absolute_file_path, line, column, include_declaration)
            .await?;

        let locations = Self::parse_references_response(&response_json)?;
        Ok(locations)
    }

    async fn handle_hover(
        &self,
        file_path: &Path,
        line: u32,
        column: u32,
        workspace_hint: Option<PathBuf>,
    ) -> Result<Option<HoverContent>> {
        let absolute_file_path = safe_canonicalize(file_path);

        // Call LSP server directly (caching handled by universal cache middleware)
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
        _file_path: &Path,
        _line: u32,
        _column: u32,
        _workspace_hint: Option<PathBuf>,
    ) -> Result<Vec<Location>> {
        // TODO: Implement implementations support in LSP server
        Err(anyhow!("Implementations operation is not yet implemented"))
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

        // Discover workspaces
        let detector = crate::language_detector::LanguageDetector::new();
        let discovered_workspaces = detector.discover_workspaces(&canonical_root, recursive)?;

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

    /// Helper method to synchronize document state with the universal cache
    /// This method prepares for future LSP document synchronization integration
    pub async fn sync_document_state(
        &self,
        file_uri: &str,
        content: Option<String>,
        version: u64,
    ) -> Result<()> {
        // Update document provider state
        if let Some(content) = content {
            if self.document_provider.is_document_open(file_uri).await {
                // Document already open, this is a change
                self.document_provider
                    .document_changed(file_uri, content, version)
                    .await;
                debug!("Updated document state for: {}", file_uri);
            } else {
                // New document
                self.document_provider
                    .document_opened(file_uri, content, version)
                    .await;
                debug!("Opened new document: {}", file_uri);
            }
        } else {
            // Document saved (content from disk)
            self.document_provider
                .document_saved(file_uri, None, version)
                .await;
            debug!("Saved document: {}", file_uri);
        }

        // Invalidate relevant cache entries when document changes
        if let Some(path_str) = file_uri.strip_prefix("file://") {
            let path = std::path::Path::new(path_str);

            // Invalidate universal cache for this file
            if let Err(e) = self.universal_cache_layer.invalidate_file(path).await {
                warn!(
                    "Failed to invalidate universal cache for file {}: {}",
                    file_uri, e
                );
            }

            // Legacy cache invalidation removed - handled by universal cache

            debug!("Invalidated caches for document: {}", file_uri);
        }

        Ok(())
    }

    /// Helper method to close a document and clean up state
    pub async fn close_document(&self, file_uri: &str) -> Result<()> {
        self.document_provider.document_closed(file_uri).await;
        debug!("Closed document: {}", file_uri);
        Ok(())
    }

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
            index_grace_secs: self.index_grace_secs,
            workspace_cache_router: self.workspace_cache_router.clone(),
            indexing_config: self.indexing_config.clone(),
            indexing_manager: self.indexing_manager.clone(),
            universal_cache_layer: self.universal_cache_layer.clone(),
            document_provider: self.document_provider.clone(),
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
            memory_budget_bytes: config
                .memory_budget_mb
                .map(|mb| mb * 1024 * 1024)
                .unwrap_or(512 * 1024 * 1024),
            memory_pressure_threshold: 0.8,
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
            self.universal_cache_layer.clone(),
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
        let _universal_cache_layer = self.universal_cache_layer.clone();
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
                    memory_usage_bytes: progress.memory_usage_bytes,
                    peak_memory_bytes: progress.peak_memory_bytes,
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
                    memory_usage_bytes: 0,
                    peak_memory_bytes: 0,
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
    #[allow(dead_code)]
    async fn warm_cache_from_persistent_storage(&self, concurrency: usize) {
        let start_time = std::time::Instant::now();
        info!("Starting cache warming from persistent storage...");

        // Get all workspace cache instances and warm them up
        let workspace_cache_router = &self.workspace_cache_router;

        // Use a semaphore to limit concurrent cache warming operations
        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(concurrency));
        let mut warming_tasks = Vec::new();

        // Get all open caches from the workspace router
        let open_caches = workspace_cache_router.get_all_open_caches().await;

        if open_caches.is_empty() {
            debug!("No open caches found for warming - will warm on first access");
            return;
        }

        info!("Found {} workspace cache(s) to warm", open_caches.len());

        for (workspace_id, persistent_cache) in open_caches {
            let semaphore_clone = semaphore.clone();
            let universal_cache = self.universal_cache_layer.clone();
            let workspace_id_clone = workspace_id.clone();

            let task = tokio::spawn(async move {
                let _permit = semaphore_clone.acquire().await.unwrap();

                match persistent_cache.iter_nodes().await {
                    Ok(nodes) => {
                        let node_count = nodes.len();
                        if node_count == 0 {
                            debug!(
                                "No cached nodes found in workspace cache: {}",
                                workspace_id_clone
                            );
                            return;
                        }

                        debug!(
                            "Warming cache for workspace {} with {} nodes",
                            workspace_id_clone, node_count
                        );
                        let mut loaded_count = 0;
                        let mut error_count = 0;

                        for (key, persisted_node) in nodes {
                            // Build the universal cache key for call hierarchy
                            let method = crate::universal_cache::LspMethod::CallHierarchy;
                            let params = format!("{}:{}", key.symbol, key.content_md5);

                            // Pre-load into universal cache using the set method
                            match universal_cache
                                .get_universal_cache()
                                .set(method, &key.file, &params, &persisted_node.info)
                                .await
                            {
                                Ok(_) => {
                                    loaded_count += 1;
                                    if loaded_count % 50 == 0 {
                                        debug!(
                                            "Cache warming progress for {}: {}/{} nodes loaded",
                                            workspace_id_clone, loaded_count, node_count
                                        );
                                    }
                                }
                                Err(e) => {
                                    error_count += 1;
                                    if error_count < 5 {
                                        // Only log first few errors to avoid spam
                                        warn!(
                                            "Failed to warm cache entry {}:{}: {}",
                                            key.file.display(),
                                            key.symbol,
                                            e
                                        );
                                    }
                                }
                            }
                        }

                        info!(
                            "Cache warming completed for workspace {}: loaded {} nodes ({} errors)",
                            workspace_id_clone, loaded_count, error_count
                        );
                    }
                    Err(e) => {
                        warn!(
                            "Failed to iterate nodes for cache warming in workspace {}: {}",
                            workspace_id_clone, e
                        );
                    }
                }
            });

            warming_tasks.push(task);
        }

        // Wait for all warming tasks to complete
        let results = futures::future::join_all(warming_tasks).await;
        let completed_count = results.iter().filter(|r| r.is_ok()).count();
        let failed_count = results.len() - completed_count;

        let elapsed = start_time.elapsed();
        if failed_count > 0 {
            warn!(
                "Cache warming completed in {:?}: {} workspace(s) succeeded, {} failed",
                elapsed, completed_count, failed_count
            );
        } else {
            info!(
                "Cache warming completed successfully in {:?} for {} workspace(s)",
                elapsed, completed_count
            );
        }
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
            "Looking for symbol at {}:{} in line: '{}'",
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
                    "Found symbol '{}' from line {}: '{}'",
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
                "Found identifier '{}' at position {}:{} in '{}'",
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

    /// Read sled database stats for cache stats (similar to management.rs but for daemon use)
    async fn read_sled_db_stats_for_cache_stats(
        &self,
        db_path: &std::path::Path,
    ) -> Result<(u64, u64, u64)> {
        // Calculate directory size
        let disk_size_bytes = self.calculate_directory_size_for_cache_stats(db_path).await;

        // Try to open the sled database for reading
        match sled::Config::default()
            .path(db_path)
            .cache_capacity(1024 * 1024)
            .open()
        {
            Ok(db) => {
                let mut entries = 0u64;
                let mut size_bytes = 0u64;

                match db.open_tree("nodes") {
                    Ok(nodes_tree) => {
                        entries = nodes_tree.len() as u64;

                        // Sample some entries to estimate size
                        let mut sample_count = 0;
                        let mut sample_total_size = 0;

                        for (key, value) in nodes_tree.iter().take(100).filter_map(Result::ok) {
                            sample_count += 1;
                            sample_total_size += key.len() + value.len();
                        }

                        if sample_count > 0 {
                            let avg_entry_size = sample_total_size / sample_count;
                            size_bytes = entries * avg_entry_size as u64;
                        }
                    }
                    Err(e) => {
                        warn!("Failed to open nodes tree for {}: {}", db_path.display(), e);
                    }
                }

                Ok((entries, size_bytes, disk_size_bytes))
            }
            Err(e) => {
                warn!(
                    "Failed to open sled database at {}: {}",
                    db_path.display(),
                    e
                );
                // Return minimal stats based on file size
                Ok((
                    if disk_size_bytes > 0 { 1 } else { 0 },
                    disk_size_bytes,
                    disk_size_bytes,
                ))
            }
        }
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
