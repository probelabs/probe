use anyhow::{anyhow, Result};
use lsp_daemon::{
    get_default_socket_path, pid_lock::is_process_running, protocol::InitializedWorkspace,
    remove_socket_file, CallHierarchyResult, DaemonRequest, DaemonResponse, DaemonStatus,
    IpcStream, Language, LanguageDetector, LanguageInfo, LogEntry, MessageCodec,
};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{sleep, timeout};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use crate::lsp_integration::position_analyzer::PositionAnalyzer;
use crate::lsp_integration::types::*;

/// Simple struct to hold workspace cache statistics when reading from disk
#[derive(Debug)]
struct WorkspaceCacheStats {
    entries: u64,
    size_bytes: u64,
    disk_size_bytes: u64,
    #[allow(dead_code)]
    files: u64,
}

#[derive(Debug)]
enum DaemonHealth {
    Healthy,
    VersionMismatch,
    Unhealthy,
}

/// Resolve the socket path with optional override.
/// If PROBE_LSP_SOCKET_PATH is set, we use it; otherwise fall back to the default.
fn effective_socket_path() -> String {
    if let Ok(p) = std::env::var("PROBE_LSP_SOCKET_PATH") {
        return p;
    }
    // Default (typically under TMPDIR)
    get_default_socket_path()
}

pub struct LspClient {
    stream: Option<IpcStream>,
    config: LspConfig,
    daemon_started_by_us: bool,
    position_analyzer: PositionAnalyzer,
}

impl Drop for LspClient {
    fn drop(&mut self) {
        // Clean up existing connection without creating new ones
        if let Some(_stream) = self.stream.take() {
            // Don't attempt to send shutdown requests in Drop
            // The daemon will detect the connection close and clean up automatically
            debug!("LspClient dropped, connection will be closed automatically");
        }

        // Note: We don't attempt to shutdown the daemon in Drop because:
        // 1. Drop must be synchronous and cannot perform async operations properly
        // 2. Creating new connections in Drop causes "early eof" errors
        // 3. The daemon can detect closed connections and manage its lifecycle
        // 4. Explicit shutdown should be done via shutdown_daemon() method before Drop
    }
}

impl LspClient {
    /// Create a new LSP client with the given configuration
    pub async fn new(config: LspConfig) -> Result<Self> {
        let use_daemon = config.use_daemon;
        let mut position_analyzer = PositionAnalyzer::new();
        position_analyzer.load_patterns(None).await?;

        let mut client = Self {
            stream: None,
            config,
            daemon_started_by_us: false,
            position_analyzer,
        };

        if use_daemon {
            client.connect().await?;
        }

        Ok(client)
    }

    /// Create a non-blocking client that doesn't wait for LSP server to be ready
    /// Returns None if LSP is not available or still initializing
    pub async fn new_non_blocking(config: LspConfig) -> Option<Self> {
        let use_daemon = config.use_daemon;
        let mut position_analyzer = PositionAnalyzer::new();
        let _ = position_analyzer.load_patterns(None).await; // Ignore errors in non-blocking mode

        let mut client = Self {
            stream: None,
            config,
            daemon_started_by_us: false,
            position_analyzer,
        };

        if use_daemon {
            // Try quick connection without auto-start or waiting
            if client.try_connect_no_wait().await.is_err() {
                return None;
            }
        }

        Some(client)
    }

    /// Try to connect without waiting for server to be ready
    /// This is used for non-blocking operations
    async fn try_connect_no_wait(&mut self) -> Result<()> {
        let socket_path = effective_socket_path();

        // Very short timeout - just check if daemon is there
        let quick_timeout = Duration::from_millis(100);

        match timeout(quick_timeout, IpcStream::connect(&socket_path)).await {
            Ok(Ok(stream)) => {
                self.stream = Some(stream);

                // Send connect message with short timeout
                let request = DaemonRequest::Connect {
                    client_id: Uuid::new_v4(),
                };

                match timeout(quick_timeout, self.send_request(request)).await {
                    Ok(Ok(response)) => {
                        if let DaemonResponse::Connected { daemon_version, .. } = response {
                            debug!("Quick connect to daemon version: {}", daemon_version);
                        }
                        Ok(())
                    }
                    Ok(Err(e)) => {
                        debug!("LSP daemon not ready: {}", e);
                        self.stream = None;
                        Err(anyhow!("LSP daemon not ready"))
                    }
                    Err(_) => {
                        debug!("LSP daemon connection timed out");
                        self.stream = None;
                        Err(anyhow!("LSP daemon not available"))
                    }
                }
            }
            Ok(Err(e)) => {
                debug!("No LSP daemon running: {}", e);
                if !self.config.auto_start {
                    return Err(anyhow!("LSP daemon not available"));
                }
                // Try to start daemon in background but don't wait
                let _ = start_embedded_daemon_background().await;
                info!("LSP daemon starting in background, skipping LSP operations");
                Err(anyhow!("LSP daemon not available (starting in background)"))
            }
            Err(_) => {
                debug!("Quick connection check timed out");
                Err(anyhow!("LSP daemon not available"))
            }
        }
    }

    /// Connect to the LSP daemon, auto-starting if necessary
    async fn connect(&mut self) -> Result<()> {
        let socket_path = effective_socket_path();
        // Use shorter timeout for initial connection attempt
        let connection_timeout = Duration::from_secs(5);

        debug!("Attempting to connect to LSP daemon at: {}", socket_path);

        // Try to connect to existing daemon and check version compatibility
        match timeout(connection_timeout, IpcStream::connect(&socket_path)).await {
            Ok(Ok(mut stream)) => {
                info!(
                    "Successfully connected to daemon socket at: {}",
                    socket_path
                );
                // Check version compatibility using the same connection (avoid a second connect without a timeout)
                match check_daemon_version_compatibility_with_stream(&mut stream).await {
                    Ok(true) => {
                        info!("Connected to existing LSP daemon with compatible version");
                        self.stream = Some(stream);

                        // Send connect message with timeout
                        let request = DaemonRequest::Connect {
                            client_id: Uuid::new_v4(),
                        };

                        match timeout(connection_timeout, self.send_request_internal(request)).await
                        {
                            Ok(Ok(response)) => {
                                if let DaemonResponse::Connected { daemon_version, .. } = response {
                                    debug!("Connected to daemon version: {}", daemon_version);
                                }
                                return Ok(());
                            }
                            Ok(Err(e)) => {
                                warn!("Failed to send connect message: {}", e);
                                self.stream = None;
                            }
                            Err(_) => {
                                warn!("Connect message timed out");
                                self.stream = None;
                            }
                        }
                    }
                    Ok(false) => {
                        info!("Daemon version mismatch detected, will restart daemon...");
                        eprintln!("\n🔄 LSP daemon version mismatch detected.");
                        eprintln!("   Shutting down old daemon...");

                        // Shutdown the existing daemon
                        drop(stream); // Close our connection first
                        if let Err(e) = shutdown_existing_daemon().await {
                            warn!("Failed to shutdown existing daemon: {}", e);
                        }
                        // Fall through to the auto-start section below
                    }
                    Err(e) => {
                        warn!("Failed to check daemon version: {}", e);
                        // Close this connection and fall through to the auto-start section
                        drop(stream);
                    }
                }
            }
            Ok(Err(e)) => {
                debug!("Failed to connect to daemon: {}", e);
            }
            Err(_) => {
                debug!("Connection attempt timed out");
            }
        }

        if !self.config.auto_start {
            return Err(anyhow!("LSP daemon is not running"));
        }

        // Auto-start daemon
        info!("Starting embedded LSP daemon (this may take a few seconds on first run)...");
        match timeout(Duration::from_secs(10), start_embedded_daemon_background()).await {
            Ok(Ok(_)) => {
                // Successfully started - mark that we started this daemon
                self.daemon_started_by_us = true;
                info!("LSP daemon started successfully, waiting for it to be ready...");
            }
            Ok(Err(e)) => {
                return Err(anyhow!("Failed to start LSP daemon: {}", e));
            }
            Err(_) => {
                return Err(anyhow!("Timeout starting LSP daemon"));
            }
        }

        // Wait for daemon to be ready with improved timing
        // First attempts are quick, then we slow down to avoid spamming
        let retry_delays = [100, 200, 300, 500, 1000, 1000, 2000, 2000, 3000, 3000];
        for (attempt, delay_ms) in retry_delays.iter().enumerate() {
            sleep(Duration::from_millis(*delay_ms)).await;

            match timeout(connection_timeout, IpcStream::connect(&socket_path)).await {
                Ok(Ok(stream)) => {
                    info!("Connected to newly started LSP daemon at: {}", socket_path);
                    self.stream = Some(stream);

                    // Send connect message with timeout
                    let request = DaemonRequest::Connect {
                        client_id: Uuid::new_v4(),
                    };

                    match timeout(connection_timeout, self.send_request_internal(request)).await {
                        Ok(Ok(response)) => {
                            if let DaemonResponse::Connected { daemon_version, .. } = response {
                                debug!("Connected to daemon version: {}", daemon_version);
                            }
                            return Ok(());
                        }
                        Ok(Err(e)) => {
                            warn!("Failed to send connect message to new daemon: {}", e);
                            continue;
                        }
                        Err(_) => {
                            warn!("Connect message to new daemon timed out");
                            continue;
                        }
                    }
                }
                Ok(Err(e)) => {
                    debug!("Connection attempt {} failed: {}", attempt + 1, e);
                }
                Err(_) => {
                    debug!("Connection attempt {} timed out", attempt + 1);
                }
            }
        }

        Err(anyhow!(
            "Failed to connect to daemon after starting (all attempts timed out)"
        ))
    }

    /// Send a request to the daemon with retry logic for connection issues
    async fn send_request_with_retry(&mut self, request: DaemonRequest) -> Result<DaemonResponse> {
        const MAX_RETRIES: u32 = 3;
        let mut last_error = None;

        for retry in 0..MAX_RETRIES {
            match self.send_request_internal(request.clone()).await {
                Ok(response) => return Ok(response),
                Err(e) => {
                    let error_msg = e.to_string();
                    let is_retryable = error_msg.contains("early eof")
                        || error_msg.contains("Failed to read message length")
                        || error_msg.contains("Connection refused")
                        || error_msg.contains("connection reset")
                        || error_msg.contains("broken pipe");

                    if !is_retryable {
                        return Err(e);
                    }

                    warn!(
                        "LSP request failed with retryable error (attempt {}): {}",
                        retry + 1,
                        e
                    );
                    last_error = Some(e);

                    if retry < MAX_RETRIES - 1 {
                        // Reconnect before retry
                        self.stream = None;
                        tokio::time::sleep(Duration::from_millis(500 * (retry + 1) as u64)).await;

                        if let Err(conn_err) = self.connect().await {
                            warn!("Failed to reconnect for retry: {}", conn_err);
                            continue;
                        }
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| anyhow!("All retry attempts failed")))
    }

    /// Send a request to the daemon and wait for response (public interface with retry)
    async fn send_request(&mut self, request: DaemonRequest) -> Result<DaemonResponse> {
        self.send_request_with_retry(request).await
    }

    /// Send a request to the daemon and wait for response (internal implementation)
    async fn send_request_internal(&mut self, request: DaemonRequest) -> Result<DaemonResponse> {
        let stream = self
            .stream
            .as_mut()
            .ok_or_else(|| anyhow!("Not connected to daemon"))?;

        debug!("Sending request: {:?}", request);

        // Encode request
        let encoded = match MessageCodec::encode(&request) {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to encode request: {}", e);
                self.stream = None; // Clean up broken socket
                return Err(e);
            }
        };

        // Send request
        if let Err(e) = stream.write_all(&encoded).await {
            error!("Failed to write request: {}", e);
            self.stream = None; // Clean up broken socket
            return Err(anyhow!("Failed to write request: {}", e));
        }

        // Flush request
        if let Err(e) = stream.flush().await {
            error!("Failed to flush request: {}", e);
            self.stream = None; // Clean up broken socket
            return Err(anyhow!("Failed to flush request: {}", e));
        }

        // Read response with timeout using proper message framing
        let timeout_duration = Duration::from_millis(self.config.timeout_ms);
        debug!(
            "Waiting for response with timeout: {}ms",
            self.config.timeout_ms
        );

        // Read message length (4 bytes)
        let mut length_buf = [0u8; 4];
        match timeout(timeout_duration, stream.read_exact(&mut length_buf)).await {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => {
                error!("Failed to read message length: {}", e);
                self.stream = None; // Clean up broken socket
                return Err(anyhow!("Failed to read message length: {}", e));
            }
            Err(_) => {
                error!(
                    "Timeout reading message length after {}ms",
                    self.config.timeout_ms
                );
                self.stream = None; // Clean up broken socket on timeout
                let sp = effective_socket_path();
                return Err(anyhow!(
                    "Timeout connecting to daemon after {}ms (socket: {})",
                    self.config.timeout_ms,
                    sp
                ));
            }
        }
        let message_len = u32::from_be_bytes(length_buf) as usize;
        debug!("Message length: {} bytes", message_len);

        // Ensure we don't try to read unreasonably large messages (10MB limit)
        if message_len > 10 * 1024 * 1024 {
            error!("Message too large: {} bytes", message_len);
            self.stream = None; // Clean up broken socket
            return Err(anyhow!("Message too large: {} bytes", message_len));
        }

        // Read the complete message body
        let mut message_buf = vec![0u8; message_len];
        match timeout(timeout_duration, stream.read_exact(&mut message_buf)).await {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => {
                error!(
                    "Failed to read message body of {} bytes: {}",
                    message_len, e
                );
                self.stream = None; // Clean up broken socket
                return Err(anyhow!("Failed to read message body: {}", e));
            }
            Err(_) => {
                error!(
                    "Timeout reading message body of {} bytes after {}ms",
                    message_len, self.config.timeout_ms
                );
                self.stream = None; // Clean up broken socket on timeout
                let sp = effective_socket_path();
                return Err(anyhow!(
                    "Timeout waiting for daemon response after {}ms (socket: {})",
                    self.config.timeout_ms,
                    sp
                ));
            }
        }

        // Reconstruct the complete message with length prefix for decoding
        let mut complete_message = Vec::with_capacity(4 + message_len);
        complete_message.extend_from_slice(&length_buf);
        complete_message.extend_from_slice(&message_buf);

        // Decode response
        let response = match MessageCodec::decode_response(&complete_message) {
            Ok(resp) => resp,
            Err(e) => {
                error!("Failed to decode response: {}", e);
                self.stream = None; // Clean up broken socket
                return Err(e);
            }
        };
        debug!("Received response: {:?}", response);

        // Check for errors
        if let DaemonResponse::Error { error, .. } = &response {
            return Err(anyhow!("Daemon error: {}", error));
        }

        Ok(response)
    }

    /// Detect the LSP server type based on file path and language
    fn detect_lsp_server(&self, file_path: &Path) -> Option<String> {
        let extension = file_path.extension()?.to_str()?;

        match extension {
            "rs" => Some("rust-analyzer".to_string()),
            "go" => Some("gopls".to_string()),
            "py" => Some("pylsp".to_string()),
            "js" | "jsx" | "ts" | "tsx" => Some("typescript-language-server".to_string()),
            "c" | "h" => Some("clangd".to_string()),
            "cpp" | "hpp" | "cc" | "cxx" => Some("clangd".to_string()),
            "java" => Some("jdtls".to_string()),
            _ => None,
        }
    }

    /// Detect the programming language from file extension
    fn detect_language(&self, file_path: &Path) -> Option<String> {
        let extension = file_path.extension()?.to_str()?;

        match extension {
            "rs" => Some("rust".to_string()),
            "go" => Some("go".to_string()),
            "py" => Some("python".to_string()),
            "js" | "jsx" => Some("javascript".to_string()),
            "ts" | "tsx" => Some("typescript".to_string()),
            "c" | "h" => Some("c".to_string()),
            "cpp" | "hpp" | "cc" | "cxx" => Some("cpp".to_string()),
            "java" => Some("java".to_string()),
            _ => None,
        }
    }

    /// Calculate the precise LSP position using position patterns
    fn calculate_lsp_position(
        &self,
        symbol_name: &str,
        tree_sitter_line: u32,
        tree_sitter_column: u32,
        file_path: &Path,
    ) -> (u32, u32) {
        let language = self.detect_language(file_path);
        let lsp_server = self.detect_lsp_server(file_path);
        let symbol_type = "function"; // For now, assume functions (could be enhanced)

        // First, snap to the identifier start using the shared resolver from lsp-daemon
        let (base_line, base_column) = {
            let lang_str = language.as_deref().unwrap_or("unknown");
            match lsp_daemon::position::resolve_symbol_position(
                file_path,
                tree_sitter_line,
                tree_sitter_column,
                lang_str,
            ) {
                Ok((l, c)) => (l, c),
                Err(_) => (tree_sitter_line, tree_sitter_column),
            }
        };

        // Then apply any learned position offset pattern from analyzer (relative to identifier start)
        let position_offset = if let Some(lang) = &language {
            self.position_analyzer
                .get_position_offset(lang, symbol_type, lsp_server.as_deref())
        } else {
            None
        };

        let identifier_len = symbol_name.len() as u32;

        // Apply the position offset or use default (start position)
        match position_offset {
            Some(offset) => offset.apply(base_line, base_column, identifier_len),
            None => {
                // Default fallback: use start position
                debug!("No position pattern found for language={:?} symbol_type={} server={:?}, using start position", 
                       language, symbol_type, lsp_server);
                (base_line, base_column)
            }
        }
    }

    /// Get call hierarchy with precise positioning using deterministic algorithm
    /// Uses the position analyzer's learned patterns to make a single, accurate LSP request
    async fn get_call_hierarchy_precise(
        &mut self,
        file_path: &Path,
        symbol_name: &str,
        tree_sitter_line: u32,
        tree_sitter_column: u32,
    ) -> Result<CallHierarchyInfo> {
        let debug_mode = std::env::var("PROBE_DEBUG").unwrap_or_default() == "1";

        if debug_mode {
            println!(
                "[DEBUG] Computing precise position for symbol '{symbol_name}' at tree-sitter position {tree_sitter_line}:{tree_sitter_column}"
            );
        }

        // Calculate the precise LSP position using learned patterns
        let (lsp_line, lsp_column) = self.calculate_lsp_position(
            symbol_name,
            tree_sitter_line,
            tree_sitter_column,
            file_path,
        );

        if debug_mode {
            let language = self.detect_language(file_path);
            let lsp_server = self.detect_lsp_server(file_path);
            println!(
                "[DEBUG] Calculated LSP position: {lsp_line}:{lsp_column} (language={language:?}, server={lsp_server:?})"
            );
        }

        // Make single LSP request with precise position
        match self
            .get_call_hierarchy(file_path, lsp_line, lsp_column)
            .await
        {
            Ok(hierarchy) => {
                let has_data =
                    !hierarchy.incoming_calls.is_empty() || !hierarchy.outgoing_calls.is_empty();

                if debug_mode {
                    println!(
                        "[DEBUG] LSP request at {}:{} returned {} incoming, {} outgoing calls",
                        lsp_line,
                        lsp_column,
                        hierarchy.incoming_calls.len(),
                        hierarchy.outgoing_calls.len()
                    );
                }

                if has_data {
                    if debug_mode {
                        println!(
                            "[DEBUG] Success! Found call hierarchy data at precise position {lsp_line}:{lsp_column}"
                        );
                    }
                    Ok(hierarchy)
                } else {
                    // Even with precise positioning, sometimes symbols don't have call hierarchy
                    if debug_mode {
                        println!("[DEBUG] Symbol has no call hierarchy data (this is normal for some symbols)");
                    }
                    Ok(CallHierarchyInfo {
                        incoming_calls: Vec::new(),
                        outgoing_calls: Vec::new(),
                    })
                }
            }
            Err(e) => {
                if debug_mode {
                    println!("[DEBUG] LSP request failed at {lsp_line}:{lsp_column}: {e}");
                    println!("[DEBUG] This might indicate the position is not on a symbol or the LSP server is not ready");
                }

                // Return empty result rather than propagating error
                // This maintains compatibility with existing code
                Ok(CallHierarchyInfo {
                    incoming_calls: Vec::new(),
                    outgoing_calls: Vec::new(),
                })
            }
        }
    }

    /// Get enhanced symbol information including call hierarchy and references
    pub async fn get_symbol_info(
        &mut self,
        file_path: &Path,
        symbol_name: &str,
        line: u32,
        column: u32,
    ) -> Result<Option<EnhancedSymbolInfo>> {
        if !self.config.use_daemon {
            return Ok(None);
        }

        // Try to connect if not connected
        if self.stream.is_none() {
            if let Err(e) = self.connect().await {
                warn!("Failed to connect to LSP daemon: {}", e);
                return Ok(None);
            }
        }

        // Get call hierarchy information using precise positioning
        let call_hierarchy = match self
            .get_call_hierarchy_precise(file_path, symbol_name, line, column)
            .await
        {
            Ok(mut hierarchy) => {
                // Optionally filter out standard library frames to reduce noise.
                if !self.config.include_stdlib {
                    // This is a client-side preference; the daemon still computes full results.
                    hierarchy.filter_stdlib_in_place();
                }
                Some(hierarchy)
            }
            Err(e) => {
                warn!("Failed to get call hierarchy: {}", e);
                None
            }
        };

        // Get references - PHP requires includeDeclaration=true to return results
        let include_declaration = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext == "php")
            .unwrap_or(false);

        let references = match self
            .call_references(file_path, line, column, include_declaration)
            .await
        {
            Ok(locations) => locations
                .into_iter()
                .map(|loc| ReferenceInfo {
                    file_path: loc.uri,
                    line: loc.range.start.line,
                    column: loc.range.start.character,
                    context: "reference".to_string(), // Could be enhanced with actual context
                })
                .collect(),
            Err(e) => {
                warn!("Failed to get references: {}", e);
                Vec::new()
            }
        };

        // No longer fetch hover/documentation information to reduce noise and improve performance

        Ok(Some(EnhancedSymbolInfo {
            name: symbol_name.to_string(),
            file_path: file_path.to_string_lossy().to_string(),
            line,
            column,
            symbol_kind: "unknown".to_string(), // Will be determined by tree-sitter
            call_hierarchy,
            references,
            documentation: None,
            type_info: None,
        }))
    }

    /// Get call hierarchy for a symbol
    pub async fn get_call_hierarchy(
        &mut self,
        file_path: &Path,
        line: u32,
        column: u32,
    ) -> Result<CallHierarchyInfo> {
        debug!(
            "Getting call hierarchy for {:?} at {}:{}",
            file_path, line, column
        );

        let request = DaemonRequest::CallHierarchy {
            request_id: Uuid::new_v4(),
            file_path: file_path.to_path_buf(),
            line,
            column,
            workspace_hint: self
                .config
                .workspace_hint
                .as_ref()
                .map(std::path::PathBuf::from),
        };

        debug!("Sending CallHierarchy request to daemon");

        // Add timeout for call hierarchy request - this can be slow due to rust-analyzer
        let call_timeout = Duration::from_millis(self.config.timeout_ms);
        let response = timeout(call_timeout, self.send_request(request))
            .await
            .map_err(|_| {
                anyhow!(
                    "Call hierarchy request timed out after {}ms",
                    self.config.timeout_ms
                )
            })??;

        debug!("Received response from daemon");

        match response {
            DaemonResponse::CallHierarchy { result, .. } => {
                debug!("Call hierarchy response received successfully");
                let converted = convert_call_hierarchy_result(result);
                Ok(converted)
            }
            DaemonResponse::Error { error, .. } => {
                debug!("Call hierarchy failed: {}", error);
                Err(anyhow!("Call hierarchy failed: {}", error))
            }
            _ => Err(anyhow!("Unexpected response type")),
        }
    }

    /// Get daemon status
    pub async fn get_status(&mut self) -> Result<LspDaemonStatus> {
        let request = DaemonRequest::Status {
            request_id: Uuid::new_v4(),
        };

        let response = self.send_request(request).await?;

        match response {
            DaemonResponse::Status { status, .. } => Ok(convert_daemon_status(status)),
            _ => Err(anyhow!("Unexpected response type")),
        }
    }

    /// List available language servers
    pub async fn list_languages(&mut self) -> Result<Vec<LanguageInfo>> {
        let request = DaemonRequest::ListLanguages {
            request_id: Uuid::new_v4(),
        };

        let response = self.send_request(request).await?;

        match response {
            DaemonResponse::LanguageList { languages, .. } => Ok(languages),
            _ => Err(anyhow!("Unexpected response type")),
        }
    }

    /// Get log entries from the daemon
    pub async fn get_logs(&mut self, lines: usize) -> Result<Vec<LogEntry>> {
        let request = DaemonRequest::GetLogs {
            request_id: Uuid::new_v4(),
            lines,
            since_sequence: None,
        };

        let response = self.send_request(request).await?;

        match response {
            DaemonResponse::Logs { entries, .. } => Ok(entries),
            _ => Err(anyhow!("Unexpected response type")),
        }
    }

    /// Get log entries since a specific sequence number
    pub async fn get_logs_since(
        &mut self,
        since_sequence: u64,
        lines: usize,
    ) -> Result<Vec<LogEntry>> {
        let request = DaemonRequest::GetLogs {
            request_id: Uuid::new_v4(),
            lines,
            since_sequence: Some(since_sequence),
        };

        let response = self.send_request(request).await?;

        match response {
            DaemonResponse::Logs { entries, .. } => Ok(entries),
            _ => Err(anyhow!("Unexpected response type")),
        }
    }

    /// Shutdown the daemon
    pub async fn shutdown_daemon(&mut self) -> Result<()> {
        let request = DaemonRequest::Shutdown {
            request_id: Uuid::new_v4(),
        };

        let response = self.send_request(request).await?;

        match response {
            DaemonResponse::Shutdown { .. } => {
                info!("LSP daemon shutdown acknowledged");
                self.stream = None;
                Ok(())
            }
            _ => Err(anyhow!("Unexpected response type")),
        }
    }

    /// Ping the daemon for health check
    pub async fn ping(&mut self) -> Result<()> {
        let request = DaemonRequest::Ping {
            request_id: Uuid::new_v4(),
        };

        let response = self.send_request(request).await?;

        match response {
            DaemonResponse::Pong { .. } => Ok(()),
            _ => Err(anyhow!("Unexpected response type")),
        }
    }

    /// Initialize workspaces
    pub async fn init_workspaces(
        &mut self,
        workspace_root: PathBuf,
        languages: Option<Vec<String>>,
        recursive: bool,
        enable_watchdog: bool,
    ) -> Result<(Vec<InitializedWorkspace>, Vec<String>)> {
        // Convert language strings to Language enum
        let languages = languages.map(|langs| {
            langs
                .into_iter()
                .filter_map(|lang| {
                    let lang_lower = lang.to_lowercase();
                    match lang_lower.as_str() {
                        "rust" => Some(Language::Rust),
                        "typescript" | "ts" => Some(Language::TypeScript),
                        "javascript" | "js" => Some(Language::JavaScript),
                        "python" | "py" => Some(Language::Python),
                        "go" => Some(Language::Go),
                        "java" => Some(Language::Java),
                        "c" => Some(Language::C),
                        "cpp" | "c++" => Some(Language::Cpp),
                        "csharp" | "c#" => Some(Language::CSharp),
                        "ruby" | "rb" => Some(Language::Ruby),
                        "php" => Some(Language::Php),
                        "swift" => Some(Language::Swift),
                        "kotlin" | "kt" => Some(Language::Kotlin),
                        "scala" => Some(Language::Scala),
                        "haskell" | "hs" => Some(Language::Haskell),
                        "elixir" | "ex" => Some(Language::Elixir),
                        "clojure" | "clj" => Some(Language::Clojure),
                        "lua" => Some(Language::Lua),
                        "zig" => Some(Language::Zig),
                        _ => None,
                    }
                })
                .collect()
        });

        let request = DaemonRequest::InitWorkspaces {
            request_id: Uuid::new_v4(),
            workspace_root,
            languages,
            recursive,
            enable_watchdog,
        };

        let response = self.send_request(request).await?;

        match response {
            DaemonResponse::WorkspacesInitialized {
                initialized,
                errors,
                ..
            } => Ok((initialized, errors)),
            _ => Err(anyhow!("Unexpected response type")),
        }
    }

    /// Check if LSP is available for the given file
    pub fn is_supported(&self, file_path: &Path) -> bool {
        let detector = LanguageDetector::new();
        if let Ok(language) = detector.detect(file_path) {
            language != Language::Unknown
        } else {
            false
        }
    }

    /// Get cache statistics from the LSP daemon
    pub async fn cache_stats(&mut self) -> Result<lsp_daemon::protocol::CacheStatistics> {
        // First try to get stats from running daemon
        match self.try_daemon_cache_stats().await {
            Ok(stats) => Ok(stats),
            Err(daemon_err) => {
                debug!("Failed to get cache stats from daemon: {}", daemon_err);
                // Fallback to reading disk cache directly
                match self.read_disk_cache_stats().await {
                    Ok(stats) => {
                        info!("Successfully read cache stats from disk (daemon not running)");
                        Ok(stats)
                    }
                    Err(disk_err) => {
                        warn!("Failed to read disk cache stats: {}", disk_err);
                        // Return the original daemon error as it's more relevant
                        Err(daemon_err)
                    }
                }
            }
        }
    }

    /// Try to get cache stats from running daemon
    async fn try_daemon_cache_stats(&mut self) -> Result<lsp_daemon::protocol::CacheStatistics> {
        let request = DaemonRequest::CacheStats {
            request_id: Uuid::new_v4(),
            detailed: false,
            git: false,
        };

        let response = self.send_request(request).await?;

        match response {
            DaemonResponse::CacheStats { stats, .. } => Ok(stats),
            _ => Err(anyhow!("Unexpected response type for cache stats")),
        }
    }

    /// Read cache statistics directly from disk files (when daemon is not running)
    async fn read_disk_cache_stats(&self) -> Result<lsp_daemon::protocol::CacheStatistics> {
        debug!("Reading cache statistics from disk files");

        // Get cache base directory
        let cache_base_dir = self.get_cache_base_directory();

        if !cache_base_dir.exists() {
            debug!("Cache directory does not exist: {:?}", cache_base_dir);
            return Ok(lsp_daemon::protocol::CacheStatistics {
                total_entries: 0,
                total_size_bytes: 0,
                disk_size_bytes: 0,
                entries_per_file: HashMap::new(),
                entries_per_language: HashMap::new(),
                hit_rate: 0.0,
                miss_rate: 0.0,
                age_distribution: lsp_daemon::protocol::AgeDistribution {
                    entries_last_hour: 0,
                    entries_last_day: 0,
                    entries_last_week: 0,
                    entries_last_month: 0,
                    entries_older: 0,
                },
                most_accessed: Vec::new(),
                memory_usage: lsp_daemon::protocol::MemoryUsage {
                    in_memory_cache_bytes: 0,
                    persistent_cache_bytes: 0,
                    metadata_bytes: 0,
                    index_bytes: 0,
                },
                per_workspace_stats: None,
                per_operation_totals: None,
            });
        }

        let mut total_entries = 0u64;
        let mut total_size_bytes = 0u64;
        let mut total_disk_size = 0u64;

        // Discover all workspace caches
        let workspaces_dir = cache_base_dir.join("workspaces");
        if workspaces_dir.exists() {
            debug!("Scanning workspace caches in: {:?}", workspaces_dir);

            match tokio::fs::read_dir(&workspaces_dir).await {
                Ok(mut entries) => {
                    while let Ok(Some(entry)) = entries.next_entry().await {
                        if entry.file_type().await.is_ok_and(|ft| ft.is_dir()) {
                            match self.read_workspace_cache_stats(&entry.path()).await {
                                Ok(workspace_stats) => {
                                    let _workspace_name =
                                        entry.file_name().to_string_lossy().to_string();

                                    total_entries += workspace_stats.entries;
                                    total_size_bytes += workspace_stats.size_bytes;
                                    total_disk_size += workspace_stats.disk_size_bytes;
                                }
                                Err(e) => {
                                    warn!(
                                        "Failed to read workspace cache at {:?}: {}",
                                        entry.path(),
                                        e
                                    );
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to read workspaces directory: {}", e);
                }
            }
        }

        // Legacy cache no longer used - all caching is done via universal cache

        debug!(
            "Read cache stats from disk: {} entries, {} bytes total",
            total_entries, total_size_bytes
        );

        Ok(lsp_daemon::protocol::CacheStatistics {
            total_entries,
            total_size_bytes,
            disk_size_bytes: total_disk_size,
            entries_per_file: HashMap::new(), // Would require parsing all cache entries
            entries_per_language: HashMap::new(), // Would require parsing all cache entries
            hit_rate: 0.0,                    // Can't determine hit rate from disk files
            miss_rate: 0.0,                   // Can't determine miss rate from disk files
            age_distribution: lsp_daemon::protocol::AgeDistribution {
                entries_last_hour: 0,
                entries_last_day: 0,
                entries_last_week: 0,
                entries_last_month: 0,
                entries_older: total_entries, // All entries are considered older since we can't read timestamps easily
            },
            most_accessed: Vec::new(), // Would require parsing all cache entries
            memory_usage: lsp_daemon::protocol::MemoryUsage {
                in_memory_cache_bytes: 0, // No in-memory cache when daemon is off
                persistent_cache_bytes: total_disk_size,
                metadata_bytes: 0, // Estimated, would need more detailed parsing
                index_bytes: 0,    // Estimated, would need more detailed parsing
            },
            per_workspace_stats: None,
            per_operation_totals: None,
        })
    }

    /// Get the cache base directory
    fn get_cache_base_directory(&self) -> PathBuf {
        // Check environment variable first
        if let Ok(cache_dir) = std::env::var("PROBE_LSP_CACHE_DIR") {
            return PathBuf::from(cache_dir);
        }

        // Use standard cache directory
        if let Some(cache_dir) = dirs::cache_dir() {
            cache_dir.join("probe").join("lsp")
        } else {
            PathBuf::from("/tmp").join("probe-lsp-cache")
        }
    }

    /// Read cache statistics for a single workspace
    async fn read_workspace_cache_stats(
        &self,
        workspace_path: &Path,
    ) -> Result<WorkspaceCacheStats> {
        let mut entries = 0u64;
        let mut size_bytes = 0u64;
        let mut disk_size_bytes = 0u64;

        // Check for cache.db file (new database format)
        let cache_db = workspace_path.join("cache.db");
        if cache_db.exists() {
            match self.read_sled_db_stats(&cache_db).await {
                Ok(stats) => {
                    entries += stats.entries;
                    size_bytes += stats.size_bytes;
                    disk_size_bytes += stats.disk_size_bytes;
                }
                Err(e) => {
                    debug!("Failed to read db stats from {:?}: {}", cache_db, e);
                }
            }
        }

        Ok(WorkspaceCacheStats {
            entries,
            size_bytes,
            disk_size_bytes,
            files: 0, // Would need to parse cache contents to get file count
        })
    }

    /// Read statistics from a database file (DEPRECATED - sled support removed)
    #[allow(dead_code)]
    async fn read_sled_db_stats(&self, db_path: &Path) -> Result<WorkspaceCacheStats> {
        debug!("Database reading is deprecated: {:?}", db_path);

        // Get directory size as disk size
        let disk_size_bytes = self.calculate_directory_size(db_path).await;

        warn!(
            "Sled database reading is deprecated. Database at {} cannot be read.",
            db_path.display()
        );

        // Return minimal stats based on file size
        Ok(WorkspaceCacheStats {
            entries: if disk_size_bytes > 0 { 1 } else { 0 },
            size_bytes: disk_size_bytes,
            disk_size_bytes,
            files: 0,
        })
    }

    /// Calculate the total size of a directory and its contents (iterative to avoid recursion issues)
    async fn calculate_directory_size(&self, dir_path: &Path) -> u64 {
        let mut total_size = 0u64;
        let mut dirs_to_process = vec![dir_path.to_path_buf()];

        while let Some(current_dir) = dirs_to_process.pop() {
            if let Ok(mut entries) = tokio::fs::read_dir(&current_dir).await {
                while let Ok(Some(entry)) = entries.next_entry().await {
                    if let Ok(metadata) = entry.metadata().await {
                        if metadata.is_file() {
                            total_size += metadata.len();
                        } else if metadata.is_dir() {
                            // Add subdirectory to processing queue
                            dirs_to_process.push(entry.path());
                        }
                    }
                }
            }
        }

        total_size
    }

    /// Clear cache entries
    pub async fn cache_clear(
        &mut self,
        older_than_days: Option<u64>,
        file_path: Option<PathBuf>,
        commit_hash: Option<String>,
        all: bool,
    ) -> Result<lsp_daemon::protocol::ClearResult> {
        let request = DaemonRequest::CacheClear {
            request_id: Uuid::new_v4(),
            older_than_days,
            file_path,
            commit_hash,
            all,
        };

        let response = self.send_request(request).await?;

        match response {
            DaemonResponse::CacheCleared { result, .. } => Ok(result),
            _ => Err(anyhow!("Unexpected response type for cache clear")),
        }
    }

    /// Export cache contents to JSON
    pub async fn cache_export(
        &mut self,
        output_path: PathBuf,
        current_branch_only: bool,
        compress: bool,
    ) -> Result<()> {
        let request = DaemonRequest::CacheExport {
            request_id: Uuid::new_v4(),
            output_path,
            current_branch_only,
            compress,
        };

        let response = self.send_request(request).await?;

        match response {
            DaemonResponse::CacheExported { .. } => Ok(()),
            _ => Err(anyhow!("Unexpected response type for cache export")),
        }
    }

    /// Clear cache for a specific symbol
    pub async fn clear_symbol_cache(
        &mut self,
        file_path: PathBuf,
        symbol_name: String,
        line: Option<u32>,
        column: Option<u32>,
        methods: Option<Vec<String>>,
        all_positions: bool,
    ) -> Result<lsp_daemon::protocol::SymbolCacheClearResult> {
        let request = DaemonRequest::ClearSymbolCache {
            request_id: Uuid::new_v4(),
            file_path,
            symbol_name,
            line,
            column,
            methods,
            all_positions,
        };

        let response = self.send_request(request).await?;

        match response {
            DaemonResponse::SymbolCacheCleared { result, .. } => Ok(result),
            DaemonResponse::Error { error, .. } => Err(anyhow!(error)),
            _ => Err(anyhow!("Unexpected response type for symbol cache clear")),
        }
    }
}

/// Get current probe binary version info
fn get_probe_version_info() -> (String, String, String) {
    (
        env!("CARGO_PKG_VERSION").to_string(),
        env!("GIT_HASH").to_string(),
        env!("BUILD_DATE").to_string(),
    )
}

/// Check daemon health and version compatibility
async fn check_daemon_health() -> Result<DaemonHealth> {
    let socket_path = effective_socket_path();

    // Try to connect to existing daemon
    let mut stream = match timeout(Duration::from_secs(2), IpcStream::connect(&socket_path)).await {
        Ok(Ok(stream)) => stream,
        Ok(Err(e)) => return Err(anyhow!("Failed to connect to daemon: {}", e)),
        Err(_) => return Err(anyhow!("Connection timeout")),
    };

    // Send ping request to check health
    let ping_request = DaemonRequest::Ping {
        request_id: Uuid::new_v4(),
    };

    let encoded = MessageCodec::encode(&ping_request)?;
    stream.write_all(&encoded).await?;
    stream.flush().await?;

    // Read ping response with timeout
    let mut length_buf = [0u8; 4];
    match timeout(Duration::from_secs(2), stream.read_exact(&mut length_buf)).await {
        Ok(Ok(_)) => {}
        _ => return Ok(DaemonHealth::Unhealthy),
    }

    let length = u32::from_be_bytes(length_buf) as usize;
    let mut response_buf = vec![0u8; length];
    match timeout(Duration::from_secs(2), stream.read_exact(&mut response_buf)).await {
        Ok(Ok(_)) => {}
        _ => return Ok(DaemonHealth::Unhealthy),
    }

    let response = MessageCodec::decode_response(&[&length_buf[..], &response_buf[..]].concat())?;

    // Check if we got a pong response
    match response {
        DaemonResponse::Pong { .. } => {
            // Daemon is responsive, now check version using the same connection
            match check_daemon_version_compatibility_with_stream(&mut stream).await {
                Ok(true) => Ok(DaemonHealth::Healthy),
                Ok(false) => Ok(DaemonHealth::VersionMismatch),
                Err(_) => Ok(DaemonHealth::Unhealthy),
            }
        }
        _ => Ok(DaemonHealth::Unhealthy),
    }
}

/// Check if daemon version matches probe binary version (creates new connection)
#[allow(dead_code)]
async fn check_daemon_version_compatibility() -> Result<bool> {
    let socket_path = effective_socket_path();

    // Try to connect to existing daemon with a short timeout
    let connect_timeout = Duration::from_secs(2);
    match timeout(connect_timeout, IpcStream::connect(&socket_path)).await {
        Ok(Ok(mut stream)) => check_daemon_version_compatibility_with_stream(&mut stream).await,
        Ok(Err(_)) => {
            // No daemon running, no version conflict
            Ok(true)
        }
        Err(_) => Err(anyhow!("Timed out connecting to daemon for version check")),
    }
}

/// Check if daemon version matches probe binary version (reuses existing connection)
async fn check_daemon_version_compatibility_with_stream(stream: &mut IpcStream) -> Result<bool> {
    // Send status request to get daemon version
    let request = DaemonRequest::Status {
        request_id: Uuid::new_v4(),
    };

    let encoded = MessageCodec::encode(&request)?;
    stream.write_all(&encoded).await?;
    stream.flush().await?;

    // Read response with timeout to prevent hanging
    let mut length_buf = [0u8; 4];
    match timeout(Duration::from_secs(2), stream.read_exact(&mut length_buf)).await {
        Ok(Ok(_)) => {}
        _ => return Err(anyhow!("Failed to read status response length")),
    }

    let length = u32::from_be_bytes(length_buf) as usize;
    let mut response_buf = vec![0u8; length];
    match timeout(Duration::from_secs(2), stream.read_exact(&mut response_buf)).await {
        Ok(Ok(_)) => {}
        _ => return Err(anyhow!("Failed to read status response body")),
    }

    let response = MessageCodec::decode_response(&[&length_buf[..], &response_buf[..]].concat())?;

    if let DaemonResponse::Status { status, .. } = response {
        let (probe_version, probe_git_hash, probe_build_date) = get_probe_version_info();

        debug!(
            "Probe version: {}, git: {}, build: {}",
            probe_version, probe_git_hash, probe_build_date
        );
        debug!(
            "Daemon version: {}, git: {}, build: {}",
            status.version, status.git_hash, status.build_date
        );

        // Check if versions match
        let version_matches = !status.version.is_empty()
            && !status.git_hash.is_empty()
            && status.git_hash == probe_git_hash;

        if !version_matches {
            info!(
                "Version mismatch detected - Probe: {} ({}), Daemon: {} ({})",
                probe_version, probe_git_hash, status.version, status.git_hash
            );
        }

        Ok(version_matches)
    } else {
        // If we can't get status, assume incompatible
        Ok(false)
    }
}

/// Shutdown existing daemon gracefully
async fn shutdown_existing_daemon() -> Result<()> {
    let socket_path = effective_socket_path();

    match IpcStream::connect(&socket_path).await {
        Ok(mut stream) => {
            // Send shutdown request
            let request = DaemonRequest::Shutdown {
                request_id: Uuid::new_v4(),
            };

            let encoded = MessageCodec::encode(&request)?;
            stream.write_all(&encoded).await?;

            info!("Sent shutdown request to existing daemon");

            // Give daemon time to shutdown
            sleep(Duration::from_millis(500)).await;
            Ok(())
        }
        Err(_) => {
            // No daemon running
            Ok(())
        }
    }
}

/// Wrapper for client startup lock file that cleans up on drop
struct ClientStartupLock {
    _file: File,
    path: String,
}

const CLIENT_LOCK_STALE_THRESHOLD: Duration = Duration::from_secs(30);

fn read_pid_from_lock(lock_path: &str) -> Option<u32> {
    std::fs::read_to_string(lock_path)
        .ok()
        .and_then(|contents| contents.trim().parse::<u32>().ok())
}

fn lock_file_age(lock_path: &str) -> Option<Duration> {
    let metadata = std::fs::metadata(lock_path).ok()?;
    let modified = metadata.modified().ok()?;
    SystemTime::now().duration_since(modified).ok()
}

fn cleanup_stale_client_lock(lock_path: &str) -> Result<bool> {
    let age = lock_file_age(lock_path);
    let pid = read_pid_from_lock(lock_path);

    if let Some(pid) = pid {
        if is_process_running(pid) {
            debug!(
                "Client startup lock at {} currently held by running PID {}",
                lock_path, pid
            );
            return Ok(false);
        }

        if age.map_or(true, |age| age > CLIENT_LOCK_STALE_THRESHOLD) {
            debug!(
                "Removing stale client startup lock at {} left by PID {}",
                lock_path, pid
            );
            std::fs::remove_file(lock_path)?;
            return Ok(true);
        }

        debug!(
            "Client startup lock at {} has PID {} but is still recent, waiting",
            lock_path, pid
        );
        return Ok(false);
    }

    if age.map_or(false, |age| age > CLIENT_LOCK_STALE_THRESHOLD) {
        debug!(
            "Removing stale client startup lock at {} with no PID information",
            lock_path
        );
        std::fs::remove_file(lock_path)?;
        return Ok(true);
    }

    Ok(false)
}

impl Drop for ClientStartupLock {
    fn drop(&mut self) {
        // Clean up the lock file when dropped
        let _ = std::fs::remove_file(&self.path);
        debug!("Released client startup lock");
    }
}

/// Global path for client startup coordination lock
fn get_client_lock_path() -> String {
    // Use platform-appropriate temp directory
    let temp_dir = std::env::temp_dir();
    temp_dir
        .join("probe-lsp-client-start.lock")
        .to_string_lossy()
        .to_string()
}

/// Start embedded LSP daemon in the background using probe binary
pub(crate) async fn start_embedded_daemon_background() -> Result<()> {
    let socket_path = effective_socket_path();

    // Use file-based locking for cross-process coordination
    let _lock = acquire_client_startup_lock()?;

    // Double-check after acquiring the lock - another process might have started the daemon
    match check_daemon_health().await {
        Ok(DaemonHealth::Healthy) => {
            debug!("Daemon already running and healthy (after acquiring lock)");
            return Ok(());
        }
        Ok(DaemonHealth::VersionMismatch) => {
            eprintln!("\n🔄 LSP daemon version mismatch detected.");
            eprintln!("   Restarting daemon with new version...");
            eprintln!("   This may take a few seconds on first run.");
            shutdown_existing_daemon().await?;
        }
        Ok(DaemonHealth::Unhealthy) => {
            warn!("Daemon is unhealthy, restarting...");
            shutdown_existing_daemon().await?;
        }
        Err(_) => {
            // No daemon running or can't connect
            debug!("No daemon running, will start new one");
        }
    }

    // Clean up any stale socket
    remove_socket_file(&socket_path)?;

    // Get current executable path (probe binary)
    let probe_binary = std::env::current_exe()
        .map_err(|e| anyhow!("Failed to get current executable path: {}", e))?;

    debug!(
        "Starting embedded daemon using probe binary: {:?}",
        probe_binary
    );

    // Start daemon using "probe lsp start" command in foreground mode
    // Environment variables are inherited by default
    let mut cmd = std::process::Command::new(&probe_binary);
    cmd.args(["lsp", "start", "-f"])
        .stdin(std::process::Stdio::null());
    // In CI or when debugging, inherit stdout/stderr so early failures (bind/lock) are visible.
    // Enable by setting LSP_VERBOSE_SPAWN=1 in the environment.
    if std::env::var("LSP_VERBOSE_SPAWN").ok().as_deref() == Some("1") {
        cmd.stdout(std::process::Stdio::inherit())
            .stderr(std::process::Stdio::inherit());
    } else {
        cmd.stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
    }
    cmd.spawn()
        .map_err(|e| anyhow!("Failed to spawn embedded daemon: {}", e))?;

    info!("Started embedded daemon in background");

    // Lock will be automatically released when _lock goes out of scope

    Ok(())
}

/// Acquire a file-based lock for client startup coordination
fn acquire_client_startup_lock() -> Result<ClientStartupLock> {
    let lock_path = get_client_lock_path();
    let mut start_time = Instant::now();
    let max_wait = Duration::from_secs(10);

    loop {
        // Try to create the lock file exclusively
        match OpenOptions::new()
            .write(true)
            .create_new(true) // Atomic creation - fails if file exists
            .open(&lock_path)
        {
            Ok(mut file) => {
                // Write our PID to help with debugging
                let _ = writeln!(file, "{}", std::process::id());
                debug!("Acquired client startup lock");
                return Ok(ClientStartupLock {
                    _file: file,
                    path: lock_path,
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                if cleanup_stale_client_lock(&lock_path)? {
                    // Stale lock removed, restart wait window and retry immediately
                    start_time = Instant::now();
                    continue;
                }

                if start_time.elapsed() > max_wait {
                    if let Some(pid) = read_pid_from_lock(&lock_path) {
                        return Err(anyhow!(
                            "Timeout waiting for client startup lock held by PID {}",
                            pid
                        ));
                    }
                    return Err(anyhow!("Timeout waiting for client startup lock"));
                }

                if let Some(pid) = read_pid_from_lock(&lock_path) {
                    debug!(
                        "Another client (PID {}) is starting the daemon, waiting...",
                        pid
                    );
                } else {
                    debug!("Another client is starting daemon, waiting...");
                }

                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => return Err(anyhow!("Failed to acquire client startup lock: {}", e)),
        }
    }
}

/// Convert lsp-daemon CallHierarchyResult to our CallHierarchyInfo
fn convert_call_hierarchy_result(result: CallHierarchyResult) -> CallHierarchyInfo {
    let incoming_calls = result
        .incoming
        .into_iter()
        .map(|call| CallInfo {
            name: call.from.name,
            file_path: call.from.uri,
            line: call.from.range.start.line,
            column: call.from.range.start.character,
            symbol_kind: call.from.kind,
        })
        .collect();

    let outgoing_calls = result
        .outgoing
        .into_iter()
        .map(|call| CallInfo {
            name: call.from.name,
            file_path: call.from.uri,
            line: call.from.range.start.line,
            column: call.from.range.start.character,
            symbol_kind: call.from.kind,
        })
        .collect();

    CallHierarchyInfo {
        incoming_calls,
        outgoing_calls,
    }
}

/// Convert lsp-daemon DaemonStatus to our LspDaemonStatus
fn convert_daemon_status(status: DaemonStatus) -> LspDaemonStatus {
    use std::collections::HashMap;

    let language_pools = status
        .pools
        .into_iter()
        .map(|pool| {
            let pool_status = LanguagePoolStatus {
                language: format!("{:?}", pool.language), // Convert Language enum to string
                ready_servers: pool.ready_servers,
                busy_servers: pool.busy_servers,
                total_servers: pool.total_servers,
                available: pool.ready_servers > 0,
                workspaces: pool.workspaces,
                uptime_secs: pool.uptime_secs,
                status: pool.status,
                readiness_info: pool.readiness_info,
            };
            (format!("{:?}", pool.language), pool_status)
        })
        .collect::<HashMap<String, LanguagePoolStatus>>();

    LspDaemonStatus {
        uptime: std::time::Duration::from_secs(status.uptime_secs),
        total_requests: status.total_requests,
        active_connections: status.active_connections,
        language_pools,
        version: status.version.clone(),
        git_hash: status.git_hash.clone(),
        build_date: status.build_date.clone(),
        universal_cache_stats: status.universal_cache_stats.clone(),
    }
}

impl LspClient {
    // Indexing management methods
    pub async fn start_indexing(
        &mut self,
        workspace_root: PathBuf,
        config: lsp_daemon::protocol::IndexingConfig,
    ) -> Result<String> {
        let request = DaemonRequest::StartIndexing {
            request_id: Uuid::new_v4(),
            workspace_root,
            config,
        };

        let response = self.send_request(request).await?;

        match response {
            DaemonResponse::IndexingStarted { session_id, .. } => Ok(session_id),
            DaemonResponse::Error { error, .. } => Err(anyhow!(error)),
            _ => Err(anyhow!("Unexpected response type")),
        }
    }

    pub async fn stop_indexing(&mut self, force: bool) -> Result<bool> {
        let request = DaemonRequest::StopIndexing {
            request_id: Uuid::new_v4(),
            force,
        };

        let response = self.send_request(request).await?;

        match response {
            DaemonResponse::IndexingStopped { was_running, .. } => Ok(was_running),
            DaemonResponse::Error { error, .. } => Err(anyhow!(error)),
            _ => Err(anyhow!("Unexpected response type")),
        }
    }

    pub async fn get_indexing_status(
        &mut self,
    ) -> Result<lsp_daemon::protocol::IndexingStatusInfo> {
        let request = DaemonRequest::IndexingStatus {
            request_id: Uuid::new_v4(),
        };

        let response = self.send_request(request).await?;

        match response {
            DaemonResponse::IndexingStatusResponse { status, .. } => Ok(status),
            DaemonResponse::Error { error, .. } => Err(anyhow!(error)),
            _ => Err(anyhow!("Unexpected response type")),
        }
    }

    pub async fn get_indexing_config(&mut self) -> Result<lsp_daemon::protocol::IndexingConfig> {
        let request = DaemonRequest::IndexingConfig {
            request_id: Uuid::new_v4(),
        };

        let response = self.send_request(request).await?;

        match response {
            DaemonResponse::IndexingConfigResponse { config, .. } => Ok(config),
            DaemonResponse::Error { error, .. } => Err(anyhow!(error)),
            _ => Err(anyhow!("Unexpected response type")),
        }
    }

    pub async fn set_indexing_config(
        &mut self,
        config: lsp_daemon::protocol::IndexingConfig,
    ) -> Result<()> {
        let request = DaemonRequest::SetIndexingConfig {
            request_id: Uuid::new_v4(),
            config,
        };

        let response = self.send_request(request).await?;

        match response {
            DaemonResponse::IndexingConfigSet { .. } => Ok(()),
            DaemonResponse::Error { error, .. } => Err(anyhow!(error)),
            _ => Err(anyhow!("Unexpected response type")),
        }
    }

    // Workspace cache management methods

    /// List all workspace caches
    pub async fn list_workspace_caches(
        &mut self,
    ) -> Result<Vec<lsp_daemon::protocol::WorkspaceCacheEntry>> {
        let request = DaemonRequest::WorkspaceCacheList {
            request_id: Uuid::new_v4(),
        };

        let response = self.send_request(request).await?;

        match response {
            DaemonResponse::WorkspaceCacheList { workspaces, .. } => Ok(workspaces),
            DaemonResponse::Error { error, .. } => Err(anyhow!(error)),
            _ => Err(anyhow!("Unexpected response type")),
        }
    }

    /// Get detailed information about workspace caches
    pub async fn get_workspace_cache_info(
        &mut self,
        workspace_path: Option<std::path::PathBuf>,
    ) -> Result<Option<Vec<lsp_daemon::protocol::WorkspaceCacheInfo>>> {
        let request = DaemonRequest::WorkspaceCacheInfo {
            request_id: Uuid::new_v4(),
            workspace_path,
        };

        let response = self.send_request(request).await?;

        match response {
            DaemonResponse::WorkspaceCacheInfo {
                workspace_info,
                all_workspaces_info,
                ..
            } => {
                if let Some(single_info) = workspace_info {
                    Ok(Some(vec![*single_info]))
                } else {
                    Ok(all_workspaces_info)
                }
            }
            DaemonResponse::Error { error, .. } => Err(anyhow!(error)),
            _ => Err(anyhow!("Unexpected response type")),
        }
    }

    /// Clear workspace cache(s)
    pub async fn clear_workspace_cache(
        &mut self,
        workspace_path: Option<std::path::PathBuf>,
        older_than_seconds: Option<u64>,
    ) -> Result<lsp_daemon::protocol::WorkspaceClearResult> {
        let request = DaemonRequest::WorkspaceCacheClear {
            request_id: Uuid::new_v4(),
            workspace_path,
            older_than_seconds,
        };

        let response = self.send_request(request).await?;

        match response {
            DaemonResponse::WorkspaceCacheCleared { result, .. } => Ok(result),
            DaemonResponse::Error { error, .. } => Err(anyhow!(error)),
            _ => Err(anyhow!("Unexpected response type")),
        }
    }

    // LSP operations methods

    /// Get definition locations for a symbol
    pub async fn call_definition(
        &mut self,
        file: &Path,
        line: u32,
        column: u32,
    ) -> Result<Vec<lsp_daemon::protocol::Location>> {
        let request = DaemonRequest::Definition {
            request_id: Uuid::new_v4(),
            file_path: file.to_path_buf(),
            line,
            column,
            workspace_hint: self.config.workspace_hint.as_ref().map(PathBuf::from),
        };

        let response = self.send_request(request).await?;

        match response {
            DaemonResponse::Definition { locations, .. } => Ok(locations),
            DaemonResponse::Error { error, .. } => Err(anyhow!(error)),
            _ => Err(anyhow!("Unexpected response type")),
        }
    }

    /// Get reference locations for a symbol
    pub async fn call_references(
        &mut self,
        file: &Path,
        line: u32,
        column: u32,
        include_declaration: bool,
    ) -> Result<Vec<lsp_daemon::protocol::Location>> {
        let request = DaemonRequest::References {
            request_id: Uuid::new_v4(),
            file_path: file.to_path_buf(),
            line,
            column,
            include_declaration,
            workspace_hint: self.config.workspace_hint.as_ref().map(PathBuf::from),
        };

        let response = self.send_request(request).await?;

        match response {
            DaemonResponse::References { locations, .. } => Ok(locations),
            DaemonResponse::Error { error, .. } => Err(anyhow!(error)),
            _ => Err(anyhow!("Unexpected response type")),
        }
    }

    /// Get hover information for a symbol
    pub async fn call_hover(
        &mut self,
        file: &Path,
        line: u32,
        column: u32,
    ) -> Result<Option<lsp_daemon::protocol::HoverContent>> {
        let request = DaemonRequest::Hover {
            request_id: Uuid::new_v4(),
            file_path: file.to_path_buf(),
            line,
            column,
            workspace_hint: self.config.workspace_hint.as_ref().map(PathBuf::from),
        };

        let response = self.send_request(request).await?;

        match response {
            DaemonResponse::Hover { content, .. } => Ok(content),
            DaemonResponse::Error { error, .. } => Err(anyhow!(error)),
            _ => Err(anyhow!("Unexpected response type")),
        }
    }

    /// Get document symbols for a file
    pub async fn call_document_symbols(
        &mut self,
        file: &Path,
    ) -> Result<Vec<lsp_daemon::protocol::DocumentSymbol>> {
        let request = DaemonRequest::DocumentSymbols {
            request_id: Uuid::new_v4(),
            file_path: file.to_path_buf(),
            workspace_hint: self.config.workspace_hint.as_ref().map(PathBuf::from),
        };

        let response = self.send_request(request).await?;

        match response {
            DaemonResponse::DocumentSymbols { symbols, .. } => Ok(symbols),
            DaemonResponse::Error { error, .. } => Err(anyhow!(error)),
            _ => Err(anyhow!("Unexpected response type")),
        }
    }

    /// Get the fully qualified name for a symbol at a specific position
    pub async fn get_symbol_fqn(
        &mut self,
        file_path: &Path,
        line: u32,
        column: u32,
    ) -> Result<String> {
        // Try AST-based FQN extraction first (more reliable)
        if let Ok(fqn) = Self::get_fqn_from_ast(file_path, line, column) {
            if !fqn.is_empty() {
                return Ok(fqn);
            }
        }

        // Fall back to hover-based extraction if AST fails
        let hover = self.call_hover(file_path, line, column).await?;

        // Extract the FQN from hover response if hover exists
        if let Some(hover_content) = hover {
            let fqn = Self::extract_fqn_from_hover(&hover_content)?;
            Ok(fqn)
        } else {
            // No hover info available
            Ok(String::new())
        }
    }

    /// Extract the fully qualified name from hover response
    fn extract_fqn_from_hover(hover: &lsp_daemon::protocol::HoverContent) -> Result<String> {
        // Hover content format varies by language server:
        // rust-analyzer: "probe_code::config::ProbeConfig\n\npub search: Option<SearchConfig>\n\n\nsize = ..."
        // pylsp: "module.ClassName.method\n\nDocstring here..."
        // The first line is the parent's FQN, then we extract the field/method name from the detail line

        let content = &hover.contents;

        // Parse based on the format of hover.contents
        // If it's a JSON object with "kind" and "value" fields, extract the value
        let text = if content.starts_with("{\"kind\":") {
            // It's a JSON object, parse it
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(content) {
                parsed["value"].as_str().unwrap_or(content).to_string()
            } else {
                content.clone()
            }
        } else {
            content.clone()
        };

        // The parent FQN is typically the first line
        let parent_fqn = if let Some(first_line) = text.lines().next() {
            // Clean up the FQN - remove any markdown formatting or extra whitespace
            let cleaned = first_line.trim();

            // If it starts with backticks (markdown code), extract the content
            if cleaned.starts_with('`') && cleaned.ends_with('`') {
                cleaned.trim_matches('`').to_string()
            } else if cleaned.starts_with("```") {
                // Multi-line code block, extract after language specifier
                if let Some(pos) = cleaned.find(' ') {
                    cleaned[pos + 1..].to_string()
                } else {
                    cleaned.to_string()
                }
            } else {
                cleaned.to_string()
            }
        } else {
            // If hover is empty, return empty FQN
            return Ok(String::new());
        };

        // Detect the language-specific separator from the parent FQN
        let separator = if parent_fqn.contains("::") {
            "::" // Rust, C++, Ruby
        } else if parent_fqn.contains('.') {
            "." // Python, JavaScript, TypeScript, Java, Go, C#
        } else if parent_fqn.contains('\\') {
            "\\" // PHP
        } else {
            // Default to :: for Rust since we're in a Rust project
            "::"
        };

        // For field/method access, extract the member name from the detail line
        // Look for patterns like "pub field_name:", "fn method_name(", "const CONST_NAME:", etc.
        let lines: Vec<&str> = text.lines().collect();

        // The member definition is usually on the third line (after parent FQN and empty line)
        if lines.len() > 2 {
            let member_line = lines[2];

            // Try to extract the member name
            if let Some(member_name) = Self::extract_member_name(member_line) {
                // Combine parent FQN with member name using appropriate separator
                return Ok(format!("{}{}{}", parent_fqn, separator, member_name));
            }
        }

        // If we couldn't extract a member name, just return the parent FQN
        Ok(parent_fqn)
    }

    /// Helper function to extract member name from a hover detail line
    fn extract_member_name(line: &str) -> Option<String> {
        let trimmed = line.trim();

        // Rust patterns: "pub field_name: Type", "fn method_name(...)", "const CONST_NAME: Type"
        // Handle "pub fn" as a special case
        if trimmed.starts_with("pub fn ") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 3 {
                let name = parts[2]; // Skip "pub" and "fn", get the actual function name
                                     // Extract just the function name before parentheses or generics
                if let Some(paren_pos) = name.find('(') {
                    return Some(name[..paren_pos].to_string());
                } else if let Some(angle_pos) = name.find('<') {
                    return Some(name[..angle_pos].to_string());
                } else {
                    return Some(name.to_string());
                }
            }
        } else if trimmed.starts_with("pub ")
            || trimmed.starts_with("fn ")
            || trimmed.starts_with("const ")
            || trimmed.starts_with("static ")
        {
            // Split by whitespace and take the second token, then clean it up
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 2 {
                let name = parts[1];
                // Remove trailing : or ( if present
                let name = name.trim_end_matches(':').trim_end_matches('(');
                return Some(name.to_string());
            }
        }

        // Python patterns: "def method_name(...):", "class ClassName:", "variable_name = ..."
        if trimmed.starts_with("def ") || trimmed.starts_with("class ") {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 2 {
                let name = parts[1];
                // Remove trailing ( or : if present
                let name = name.trim_end_matches('(').trim_end_matches(':');
                return Some(name.to_string());
            }
        }

        // JavaScript/TypeScript patterns: "function name(...)", "const name = ...", "let name = ...", "var name = ..."
        if trimmed.starts_with("function ")
            || trimmed.starts_with("const ")
            || trimmed.starts_with("let ")
            || trimmed.starts_with("var ")
        {
            let parts: Vec<&str> = trimmed.split_whitespace().collect();
            if parts.len() >= 2 {
                let name = parts[1];
                // Remove trailing ( or = if present
                let name = name.trim_end_matches('(').trim_end_matches('=');
                return Some(name.to_string());
            }
        }

        // Generic pattern: if the line contains a colon, take everything before it
        if let Some(colon_pos) = trimmed.find(':') {
            let before_colon = &trimmed[..colon_pos];
            // Take the last word before the colon
            if let Some(name) = before_colon.split_whitespace().last() {
                return Some(name.to_string());
            }
        }

        // Generic pattern: if the line contains parentheses, take the word before them
        if let Some(paren_pos) = trimmed.find('(') {
            let before_paren = &trimmed[..paren_pos];
            // Take the last word before the parenthesis
            if let Some(name) = before_paren.split_whitespace().last() {
                return Some(name.to_string());
            }
        }

        None
    }

    /// Search for symbols in the workspace
    pub async fn call_workspace_symbols(
        &mut self,
        query: &str,
        max_results: Option<usize>,
    ) -> Result<Vec<lsp_daemon::protocol::SymbolInformation>> {
        let request = DaemonRequest::WorkspaceSymbols {
            request_id: Uuid::new_v4(),
            query: query.to_string(),
            workspace_hint: self.config.workspace_hint.as_ref().map(PathBuf::from),
        };

        let response = self.send_request(request).await?;

        match response {
            DaemonResponse::WorkspaceSymbols { mut symbols, .. } => {
                if let Some(max) = max_results {
                    symbols.truncate(max);
                }
                Ok(symbols)
            }
            DaemonResponse::Error { error, .. } => Err(anyhow!(error)),
            _ => Err(anyhow!("Unexpected response type")),
        }
    }

    /// Get readiness status for LSP servers
    pub async fn get_readiness_status(
        &mut self,
        file_path: &Path,
    ) -> Result<crate::lsp_integration::readiness::ReadinessCheckResult> {
        // Determine language from file extension to get the right server type
        let language = self.detect_language(file_path);
        let server_type = language.clone().unwrap_or_else(|| "unknown".to_string());

        // Try to get server status from daemon
        match self.get_status().await {
            Ok(status) => {
                // Look for the appropriate language pool
                if let Some(pool) = status.language_pools.get(&server_type) {
                    // Use comprehensive readiness information if available
                    let (is_ready, status_message, expected_timeout) =
                        if let Some(readiness_info) = &pool.readiness_info {
                            // Use the comprehensive readiness detection from ReadinessTracker
                            let timeout_secs = readiness_info.expected_timeout_secs as u64;
                            (
                                readiness_info.is_ready,
                                readiness_info.status_description.clone(),
                                Some(timeout_secs),
                            )
                        } else {
                            // Fallback to basic pool status (less reliable)
                            let basic_ready = pool.ready_servers > 0;
                            let expected_timeout = if basic_ready {
                                Some(0)
                            } else {
                                Some(30) // Default timeout for initialization
                            };
                            let message = if basic_ready {
                                "Ready (basic check)".to_string()
                            } else if pool.busy_servers > 0 {
                                "Initializing (basic check)".to_string()
                            } else {
                                "Starting (basic check)".to_string()
                            };
                            (basic_ready, message, expected_timeout)
                        };

                    Ok(crate::lsp_integration::readiness::ReadinessCheckResult {
                        is_ready,
                        server_type: Some(server_type),
                        expected_timeout_secs: expected_timeout,
                        elapsed_secs: pool.uptime_secs,
                        status_message,
                    })
                } else {
                    // Language not found in pools - may not be supported or daemon not ready
                    Ok(crate::lsp_integration::readiness::ReadinessCheckResult {
                        is_ready: false,
                        server_type: Some(server_type),
                        expected_timeout_secs: Some(30),
                        elapsed_secs: 0,
                        status_message: "Language not supported or daemon not ready".to_string(),
                    })
                }
            }
            Err(e) => {
                // Can't reach daemon - assume not ready
                Ok(crate::lsp_integration::readiness::ReadinessCheckResult {
                    is_ready: false,
                    server_type: Some(server_type),
                    expected_timeout_secs: Some(30),
                    elapsed_secs: 0,
                    status_message: format!("Failed to connect to LSP daemon: {}", e),
                })
            }
        }
    }

    /// Get implementation locations for a symbol
    pub async fn call_implementations(
        &mut self,
        file: &Path,
        line: u32,
        column: u32,
    ) -> Result<Vec<lsp_daemon::protocol::Location>> {
        let request = DaemonRequest::Implementations {
            request_id: Uuid::new_v4(),
            file_path: file.to_path_buf(),
            line,
            column,
            workspace_hint: self.config.workspace_hint.as_ref().map(PathBuf::from),
        };

        let response = self.send_request(request).await?;

        match response {
            DaemonResponse::Implementations { locations, .. } => Ok(locations),
            DaemonResponse::Error { error, .. } => Err(anyhow!(error)),
            _ => Err(anyhow!("Unexpected response type")),
        }
    }

    /// Get type definition locations for a symbol
    pub async fn call_type_definition(
        &mut self,
        file: &Path,
        line: u32,
        column: u32,
    ) -> Result<Vec<lsp_daemon::protocol::Location>> {
        let request = DaemonRequest::TypeDefinition {
            request_id: Uuid::new_v4(),
            file_path: file.to_path_buf(),
            line,
            column,
            workspace_hint: self.config.workspace_hint.as_ref().map(PathBuf::from),
        };

        let response = self.send_request(request).await?;

        match response {
            DaemonResponse::TypeDefinition { locations, .. } => Ok(locations),
            DaemonResponse::Error { error, .. } => Err(anyhow!(error)),
            _ => Err(anyhow!("Unexpected response type")),
        }
    }

    /// Extract FQN using centralized daemon logic
    fn get_fqn_from_ast(file_path: &Path, line: u32, column: u32) -> Result<String> {
        lsp_daemon::fqn::get_fqn_from_ast(file_path, line, column, None)
            .map_err(|e| anyhow::anyhow!("FQN extraction failed: {}", e))
    }

    /// Send cache list keys request
    pub async fn send_cache_list_keys_request(
        &mut self,
        request: DaemonRequest,
    ) -> Result<DaemonResponse> {
        self.send_request(request).await
    }

    /// Send index export request to daemon
    pub async fn export_index(
        &mut self,
        workspace_path: Option<PathBuf>,
        output_path: PathBuf,
        checkpoint: bool,
    ) -> Result<DaemonResponse> {
        let request = DaemonRequest::IndexExport {
            request_id: Uuid::new_v4(),
            workspace_path,
            output_path,
            checkpoint,
        };

        self.send_request(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_is_supported() {
        let config = LspConfig::default();
        let client = LspClient {
            stream: None,
            config,
            daemon_started_by_us: false,
            position_analyzer: PositionAnalyzer::new(),
        };

        // Test supported file types
        assert!(client.is_supported(&PathBuf::from("test.rs")));
        assert!(client.is_supported(&PathBuf::from("test.py")));
        assert!(client.is_supported(&PathBuf::from("test.js")));

        // Test unsupported file type
        assert!(!client.is_supported(&PathBuf::from("test.txt")));
    }
}
