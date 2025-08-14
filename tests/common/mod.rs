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
            LanguageServer::TypeScriptLanguageServer => "typescript-language-server (TypeScript/JavaScript language server)",
        }
    }

    /// Get installation instructions for this language server
    pub fn installation_instructions(&self) -> &'static str {
        match self {
            LanguageServer::Gopls => "Install with: go install golang.org/x/tools/gopls@latest",
            LanguageServer::TypeScriptLanguageServer => "Install with: npm install -g typescript-language-server typescript",
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
        let mut error_msg = String::from("CRITICAL: Missing required language servers for CI tests:\n\n");
        
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
        LanguageServer::Gopls => {
            Command::new("gopls")
                .arg("version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|status| status.success())
                .unwrap_or(false)
        }
        LanguageServer::TypeScriptLanguageServer => {
            Command::new("typescript-language-server")
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|status| status.success())
                .unwrap_or(false)
        }
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
pub fn run_probe_command(args: &[&str]) -> Result<(String, String, bool)> {
    run_probe_command_with_timeout(args, Duration::from_secs(30))
}

/// Helper to run probe commands with custom timeout
pub fn run_probe_command_with_timeout(args: &[&str], timeout: Duration) -> Result<(String, String, bool)> {
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

/// Helper to start daemon and wait for it to be ready
pub fn start_daemon_and_wait() -> Result<()> {
    // Start daemon in background
    let _ = Command::new("./target/debug/probe")
        .args(["lsp", "start"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("Failed to start LSP daemon")?;

    // Wait for daemon to be ready (try status command)
    for attempt in 0..20 {
        thread::sleep(Duration::from_millis(500));

        let output = Command::new("./target/debug/probe")
            .args(["lsp", "status"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .output();

        if let Ok(output) = output {
            if output.status.success() {
                return Ok(());
            }
        }

        if attempt >= 19 {
            return Err(anyhow::anyhow!("Daemon failed to start within timeout (10 seconds)"));
        }
    }

    unreachable!()
}

/// Initialize LSP workspace for testing
pub fn init_lsp_workspace(workspace_path: &str, languages: &[&str]) -> Result<()> {
    let languages_str = languages.join(",");
    let mut args = vec!["lsp", "init", "-w", workspace_path, "--languages"];
    args.push(&languages_str);

    let (stdout, stderr, success) = run_probe_command_with_timeout(&args, Duration::from_secs(60))?;

    if !success {
        return Err(anyhow::anyhow!(
            "LSP workspace initialization failed.\nArgs: {:?}\nStdout: {}\nStderr: {}",
            args,
            stdout,
            stderr
        ));
    }

    Ok(())
}

/// Wait for language server to be ready (indexed)
pub fn wait_for_language_server_ready(timeout: Duration) {
    thread::sleep(timeout);
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

    /// Maximum time allowed for extraction with LSP
    pub const MAX_EXTRACT_TIME: Duration = Duration::from_secs(3);

    /// Maximum time allowed for search with LSP
    pub const MAX_SEARCH_TIME: Duration = Duration::from_secs(5);

    /// Maximum time to wait for language server initialization
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
                "Expected {} incoming calls, found {}. Section content: {}",
                expected_count, actual_count, incoming_section
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
                "Expected {} outgoing calls, found {}. Section content: {}",
                expected_count, actual_count, outgoing_section
            ));
        }
        
        Ok(())
    }

    /// Extract a specific call hierarchy section from output
    fn extract_call_hierarchy_section(output: &str, section_name: &str) -> Result<String, String> {
        let section_start = format!("## {}", section_name);
        
        if let Some(start_pos) = output.find(&section_start) {
            let after_header = &output[start_pos + section_start.len()..];
            
            // Find the end of this section (next ## header or end of string)
            let end_pos = after_header.find("\n## ").unwrap_or(after_header.len());
            let section = &after_header[..end_pos];
            
            Ok(section.to_string())
        } else {
            Err(format!("Section '{}' not found in output", section_name))
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
        assert_eq!(LanguageServer::TypeScriptLanguageServer.command_name(), "typescript-language-server");
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