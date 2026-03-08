//! Mock LSP server implementation with configurable response patterns
//!
//! This module provides a mock LSP server that can simulate various language server
//! behaviors for testing purposes. It supports configurable response patterns,
//! delays, errors, and timeouts.

use super::protocol::{
    default_initialize_result, LspError, LspNotification, LspRequest, LspResponse,
};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::sleep;

/// Configuration for mock response patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MockServerConfig {
    /// Server name for identification
    pub server_name: String,
    /// Default response patterns by method
    pub method_patterns: HashMap<String, MockResponsePattern>,
    /// Global delay before all responses (in milliseconds)
    pub global_delay_ms: Option<u64>,
    /// Whether to enable verbose logging
    pub verbose: bool,
}

impl Default for MockServerConfig {
    fn default() -> Self {
        Self {
            server_name: "mock-lsp-server".to_string(),
            method_patterns: HashMap::new(),
            global_delay_ms: None,
            verbose: false,
        }
    }
}

/// Configurable response pattern for LSP methods
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MockResponsePattern {
    /// Return a successful response with data
    Success {
        result: Value,
        delay_ms: Option<u64>,
    },
    /// Return an empty array []
    EmptyArray { delay_ms: Option<u64> },
    /// Return null
    Null { delay_ms: Option<u64> },
    /// Return an error
    Error {
        code: i32,
        message: String,
        data: Option<Value>,
        delay_ms: Option<u64>,
    },
    /// Never respond (timeout simulation)
    Timeout,
    /// Respond with a sequence of patterns (for testing retry logic)
    Sequence {
        patterns: Vec<MockResponsePattern>,
        current_index: usize,
    },
}

impl Default for MockResponsePattern {
    fn default() -> Self {
        MockResponsePattern::EmptyArray { delay_ms: None }
    }
}

/// Mock LSP server that can simulate different language server behaviors
pub struct MockLspServer {
    config: MockServerConfig,
    process: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout: Option<BufReader<ChildStdout>>,
    request_count: Arc<RwLock<HashMap<String, usize>>>,
    initialized: Arc<RwLock<bool>>,
}

impl MockLspServer {
    /// Create a new mock LSP server with the given configuration
    pub fn new(config: MockServerConfig) -> Self {
        Self {
            config,
            process: None,
            stdin: None,
            stdout: None,
            request_count: Arc::new(RwLock::new(HashMap::new())),
            initialized: Arc::new(RwLock::new(false)),
        }
    }

    /// Start the mock server as a subprocess that communicates via stdio
    pub async fn start(&mut self) -> Result<()> {
        // Create a subprocess that runs this mock server
        let mut child = Command::new("cargo")
            .args(&[
                "run",
                "--bin",
                "mock-lsp-server-subprocess",
                "--",
                &serde_json::to_string(&self.config)?,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("Failed to get stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Failed to get stdout"))?;

        self.stdin = Some(stdin);
        self.stdout = Some(BufReader::new(stdout));
        self.process = Some(child);

        Ok(())
    }

    /// Stop the mock server
    pub async fn stop(&mut self) -> Result<()> {
        if let Some(mut process) = self.process.take() {
            // Try to terminate gracefully first
            if let Some(mut stdin) = self.stdin.take() {
                let shutdown_request = LspRequest {
                    jsonrpc: "2.0".to_string(),
                    id: Some(Value::Number(serde_json::Number::from(999))),
                    method: "shutdown".to_string(),
                    params: None,
                };

                let request_str = serde_json::to_string(&shutdown_request)?;
                let message = format!(
                    "Content-Length: {}\r\n\r\n{}",
                    request_str.len(),
                    request_str
                );
                let _ = stdin.write_all(message.as_bytes());
                let _ = stdin.flush();

                // Give the process a moment to shut down gracefully
                sleep(Duration::from_millis(100)).await;
            }

            // Force kill if still running
            let _ = process.kill();
            let _ = process.wait();
        }

        self.stdin = None;
        self.stdout = None;
        Ok(())
    }

    /// Send a request to the mock server and get response
    pub async fn send_request(&mut self, request: LspRequest) -> Result<Option<LspResponse>> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow!("Server not started"))?;

        // Serialize request
        let request_str = serde_json::to_string(&request)?;
        let message = format!(
            "Content-Length: {}\r\n\r\n{}",
            request_str.len(),
            request_str
        );

        if self.config.verbose {
            eprintln!("Sending request: {}", request_str);
        }

        // Send request
        stdin.write_all(message.as_bytes())?;
        stdin.flush()?;

        // Read response if this is a request (has id)
        if request.id.is_some() {
            self.read_response().await
        } else {
            Ok(None) // Notification
        }
    }

    /// Read a response from the mock server
    async fn read_response(&mut self) -> Result<Option<LspResponse>> {
        let stdout = self
            .stdout
            .as_mut()
            .ok_or_else(|| anyhow!("Server not started"))?;

        // Read Content-Length header
        let mut header_line = String::new();
        stdout.read_line(&mut header_line)?;

        if !header_line.starts_with("Content-Length:") {
            return Err(anyhow!("Invalid response header: {}", header_line));
        }

        let content_length: usize = header_line
            .trim_start_matches("Content-Length:")
            .trim()
            .parse()?;

        // Read empty line
        let mut empty_line = String::new();
        stdout.read_line(&mut empty_line)?;

        // Read content
        let mut content = vec![0u8; content_length];
        std::io::Read::read_exact(stdout, &mut content)?;

        let response_str = String::from_utf8(content)?;

        if self.config.verbose {
            eprintln!("Received response: {}", response_str);
        }

        let response: LspResponse = serde_json::from_str(&response_str)?;
        Ok(Some(response))
    }

    /// Set a response pattern for a specific method
    pub async fn set_method_pattern(&mut self, method: String, pattern: MockResponsePattern) {
        self.config.method_patterns.insert(method, pattern);
    }

    /// Get the number of times a method has been called
    pub async fn get_request_count(&self, method: &str) -> usize {
        self.request_count
            .read()
            .await
            .get(method)
            .copied()
            .unwrap_or(0)
    }

    /// Reset all request counts
    pub async fn reset_request_counts(&self) {
        self.request_count.write().await.clear();
    }

    /// Check if the server has been initialized
    pub async fn is_initialized(&self) -> bool {
        *self.initialized.read().await
    }
}

/// Standalone mock server process that handles LSP protocol
pub struct MockLspServerProcess {
    config: MockServerConfig,
    request_count: HashMap<String, usize>,
    initialized: bool,
}

impl MockLspServerProcess {
    pub fn new(config: MockServerConfig) -> Self {
        Self {
            config,
            request_count: HashMap::new(),
            initialized: false,
        }
    }

    /// Run the mock server process (reads from stdin, writes to stdout)
    pub async fn run(&mut self) -> Result<()> {
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();

        loop {
            // Read LSP message from stdin
            let message = match self.read_lsp_message(&stdin) {
                Ok(msg) => msg,
                Err(e) => {
                    if self.config.verbose {
                        eprintln!("Error reading message: {}", e);
                    }
                    continue;
                }
            };

            if self.config.verbose {
                eprintln!("Received message: {}", message);
            }

            // Parse as LSP request or notification
            if let Ok(request) = serde_json::from_str::<LspRequest>(&message) {
                // Handle request
                if let Some(response) = self.handle_request(request).await? {
                    let response_str = serde_json::to_string(&response)?;
                    let lsp_message = format!(
                        "Content-Length: {}\r\n\r\n{}",
                        response_str.len(),
                        response_str
                    );

                    if self.config.verbose {
                        eprintln!("Sending response: {}", response_str);
                    }

                    stdout.write_all(lsp_message.as_bytes())?;
                    stdout.flush()?;
                }
            } else if let Ok(notification) = serde_json::from_str::<LspNotification>(&message) {
                // Handle notification
                self.handle_notification(notification).await?;
            }
        }
    }

    /// Read an LSP message from stdin
    fn read_lsp_message(&self, stdin: &std::io::Stdin) -> Result<String> {
        let stdin_lock = stdin.lock();
        let mut lines = stdin_lock.lines();

        // Read Content-Length header
        let header_line = lines.next().ok_or_else(|| anyhow!("EOF"))??;

        if !header_line.starts_with("Content-Length:") {
            return Err(anyhow!("Invalid header: {}", header_line));
        }

        let content_length: usize = header_line
            .trim_start_matches("Content-Length:")
            .trim()
            .parse()?;

        // Read empty line
        let _empty_line = lines
            .next()
            .ok_or_else(|| anyhow!("Missing empty line"))??;

        // Read content
        let mut content = vec![0u8; content_length];
        std::io::Read::read_exact(&mut stdin.lock(), &mut content)?;

        Ok(String::from_utf8(content)?)
    }

    /// Handle an LSP request
    async fn handle_request(&mut self, request: LspRequest) -> Result<Option<LspResponse>> {
        // Increment request count
        *self
            .request_count
            .entry(request.method.clone())
            .or_insert(0) += 1;

        let method = &request.method;
        let id = request.id.clone();

        // Handle shutdown request
        if method == "shutdown" {
            return Ok(Some(LspResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(Value::Null),
                error: None,
            }));
        }

        // Handle initialize request specially
        if method == "initialize" {
            self.initialized = true;
            let result = default_initialize_result(&self.config.server_name);
            return Ok(Some(LspResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(serde_json::to_value(result)?),
                error: None,
            }));
        }

        // Get pattern for this method
        let pattern = self
            .config
            .method_patterns
            .get(method)
            .cloned()
            .unwrap_or_default();

        // Apply global delay
        if let Some(delay_ms) = self.config.global_delay_ms {
            sleep(Duration::from_millis(delay_ms)).await;
        }

        // Generate response based on pattern
        self.generate_response(pattern, id).await
    }

    /// Handle an LSP notification
    async fn handle_notification(&mut self, notification: LspNotification) -> Result<()> {
        // Increment request count
        *self
            .request_count
            .entry(notification.method.clone())
            .or_insert(0) += 1;

        if self.config.verbose {
            eprintln!("Handled notification: {}", notification.method);
        }

        // Handle exit notification
        if notification.method == "exit" {
            std::process::exit(0);
        }

        Ok(())
    }

    /// Generate response based on pattern
    async fn generate_response(
        &mut self,
        pattern: MockResponsePattern,
        id: Option<Value>,
    ) -> Result<Option<LspResponse>> {
        self.generate_response_inner(pattern, id, 0).await
    }

    /// Internal recursive helper with recursion depth tracking
    fn generate_response_inner(
        &mut self,
        pattern: MockResponsePattern,
        id: Option<Value>,
        depth: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Option<LspResponse>>> + Send + '_>>
    {
        Box::pin(async move {
            // Prevent infinite recursion
            if depth > 100 {
                return Ok(Some(LspResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(Value::Array(vec![])),
                    error: None,
                }));
            }

            match pattern {
                MockResponsePattern::Success { result, delay_ms } => {
                    if let Some(delay_ms) = delay_ms {
                        sleep(Duration::from_millis(delay_ms)).await;
                    }
                    Ok(Some(LspResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: Some(result),
                        error: None,
                    }))
                }
                MockResponsePattern::EmptyArray { delay_ms } => {
                    if let Some(delay_ms) = delay_ms {
                        sleep(Duration::from_millis(delay_ms)).await;
                    }
                    Ok(Some(LspResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: Some(Value::Array(vec![])),
                        error: None,
                    }))
                }
                MockResponsePattern::Null { delay_ms } => {
                    if let Some(delay_ms) = delay_ms {
                        sleep(Duration::from_millis(delay_ms)).await;
                    }
                    Ok(Some(LspResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: Some(Value::Null),
                        error: None,
                    }))
                }
                MockResponsePattern::Error {
                    code,
                    message,
                    data,
                    delay_ms,
                } => {
                    if let Some(delay_ms) = delay_ms {
                        sleep(Duration::from_millis(delay_ms)).await;
                    }
                    Ok(Some(LspResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: None,
                        error: Some(LspError {
                            code,
                            message,
                            data,
                        }),
                    }))
                }
                MockResponsePattern::Timeout => {
                    // Never respond - this simulates a timeout
                    loop {
                        sleep(Duration::from_secs(3600)).await; // Sleep forever
                    }
                }
                MockResponsePattern::Sequence {
                    mut patterns,
                    current_index,
                } => {
                    if current_index < patterns.len() {
                        let pattern = patterns.remove(current_index);
                        self.generate_response_inner(pattern, id, depth + 1).await
                    } else {
                        // Default to empty array when sequence is exhausted
                        self.generate_response_inner(
                            MockResponsePattern::EmptyArray { delay_ms: None },
                            id,
                            depth + 1,
                        )
                        .await
                    }
                }
            }
        })
    }
}

impl Drop for MockLspServer {
    fn drop(&mut self) {
        // Try to clean up the process
        if let Some(mut process) = self.process.take() {
            let _ = process.kill();
            let _ = process.wait();
        }
    }
}
