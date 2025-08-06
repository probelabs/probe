use crate::lsp_registry::LspServerConfig;
use crate::socket_path::normalize_executable;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration, Instant};
use tracing::{debug, info, warn};
use url::Url;

pub struct LspServer {
    child: Arc<Mutex<Option<Child>>>,
    stdin: Arc<Mutex<Box<dyn Write + Send>>>,
    stdout: Arc<Mutex<Box<dyn BufRead + Send>>>,
    request_id: Arc<Mutex<i64>>,
    project_root: Option<PathBuf>,
    initialized: bool,
}

impl std::fmt::Debug for LspServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LspServer")
            .field("project_root", &self.project_root)
            .field("initialized", &self.initialized)
            .finish()
    }
}

impl LspServer {
    pub fn spawn_with_workspace(
        config: &LspServerConfig,
        workspace_root: &PathBuf,
    ) -> Result<Self> {
        Self::spawn_internal(config, Some(workspace_root))
    }

    pub fn spawn(config: &LspServerConfig) -> Result<Self> {
        Self::spawn_internal(config, None)
    }

    fn spawn_internal(config: &LspServerConfig, workspace_root: Option<&PathBuf>) -> Result<Self> {
        let command = normalize_executable(&config.command);
        info!("Spawning LSP server: {} {:?}", command, config.args);

        // Set working directory - use workspace root if provided
        let mut cmd = Command::new(&command);
        if let Some(workspace) = workspace_root {
            cmd.current_dir(workspace);
        } else if config.language == crate::language_detector::Language::Go {
            cmd.current_dir("/tmp");
        }

        let mut child = cmd
            .args(&config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| anyhow!("Failed to spawn {}: {}", command, e))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Failed to get stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Failed to get stdout"))?;

        Ok(Self {
            child: Arc::new(Mutex::new(Some(child))),
            stdin: Arc::new(Mutex::new(Box::new(stdin) as Box<dyn Write + Send>)),
            stdout: Arc::new(Mutex::new(
                Box::new(BufReader::new(stdout)) as Box<dyn BufRead + Send>
            )),
            request_id: Arc::new(Mutex::new(1)),
            project_root: None,
            initialized: false,
        })
    }

    pub async fn initialize_with_workspace(
        &mut self,
        config: &LspServerConfig,
        workspace_root: &PathBuf,
    ) -> Result<()> {
        self.initialize_internal(config, Some(workspace_root)).await
    }

    pub async fn initialize(&mut self, config: &LspServerConfig) -> Result<()> {
        self.initialize_internal(config, None).await
    }

    async fn initialize_internal(
        &mut self,
        config: &LspServerConfig,
        workspace_root: Option<&PathBuf>,
    ) -> Result<()> {
        if self.initialized {
            return Ok(());
        }

        // Use provided workspace root, or fallback to current directory or /tmp for Go
        let project_root = if let Some(workspace) = workspace_root {
            workspace.clone()
        } else if config.language == crate::language_detector::Language::Go {
            PathBuf::from("/tmp")
        } else {
            std::env::current_dir()?
        };
        self.project_root = Some(project_root.clone());

        let request_id = self.next_request_id().await;

        let init_params = json!({
            "processId": std::process::id(),
            "rootUri": Url::from_directory_path(&project_root)
                .map_err(|_| anyhow!("Failed to convert path"))?
                .to_string(),
            "capabilities": {
                "textDocument": {
                    "callHierarchy": {
                        "dynamicRegistration": false
                    },
                    "definition": {
                        "dynamicRegistration": false
                    },
                    "references": {
                        "dynamicRegistration": false
                    },
                    "hover": {
                        "dynamicRegistration": false
                    },
                    "completion": {
                        "dynamicRegistration": false,
                        "completionItem": {
                            "snippetSupport": true
                        }
                    }
                },
                "window": {
                    "workDoneProgress": true
                },
                "experimental": {
                    "statusNotification": true
                }
            },
            "initializationOptions": config.initialization_options
        });

        self.send_request("initialize", init_params, request_id)
            .await?;

        // Wait for initialize response
        let response = self
            .wait_for_response(
                request_id,
                Duration::from_secs(config.initialization_timeout_secs),
            )
            .await?;

        if response.get("error").is_some() {
            return Err(anyhow!("Initialize failed: {:?}", response["error"]));
        }

        // Send initialized notification
        self.send_notification("initialized", json!({})).await?;

        self.initialized = true;
        info!("LSP server initialized for {:?}", config.language);

        Ok(())
    }

    pub async fn wait_until_ready(&mut self) -> Result<()> {
        // This method monitors LSP server messages to determine when it's ready
        // Similar to the original implementation but adapted for async

        let start = Instant::now();
        let max_wait = Duration::from_secs(30);
        let required_silence = Duration::from_secs(1);

        let mut cache_priming_completed = false;
        let mut silence_start: Option<Instant> = None;

        while start.elapsed() < max_wait {
            // Try to read a message with timeout
            match self.read_message_timeout(Duration::from_millis(100)).await {
                Ok(Some(msg)) => {
                    silence_start = None;

                    if let Some(method) = msg.get("method").and_then(|m| m.as_str()) {
                        // Handle progress notifications
                        if method == "$/progress" {
                            if let Some(params) = msg.get("params") {
                                if let Some(token) = params.get("token").and_then(|t| t.as_str()) {
                                    if let Some(value) = params.get("value") {
                                        if let Some(kind) =
                                            value.get("kind").and_then(|k| k.as_str())
                                        {
                                            if kind == "end" && token.contains("cachePriming") {
                                                cache_priming_completed = true;
                                                debug!("Cache priming completed");
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Handle status notifications
                        if method == "rust-analyzer/status" {
                            if let Some(params) = msg.get("params") {
                                if let Some(status) = params.as_str() {
                                    if status == "ready" {
                                        debug!("LSP server reports ready");
                                        if cache_priming_completed {
                                            return Ok(());
                                        }
                                    }
                                }
                            }
                        }

                        // Respond to window/workDoneProgress/create requests
                        if method == "window/workDoneProgress/create" {
                            if let Some(id) = msg.get("id") {
                                self.send_response(id.as_i64().unwrap_or(0), json!(null))
                                    .await?;
                            }
                        }
                    }
                }
                Ok(None) | Err(_) => {
                    // No message available
                    if silence_start.is_none() {
                        silence_start = Some(Instant::now());
                    }

                    if let Some(silence_time) = silence_start {
                        if cache_priming_completed && silence_time.elapsed() >= required_silence {
                            debug!("Server ready after cache priming and silence period");
                            return Ok(());
                        }
                    }
                }
            }

            // Small delay before next iteration
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        // If we've waited long enough, assume ready
        warn!("LSP server readiness timeout, proceeding anyway");
        Ok(())
    }

    async fn next_request_id(&self) -> i64 {
        let mut id = self.request_id.lock().await;
        let current = *id;
        *id += 1;
        current
    }

    async fn send_message(&self, msg: &Value) -> Result<()> {
        let bytes = msg.to_string();
        let message = format!("Content-Length: {}\r\n\r\n{}", bytes.len(), bytes);

        let mut stdin = self.stdin.lock().await;
        stdin.write_all(message.as_bytes())?;
        stdin.flush()?;

        Ok(())
    }

    pub async fn send_request(&self, method: &str, params: Value, id: i64) -> Result<()> {
        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        });

        self.send_message(&msg).await
    }

    pub async fn send_notification(&self, method: &str, params: Value) -> Result<()> {
        let msg = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params
        });

        self.send_message(&msg).await
    }

    async fn send_response(&self, id: i64, result: Value) -> Result<()> {
        let msg = json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result
        });

        self.send_message(&msg).await
    }

    async fn read_message(&self) -> Result<Value> {
        let mut stdout = self.stdout.lock().await;

        let mut header = String::new();
        stdout.read_line(&mut header)?;

        if !header.starts_with("Content-Length:") {
            return Err(anyhow!("Invalid header: {}", header));
        }

        let len: usize = header["Content-Length:".len()..].trim().parse()?;

        // Skip empty line
        stdout.read_line(&mut String::new())?;

        let mut body = vec![0; len];
        stdout.read_exact(&mut body)?;

        Ok(serde_json::from_slice(&body)?)
    }

    async fn read_message_timeout(&self, duration: Duration) -> Result<Option<Value>> {
        match timeout(duration, self.read_message()).await {
            Ok(Ok(msg)) => Ok(Some(msg)),
            Ok(Err(e)) => Err(e),
            Err(_) => Ok(None), // Timeout
        }
    }

    async fn wait_for_response(&self, id: i64, timeout_duration: Duration) -> Result<Value> {
        let start = Instant::now();

        while start.elapsed() < timeout_duration {
            if let Some(msg) = self
                .read_message_timeout(Duration::from_millis(100))
                .await?
            {
                if msg.get("id").and_then(|i| i.as_i64()) == Some(id) {
                    return Ok(msg);
                }
            }
        }

        Err(anyhow!("Timeout waiting for response to request {}", id))
    }

    pub async fn open_document(&self, file_path: &Path, content: &str) -> Result<()> {
        let uri =
            Url::from_file_path(file_path).map_err(|_| anyhow!("Failed to convert file path"))?;

        let params = json!({
            "textDocument": {
                "uri": uri.to_string(),
                "languageId": self.detect_language_id(file_path),
                "version": 1,
                "text": content
            }
        });

        self.send_notification("textDocument/didOpen", params).await
    }

    pub async fn close_document(&self, file_path: &Path) -> Result<()> {
        let uri =
            Url::from_file_path(file_path).map_err(|_| anyhow!("Failed to convert file path"))?;

        let params = json!({
            "textDocument": {
                "uri": uri.to_string()
            }
        });

        self.send_notification("textDocument/didClose", params)
            .await
    }

    pub async fn call_hierarchy(&self, file_path: &Path, line: u32, column: u32) -> Result<Value> {
        let uri =
            Url::from_file_path(file_path).map_err(|_| anyhow!("Failed to convert file path"))?;

        let request_id = self.next_request_id().await;

        // Prepare call hierarchy
        let params = json!({
            "textDocument": { "uri": uri.to_string() },
            "position": { "line": line, "character": column }
        });

        self.send_request("textDocument/prepareCallHierarchy", params, request_id)
            .await?;
        let response = self
            .wait_for_response(request_id, Duration::from_secs(5))
            .await?;

        if let Some(error) = response.get("error") {
            return Err(anyhow!("Call hierarchy prepare failed: {:?}", error));
        }

        let items = response["result"]
            .as_array()
            .ok_or_else(|| anyhow!("No call hierarchy items found"))?;

        if items.is_empty() {
            return Ok(json!({
                "incoming": [],
                "outgoing": []
            }));
        }

        let item = &items[0];

        // Get incoming calls
        let request_id = self.next_request_id().await;
        self.send_request(
            "callHierarchy/incomingCalls",
            json!({ "item": item }),
            request_id,
        )
        .await?;
        let incoming_response = self
            .wait_for_response(request_id, Duration::from_secs(5))
            .await?;

        // Get outgoing calls
        let request_id = self.next_request_id().await;
        self.send_request(
            "callHierarchy/outgoingCalls",
            json!({ "item": item }),
            request_id,
        )
        .await?;
        let outgoing_response = self
            .wait_for_response(request_id, Duration::from_secs(5))
            .await?;

        Ok(json!({
            "item": item,
            "incoming": incoming_response["result"],
            "outgoing": outgoing_response["result"]
        }))
    }

    pub async fn shutdown(&self) -> Result<()> {
        let request_id = self.next_request_id().await;
        self.send_request("shutdown", json!(null), request_id)
            .await?;

        // Wait for shutdown response
        let _ = self
            .wait_for_response(request_id, Duration::from_secs(2))
            .await;

        // Send exit notification
        self.send_notification("exit", json!(null)).await?;

        // Kill the process if still running
        let mut child_opt = self.child.lock().await;
        if let Some(ref mut child) = *child_opt {
            let _ = child.kill();
        }

        Ok(())
    }

    fn detect_language_id(&self, file_path: &Path) -> &str {
        match file_path.extension().and_then(|e| e.to_str()) {
            Some("rs") => "rust",
            Some("ts") | Some("tsx") => "typescript",
            Some("js") | Some("jsx") => "javascript",
            Some("py") => "python",
            Some("go") => "go",
            Some("java") => "java",
            Some("c") | Some("h") => "c",
            Some("cpp") | Some("cxx") | Some("cc") | Some("hpp") => "cpp",
            Some("cs") => "csharp",
            Some("rb") => "ruby",
            Some("php") => "php",
            Some("swift") => "swift",
            Some("kt") | Some("kts") => "kotlin",
            Some("scala") | Some("sc") => "scala",
            Some("hs") => "haskell",
            Some("ex") | Some("exs") => "elixir",
            Some("clj") | Some("cljs") => "clojure",
            Some("lua") => "lua",
            Some("zig") => "zig",
            _ => "plaintext",
        }
    }
}
