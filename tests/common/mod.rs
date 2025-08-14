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
                "Install with: npm install -g typescript-language-server typescript"
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
    env::var("PATH")
        .unwrap_or_default()
        .split(if cfg!(windows) { ';' } else { ':' })
        .any(|path| {
            let mut cmd_path = std::path::PathBuf::from(path);
            cmd_path.push(command);

            // On Windows, try with .exe extension too
            if cfg!(windows) {
                cmd_path.set_extension("exe");
            }

            cmd_path.exists() && cmd_path.is_file()
        })
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

    let output = Command::new("./target/debug/probe")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("Failed to execute probe command")?;

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
    let success = output.status.success();

    Ok((stdout, stderr, success))
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
    start_daemon_and_wait_with_retries(3)
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
            eprintln!("Waiting {:?} before retry...", wait_time);
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

/// Wait for language server to be ready (indexed) with dynamic timeout
pub fn wait_for_language_server_ready(timeout: Duration) {
    // Use the larger of the provided timeout or the CI-aware timeout
    let ci_aware_timeout = performance::language_server_ready_time();
    let actual_timeout = std::cmp::max(timeout, ci_aware_timeout);

    if performance::is_ci_environment() {
        println!(
            "CI environment detected: waiting {:?} for language server to be ready",
            actual_timeout
        );
    } else {
        println!(
            "Waiting {:?} for language server to be ready",
            actual_timeout
        );
    }

    thread::sleep(actual_timeout);
}

/// Wait for language server with health check polling
pub fn wait_for_language_server_ready_with_health_check(_workspace_path: &str) -> Result<()> {
    let timeout = performance::language_server_ready_time();
    let poll_interval = Duration::from_millis(2000);
    let max_polls = (timeout.as_millis() / poll_interval.as_millis()) as u32;

    println!("Waiting for language server to be ready with health checks...");

    for poll in 0..max_polls {
        thread::sleep(poll_interval);

        // Check daemon status to see if language servers are healthy
        if let Ok((stdout, _, success)) =
            run_probe_command_with_timeout(&["lsp", "status"], Duration::from_secs(5))
        {
            if success && (stdout.contains("Ready") || stdout.contains("Healthy")) {
                println!(
                    "Language server appears ready after {:?}",
                    poll_interval * (poll + 1)
                );
                return Ok(());
            }
        }

        if poll % 5 == 0 && poll > 0 {
            println!(
                "Still waiting for language server... ({:?} elapsed)",
                poll_interval * (poll + 1)
            );
        }
    }

    println!("Timeout waiting for language server health check, proceeding anyway");
    Ok(())
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
            Duration::from_secs(15) // Increased for CI
        } else {
            Duration::from_secs(3)
        }
    }

    /// Maximum time allowed for search with LSP
    pub fn max_search_time() -> Duration {
        if is_ci_environment() {
            Duration::from_secs(20) // Increased for CI
        } else {
            Duration::from_secs(5)
        }
    }

    /// Maximum time to wait for language server initialization
    pub fn max_init_time() -> Duration {
        if is_ci_environment() {
            Duration::from_secs(120) // Increased for CI
        } else {
            Duration::from_secs(60)
        }
    }

    /// Language server ready wait time
    pub fn language_server_ready_time() -> Duration {
        if is_ci_environment() {
            Duration::from_secs(30) // Increased for CI
        } else {
            Duration::from_secs(15)
        }
    }

    /// Daemon startup timeout
    pub fn daemon_startup_timeout() -> Duration {
        if is_ci_environment() {
            Duration::from_secs(30)
        } else {
            Duration::from_secs(10)
        }
    }

    // Legacy constants for backward compatibility
    pub const MAX_EXTRACT_TIME: Duration = Duration::from_secs(3);
    pub const MAX_SEARCH_TIME: Duration = Duration::from_secs(5);
    pub const MAX_INIT_TIME: Duration = Duration::from_secs(60);
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
        let section_start = format!("## {section_name}");

        if let Some(start_pos) = output.find(&section_start) {
            let after_header = &output[start_pos + section_start.len()..];

            // Find the end of this section (next ## header or end of string)
            let end_pos = after_header.find("\n## ").unwrap_or(after_header.len());
            let section = &after_header[..end_pos];

            Ok(section.to_string())
        } else {
            Err(format!("Section '{section_name}' not found in output"))
        }
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
