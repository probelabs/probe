use anyhow::Result;
use colored::*;
use serde_json::json;
use std::time::Duration;
use std::path::Path;

use crate::lsp_integration::client::LspClient;
use crate::lsp_integration::types::*;
use crate::lsp_integration::LspSubcommands;

pub struct LspManager;

impl LspManager {
    /// Ensure project is built to avoid cargo build lock conflicts
    fn ensure_project_built() -> Result<()> {
        let target_debug = Path::new("target/debug/probe");
        
        if !target_debug.exists() {
            eprintln!("⚠️  Project not built, building to avoid cargo lock conflicts...");
            let output = std::process::Command::new("cargo")
                .arg("build")
                .output()?;
                
            if !output.status.success() {
                eprintln!("❌ Build failed:");
                eprintln!("{}", String::from_utf8_lossy(&output.stderr));
                return Err(anyhow::anyhow!("Failed to build project"));
            }
            eprintln!("✅ Build completed successfully");
        }
        Ok(())
    }

    /// Handle LSP subcommands
    pub async fn handle_command(subcommand: &LspSubcommands, format: &str) -> Result<()> {
        match subcommand {
            LspSubcommands::Status { daemon, workspace_hint } => {
                Self::show_status(*daemon, workspace_hint.clone(), format).await
            }
            LspSubcommands::Languages => {
                Self::list_languages(format).await
            }
            LspSubcommands::Ping { daemon, workspace_hint } => {
                Self::ping(*daemon, workspace_hint.clone(), format).await
            }
            LspSubcommands::Shutdown => {
                Self::shutdown_daemon(format).await
            }
            LspSubcommands::Restart { workspace_hint } => {
                Self::restart_daemon(workspace_hint.clone(), format).await
            }
            LspSubcommands::Version => {
                Self::show_version(format).await
            }
            LspSubcommands::Start { socket, log_level, foreground } => {
                Self::start_embedded_daemon(socket.clone(), log_level.clone(), *foreground).await
            }
            LspSubcommands::Logs { follow, lines, clear } => {
                Self::handle_logs(*follow, *lines, *clear).await
            }
        }
    }

    /// Show daemon status
    async fn show_status(use_daemon: bool, workspace_hint: Option<String>, format: &str) -> Result<()> {
        // Check if we're being run via cargo and warn about potential conflicts
        if std::env::current_exe()
            .map(|path| path.to_string_lossy().contains("cargo"))
            .unwrap_or(false)
        {
            eprintln!("⚠️  WARNING: Running via 'cargo run' may cause build lock conflicts with daemon.");
            eprintln!("   If this hangs, use: cargo build && ./target/debug/probe lsp status");
        }

        let config = LspConfig {
            use_daemon,
            workspace_hint,
            timeout_ms: 240000, // Increased to 4 minutes for full rust-analyzer indexing (90s) + call hierarchy (60s)
        };

        let mut client = LspClient::new(config).await?;
        let status = client.get_status().await?;

        match format {
            "json" => {
                let json_output = json!({
                    "status": "connected",
                    "uptime_seconds": status.uptime.as_secs(),
                    "total_requests": status.total_requests,
                    "active_connections": status.active_connections,
                    "language_pools": status.language_pools
                });
                println!("{}", serde_json::to_string_pretty(&json_output)?);
            }
            _ => {
                println!("{}", "LSP Daemon Status".bold().green());
                println!("  {} {}", "Status:".bold(), "Connected".green());
                if !status.version.is_empty() {
                    println!("  {} {}", "Version:".bold(), status.version.cyan());
                }
                if !status.git_hash.is_empty() {
                    println!("  {} {}", "Git Hash:".bold(), status.git_hash.dimmed());
                }
                if !status.build_date.is_empty() {
                    println!("  {} {}", "Build Date:".bold(), status.build_date.dimmed());
                }
                println!("  {} {}", "Uptime:".bold(), format_duration(status.uptime));
                println!("  {} {}", "Total Requests:".bold(), status.total_requests.to_string().cyan());
                println!("  {} {}", "Active Connections:".bold(), status.active_connections.to_string().cyan());
                
                if !status.language_pools.is_empty() {
                    println!("\n{}", "Language Servers:".bold());
                    for (language, pool) in status.language_pools {
                        let status_text = if pool.available {
                            "Available".green()
                        } else {
                            "Unavailable".red()
                        };
                        
                        println!("  {} {} ({})", 
                            format!("{}:", language).bold(), 
                            status_text,
                            pool.status.dimmed());
                            
                        if pool.uptime_secs > 0 {
                            let uptime = format_duration(std::time::Duration::from_secs(pool.uptime_secs));
                            println!("    {} {}", "Uptime:".bold(), uptime.cyan());
                        }
                        
                        println!("    {} Ready: {}, Busy: {}, Total: {}", 
                            "Servers:".bold(),
                            pool.ready_servers.to_string().green(),
                            pool.busy_servers.to_string().yellow(),
                            pool.total_servers.to_string().cyan());
                            
                        if !pool.workspaces.is_empty() {
                            println!("    {} {}", "Workspaces:".bold(), pool.workspaces.len().to_string().cyan());
                            for workspace in &pool.workspaces {
                                if let Some(name) = std::path::Path::new(workspace).file_name() {
                                    println!("      • {}", name.to_string_lossy().dimmed());
                                }
                            }
                        }
                    }
                } else {
                    println!("\n{}", "No language servers initialized".yellow());
                }
            }
        }

        Ok(())
    }

    /// List available languages
    async fn list_languages(format: &str) -> Result<()> {
        let config = LspConfig::default();
        let mut client = LspClient::new(config).await?;
        let languages = client.list_languages().await?;

        match format {
            "json" => {
                println!("{}", serde_json::to_string_pretty(&languages)?);
            }
            _ => {
                println!("{}", "Available Language Servers".bold().green());
                
                if languages.is_empty() {
                    println!("  {}", "No language servers configured".yellow());
                    return Ok(());
                }

                for lang in languages {
                    let status_icon = if lang.available { "✓".green() } else { "✗".red() };
                    let status_text = if lang.available { "Available" } else { "Not Available" };
                    
                    println!("  {} {} {} ({})", 
                        status_icon,
                        format!("{:?}", lang.language).bold(),
                        status_text.dimmed(),
                        lang.lsp_server.dimmed());
                    
                    if !lang.available {
                        println!("    {} {}", "LSP Server:".yellow(), lang.lsp_server.dimmed());
                    }
                }
            }
        }

        Ok(())
    }

    /// Ping daemon for health check
    async fn ping(use_daemon: bool, workspace_hint: Option<String>, format: &str) -> Result<()> {
        let config = LspConfig {
            use_daemon,
            workspace_hint,
            timeout_ms: 30000, // Increased for rust-analyzer
        };

        let start_time = std::time::Instant::now();
        let mut client = LspClient::new(config).await?;
        
        client.ping().await?;
        let response_time = start_time.elapsed();

        match format {
            "json" => {
                let json_output = json!({
                    "status": "ok",
                    "response_time_ms": response_time.as_millis()
                });
                println!("{}", serde_json::to_string_pretty(&json_output)?);
            }
            _ => {
                println!("{} {} ({}ms)", 
                    "✓".green(), 
                    "LSP daemon is responsive".bold().green(),
                    response_time.as_millis().to_string().cyan());
            }
        }

        Ok(())
    }

    /// Shutdown daemon
    async fn shutdown_daemon(format: &str) -> Result<()> {
        let config = LspConfig::default();
        let mut client = LspClient::new(config).await?;
        
        client.shutdown_daemon().await?;

        match format {
            "json" => {
                let json_output = json!({
                    "status": "shutdown",
                    "message": "LSP daemon shutdown successfully"
                });
                println!("{}", serde_json::to_string_pretty(&json_output)?);
            }
            _ => {
                println!("{} {}", "✓".green(), "LSP daemon shutdown successfully".bold().green());
            }
        }

        Ok(())
    }

    /// Restart daemon
    async fn restart_daemon(workspace_hint: Option<String>, format: &str) -> Result<()> {
        // First shutdown existing daemon
        let config = LspConfig {
            use_daemon: true,
            workspace_hint: workspace_hint.clone(),
            timeout_ms: 30000, // Increased for rust-analyzer
        };

        let mut client = LspClient::new(config).await;
        
        // Try to shutdown if connected
        if let Ok(ref mut client) = client {
            let _ = client.shutdown_daemon().await;
        }

        // Wait a moment for shutdown to complete
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Start new daemon
        let config = LspConfig {
            use_daemon: true,
            workspace_hint,
            timeout_ms: 240000, // Increased to 4 minutes for full rust-analyzer indexing (90s) + call hierarchy (60s)
        };

        let mut client = LspClient::new(config).await?;
        
        // Verify it's working
        client.ping().await?;

        match format {
            "json" => {
                let json_output = json!({
                    "status": "restarted",
                    "message": "LSP daemon restarted successfully"
                });
                println!("{}", serde_json::to_string_pretty(&json_output)?);
            }
            _ => {
                println!("{} {}", "✓".green(), "LSP daemon restarted successfully".bold().green());
            }
        }

        Ok(())
    }

    /// Show version information
    async fn show_version(format: &str) -> Result<()> {
        let version = env!("CARGO_PKG_VERSION");
        let git_hash = env!("GIT_HASH");
        let build_date = env!("BUILD_DATE");
        
        match format {
            "json" => {
                let json_output = json!({
                    "version": version,
                    "git_hash": git_hash,
                    "build_date": build_date,
                    "component": "probe-lsp-client"
                });
                println!("{}", serde_json::to_string_pretty(&json_output)?);
            }
            _ => {
                println!("{}", "Probe LSP Version Information".bold().green());
                println!("  {} {}", "Version:".bold(), version.cyan());
                println!("  {} {}", "Git Hash:".bold(), git_hash.dimmed());
                println!("  {} {}", "Build Date:".bold(), build_date.dimmed());
                println!("  {} {}", "Component:".bold(), "LSP Client".green());
            }
        }

        Ok(())
    }

    /// Handle LSP logs command
    async fn handle_logs(follow: bool, lines: usize, clear: bool) -> Result<()> {
        use std::fs;
        use std::io::{BufRead, BufReader};
        use std::path::Path;
        
        let log_path = Path::new("/tmp/lsp-daemon.log");
        
        // Handle clear flag
        if clear {
            if log_path.exists() {
                fs::remove_file(log_path)?;
                println!("✓ Log file cleared");
            } else {
                println!("No log file found");
            }
            return Ok(());
        }
        
        // Check if log file exists
        if !log_path.exists() {
            println!("{}", "No LSP daemon log file found".yellow());
            println!("To enable logging, set the LSP_LOG environment variable:");
            println!("  {} cargo run -- lsp start", "LSP_LOG=1".cyan());
            println!("or");
            println!("  {} probe extract <file> --lsp", "LSP_LOG=1".cyan());
            return Ok(());
        }
        
        if follow {
            // Follow mode (like tail -f)
            println!("{}", "Following LSP daemon log (Ctrl+C to stop)...".green().bold());
            println!("{}", "─".repeat(60).dimmed());
            
            // First show the last N lines
            let file = fs::File::open(log_path)?;
            let reader = BufReader::new(file);
            let all_lines: Vec<String> = reader.lines().collect::<Result<Vec<_>, _>>()?;
            
            let start_idx = all_lines.len().saturating_sub(lines);
            for line in &all_lines[start_idx..] {
                println!("{}", line);
            }
            
            // Then follow new lines using a more robust approach
            use std::io::{Seek, SeekFrom};
            
            let mut last_size = fs::metadata(log_path)?.len();
            
            loop {
                let current_size = fs::metadata(log_path)?.len();
                
                if current_size > last_size {
                    let file = fs::File::open(log_path)?;
                    let mut reader = BufReader::new(file);
                    
                    // Seek to where we left off
                    reader.seek(SeekFrom::Start(last_size))?;
                    
                    // Read all new lines
                    for line_result in reader.lines() {
                        match line_result {
                            Ok(line) => {
                                // Apply the same coloring as the static view and pretty-print JSON
                                if line.contains(">>> TO LSP:") || line.contains("<<< FROM LSP:") {
                                    Self::print_formatted_lsp_line(&line);
                                } else if line.contains("ERROR") || line.contains("error") {
                                    println!("{}", line.red());
                                } else if line.contains("WARN") || line.contains("warning") {
                                    println!("{}", line.yellow());
                                } else if line.contains("INFO") {
                                    println!("{}", line.blue());
                                } else if line.contains("DEBUG") {
                                    println!("{}", line.dimmed());
                                } else if line.contains("==========") {
                                    println!("{}", line.bold());
                                } else {
                                    println!("{}", line);
                                }
                            },
                            Err(_) => break,
                        }
                    }
                    
                    last_size = current_size;
                } else {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                }
            }
        } else {
            // Show last N lines
            let file = fs::File::open(log_path)?;
            let reader = BufReader::new(file);
            let all_lines: Vec<String> = reader.lines().collect::<Result<Vec<_>, _>>()?;
            
            if all_lines.is_empty() {
                println!("{}", "Log file is empty".yellow());
                return Ok(());
            }
            
            let total_lines = all_lines.len();
            let start_idx = total_lines.saturating_sub(lines);
            
            println!("{}", format!("LSP Daemon Log (last {} lines of {})", 
                lines.min(total_lines), total_lines).bold().green());
            println!("{}", "─".repeat(60).dimmed());
            
            for line in &all_lines[start_idx..] {
                // Highlight different log levels and format LSP JSON
                if line.contains(">>> TO LSP:") || line.contains("<<< FROM LSP:") {
                    Self::print_formatted_lsp_line(line);
                } else if line.contains("ERROR") || line.contains("error") {
                    println!("{}", line.red());
                } else if line.contains("WARN") || line.contains("warning") {
                    println!("{}", line.yellow());
                } else if line.contains("INFO") {
                    println!("{}", line.blue());
                } else if line.contains("DEBUG") {
                    println!("{}", line.dimmed());
                } else if line.contains("==========") {
                    println!("{}", line.bold());
                } else {
                    println!("{}", line);
                }
            }
            
            println!("{}", "─".repeat(60).dimmed());
            println!("Use {} to follow log in real-time", "--follow".cyan());
            println!("Use {} to clear the log file", "--clear".cyan());
        }
        
        Ok(())
    }

    /// Start embedded LSP daemon
    async fn start_embedded_daemon(socket: Option<String>, log_level: String, foreground: bool) -> Result<()> {
        use lsp_daemon::LspDaemon;
        use tracing_subscriber::EnvFilter;

        // Check if we're being run via cargo and warn about potential conflicts
        if std::env::current_exe()
            .map(|path| path.to_string_lossy().contains("cargo"))
            .unwrap_or(false)
        {
            eprintln!("⚠️  WARNING: Running LSP daemon via 'cargo run' may cause build lock conflicts.");
            eprintln!("   For better performance, build first: cargo build");
            eprintln!("   Then use: ./target/debug/probe lsp start -f");
        }

        // Initialize logging
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(&log_level));

        // Check if LSP_LOG is set to enable file logging
        if std::env::var("LSP_LOG").is_ok() {
            // Set up file logging to /tmp/lsp-daemon.log
            use std::fs::OpenOptions;
            use std::io::Write;
            use tracing_subscriber::fmt::writer::MakeWriterExt;
            
            let log_file = OpenOptions::new()
                .create(true)
                .append(true)
                .open("/tmp/lsp-daemon.log")
                .expect("Failed to open log file");
            
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_target(false)
                .with_writer(log_file.and(std::io::stderr))
                .init();
                
            eprintln!("LSP logging enabled - writing to /tmp/lsp-daemon.log");
        } else {
            tracing_subscriber::fmt()
                .with_env_filter(filter)
                .with_target(false)
                .init();
        }

        // Determine socket path
        let socket_path = socket.unwrap_or_else(|| lsp_daemon::get_default_socket_path());

        println!("🚀 Starting embedded LSP daemon...");
        println!("   Socket: {}", socket_path);
        println!("   Log Level: {}", log_level);
        
        if foreground {
            println!("   Mode: Foreground");
        } else {
            println!("   Mode: Background");
        }

        // Create and start daemon
        let daemon = LspDaemon::new(socket_path)?;

        if foreground {
            println!("✓ LSP daemon started in foreground mode");
            daemon.run().await?;
        } else {
            println!("✓ LSP daemon started in background mode");
            // For background mode, we would typically daemonize the process
            // For now, just run in foreground since we're embedded
            daemon.run().await?;
        }

        Ok(())
    }

    /// Format and print LSP log line with proper JSON formatting
    fn print_formatted_lsp_line(line: &str) {
        // Extract the JSON part from the log line
        // Format: "[15:01:18.346] >>> TO LSP: {json content}"
        if let Some(json_start) = line.find('{') {
            let timestamp_and_prefix = &line[..json_start];
            let json_part = &line[json_start..];
            
            // Try to parse and pretty-print the JSON
            match serde_json::from_str::<serde_json::Value>(json_part) {
                Ok(parsed) => {
                    match serde_json::to_string_pretty(&parsed) {
                        Ok(pretty_json) => {
                            // Print the timestamp/prefix in color, then the pretty JSON
                            if line.contains(">>> TO LSP:") {
                                print!("{}", timestamp_and_prefix.cyan());
                            } else {
                                print!("{}", timestamp_and_prefix.green());
                            }
                            println!("{}", pretty_json);
                        }
                        Err(_) => {
                            // Fallback to original line with coloring
                            if line.contains(">>> TO LSP:") {
                                println!("{}", line.cyan());
                            } else {
                                println!("{}", line.green());
                            }
                        }
                    }
                }
                Err(_) => {
                    // Fallback to original line with coloring
                    if line.contains(">>> TO LSP:") {
                        println!("{}", line.cyan());
                    } else {
                        println!("{}", line.green());
                    }
                }
            }
        } else {
            // No JSON found, just color the line
            if line.contains(">>> TO LSP:") {
                println!("{}", line.cyan());
            } else {
                println!("{}", line.green());
            }
        }
    }
}

/// Format duration in a human-readable way
fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    
    if total_seconds < 60 {
        format!("{}s", total_seconds)
    } else if total_seconds < 3600 {
        let minutes = total_seconds / 60;
        let seconds = total_seconds % 60;
        format!("{}m {}s", minutes, seconds)
    } else {
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        format!("{}h {}m", hours, minutes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(Duration::from_secs(90)), "1m 30s");
        assert_eq!(format_duration(Duration::from_secs(3661)), "1h 1m");
    }
}