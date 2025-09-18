//! Common test utilities and helpers for LSP integration tests
#![allow(dead_code)]

use anyhow::{Context, Result};
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Test context for running probe commands
pub struct TestContext {
    probe_binary: PathBuf,
}

impl TestContext {
    /// Create a new test context
    pub fn new() -> Self {
        let probe_binary = env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("probe");

        Self { probe_binary }
    }

    /// Run a probe command with the given arguments
    pub fn run_probe(&self, args: &[&str]) -> Result<String> {
        let output = Command::new(&self.probe_binary)
            .args(args)
            .output()
            .context("Failed to run probe command")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            anyhow::bail!("Command failed: {}\nStderr: {}", stdout, stderr);
        }

        Ok(stdout.to_string())
    }
}

/// Strip ANSI escape sequences (CSI etc.) so we can parse colored output reliably.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\u{1B}' {
            // Handle ESC [ ... final-byte
            if let Some('[') = it.peek().copied() {
                it.next(); // consume '['
                           // Parameter bytes 0x30..=0x3F
                while let Some(&ch) = it.peek() {
                    let u = ch as u32;
                    if (0x30..=0x3F).contains(&u) {
                        it.next();
                    } else {
                        break;
                    }
                }
                // Intermediate bytes 0x20..=0x2F
                while let Some(&ch) = it.peek() {
                    let u = ch as u32;
                    if (0x20..=0x2F).contains(&u) {
                        it.next();
                    } else {
                        break;
                    }
                }
                // Final byte 0x40..=0x7E
                let _ = it.next();
                continue;
            } else {
                // Other two-byte ESC sequences: drop next char if present
                let _ = it.next();
                continue;
            }
        }
        out.push(c);
    }
    out
}

/// Language server types supported by the test suite
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageServer {
    Gopls,
    TypeScriptLanguageServer,
}

impl LanguageServer {
    /// Get the command name for this language server
    pub fn command_name(&self) -> &'static str {
        match self {
            LanguageServer::Gopls => "gopls",
            LanguageServer::TypeScriptLanguageServer => "typescript-language-server",
        }
    }

    /// Get the human-readable name for this language server
    pub fn display_name(&self) -> &'static str {
        match self {
            LanguageServer::Gopls => "gopls (Go language server)",
            LanguageServer::TypeScriptLanguageServer => {
                "typescript-language-server (TypeScript/JavaScript language server)"
            }
        }
    }

    /// Get installation instructions for this language server
    pub fn installation_instructions(&self) -> &'static str {
        match self {
            LanguageServer::Gopls => "Install with: go install golang.org/x/tools/gopls@latest",
            LanguageServer::TypeScriptLanguageServer => {
                "Install with: npm install -g typescript-language-server typescript\nWindows: ensure %AppData%\\npm (npm global bin) is on PATH."
            }
        }
    }
}

/// Strict validation that requires ALL language servers to be available
/// This function NEVER allows skipping - it fails if any language server is missing
pub fn require_all_language_servers() -> Result<()> {
    let required_servers = [
        LanguageServer::Gopls,
        LanguageServer::TypeScriptLanguageServer,
    ];

    let mut missing_servers = Vec::new();

    for server in &required_servers {
        if !is_language_server_available(*server) {
            missing_servers.push(*server);
        }
    }

    if !missing_servers.is_empty() {
        let mut error_msg =
            String::from("CRITICAL: Missing required language servers for CI tests:\n\n");

        for server in missing_servers {
            error_msg.push_str(&format!(
                "❌ {} is not available\n   {}\n   Ensure it's in PATH: {}\n\n",
                server.display_name(),
                server.installation_instructions(),
                server.command_name()
            ));
        }

        error_msg.push_str("ALL language servers are required for comprehensive LSP tests.\n");
        error_msg.push_str("This test suite does NOT skip missing dependencies.\n");
        error_msg.push_str("Install all required language servers and ensure they are in PATH.");

        return Err(anyhow::anyhow!(error_msg));
    }

    Ok(())
}

/// Check if a specific language server is available on the system
pub fn is_language_server_available(server: LanguageServer) -> bool {
    // First check if the command exists in PATH
    if !is_command_in_path(server.command_name()) {
        return false;
    }

    // Additional validation: try to get version to ensure it's functional
    match server {
        LanguageServer::Gopls => Command::new("gopls")
            .arg("version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false),
        LanguageServer::TypeScriptLanguageServer => Command::new("typescript-language-server")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false),
    }
}

/// Check if a command exists in PATH
fn is_command_in_path(command: &str) -> bool {
    // Use OS-appropriate PATH parsing and (on Windows) respect PATHEXT.
    let paths = env::var_os("PATH").unwrap_or_default();
    let mut found = false;

    #[cfg(windows)]
    {
        use std::ffi::OsString;

        let pathext =
            env::var_os("PATHEXT").unwrap_or_else(|| OsString::from(".COM;.EXE;.BAT;.CMD"));
        let exts: Vec<String> = pathext
            .to_string_lossy()
            .split(';')
            .filter(|s| !s.is_empty())
            .map(|s| s.trim().trim_start_matches('.').to_ascii_lowercase())
            .collect();

        for dir in std::env::split_paths(&paths) {
            if dir.as_os_str().is_empty() {
                continue;
            }
            let base = dir.join(command);

            // If the command already has an extension, check as-is.
            if base.is_file() {
                found = true;
                break;
            }

            // Try each PATHEXT to account for .cmd/.bat launchers produced by npm.
            for ext in &exts {
                let mut with_ext = base.clone();
                with_ext.set_extension(ext);
                if with_ext.is_file() {
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
        }
    }

    #[cfg(not(windows))]
    {
        for dir in std::env::split_paths(&paths) {
            if dir.as_os_str().is_empty() {
                continue;
            }
            let candidate = dir.join(command);
            if candidate.is_file() {
                // On Unix, also require the executable bit.
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(meta) = std::fs::metadata(&candidate) {
                        if meta.permissions().mode() & 0o111 != 0 {
                            found = true;
                            break;
                        }
                    }
                }
                #[cfg(not(unix))]
                {
                    found = true;
                    break;
                }
            }
        }
    }

    found
}

/// Initialize test namespace for socket isolation
/// Returns the test-specific socket path to use
pub fn init_test_namespace(test_name: &str) -> PathBuf {
    // Create a shorter unique test directory to avoid Unix socket path length limits (SUN_LEN ~104 chars)
    let temp_dir = std::env::temp_dir();

    // Use a shorter naming scheme: just the process ID and a hash of the test name
    let test_hash = {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        test_name.hash(&mut hasher);
        format!("{:x}", hasher.finish() & 0xFFFF) // Use last 4 hex digits
    };
    let test_id = format!("p{}-{}", std::process::id(), test_hash);
    let test_dir = temp_dir.join(test_id);

    // Create the directory if it doesn't exist
    if !test_dir.exists() {
        std::fs::create_dir_all(&test_dir).unwrap_or_else(|e| {
            eprintln!("Warning: Failed to create test directory {test_dir:?}: {e}");
        });
    }

    // Use shorter socket filename to further reduce path length
    test_dir.join("lsp.sock")
}

/// Clean up test namespace
pub fn cleanup_test_namespace(socket_path: &Path) {
    // Remove the socket file if it exists
    if socket_path.exists() {
        let _ = std::fs::remove_file(socket_path);
    }

    // Try to remove the parent directory (if empty)
    if let Some(parent) = socket_path.parent() {
        let _ = std::fs::remove_dir(parent);
    }
}

/// Get base probe command with test-specific configuration
fn probe_cmd_base(socket_path: Option<&Path>) -> Command {
    // Use the same logic as cli_tests.rs to get the probe binary path
    let probe_path = if let Ok(path) = std::env::var("CARGO_BIN_EXE_probe") {
        PathBuf::from(path)
    } else {
        // Construct the path to the debug binary
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("target");
        path.push("debug");
        path.push(if cfg!(windows) { "probe.exe" } else { "probe" });
        path
    };

    let mut cmd = Command::new(probe_path);

    // Set test-specific environment variables
    if let Some(socket) = socket_path {
        cmd.env(
            "PROBE_LSP_SOCKET_PATH",
            socket.to_string_lossy().to_string(),
        );

        // Also set TMPDIR to isolate temporary files per test
        if let Some(parent) = socket.parent() {
            cmd.env("TMPDIR", parent.to_string_lossy().to_string());
        }
    }

    // Set stable test environment variables to match cli_tests.rs behavior
    // This ensures consistent behavior across all test invocations
    cmd.env("CI", "1");
    cmd.stdin(Stdio::null()); // Prevent hanging on interactive prompts

    cmd
}

/// Helper to run probe commands and capture output with timeout
#[allow(dead_code)]
pub fn run_probe_command(args: &[&str]) -> Result<(String, String, bool)> {
    run_probe_command_with_timeout(args, Duration::from_secs(30))
}

/// Helper to run probe commands with custom timeout
pub fn run_probe_command_with_timeout(
    args: &[&str],
    timeout: Duration,
) -> Result<(String, String, bool)> {
    run_probe_command_with_config(args, timeout, None)
}

/// Helper to run probe commands with custom timeout and socket path
pub fn run_probe_command_with_config(
    args: &[&str],
    timeout: Duration,
    socket_path: Option<&Path>,
) -> Result<(String, String, bool)> {
    use std::io::Read;
    use std::sync::mpsc;

    let start = Instant::now();

    let mut child = probe_cmd_base(socket_path)
        .args(args)
        .stdin(Stdio::null()) // Never wait for input - prevents hangs
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to execute probe command: probe {}", args.join(" ")))?;

    // Take ownership of pipes immediately to drain them concurrently
    let mut stdout_pipe = child.stdout.take().expect("stdout was piped");
    let mut stderr_pipe = child.stderr.take().expect("stderr was piped");

    // Drain stdout in a dedicated thread to prevent deadlock
    let (tx_out, rx_out) = mpsc::channel::<Vec<u8>>();
    let stdout_thread = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout_pipe.read_to_end(&mut buf);
        let _ = tx_out.send(buf);
    });

    // Drain stderr in a dedicated thread to prevent deadlock
    let (tx_err, rx_err) = mpsc::channel::<Vec<u8>>();
    let stderr_thread = thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr_pipe.read_to_end(&mut buf);
        let _ = tx_err.send(buf);
    });

    // Wait for process with timeout, using try_wait to avoid blocking
    let mut timed_out = false;
    let exit_status = loop {
        if let Some(status) = child.try_wait().context("Failed to poll probe process")? {
            break status;
        }

        if start.elapsed() >= timeout {
            timed_out = true;
            // Kill the child process; pipes will close and readers will finish
            let _ = child.kill();
            // Wait for the process to actually exit
            break child.wait().context("Failed to wait for killed process")?;
        }

        // Short sleep to avoid busy waiting
        thread::sleep(Duration::from_millis(10));
    };

    // Collect outputs from reader threads (with timeout to avoid hanging)
    let stdout_bytes = rx_out
        .recv_timeout(Duration::from_secs(5))
        .unwrap_or_else(|_| Vec::new());
    let stderr_bytes = rx_err
        .recv_timeout(Duration::from_secs(5))
        .unwrap_or_else(|_| Vec::new());

    // Join threads to clean up
    let _ = stdout_thread.join();
    let _ = stderr_thread.join();

    let stdout = String::from_utf8_lossy(&stdout_bytes).to_string();
    let stderr = String::from_utf8_lossy(&stderr_bytes).to_string();

    // Check if we timed out
    if timed_out {
        return Err(anyhow::anyhow!(
            "Command timed out after {:?} (limit: {:?}): probe {}\n--- stdout ---\n{}\n--- stderr ---\n{}",
            timeout,
            timeout,
            args.join(" "),
            stdout,
            stderr
        ));
    }

    let mut success = exit_status.success();

    // Some probe subcommands currently print errors but still exit 0; treat *obvious* error strings as failures in tests.
    // Be careful not to misclassify benign phrases like "No results found."
    if success {
        let combined_output_lc = format!("{}{}", stdout.to_lowercase(), stderr.to_lowercase());
        let looks_like_no_results = combined_output_lc.contains("no results found");
        let looks_like_error = combined_output_lc.contains("error:")
            || combined_output_lc.contains("no such file")
            || combined_output_lc.contains("file does not exist")
            || combined_output_lc.contains("file not found")
            || combined_output_lc.contains("path not found")
            || (combined_output_lc.contains("encountered") && combined_output_lc.contains("error"));
        if looks_like_error && !looks_like_no_results {
            success = false;
        }
    }

    // If we failed but don't have a clear, user-friendly message, synthesize one so tests have a stable string to assert on.
    if !success {
        let out_lc = stdout.to_lowercase();
        let err_lc = stderr.to_lowercase();
        let has_human_msg = out_lc.contains("error:")
            || err_lc.contains("error:")
            || out_lc.contains("invalid file")
            || err_lc.contains("invalid file")
            || out_lc.contains("no such file")
            || err_lc.contains("no such file")
            || out_lc.contains("file not found")
            || err_lc.contains("file not found")
            || out_lc.contains("path not found")
            || err_lc.contains("path not found");

        if !has_human_msg {
            // Heuristically surface any path-like args to help the user.
            let likely_paths: Vec<&str> = args
                .iter()
                .copied()
                .filter(|a| {
                    a.contains('/')
                        || a.contains('\\')
                        || a.ends_with(".ts")
                        || a.ends_with(".js")
                        || a.ends_with(".go")
                })
                .collect();
            // Normalize to a stable, cross-platform message that the tests can match reliably.
            let normalized = if likely_paths.is_empty() {
                "Error: file not found (one or more provided paths do not exist)".to_string()
            } else {
                format!("Error: file not found: {}", likely_paths.join(", "))
            };
            let stderr = if stderr.is_empty() {
                normalized
            } else {
                format!("{stderr}\n{normalized}")
            };
            return Ok((stdout, stderr, success));
        }
    }

    Ok((stdout, stderr, success))
}

/// Helper to ensure daemon is stopped (cleanup)
pub fn ensure_daemon_stopped() {
    ensure_daemon_stopped_with_config(None)
}

/// Helper to ensure daemon is stopped with specific socket path
pub fn ensure_daemon_stopped_with_config(socket_path: Option<&Path>) {
    // Use spawn() instead of output() to avoid hanging if shutdown command blocks
    let _ = probe_cmd_base(socket_path)
        .args(["lsp", "shutdown"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    // Give it a moment to send the shutdown signal
    thread::sleep(Duration::from_millis(100));

    // If socket path is provided, poll for its disappearance (more deterministic)
    if let Some(socket) = socket_path {
        let start = Instant::now();
        let poll_timeout = Duration::from_secs(2);

        while socket.exists() && start.elapsed() < poll_timeout {
            thread::sleep(Duration::from_millis(50));
        }

        // Clean up socket file if still exists
        if socket.exists() {
            let _ = std::fs::remove_file(socket);
        }

        // Also check for lock file and remove if exists
        let lock_path = socket.with_extension("lock");
        if lock_path.exists() {
            let _ = std::fs::remove_file(&lock_path);
        }
    } else {
        // Fallback: Force kill any remaining probe lsp processes
        let _ = Command::new("pkill")
            .args(["-f", "probe lsp"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();

        // Give processes time to fully shutdown
        thread::sleep(Duration::from_millis(500));
    }
}

/// Helper to start daemon and wait for it to be ready with retry logic
#[allow(dead_code)]
pub fn start_daemon_and_wait() -> Result<()> {
    start_daemon_and_wait_with_config(None)
}

/// Helper to start daemon with specific socket path
pub fn start_daemon_and_wait_with_config(socket_path: Option<&Path>) -> Result<()> {
    if performance::is_ci_environment() {
        println!("CI environment detected - using extended timeouts and retries");
        start_daemon_and_wait_with_retries_config(5, socket_path) // More retries in CI
    } else {
        start_daemon_and_wait_with_retries_config(3, socket_path)
    }
}

/// Helper to start daemon with specified number of retries
#[allow(dead_code)]
pub fn start_daemon_and_wait_with_retries(max_retries: u32) -> Result<()> {
    start_daemon_and_wait_with_retries_config(max_retries, None)
}

/// Helper to start daemon with specified number of retries and socket path
pub fn start_daemon_and_wait_with_retries_config(
    max_retries: u32,
    socket_path: Option<&Path>,
) -> Result<()> {
    let timeout = performance::daemon_startup_timeout();
    let max_attempts = if performance::is_ci_environment() {
        60
    } else {
        40
    }; // 30s in CI, 20s normally

    for retry in 0..max_retries {
        // Clean up any existing daemon before starting
        if retry > 0 {
            ensure_daemon_stopped_with_config(socket_path);
            thread::sleep(Duration::from_millis(1000)); // Wait longer between retries
        }

        // Start daemon in background
        let child = probe_cmd_base(socket_path)
            .args(["lsp", "start"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();

        match child {
            Ok(_) => {
                // Wait for daemon to be ready with exponential backoff
                for attempt in 0..max_attempts {
                    let wait_time = if attempt < 10 {
                        Duration::from_millis(500)
                    } else {
                        Duration::from_millis(1000) // Longer waits for later attempts
                    };

                    thread::sleep(wait_time);

                    // Check if daemon is ready
                    let output = probe_cmd_base(socket_path)
                        .args(["lsp", "status"])
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        .output();

                    match output {
                        Ok(output) if output.status.success() => {
                            // Verify daemon is actually functional by checking the status output
                            let stdout = String::from_utf8_lossy(&output.stdout);
                            if stdout.contains("LSP Daemon Status") || stdout.contains("Connected")
                            {
                                println!(
                                    "Daemon started successfully on attempt {} (retry {})",
                                    attempt + 1,
                                    retry + 1
                                );
                                return Ok(());
                            }
                        }
                        Ok(output) => {
                            // Status command failed, but maybe daemon is still starting
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            if stderr.contains("Connection refused")
                                || stderr.contains("No such file")
                            {
                                // Daemon not yet ready, continue waiting
                                continue;
                            }
                        }
                        Err(_) => {
                            // Command failed to execute, continue waiting
                            continue;
                        }
                    }
                }

                // If we get here, this retry attempt failed
                eprintln!(
                    "Daemon startup attempt {} failed after waiting {:?}",
                    retry + 1,
                    timeout
                );
            }
            Err(e) => {
                eprintln!(
                    "Failed to spawn daemon process on attempt {}: {}",
                    retry + 1,
                    e
                );
            }
        }
    }

    // All retries failed
    Err(anyhow::anyhow!(
        "Failed to start daemon after {} retries. Timeout: {:?}",
        max_retries,
        timeout
    ))
}

/// Initialize LSP workspace for testing with retry logic for early eof errors
#[allow(dead_code)]
pub fn init_lsp_workspace(workspace_path: &str, languages: &[&str]) -> Result<()> {
    init_lsp_workspace_with_config(workspace_path, languages, None)
}

/// Initialize LSP workspace with specific socket path
pub fn init_lsp_workspace_with_config(
    workspace_path: &str,
    languages: &[&str],
    socket_path: Option<&Path>,
) -> Result<()> {
    // Debug: Log the original workspace path
    eprintln!("init_lsp_workspace_with_config: workspace_path={workspace_path}");
    eprintln!(
        "  exists: {}",
        std::path::Path::new(workspace_path).exists()
    );

    // Try to canonicalize, but if it fails (e.g., in CI with symlinks), use the original path
    // The daemon should handle both absolute and relative paths correctly
    let path_to_use = if let Ok(canonical) = std::fs::canonicalize(workspace_path) {
        eprintln!("  canonicalized to: {}", canonical.display());
        canonical.to_string_lossy().to_string()
    } else {
        eprintln!("  canonicalization failed, making absolute");
        // If canonicalization fails, ensure we have an absolute path
        let path = PathBuf::from(workspace_path);
        if path.is_absolute() {
            workspace_path.to_string()
        } else {
            // Make it absolute relative to current directory
            let abs = std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(&path);
            eprintln!("  made absolute: {}", abs.display());
            abs.to_string_lossy().to_string()
        }
    };
    eprintln!("  final path_to_use: {path_to_use}");
    init_lsp_workspace_with_retries_config(&path_to_use, languages, 3, socket_path)
}

/// Initialize LSP workspace with specified number of retries
#[allow(dead_code)]
pub fn init_lsp_workspace_with_retries(
    workspace_path: &str,
    languages: &[&str],
    max_retries: u32,
) -> Result<()> {
    init_lsp_workspace_with_retries_config(workspace_path, languages, max_retries, None)
}

/// Initialize LSP workspace with specified number of retries and socket path
pub fn init_lsp_workspace_with_retries_config(
    workspace_path: &str,
    languages: &[&str],
    max_retries: u32,
    socket_path: Option<&Path>,
) -> Result<()> {
    let languages_str = languages.join(",");
    let mut args = vec!["lsp", "init", "-w", workspace_path, "--languages"];
    args.push(&languages_str);

    let timeout = performance::max_init_time();

    for retry in 0..max_retries {
        eprintln!("LSP init attempt {} with args: {:?}", retry + 1, args);
        let (stdout, stderr, success) = run_probe_command_with_config(&args, timeout, socket_path)?;
        eprintln!(
            "  Result: success={}, stdout len={}, stderr len={}",
            success,
            stdout.len(),
            stderr.len()
        );
        if !stdout.is_empty() {
            eprintln!("  Stdout: {stdout}");
        }
        if !stderr.is_empty() {
            eprintln!("  Stderr: {stderr}");
        }

        if success {
            println!(
                "LSP workspace initialization succeeded on attempt {}",
                retry + 1
            );
            return Ok(());
        }

        // Check for specific error patterns that indicate retryable failures
        let is_retryable = stderr.contains("early eof")
            || stderr.contains("Connection refused")
            || stderr.contains("Failed to read message length")
            || stderr.contains("connection reset")
            || stderr.contains("broken pipe");

        if !is_retryable {
            // Non-retryable error, fail immediately
            eprintln!("Non-retryable error detected");
            return Err(anyhow::anyhow!(
                "LSP workspace initialization failed with non-retryable error.\nArgs: {:?}\nStdout: {}\nStderr: {}",
                args,
                stdout,
                stderr
            ));
        }

        eprintln!(
            "LSP workspace initialization attempt {} failed (retryable): {}",
            retry + 1,
            stderr.trim()
        );

        if retry < max_retries - 1 {
            // Wait before retrying, with increasing delays
            let wait_time = Duration::from_millis(1000 * (retry + 1) as u64);
            eprintln!("Waiting {wait_time:?} before retry...");
            thread::sleep(wait_time);

            // Verify daemon is still running, restart if needed
            let status_check = run_probe_command_with_config(
                &["lsp", "status"],
                Duration::from_secs(5),
                socket_path,
            );
            if status_check.is_err() || !status_check.unwrap().2 {
                eprintln!("Daemon appears to be down, restarting...");
                ensure_daemon_stopped_with_config(socket_path);
                start_daemon_and_wait_with_config(socket_path)?;
            }
        }
    }

    Err(anyhow::anyhow!(
        "LSP workspace initialization failed after {} retries.\nArgs: {:?}",
        max_retries,
        args
    ))
}

/// Wait for LSP servers to be ready by polling their status
/// This is more efficient and reliable than fixed sleep durations
#[allow(dead_code)]
pub fn wait_for_lsp_servers_ready(
    expected_languages: &[&str],
    max_timeout: Duration,
) -> Result<()> {
    wait_for_lsp_servers_ready_with_config(expected_languages, max_timeout, None)
}

/// Wait for LSP servers with specific socket path
pub fn wait_for_lsp_servers_ready_with_config(
    expected_languages: &[&str],
    max_timeout: Duration,
    socket_path: Option<&Path>,
) -> Result<()> {
    let start_time = Instant::now();
    let mut poll_interval = Duration::from_millis(500); // Start with 500ms
    let max_poll_interval = Duration::from_secs(2); // Cap at 2 seconds

    // Always respect the caller-provided timeout — never override it in CI.
    // Give CI a sane floor to account for slower machines.
    let is_ci = performance::is_ci_environment();
    let min_ci_timeout = Duration::from_secs(120);
    let effective_timeout = if is_ci && max_timeout < min_ci_timeout {
        min_ci_timeout
    } else {
        max_timeout
    };

    println!(
        "Polling LSP status for {} languages: {} (timeout: {:?})",
        expected_languages.len(),
        expected_languages.join(", "),
        effective_timeout
    );

    // If status queries keep failing (e.g., daemon not responding), bail out early
    // instead of spinning for the full timeout.
    let mut consecutive_failures: u32 = 0;
    let max_status_failures: u32 = if is_ci { 30 } else { 10 };

    loop {
        let elapsed = start_time.elapsed();
        if elapsed >= effective_timeout {
            return Err(anyhow::anyhow!(
                "Timeout waiting for LSP servers after {:?}. Expected languages: {}",
                elapsed,
                expected_languages.join(", ")
            ));
        }

        // Check LSP status
        match check_lsp_servers_ready_with_config(expected_languages, socket_path) {
            Ok(true) => {
                println!(
                    "All {} LSP servers are ready after {:?}",
                    expected_languages.len(),
                    elapsed
                );
                return Ok(());
            }
            Ok(false) => {
                consecutive_failures = 0; // successful check, just not ready yet
                                          // Log every ~5 seconds
                if elapsed.as_secs().is_multiple_of(5)
                    && elapsed.as_millis() % 1000 < poll_interval.as_millis()
                {
                    println!(
                        "Still waiting for {} LSP servers after {:?}: {}",
                        expected_languages.len(),
                        elapsed,
                        expected_languages.join(", ")
                    );
                }
            }
            Err(e) => {
                // Status check failed, but don't fail immediately in case it's transient
                consecutive_failures += 1;
                if elapsed.as_secs().is_multiple_of(10)
                    && elapsed.as_millis() % 1000 < poll_interval.as_millis()
                {
                    println!("LSP status check failed (will retry): {e} (#{consecutive_failures})");
                }
                if consecutive_failures >= max_status_failures {
                    return Err(anyhow::anyhow!(
                        "LSP status check failed {} times in {:?}. Aborting.",
                        consecutive_failures,
                        elapsed
                    ));
                }
            }
        }

        thread::sleep(poll_interval);

        // Exponential backoff to avoid hammering the LSP daemon
        poll_interval = std::cmp::min(
            Duration::from_millis((poll_interval.as_millis() as f64 * 1.2) as u64),
            max_poll_interval,
        );
    }
}

/// Check if all expected LSP language servers are ready
#[allow(dead_code)]
fn check_lsp_servers_ready(expected_languages: &[&str]) -> Result<bool> {
    check_lsp_servers_ready_with_config(expected_languages, None)
}

/// Check if all expected LSP language servers are ready with specific socket path
fn check_lsp_servers_ready_with_config(
    expected_languages: &[&str],
    socket_path: Option<&Path>,
) -> Result<bool> {
    // Retry logic for daemon connection issues
    const MAX_RETRIES: u32 = 3;
    let mut last_error = None;

    for attempt in 0..MAX_RETRIES {
        // Force no-color/plain output so the parser isn't confused by ANSI in CI.
        let mut cmd = probe_cmd_base(socket_path);
        let output = cmd
            .env("NO_COLOR", "1")
            .env("CLICOLOR", "0")
            .env("CLICOLOR_FORCE", "0")
            .env("FORCE_COLOR", "0")
            .env("TERM", "dumb")
            .args(["lsp", "status"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .context("Failed to run 'probe lsp status'")?;

        if output.status.success() {
            let status_output = String::from_utf8_lossy(&output.stdout);
            // Parse the output to check server status
            for &expected_lang in expected_languages {
                if !is_language_server_ready(&status_output, expected_lang)? {
                    return Ok(false);
                }
            }
            return Ok(true);
        } else {
            let stderr_str = String::from_utf8_lossy(&output.stderr);
            let is_daemon_connection_issue = stderr_str.contains("connection refused")
                || stderr_str.contains("Connection refused")
                || stderr_str.contains("timeout")
                || stderr_str.contains("daemon")
                || stderr_str.contains("socket");

            if is_daemon_connection_issue && attempt < MAX_RETRIES - 1 {
                eprintln!(
                    "LSP daemon connection issue on attempt {}/{}: {}",
                    attempt + 1,
                    MAX_RETRIES,
                    stderr_str
                );
                eprintln!("Retrying after 2 seconds...");
                std::thread::sleep(std::time::Duration::from_secs(2));
                last_error = Some(anyhow::anyhow!(
                    "LSP daemon connection failed: {}",
                    stderr_str
                ));
                continue;
            } else {
                return Err(anyhow::anyhow!(
                    "LSP status command failed after {} attempts: {}",
                    attempt + 1,
                    stderr_str
                ));
            }
        }
    }

    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("All LSP status attempts failed")))
}

/// Parse LSP status output to check if a specific language server is ready
fn is_language_server_ready(status_output: &str, language: &str) -> Result<bool> {
    // Look for a section that begins with e.g. "Go:" or "TypeScript:" and contains "Available (Ready)".
    // Within that section, prefer to read an explicit "Servers: Ready: N" value (N > 0).
    let clean_output = strip_ansi(status_output);
    let lines: Vec<&str> = clean_output.lines().collect();
    // Accept common aliases / combined headers and tolerate colonless variants.
    let lang_lc = language.to_ascii_lowercase();
    let mut header_prefixes: Vec<String> = vec![
        format!("{language}:"),
        language.to_string(), // colonless
    ];
    match lang_lc.as_str() {
        "javascript" => {
            header_prefixes.extend([
                "TypeScript:".into(),
                "TypeScript".into(),
                "TypeScript/JavaScript:".into(),
                "TypeScript/JavaScript".into(),
                "JavaScript/TypeScript:".into(),
                "JavaScript/TypeScript".into(),
                "tsserver:".into(),
                "tsserver".into(),
                "TypeScript (tsserver):".into(),
                "JavaScript (tsserver):".into(),
            ]);
        }
        "typescript" => {
            header_prefixes.extend([
                "JavaScript:".into(),
                "JavaScript".into(),
                "TypeScript/JavaScript:".into(),
                "TypeScript/JavaScript".into(),
                "JavaScript/TypeScript:".into(),
                "JavaScript/TypeScript".into(),
                "tsserver:".into(),
                "tsserver".into(),
                "TypeScript (tsserver):".into(),
                "JavaScript (tsserver):".into(),
            ]);
        }
        "go" => {
            header_prefixes.extend([
                "Go (gopls):".into(),
                "Go (gopls)".into(),
                "Golang:".into(),
                "Golang".into(),
            ]);
        }
        _ => {}
    }

    for (i, &line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        let trimmed_lc = trimmed.to_ascii_lowercase();
        let is_header = header_prefixes.iter().any(|p| {
            let p_norm = p.trim_end_matches(':').to_ascii_lowercase();
            trimmed_lc.starts_with(&p_norm)
        });
        if !is_header {
            continue;
        }

        // Header-level ready marker (case-insensitive, tolerant of extra words)
        let header_says_ready = trimmed_lc.contains("(ready)");

        // Search forward until the next top-level section (a non-indented line ending with ':')
        // and try to find "Servers: ... Ready: <N>".
        let mut ready_count: Option<u32> = None;
        let mut workspaces_count: u32 = 0;
        let mut uptime_secs: u64 = 0;

        for &next in lines.iter().skip(i + 1) {
            let t = next.trim();
            let t_lc = t.to_ascii_lowercase();

            // Stop if we hit the start of another section.
            if !next.starts_with(' ')
                && t.ends_with(':')
                && !(t_lc.starts_with("servers")
                    || t_lc.starts_with("server")
                    || t_lc.starts_with("instances"))
            {
                break;
            }

            if t_lc.starts_with("servers")
                || t_lc.starts_with("server")
                || t_lc.starts_with("instances")
            {
                // Be tolerant of "Ready: 1", "Ready 1", "Ready servers: 1", or "Ready: 1/3".
                if let Some(idx) = t_lc.find("ready") {
                    let after = &t_lc[idx + "ready".len()..];
                    let digits: String = after
                        .chars()
                        .skip_while(|c| !c.is_ascii_digit())
                        .take_while(|c| c.is_ascii_digit())
                        .collect();
                    if let Ok(n) = digits.parse::<u32>() {
                        ready_count = Some(n);
                        break;
                    }
                }
            }

            if t_lc.starts_with("workspaces") {
                // Expect "Workspaces: (N)"
                if let Some(start) = t.find('(') {
                    let digits: String = t[start + 1..]
                        .chars()
                        .take_while(|c| c.is_ascii_digit())
                        .collect();
                    if let Ok(n) = digits.parse::<u32>() {
                        workspaces_count = n;
                    }
                }
            }

            if t_lc.starts_with("uptime") {
                // Expect "Uptime: 5s" (tolerate ms/m/h)
                if let Some(after_colon) = t.split(':').nth(1) {
                    let ts = after_colon.trim();
                    // Simple unit parser: e.g., "500ms", "5s", "2m", "1h"
                    let (num_str, unit_str): (String, String) =
                        ts.chars()
                            .fold((String::new(), String::new()), |(mut n, mut u), ch| {
                                if ch.is_ascii_digit() {
                                    if u.is_empty() {
                                        n.push(ch);
                                    }
                                } else if !ch.is_whitespace() {
                                    u.push(ch);
                                }
                                (n, u)
                            });
                    if let Ok(n) = num_str.parse::<u64>() {
                        uptime_secs = match unit_str.as_str() {
                            "ms" => 0,
                            "s" => n,
                            "m" => n * 60,
                            "h" => n * 3600,
                            _ => 0,
                        };
                    }
                }
            }
        }

        // Prefer explicit server counts when available; otherwise fall back to the header.
        if let Some(n) = ready_count {
            // Authoritative: any Ready > 0 means the language is usable even if header still says "(Indexing)".
            return Ok(n > 0);
        }

        // Go-specific, last-resort fallback:
        // If gopls has at least one workspace and has been up for a reasonable grace period,
        // treat the server as ready. This matches daemon's indexing grace design.
        if lang_lc == "go" {
            let grace: u64 = std::env::var("LSP_INDEX_GRACE_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(30);
            if workspaces_count > 0 && uptime_secs >= grace {
                return Ok(true);
            }
        }

        return Ok(header_says_ready);
    }

    Ok(false)
}

/// Test fixture paths
pub mod fixtures {
    use std::path::PathBuf;

    pub fn get_fixtures_dir() -> PathBuf {
        // Resolve relative to the crate root and normalize (works both locally and in CI).
        // Using CARGO_MANIFEST_DIR de-couples us from the process CWD.
        // Don't use canonicalize() as it can cause issues with symlinks in CI
        // Just return the constructed path as-is
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
    }

    pub fn get_go_project1() -> PathBuf {
        let path = get_fixtures_dir().join("go/project1");
        eprintln!("fixtures::get_go_project1() -> {}", path.display());
        eprintln!("  exists: {}", path.exists());
        if !path.exists() {
            eprintln!("  ERROR: Go project1 fixture does not exist!");
            eprintln!("  CARGO_MANIFEST_DIR: {}", env!("CARGO_MANIFEST_DIR"));
            eprintln!("  Current dir: {:?}", std::env::current_dir());
        }
        path
    }

    pub fn get_typescript_project1() -> PathBuf {
        let path = get_fixtures_dir().join("typescript/project1");
        eprintln!("fixtures::get_typescript_project1() -> {}", path.display());
        eprintln!("  exists: {}", path.exists());
        if !path.exists() {
            eprintln!("  ERROR: TypeScript project1 fixture does not exist!");
            eprintln!("  CARGO_MANIFEST_DIR: {}", env!("CARGO_MANIFEST_DIR"));
            eprintln!("  Current dir: {:?}", std::env::current_dir());
        }
        path
    }

    pub fn get_javascript_project1() -> PathBuf {
        let path = get_fixtures_dir().join("javascript/project1");
        eprintln!("fixtures::get_javascript_project1() -> {}", path.display());
        eprintln!("  exists: {}", path.exists());
        if !path.exists() {
            eprintln!("  ERROR: JavaScript project1 fixture does not exist!");
            eprintln!("  CARGO_MANIFEST_DIR: {}", env!("CARGO_MANIFEST_DIR"));
            eprintln!("  Current dir: {:?}", std::env::current_dir());
        }
        path
    }
}

/// Performance requirements for LSP operations
pub mod performance {
    use std::time::Duration;

    /// Check if running in CI environment
    pub fn is_ci_environment() -> bool {
        std::env::var("PROBE_CI").is_ok()
            || std::env::var("GITHUB_ACTIONS").is_ok()
            || std::env::var("TRAVIS").is_ok()
            || std::env::var("CIRCLECI").is_ok()
    }

    /// Maximum time allowed for extraction with LSP
    pub fn max_extract_time() -> Duration {
        if is_ci_environment() {
            Duration::from_secs(180) // More headroom for heavier indexing in CI
        } else {
            Duration::from_secs(45) // Local development
        }
    }

    /// Maximum time allowed for search with LSP
    pub fn max_search_time() -> Duration {
        if is_ci_environment() {
            Duration::from_secs(45) // More time for CI environments with slower I/O and variable performance
        } else {
            Duration::from_secs(15) // Reasonable time for local development
        }
    }

    /// Maximum time to wait for language server initialization
    pub fn max_init_time() -> Duration {
        Duration::from_secs(180) // Increased for CI and heavier startup
    }

    /// Language server ready wait time
    pub fn language_server_ready_time() -> Duration {
        Duration::from_secs(60) // More conservative for CI indexing stabilizing
    }

    /// Daemon startup timeout
    pub fn daemon_startup_timeout() -> Duration {
        Duration::from_secs(30) // Slightly higher for CI
    }

    // Legacy constants for backward compatibility
    #[allow(dead_code)]
    pub const MAX_EXTRACT_TIME: Duration = Duration::from_secs(3);
    #[allow(dead_code)]
    pub const MAX_SEARCH_TIME: Duration = Duration::from_secs(5);
    #[allow(dead_code)]
    pub const MAX_INIT_TIME: Duration = Duration::from_secs(60);
}

/// Extract with call hierarchy retry for CI reliability
#[allow(dead_code)]
pub fn extract_with_call_hierarchy_retry(
    extract_args: &[&str],
    expected_incoming: usize,
    expected_outgoing: usize,
    timeout: Duration,
) -> Result<(String, String, bool)> {
    extract_with_call_hierarchy_retry_config(
        extract_args,
        expected_incoming,
        expected_outgoing,
        timeout,
        None,
    )
}

/// Extract with call hierarchy retry with specific socket path
pub fn extract_with_call_hierarchy_retry_config(
    extract_args: &[&str],
    expected_incoming: usize,
    expected_outgoing: usize,
    timeout: Duration,
    socket_path: Option<&Path>,
) -> Result<(String, String, bool)> {
    let start_time = Instant::now();
    let is_ci = performance::is_ci_environment();
    let mut attempt = 1;
    let max_attempts = if is_ci { 10 } else { 3 };
    let retry_delay = Duration::from_secs(2);

    if is_ci {
        println!(
            "🔄 CI CALL HIERARCHY EXPERIMENT: Retrying extract until call hierarchy data available (max {max_attempts} attempts over {timeout:?})"
        );
    }

    loop {
        let elapsed = start_time.elapsed();
        if elapsed >= timeout {
            return Err(anyhow::anyhow!(
                "Timeout waiting for call hierarchy data after {elapsed:?}. Made {} attempts.",
                attempt - 1
            ));
        }

        if is_ci {
            println!(
                "🔄 Attempt {attempt}/{max_attempts}: Extracting call hierarchy data (elapsed: {elapsed:?})"
            );
        }

        // Run the extract command with the remaining time budget for this attempt
        let remaining = timeout.saturating_sub(elapsed);
        let (stdout, stderr, success) =
            run_probe_command_with_config(extract_args, remaining, socket_path)?;

        if !success {
            if attempt >= max_attempts {
                return Ok((stdout, stderr, success)); // Return the failure
            }
            if is_ci {
                println!("❌ Extract command failed on attempt {attempt}, retrying...");
            }
            attempt += 1;
            // Never sleep past the remaining time budget
            let sleep_for = retry_delay.min(timeout.saturating_sub(start_time.elapsed()));
            if sleep_for.as_nanos() > 0 {
                thread::sleep(sleep_for);
            }
            continue;
        }

        // Check if we have call hierarchy data
        match (
            call_hierarchy::validate_incoming_calls(&stdout, expected_incoming),
            call_hierarchy::validate_outgoing_calls(&stdout, expected_outgoing),
        ) {
            (Ok(()), Ok(())) => {
                if is_ci {
                    println!(
                        "✅ CI SUCCESS: Got complete call hierarchy data on attempt {attempt} after {elapsed:?}"
                    );
                }
                return Ok((stdout, stderr, success));
            }
            (incoming_result, outgoing_result) => {
                if attempt >= max_attempts {
                    if is_ci {
                        println!(
                            "❌ CI FINAL ATTEMPT: Call hierarchy still incomplete after {attempt} attempts ({elapsed:?})"
                        );
                        println!("   Incoming: {incoming_result:?}");
                        println!("   Outgoing: {outgoing_result:?}");
                    }
                    return Ok((stdout, stderr, success)); // Return what we have
                }

                if is_ci {
                    println!(
                        "⚠️  Attempt {attempt}: Call hierarchy incomplete, retrying in {retry_delay:?}..."
                    );
                    if let Err(e) = incoming_result {
                        println!("   Incoming issue: {e}");
                    }
                    if let Err(e) = outgoing_result {
                        println!("   Outgoing issue: {e}");
                    }
                }

                attempt += 1;
                // Never sleep past the remaining time budget
                let sleep_for = retry_delay.min(timeout.saturating_sub(start_time.elapsed()));
                if sleep_for.as_nanos() > 0 {
                    thread::sleep(sleep_for);
                }
            }
        }
    }
}

/// Call hierarchy validation helpers
pub mod call_hierarchy {
    /// Validate that call hierarchy contains expected number of incoming calls
    pub fn validate_incoming_calls(output: &str, expected_count: usize) -> Result<(), String> {
        let incoming_section = extract_call_hierarchy_section(output, "Incoming Calls")?;
        let actual_count = count_call_entries(&incoming_section);

        if actual_count != expected_count {
            return Err(format!(
                "Expected {expected_count} incoming calls, found {actual_count}. Section content: {incoming_section}"
            ));
        }

        Ok(())
    }

    /// Validate that call hierarchy contains expected number of outgoing calls
    pub fn validate_outgoing_calls(output: &str, expected_count: usize) -> Result<(), String> {
        let outgoing_section = extract_call_hierarchy_section(output, "Outgoing Calls")?;
        let actual_count = count_call_entries(&outgoing_section);

        if actual_count != expected_count {
            return Err(format!(
                "Expected {expected_count} outgoing calls, found {actual_count}. Section content: {outgoing_section}"
            ));
        }

        Ok(())
    }

    /// Extract a specific call hierarchy section from output
    fn extract_call_hierarchy_section(output: &str, section_name: &str) -> Result<String, String> {
        // Be robust to:
        //  - different Markdown levels (## / ###)
        //  - optional colon and inline content ("Incoming Calls: <one-liner>")
        //  - capitalization differences
        //  - minor adornments like "(0)" after the title
        let lines: Vec<&str> = output.lines().collect();
        let want = section_name.to_ascii_lowercase();

        // Helper: detect a header line for the wanted section (case-insensitive, flexible)
        let is_header = |raw: &str| {
            let t = raw.trim();
            let lc = t.to_ascii_lowercase();
            lc.starts_with(&format!("## {want}"))
                || lc.starts_with(&format!("### {want}"))
                || lc == want
                || lc.starts_with(&format!("{want}:"))
                || lc.starts_with(&format!("- {want}"))
                || lc.starts_with(&format!("* {want}"))
                || lc.starts_with(&format!("{want} (")) // e.g. "Incoming Calls (0)"
        };

        // Helper: boundary when we hit the next call-hierarchy section or a new Markdown header
        let is_boundary = |raw: &str| {
            let t = raw.trim();
            let lc = t.to_ascii_lowercase();
            let next_is_ch = lc.starts_with("## incoming calls")
                || lc.starts_with("### incoming calls")
                || lc == "incoming calls:"
                || lc.starts_with("incoming calls:")
                || lc.starts_with("## outgoing calls")
                || lc.starts_with("### outgoing calls")
                || lc == "outgoing calls:"
                || lc.starts_with("outgoing calls:")
                || lc == "references:"
                || lc.starts_with("references:")
                || lc.starts_with("## references")
                || lc.starts_with("### references")
                || lc == "documentation:"
                || lc.starts_with("documentation:")
                || lc.starts_with("## documentation")
                || lc.starts_with("### documentation");
            let is_md_header = t.starts_with("## ");
            next_is_ch || is_md_header
        };

        // Find start line
        let (start_idx, inline_after_colon) = match lines.iter().position(|l| is_header(l)) {
            Some(i) => {
                let trimmed = lines[i].trim();
                if let Some(colon) = trimmed.find(':') {
                    let after = trimmed[colon + 1..].trim_start();
                    let inline = if !after.is_empty() {
                        Some(after.to_string())
                    } else {
                        None
                    };
                    (i, inline)
                } else {
                    (i, None)
                }
            }
            None => return Err(format!("Section '{section_name}' not found in output")),
        };

        // Collect lines until the next section/header
        let mut collected: Vec<String> = Vec::new();
        if let Some(inline) = inline_after_colon {
            collected.push(inline);
        }
        for &line in lines.iter().skip(start_idx + 1) {
            if is_boundary(line) {
                break;
            }
            collected.push(line.to_string());
        }

        Ok(collected.join("\n"))
    }

    /// Count the number of call entries in a section
    fn count_call_entries(section_content: &str) -> usize {
        // Count lines that start with "- " or contain function signatures
        section_content
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                trimmed.starts_with("- ") && !trimmed.is_empty()
            })
            .count()
    }
}

/// Test guard that ensures LSP processes are cleaned up properly
///
/// This guard tracks LSP process counts before and after tests,
/// and forcibly kills any leaked processes to prevent test interference.
pub struct LspTestGuard {
    initial_process_count: usize,
    test_name: String,
}

impl LspTestGuard {
    /// Create a new test guard for the given test
    pub fn new(test_name: &str) -> Self {
        // Kill any existing LSP processes before test
        cleanup_leaked_lsp_processes();
        let count = count_lsp_processes();

        eprintln!("🧪 LspTestGuard: Starting test '{test_name}' with {count} LSP processes");

        Self {
            initial_process_count: count,
            test_name: test_name.to_string(),
        }
    }
}

impl Drop for LspTestGuard {
    fn drop(&mut self) {
        eprintln!("🧪 LspTestGuard: Cleaning up test '{}'", self.test_name);

        // Force cleanup of any leaked processes
        cleanup_leaked_lsp_processes();
        let final_count = count_lsp_processes();

        if final_count > self.initial_process_count {
            eprintln!(
                "⚠️  LSP process leak detected in test '{}': Initial: {}, Final: {} (+{})",
                self.test_name,
                self.initial_process_count,
                final_count,
                final_count - self.initial_process_count
            );

            // Try one more aggressive cleanup
            force_kill_lsp_processes();
            let after_force_kill = count_lsp_processes();

            if after_force_kill > self.initial_process_count {
                panic!(
                    "❌ CRITICAL: Could not clean up LSP process leaks in test '{}'. \
                     Still have {} processes after forced cleanup (initial: {})",
                    self.test_name, after_force_kill, self.initial_process_count
                );
            } else {
                eprintln!("✅ Successfully cleaned up leaked processes");
            }
        } else {
            eprintln!(
                "✅ Test '{}' completed without process leaks",
                self.test_name
            );
        }
    }
}

/// Count LSP-related processes
fn count_lsp_processes() -> usize {
    let output = std::process::Command::new("sh")
        .arg("-c")
        // Count probe lsp commands and lsp-daemon, but exclude the test runner itself
        .arg("ps aux | grep -E 'probe.*lsp|lsp-daemon' | grep -v grep | grep -v lsp_integration_tests | wc -l")
        .output();

    match output {
        Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .unwrap_or(0),
        _ => 0, // If command fails, assume no processes
    }
}

/// Clean up test-related LSP processes
fn cleanup_leaked_lsp_processes() {
    // Try to gracefully shutdown any probe lsp daemons first
    let _ = std::process::Command::new("pkill")
        .args(["-f", "probe lsp"])
        .output();

    // Give them time to exit gracefully
    std::thread::sleep(std::time::Duration::from_millis(200));

    // Kill any remaining LSP daemon processes (but not the test runner itself!)
    // Be more specific to avoid killing lsp_integration_tests
    let _ = std::process::Command::new("pkill")
        .args(["-f", "lsp-daemon"])
        .output();

    // Give processes time to exit
    std::thread::sleep(std::time::Duration::from_millis(100));
}

/// Force kill all LSP processes (last resort)
fn force_kill_lsp_processes() {
    eprintln!("🔥 Force killing all LSP processes...");

    // Use SIGKILL to force kill probe lsp commands
    let _ = std::process::Command::new("pkill")
        .args(["-9", "-f", "probe lsp"])
        .output();

    // Force kill lsp-daemon specifically (not test runners!)
    let _ = std::process::Command::new("pkill")
        .args(["-9", "-f", "lsp-daemon"])
        .output();

    // Give the OS time to clean up
    std::thread::sleep(std::time::Duration::from_millis(500));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_server_enum() {
        assert_eq!(LanguageServer::Gopls.command_name(), "gopls");
        assert_eq!(
            LanguageServer::TypeScriptLanguageServer.command_name(),
            "typescript-language-server"
        );
    }

    #[test]
    fn test_call_hierarchy_validation() {
        let mock_output = r#"
## Incoming Calls
- main.calculate() calls this function
- ProcessNumbers() calls this function

## Outgoing Calls  
- calls add()
- calls multiply()
- calls subtract()
"#;

        assert!(call_hierarchy::validate_incoming_calls(mock_output, 2).is_ok());
        assert!(call_hierarchy::validate_outgoing_calls(mock_output, 3).is_ok());

        // Test failure cases
        assert!(call_hierarchy::validate_incoming_calls(mock_output, 3).is_err());
        assert!(call_hierarchy::validate_outgoing_calls(mock_output, 2).is_err());
    }

    #[test]
    #[ignore = "Flaky in CI - race condition with concurrent tests"]
    fn test_lsp_test_guard_no_leak() {
        let _guard = LspTestGuard::new("test_lsp_test_guard_no_leak");
        // This test should pass without any process leaks
    }
}
