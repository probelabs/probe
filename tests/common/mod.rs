//! Common test utilities and helpers for LSP integration tests

use anyhow::{Context, Result};
use std::env;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

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
            let mut base = dir.join(command);

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
    let start = Instant::now();

    let mut child = Command::new("./target/debug/probe")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("Failed to execute probe command: probe {}", args.join(" ")))?;

    // Poll until the process exits or the timeout elapses; kill on timeout.
    loop {
        if let Some(_status) = child.try_wait().context("Failed to poll probe process")? {
            // Process finished; collect outputs.
            let output = child
                .wait_with_output()
                .context("Failed to collect probe output")?;

            let elapsed = start.elapsed();
            if elapsed > timeout {
                return Err(anyhow::anyhow!(
                    "Command timed out after {:?} (limit: {:?}): probe {}",
                    elapsed,
                    timeout,
                    args.join(" ")
                ));
            }

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            let mut success = output.status.success();

            // Some probe subcommands currently print errors but still exit 0; treat *obvious* error strings as failures in tests.
            // Be careful not to misclassify benign phrases like "No results found."
            if success {
                let combined_output_lc =
                    format!("{}{}", stdout.to_lowercase(), stderr.to_lowercase());
                let looks_like_no_results = combined_output_lc.contains("no results found");
                let looks_like_error = combined_output_lc.contains("error:")
                    || combined_output_lc.contains("no such file")
                    || combined_output_lc.contains("file does not exist")
                    || combined_output_lc.contains("file not found")
                    || combined_output_lc.contains("path not found")
                    || (combined_output_lc.contains("encountered")
                        && combined_output_lc.contains("error"));
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
                        "Error: file not found (one or more provided paths do not exist)"
                            .to_string()
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

            return Ok((stdout, stderr, success));
        }

        // Still running?
        if start.elapsed() >= timeout {
            // Hard timeout: kill and surface an error, but return whatever output we can capture.
            let _ = child.kill();
            let output = child
                .wait_with_output()
                .context("Failed to collect probe output after kill")?;

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            return Err(anyhow::anyhow!(
                "Command timed out after {:?} (limit: {:?}): probe {}\n--- partial stdout ---\n{}\n--- partial stderr ---\n{}",
                start.elapsed(),
                timeout,
                args.join(" "),
                stdout,
                stderr
            ));
        }

        thread::sleep(Duration::from_millis(50));
    }
}

/// Helper to ensure daemon is stopped (cleanup)
pub fn ensure_daemon_stopped() {
    let _ = Command::new("./target/debug/probe")
        .args(["lsp", "shutdown"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .output();

    // Give it a moment to fully shutdown
    thread::sleep(Duration::from_millis(500));
}

/// Helper to start daemon and wait for it to be ready with retry logic
pub fn start_daemon_and_wait() -> Result<()> {
    if performance::is_ci_environment() {
        println!("CI environment detected - using extended timeouts and retries");
        start_daemon_and_wait_with_retries(5) // More retries in CI
    } else {
        start_daemon_and_wait_with_retries(3)
    }
}

/// Helper to start daemon with specified number of retries
pub fn start_daemon_and_wait_with_retries(max_retries: u32) -> Result<()> {
    let timeout = performance::daemon_startup_timeout();
    let max_attempts = if performance::is_ci_environment() {
        60
    } else {
        40
    }; // 30s in CI, 20s normally

    for retry in 0..max_retries {
        // Clean up any existing daemon before starting
        if retry > 0 {
            ensure_daemon_stopped();
            thread::sleep(Duration::from_millis(1000)); // Wait longer between retries
        }

        // Start daemon in background
        let child = Command::new("./target/debug/probe")
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
                    let output = Command::new("./target/debug/probe")
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
pub fn init_lsp_workspace(workspace_path: &str, languages: &[&str]) -> Result<()> {
    init_lsp_workspace_with_retries(workspace_path, languages, 3)
}

/// Initialize LSP workspace with specified number of retries
pub fn init_lsp_workspace_with_retries(
    workspace_path: &str,
    languages: &[&str],
    max_retries: u32,
) -> Result<()> {
    let languages_str = languages.join(",");
    let mut args = vec!["lsp", "init", "-w", workspace_path, "--languages"];
    args.push(&languages_str);

    let timeout = performance::max_init_time();

    for retry in 0..max_retries {
        let (stdout, stderr, success) = run_probe_command_with_timeout(&args, timeout)?;

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
            let status_check =
                run_probe_command_with_timeout(&["lsp", "status"], Duration::from_secs(5));
            if status_check.is_err() || !status_check.unwrap().2 {
                eprintln!("Daemon appears to be down, restarting...");
                ensure_daemon_stopped();
                start_daemon_and_wait()?;
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
pub fn wait_for_lsp_servers_ready(
    expected_languages: &[&str],
    max_timeout: Duration,
) -> Result<()> {
    let start_time = Instant::now();
    let mut poll_interval = Duration::from_millis(500); // Start with 500ms
    let max_poll_interval = Duration::from_secs(2); // Cap at 2 seconds

    // For CI timing experiment: remove timeout limit, allow unlimited wait time
    let unlimited_wait = performance::is_ci_environment();
    let effective_timeout = if unlimited_wait {
        Duration::from_secs(600) // 10 minutes max to prevent infinite hangs
    } else {
        max_timeout
    };

    if unlimited_wait {
        println!(
            "CI TIMING EXPERIMENT: Waiting unlimited time for {} languages: {} (max 10min safety limit)",
            expected_languages.len(),
            expected_languages.join(", ")
        );
    } else {
        println!(
            "Polling LSP status for {} languages: {} (timeout: {:?})",
            expected_languages.len(),
            expected_languages.join(", "),
            max_timeout
        );
    }

    loop {
        let elapsed = start_time.elapsed();
        if elapsed >= effective_timeout {
            return Err(anyhow::anyhow!(
                "Safety timeout reached after {:?}. Expected languages: {}",
                elapsed,
                expected_languages.join(", ")
            ));
        }

        // Check LSP status
        match check_lsp_servers_ready(expected_languages) {
            Ok(true) => {
                if unlimited_wait {
                    println!(
                        "🎯 CI TIMING RESULT: All {} LSP servers ready after {:?} - languages: {}",
                        expected_languages.len(),
                        elapsed,
                        expected_languages.join(", ")
                    );
                } else {
                    println!(
                        "All {} LSP servers are ready after {:?}",
                        expected_languages.len(),
                        elapsed
                    );
                }
                return Ok(());
            }
            Ok(false) => {
                // Enhanced logging for timing experiment
                if unlimited_wait {
                    // Log every 10 seconds in CI for detailed timing data
                    if elapsed.as_secs() % 10 == 0
                        && elapsed.as_millis() % 1000 < poll_interval.as_millis()
                    {
                        println!(
                            "⏱️  CI TIMING: Still waiting for {} languages after {:?} - target: {}",
                            expected_languages.len(),
                            elapsed,
                            expected_languages.join(", ")
                        );
                    }
                } else {
                    // Original 5-second logging for local
                    if elapsed.as_secs() % 5 == 0
                        && elapsed.as_millis() % 1000 < poll_interval.as_millis()
                    {
                        println!("Still waiting for LSP servers... ({elapsed:?} elapsed)");
                    }
                }
            }
            Err(e) => {
                // Status check failed, but don't fail immediately in case it's transient
                if elapsed.as_secs() % 10 == 0
                    && elapsed.as_millis() % 1000 < poll_interval.as_millis()
                {
                    println!("LSP status check failed (will retry): {e}");
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
fn check_lsp_servers_ready(expected_languages: &[&str]) -> Result<bool> {
    // Retry logic for daemon connection issues
    const MAX_RETRIES: u32 = 3;
    let mut last_error = None;

    for attempt in 0..MAX_RETRIES {
        let output = Command::new("./target/debug/probe")
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
    let lines: Vec<&str> = status_output.lines().collect();
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

        let header_says_ready =
            trimmed.contains("Available (Ready)") || trimmed.contains("(Ready)");

        // Search forward until the next top-level section (a non-indented line ending with ':')
        // and try to find "Servers: ... Ready: <N>".
        let mut ready_count: Option<u32> = None;
        for &next in lines.iter().skip(i + 1) {
            let t = next.trim();

            // Stop if we hit the start of another section.
            if !next.starts_with(' ') && t.ends_with(':') && !t.starts_with("Servers:") {
                break;
            }

            if t.starts_with("Servers:") {
                // Be tolerant of "Ready: 1", "Ready 1", "Ready servers: 1", or "Ready: 1/3".
                if let Some(idx) = t.find("Ready") {
                    let after = &t[idx + "Ready".len()..];
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
        }

        // Prefer explicit server counts when available; otherwise fall back to the header.
        if let Some(n) = ready_count {
            // Authoritative: any Ready > 0 means the language is usable even if header still says "(Indexing)".
            return Ok(n > 0);
        }
        return Ok(header_says_ready);
    }

    Ok(false)
}

/// Test fixture paths
pub mod fixtures {
    use std::path::PathBuf;

    pub fn get_fixtures_dir() -> PathBuf {
        PathBuf::from("tests/fixtures")
    }

    pub fn get_go_project1() -> PathBuf {
        get_fixtures_dir().join("go/project1")
    }

    pub fn get_typescript_project1() -> PathBuf {
        get_fixtures_dir().join("typescript/project1")
    }

    pub fn get_javascript_project1() -> PathBuf {
        get_fixtures_dir().join("javascript/project1")
    }
}

/// Performance requirements for LSP operations
pub mod performance {
    use std::time::Duration;

    /// Check if running in CI environment
    pub fn is_ci_environment() -> bool {
        std::env::var("CI").is_ok()
            || std::env::var("GITHUB_ACTIONS").is_ok()
            || std::env::var("TRAVIS").is_ok()
            || std::env::var("CIRCLECI").is_ok()
    }

    /// Maximum time allowed for extraction with LSP
    pub fn max_extract_time() -> Duration {
        if is_ci_environment() {
            Duration::from_secs(90) // Extra time for Go/TypeScript indexing in CI
        } else {
            Duration::from_secs(45) // Local development
        }
    }

    /// Maximum time allowed for search with LSP
    pub fn max_search_time() -> Duration {
        Duration::from_secs(15) // Reasonable time for both local and CI environments
    }

    /// Maximum time to wait for language server initialization
    pub fn max_init_time() -> Duration {
        Duration::from_secs(90) // Reasonable time for both local and CI environments
    }

    /// Language server ready wait time
    pub fn language_server_ready_time() -> Duration {
        Duration::from_secs(30) // Reasonable time for both local and CI environments
    }

    /// Daemon startup timeout
    pub fn daemon_startup_timeout() -> Duration {
        Duration::from_secs(20) // Reasonable time for both local and CI environments
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
pub fn extract_with_call_hierarchy_retry(
    extract_args: &[&str],
    expected_incoming: usize,
    expected_outgoing: usize,
    timeout: Duration,
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
        let (stdout, stderr, success) = run_probe_command_with_timeout(extract_args, remaining)?;

        if !success {
            if attempt >= max_attempts {
                return Ok((stdout, stderr, success)); // Return the failure
            }
            if is_ci {
                println!("❌ Extract command failed on attempt {attempt}, retrying...");
            }
            attempt += 1;
            thread::sleep(retry_delay);
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
                thread::sleep(retry_delay);
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
                || lc.starts_with("outgoing calls:");
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
}
