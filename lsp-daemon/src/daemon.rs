use crate::ipc::{IpcListener, IpcStream};
use crate::language_detector::{Language, LanguageDetector};
use crate::lsp_registry::LspRegistry;
use crate::pool::PoolManager;
use crate::protocol::{
    parse_call_hierarchy_from_lsp, CallHierarchyResult, DaemonRequest, DaemonResponse,
    DaemonStatus, LanguageInfo, MessageCodec, PoolStatus,
};
use crate::socket_path::{get_default_socket_path, remove_socket_file};
use anyhow::{anyhow, Result};
use dashmap::DashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::RwLock;
use tracing::{debug, error, info};
use uuid::Uuid;

pub struct LspDaemon {
    socket_path: String,
    registry: Arc<LspRegistry>,
    detector: Arc<LanguageDetector>,
    pool_manager: Arc<PoolManager>,
    connections: Arc<DashMap<Uuid, Instant>>,
    start_time: Instant,
    request_count: Arc<RwLock<u64>>,
    shutdown: Arc<RwLock<bool>>,
}

impl LspDaemon {
    pub fn new(socket_path: String) -> Result<Self> {
        let registry = Arc::new(LspRegistry::new()?);
        let detector = Arc::new(LanguageDetector::new());
        let pool_manager = Arc::new(PoolManager::new());

        Ok(Self {
            socket_path,
            registry,
            detector,
            pool_manager,
            connections: Arc::new(DashMap::new()),
            start_time: Instant::now(),
            request_count: Arc::new(RwLock::new(0)),
            shutdown: Arc::new(RwLock::new(false)),
        })
    }

    pub async fn run(&self) -> Result<()> {
        // Clean up any existing socket
        remove_socket_file(&self.socket_path)?;

        let listener = IpcListener::bind(&self.socket_path).await?;
        info!("LSP daemon listening on {}", self.socket_path);

        // Start idle checker
        let daemon = self.clone_refs();
        tokio::spawn(async move {
            daemon.idle_checker().await;
        });

        loop {
            // Check shutdown flag
            if *self.shutdown.read().await {
                info!("Daemon shutting down...");
                break;
            }

            match listener.accept().await {
                Ok(stream) => {
                    let daemon = self.clone_refs();
                    tokio::spawn(async move {
                        if let Err(e) = daemon.handle_client(stream).await {
                            error!("Error handling client: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("Error accepting connection: {}", e);
                }
            }
        }

        // Cleanup
        self.cleanup().await?;
        Ok(())
    }

    async fn handle_client(&self, mut stream: IpcStream) -> Result<()> {
        let client_id = Uuid::new_v4();
        info!("New client connected: {}", client_id);

        // Store connection timestamp
        self.connections.insert(client_id, Instant::now());

        let mut buffer = vec![0; 65536]; // 64KB buffer

        loop {
            // Read message length
            let n = stream.read(&mut buffer[..4]).await?;
            if n == 0 {
                break; // Connection closed
            }

            let msg_len = u32::from_be_bytes([buffer[0], buffer[1], buffer[2], buffer[3]]) as usize;

            // Read message body
            if msg_len > buffer.len() - 4 {
                buffer.resize(msg_len + 4, 0);
            }

            stream.read_exact(&mut buffer[4..4 + msg_len]).await?;

            // Decode request
            let request = MessageCodec::decode_request(&buffer[..4 + msg_len])?;

            // Update activity timestamp
            self.connections.insert(client_id, Instant::now());

            // Increment request count
            *self.request_count.write().await += 1;

            // Handle request
            let response = self.handle_request(request).await;

            // Send response
            let encoded = MessageCodec::encode_response(&response)?;
            stream.write_all(&encoded).await?;
            stream.flush().await?;

            // Check if shutdown was requested
            if matches!(response, DaemonResponse::Shutdown { .. }) {
                *self.shutdown.write().await = true;
                break;
            }
        }

        // Remove connection
        self.connections.remove(&client_id);
        info!("Client disconnected: {}", client_id);

        Ok(())
    }

    async fn handle_request(&self, request: DaemonRequest) -> DaemonResponse {
        match request {
            DaemonRequest::Connect { client_id } => DaemonResponse::Connected {
                request_id: client_id,
                daemon_version: env!("CARGO_PKG_VERSION").to_string(),
            },

            DaemonRequest::CallHierarchy {
                request_id,
                file_path,
                pattern,
            } => match self.handle_call_hierarchy(&file_path, &pattern).await {
                Ok(result) => DaemonResponse::CallHierarchy { request_id, result },
                Err(e) => DaemonResponse::Error {
                    request_id,
                    error: e.to_string(),
                },
            },

            DaemonRequest::Status { request_id } => {
                let pools = self.pool_manager.get_all_stats().await;
                let pool_status: Vec<PoolStatus> = pools
                    .into_iter()
                    .map(|p| PoolStatus {
                        language: p.language,
                        ready_servers: p.ready_servers,
                        busy_servers: p.busy_servers,
                        total_servers: p.total_servers,
                    })
                    .collect();

                DaemonResponse::Status {
                    request_id,
                    status: DaemonStatus {
                        uptime_secs: self.start_time.elapsed().as_secs(),
                        pools: pool_status,
                        total_requests: *self.request_count.read().await,
                        active_connections: self.connections.len(),
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

            _ => DaemonResponse::Error {
                request_id: Uuid::new_v4(),
                error: "Unsupported request type".to_string(),
            },
        }
    }

    async fn handle_call_hierarchy(
        &self,
        file_path: &Path,
        pattern: &str,
    ) -> Result<CallHierarchyResult> {
        // Detect language
        let language = self.detector.detect(file_path)?;

        if language == Language::Unknown {
            return Err(anyhow!("Unknown language for file: {:?}", file_path));
        }

        // Get LSP server config
        let config = self
            .registry
            .get(language)
            .ok_or_else(|| anyhow!("No LSP server configured for {:?}", language))?
            .clone();

        // Get server from pool
        let pool = self.pool_manager.get_pool(language, config).await;
        let pooled_server = pool.get_server().await?;

        // Read file content
        let content = fs::read_to_string(file_path)?;

        // Find pattern position
        let (line, column) = find_pattern_position(&content, pattern)
            .ok_or_else(|| anyhow!("Pattern '{}' not found in file", pattern))?;

        // Open document
        pooled_server
            .server
            .open_document(file_path, &content)
            .await?;

        // Get call hierarchy
        let result = pooled_server
            .server
            .call_hierarchy(file_path, line, column)
            .await?;

        // Close document
        pooled_server.server.close_document(file_path).await?;

        // Return server to pool
        pool.return_server(pooled_server).await;

        // Parse result
        parse_call_hierarchy_from_lsp(&result)
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

    async fn cleanup(&self) -> Result<()> {
        info!("Cleaning up daemon resources");

        // Shutdown all pools
        self.pool_manager.shutdown_all().await;

        // Remove socket file (Unix only)
        remove_socket_file(&self.socket_path)?;

        Ok(())
    }

    fn clone_refs(&self) -> Self {
        Self {
            socket_path: self.socket_path.clone(),
            registry: self.registry.clone(),
            detector: self.detector.clone(),
            pool_manager: self.pool_manager.clone(),
            connections: self.connections.clone(),
            start_time: self.start_time,
            request_count: self.request_count.clone(),
            shutdown: self.shutdown.clone(),
        }
    }
}

fn find_pattern_position(content: &str, pattern: &str) -> Option<(u32, u32)> {
    for (line_idx, line) in content.lines().enumerate() {
        if let Some(col_idx) = line.find(pattern) {
            let char_col = line[..col_idx].chars().count() as u32;
            return Some((line_idx as u32, char_col));
        }
    }
    None
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

    // 3. Check common installation directories
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
    let socket_path = get_default_socket_path();

    // Check if daemon is already running by trying to connect
    if let Ok(_) = crate::ipc::IpcStream::connect(&socket_path).await {
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
