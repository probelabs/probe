use anyhow::Result;
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::time::sleep;

use crate::lsp_integration::{LspClient, LspConfig};

/// Result of checking LSP server readiness
#[derive(Debug, Clone)]
pub struct ReadinessCheckResult {
    pub is_ready: bool,
    pub server_type: Option<String>,
    pub expected_timeout_secs: Option<u64>,
    pub elapsed_secs: u64,
    pub status_message: String,
}

/// Configuration for readiness checking
#[derive(Debug, Clone)]
pub struct ReadinessConfig {
    /// Maximum time to wait for server to become ready
    pub max_wait_secs: u64,
    /// How often to poll for readiness (in milliseconds)
    pub poll_interval_ms: u64,
    /// Whether to show progress messages to the user
    pub show_progress: bool,
    /// Whether to auto-start the daemon if not running
    pub auto_start_daemon: bool,
}

impl Default for ReadinessConfig {
    fn default() -> Self {
        Self {
            max_wait_secs: 30,
            poll_interval_ms: 500,
            show_progress: true,
            auto_start_daemon: true,
        }
    }
}

/// Check if LSP server is ready for the given file
/// This function will wait for server readiness up to the configured timeout
pub async fn check_lsp_readiness_for_file<P: AsRef<Path>>(
    file_path: P,
    config: ReadinessConfig,
) -> Result<ReadinessCheckResult> {
    let start_time = Instant::now();
    let file_path = file_path.as_ref();

    // Determine language from file extension
    let language = determine_language_from_path(file_path);

    if config.show_progress && language.is_some() {
        println!(
            "Checking LSP server readiness for {} files...",
            language.as_ref().unwrap()
        );
    }

    // Create LSP client configuration
    let lsp_config = LspConfig {
        use_daemon: true,
        workspace_hint: file_path
            .parent()
            .and_then(|p| p.to_str().map(|s| s.to_string())),
        timeout_ms: 5000, // Short timeout for readiness checks
        include_stdlib: false,
        auto_start: config.auto_start_daemon,
    };

    // Try to connect to LSP daemon (auto-start if needed)
    let mut client = if config.auto_start_daemon {
        match LspClient::new(lsp_config).await {
            Ok(client) => client,
            Err(e) => {
                return Ok(ReadinessCheckResult {
                    is_ready: false,
                    server_type: language.clone(),
                    expected_timeout_secs: Some(config.max_wait_secs),
                    elapsed_secs: start_time.elapsed().as_secs(),
                    status_message: format!("Failed to connect to LSP daemon: {}", e),
                });
            }
        }
    } else {
        // Try non-blocking connection first
        match LspClient::new_non_blocking(lsp_config.clone()).await {
            Some(client) => client,
            None => {
                return Ok(ReadinessCheckResult {
                    is_ready: false,
                    server_type: language.clone(),
                    expected_timeout_secs: Some(config.max_wait_secs),
                    elapsed_secs: start_time.elapsed().as_secs(),
                    status_message: "LSP daemon not running and auto-start disabled".to_string(),
                });
            }
        }
    };

    // Poll for readiness
    let poll_interval = Duration::from_millis(config.poll_interval_ms);
    let max_wait = Duration::from_secs(config.max_wait_secs);

    let mut last_status_update = Instant::now();
    let status_update_interval = Duration::from_secs(5); // Update every 5 seconds

    while start_time.elapsed() < max_wait {
        // Check readiness status
        match client.get_readiness_status(file_path).await {
            Ok(status) => {
                if status.is_ready {
                    if config.show_progress {
                        println!(
                            "✓ LSP server ready for {} (took {:.1}s)",
                            language.as_deref().unwrap_or("unknown"),
                            start_time.elapsed().as_secs_f64()
                        );
                    }

                    return Ok(ReadinessCheckResult {
                        is_ready: true,
                        server_type: language,
                        expected_timeout_secs: status.expected_timeout_secs,
                        elapsed_secs: start_time.elapsed().as_secs(),
                        status_message: "Ready".to_string(),
                    });
                }

                // Show progress updates periodically
                if config.show_progress && last_status_update.elapsed() >= status_update_interval {
                    let elapsed = start_time.elapsed().as_secs();
                    let remaining = config.max_wait_secs.saturating_sub(elapsed);

                    println!(
                        "Waiting for {} server to initialize... ({:.1}s elapsed, {}s remaining)",
                        language.as_deref().unwrap_or("LSP"),
                        start_time.elapsed().as_secs_f64(),
                        remaining
                    );

                    // Show detailed status if available
                    if !status.status_message.is_empty() && status.status_message != "Ready" {
                        println!("  Status: {}", status.status_message);
                    }

                    if let Some(expected) = status.expected_timeout_secs {
                        if elapsed < expected {
                            println!("  Expected initialization time: {}s", expected);
                        }
                    }

                    last_status_update = Instant::now();
                }
            }
            Err(e) => {
                if config.show_progress {
                    eprintln!("Warning: Failed to check readiness status: {}", e);
                }
                // Continue polling - daemon might be starting up
            }
        }

        sleep(poll_interval).await;
    }

    // Timeout reached
    let elapsed_secs = start_time.elapsed().as_secs();
    let status_message = format!(
        "Timeout waiting for {} server to become ready (waited {}s)",
        language.as_deref().unwrap_or("LSP"),
        elapsed_secs
    );

    if config.show_progress {
        println!("⚠ {}", status_message);
    }

    Ok(ReadinessCheckResult {
        is_ready: false,
        server_type: language,
        expected_timeout_secs: Some(config.max_wait_secs),
        elapsed_secs,
        status_message,
    })
}

/// Check if any of the given file paths need LSP and wait for readiness
pub async fn check_lsp_readiness_for_files<P: AsRef<Path>>(
    file_paths: &[P],
    config: ReadinessConfig,
) -> Result<Vec<ReadinessCheckResult>> {
    let mut results = Vec::new();
    let mut languages_checked = std::collections::HashSet::new();

    // Group files by language to avoid duplicate checks
    for file_path in file_paths {
        let language = determine_language_from_path(file_path.as_ref());

        if let Some(lang) = &language {
            if !languages_checked.contains(lang) {
                languages_checked.insert(lang.clone());

                let result = check_lsp_readiness_for_file(file_path, config.clone()).await?;
                results.push(result);

                // If this language server isn't ready, no point checking others of the same type
                if !results.last().unwrap().is_ready {
                    if config.show_progress {
                        eprintln!("Skipping other {} files since server is not ready", lang);
                    }
                }
            }
        }
    }

    Ok(results)
}

/// Wait for LSP readiness with user-friendly output
pub async fn wait_for_lsp_readiness<P: AsRef<Path>>(
    file_path: P,
    timeout_secs: Option<u64>,
    show_progress: bool,
) -> Result<bool> {
    let config = ReadinessConfig {
        max_wait_secs: timeout_secs.unwrap_or(30),
        poll_interval_ms: 500,
        show_progress,
        auto_start_daemon: true,
    };

    let result = check_lsp_readiness_for_file(file_path, config).await?;

    if !result.is_ready && show_progress {
        println!("Proceeding without LSP enhancement due to server readiness timeout");
    }

    Ok(result.is_ready)
}

/// Determine the language/server type from a file path
fn determine_language_from_path(file_path: &Path) -> Option<String> {
    use probe_code::language::factory::get_language_impl;

    if let Some(extension) = file_path.extension().and_then(|e| e.to_str()) {
        // Check if we have language support for this extension
        if get_language_impl(extension).is_some() {
            // Map common extensions to language names
            let language_name = match extension {
                "rs" => "rust",
                "py" | "pyw" => "python",
                "js" | "jsx" => "javascript",
                "ts" | "tsx" => "typescript",
                "go" => "go",
                "java" => "java",
                "c" => "c",
                "cpp" | "cc" | "cxx" => "cpp",
                "cs" => "csharp",
                "rb" => "ruby",
                "php" => "php",
                "swift" => "swift",
                _ => extension, // fallback to extension
            };
            return Some(language_name.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_determine_language_from_path() {
        assert_eq!(
            determine_language_from_path(&PathBuf::from("test.rs")),
            Some("rust".to_string())
        );
        assert_eq!(
            determine_language_from_path(&PathBuf::from("test.py")),
            Some("python".to_string())
        );
        assert_eq!(
            determine_language_from_path(&PathBuf::from("test.js")),
            Some("javascript".to_string())
        );
        assert_eq!(
            determine_language_from_path(&PathBuf::from("test.unknown")),
            None
        );
    }

    #[test]
    fn test_readiness_config_default() {
        let config = ReadinessConfig::default();
        assert_eq!(config.max_wait_secs, 30);
        assert_eq!(config.poll_interval_ms, 500);
        assert!(config.show_progress);
        assert!(config.auto_start_daemon);
    }
}
