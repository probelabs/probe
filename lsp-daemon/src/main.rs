use anyhow::Result;
use clap::Parser;
use lsp_daemon::get_default_socket_path;
use lsp_daemon::LspDaemon;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[clap(
    author,
    version,
    about = "LSP Daemon - Multi-language LSP server pool manager"
)]
struct Args {
    /// Path to the IPC endpoint (Unix socket or Windows named pipe)
    #[clap(short, long, default_value_t = get_default_socket_path())]
    socket: String,

    /// Log level (trace, debug, info, warn, error)
    #[clap(short, long, default_value = "info")]
    log_level: String,

    /// Run in foreground (don't daemonize)
    #[clap(short, long)]
    foreground: bool,
}

fn setup_crash_logging() -> PathBuf {
    let crash_log_path = if cfg!(target_os = "macos") {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("probe")
            .join("lsp-daemon-crashes.log")
    } else if cfg!(target_os = "windows") {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("C:\\temp"))
            .join("probe")
            .join("lsp-daemon-crashes.log")
    } else {
        dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("probe")
            .join("lsp-daemon-crashes.log")
    };

    // Create directory if it doesn't exist
    if let Some(parent) = crash_log_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let log_path_for_hook = crash_log_path.clone();

    // Set up panic hook to write crashes to file
    std::panic::set_hook(Box::new(move |panic_info| {
        let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S%.3f UTC");
        let backtrace = std::backtrace::Backtrace::force_capture();

        // Check if this is a turso/SQL parsing panic
        let panic_str = format!("{}", panic_info);
        let is_turso_panic = panic_str.contains("turso")
            || panic_str
                .contains("Successful parse on nonempty input string should produce a command")
            || panic_str.contains("SQL parsing");

        if is_turso_panic {
            // For turso panics, log as error but try to continue
            let error_message = format!(
                "\n=== TURSO SQL ERROR (HANDLED) ===\nTimestamp: {}\nError: {}\n=================================\n\n",
                timestamp, panic_info
            );

            // Log to file
            if let Ok(mut file) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_path_for_hook)
            {
                let _ = file.write_all(error_message.as_bytes());
                let _ = file.flush();
            }

            // Log to stderr but don't crash
            eprintln!("TURSO SQL ERROR: {}", panic_info);
            return; // Don't abort, just return
        }

        // For non-turso panics, proceed with normal crash handling
        let crash_message = format!(
            "\n=== LSP DAEMON CRASH ===\nTimestamp: {}\nPanic: {}\nBacktrace:\n{}\n======================\n\n",
            timestamp, panic_info, backtrace
        );

        // Try to write to crash log file
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path_for_hook)
        {
            let _ = file.write_all(crash_message.as_bytes());
            let _ = file.flush();
        }

        // Also try to write to stderr
        eprintln!("{}", crash_message);
    }));

    crash_log_path
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Set up crash logging first
    let crash_log_path = setup_crash_logging();

    // Initialize logging
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&args.log_level));

    tracing_subscriber::fmt().with_env_filter(filter).init();

    info!("Starting LSP daemon v{}", env!("CARGO_PKG_VERSION"));
    info!(
        "Crash logs will be written to: {}",
        crash_log_path.display()
    );

    // Create daemon with async initialization for persistence support
    let daemon = LspDaemon::new_async(args.socket).await?;

    // Run daemon
    if let Err(e) = daemon.run().await {
        error!("Daemon error: {}", e);
        return Err(e);
    }

    info!("Daemon shutdown complete");
    Ok(())
}
