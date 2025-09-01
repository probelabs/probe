//! Position Analyzer CLI Tool
//!
//! This tool analyzes LSP server position requirements by systematically testing
//! different position offsets and building deterministic mappings.
//!
//! Usage examples:
//!   cargo run --bin position-analyzer tests/fixtures/rust/main.rs calculate
//!   cargo run --bin position-analyzer --lsp-server rust-analyzer src/main.rs my_function
//!   cargo run --bin position-analyzer --operation call-hierarchy --verbose src/lib.rs MyStruct

use anyhow::Result;
use clap::{Parser, ValueEnum};
use colored::*;
use std::path::PathBuf;
use tracing::{info, Level};

use probe_code::lsp_integration::{LspClient, LspConfig, LspOperation, PositionAnalyzer};

#[derive(Parser)]
#[command(
    name = "position-analyzer",
    about = "Analyze LSP server position requirements and build deterministic position mappings",
    long_about = "This tool systematically tests different position offsets with LSP servers to discover \
                  which positions work reliably for different symbol types and languages. It replaces \
                  guessing algorithms with empirically-derived position patterns."
)]
struct Args {
    /// File path to analyze
    #[arg(help = "Path to the source file containing the symbol")]
    file_path: Option<PathBuf>,

    /// Symbol name to analyze
    #[arg(help = "Name of the symbol (function, struct, etc.) to test")]
    symbol_name: Option<String>,

    /// LSP server name to test with
    #[arg(
        long,
        help = "Specific LSP server to test (rust-analyzer, gopls, pylsp, etc.)"
    )]
    lsp_server: Option<String>,

    /// LSP operation to test
    #[arg(
        long,
        value_enum,
        default_value = "call-hierarchy",
        help = "LSP operation to test with the symbol"
    )]
    operation: CliLspOperation,

    /// Enable verbose output
    #[arg(short, long, help = "Show detailed position testing information")]
    verbose: bool,

    /// Timeout for LSP operations in seconds
    #[arg(
        long,
        default_value = "30",
        help = "Timeout for LSP operations in seconds"
    )]
    timeout: u64,

    /// Only test specific position offsets
    #[arg(
        long,
        help = "Comma-separated list of offsets to test (start, middle, end, start+1, start+2, etc.)"
    )]
    test_offsets: Option<String>,

    /// Show pattern statistics and exit
    #[arg(long, help = "Show statistics about discovered patterns and exit")]
    show_stats: bool,

    /// Save discovered patterns to file
    #[arg(long, help = "Save discovered patterns to the specified file")]
    save_patterns: Option<PathBuf>,

    /// Load existing patterns from file
    #[arg(long, help = "Load existing patterns from the specified file")]
    load_patterns: Option<PathBuf>,
}

#[derive(Debug, Clone, ValueEnum)]
enum CliLspOperation {
    CallHierarchy,
    GoToDefinition,
    FindReferences,
    Hover,
    DocumentSymbols,
}

impl From<CliLspOperation> for LspOperation {
    fn from(cli_op: CliLspOperation) -> Self {
        match cli_op {
            CliLspOperation::CallHierarchy => LspOperation::CallHierarchy,
            CliLspOperation::GoToDefinition => LspOperation::GoToDefinition,
            CliLspOperation::FindReferences => LspOperation::FindReferences,
            CliLspOperation::Hover => LspOperation::Hover,
            CliLspOperation::DocumentSymbols => LspOperation::DocumentSymbols,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Setup logging
    let log_level = if args.verbose {
        Level::DEBUG
    } else {
        Level::INFO
    };

    tracing_subscriber::fmt()
        .with_max_level(log_level)
        .with_target(false)
        .init();

    // Initialize analyzer
    let mut analyzer = PositionAnalyzer::new();

    // Load existing patterns if specified
    if let Some(patterns_file) = &args.load_patterns {
        analyzer.load_patterns(Some(patterns_file)).await?;
        info!("Loaded patterns from {}", patterns_file.display());
    } else {
        analyzer.load_patterns(None).await?;
    }

    // Show statistics if requested
    if args.show_stats {
        show_analyzer_statistics(&analyzer);
        return Ok(());
    }

    // Validate required arguments when not showing stats
    let file_path = args
        .file_path
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("File path is required when not using --show-stats"))?;

    let symbol_name = args
        .symbol_name
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("Symbol name is required when not using --show-stats"))?;

    // Validate file exists
    if !file_path.exists() {
        eprintln!("{}: File not found: {}", "Error".red(), file_path.display());
        std::process::exit(1);
    }

    println!("{} Analyzing LSP position requirements", "🔍".blue());
    println!("  File: {}", file_path.display().to_string().cyan());
    println!("  Symbol: {}", symbol_name.yellow());
    println!("  Operation: {}", format!("{:?}", args.operation).green());

    if let Some(ref server) = args.lsp_server {
        println!("  LSP Server: {}", server.magenta());
    }
    println!();

    // Setup LSP client
    let config = LspConfig {
        use_daemon: true,
        workspace_hint: None,
        timeout_ms: args.timeout * 1000,
        include_stdlib: false,
    };

    let mut lsp_client = match LspClient::new(config).await {
        Ok(client) => {
            info!("Successfully connected to LSP daemon");
            client
        }
        Err(e) => {
            eprintln!("{}: Failed to connect to LSP daemon: {}", "Error".red(), e);
            eprintln!("Make sure the LSP daemon is running and supports the file type");
            std::process::exit(1);
        }
    };

    // Check if LSP supports this file
    if !lsp_client.is_supported(file_path) {
        eprintln!(
            "{}: LSP not supported for file type: {}",
            "Error".red(),
            file_path
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("unknown")
        );
        std::process::exit(1);
    }

    // Parse operation
    let operation: LspOperation = args.operation.clone().into();

    // Override test offsets if specified
    if let Some(ref _offset_str) = args.test_offsets {
        // This would require modifying PositionAnalyzer to accept custom offsets
        eprintln!(
            "{}: Custom test offsets not yet implemented",
            "Warning".yellow()
        );
    }

    // Run the analysis
    println!("{} Starting position analysis...", "🚀".green());

    let start_time = std::time::Instant::now();
    let results = match analyzer
        .analyze_symbol_position(file_path, symbol_name, operation, &mut lsp_client)
        .await
    {
        Ok(results) => results,
        Err(e) => {
            eprintln!("{}: Analysis failed: {}", "Error".red(), e);
            std::process::exit(1);
        }
    };

    let elapsed = start_time.elapsed();

    println!();
    println!(
        "{} Analysis complete in {:.2}s",
        "✅".green(),
        elapsed.as_secs_f64()
    );
    println!();

    // Display results
    display_analysis_results(&results);

    // Show discovered patterns
    show_discovered_patterns(&analyzer, &args);

    // Save patterns if requested
    if let Some(save_file) = &args.save_patterns {
        analyzer.save_patterns(save_file).await?;
        println!(
            "\n{} Patterns saved to {}",
            "💾".blue(),
            save_file.display().to_string().cyan()
        );
    }

    Ok(())
}

fn display_analysis_results(
    results: &[probe_code::lsp_integration::position_analyzer::PositionTestResult],
) {
    println!("{} Position Test Results:", "📊".blue());
    println!();

    let mut successful_positions = Vec::new();
    let mut failed_positions = Vec::new();

    for result in results {
        let status_icon = if result.success { "✓" } else { "✗" };
        let status_color = if result.success { "green" } else { "red" };

        let position_str = format!("{}:{}", result.position.0 + 1, result.position.1 + 1); // Convert to 1-based
        let offset_desc = result.offset.description();

        print!(
            "  {} {:<12} ",
            status_icon.color(status_color),
            position_str.cyan()
        );
        print!("({:<20}) ", offset_desc.dimmed());

        if result.success {
            if let Some(response_time) = result.response_time {
                println!(
                    "{} in {:.0}ms",
                    "SUCCESS".green(),
                    response_time.as_millis()
                );
            } else {
                println!("{}", "SUCCESS".green());
            }
            successful_positions.push((result.position, &result.offset));
        } else {
            print!("{}", "FAILED".red());
            if let Some(ref error) = result.error {
                let error_preview = if error.len() > 50 {
                    format!("{}...", &error[..47])
                } else {
                    error.clone()
                };
                print!(" - {}", error_preview.dimmed());
            }
            println!();
            failed_positions.push((result.position, &result.offset));
        }
    }

    println!();

    // Summary
    let total_tests = results.len();
    let successful_tests = successful_positions.len();
    let success_rate = if total_tests > 0 {
        (successful_tests as f64 / total_tests as f64) * 100.0
    } else {
        0.0
    };

    println!("{} Summary:", "📈".blue());
    println!("  Total tests: {}", total_tests.to_string().cyan());
    println!("  Successful: {}", successful_tests.to_string().green());
    println!("  Failed: {}", failed_positions.len().to_string().red());
    println!("  Success rate: {:.1}%", success_rate.to_string().yellow());

    if !successful_positions.is_empty() {
        println!();
        println!("{} Recommended positions:", "🎯".green());
        for (pos, offset) in &successful_positions {
            println!(
                "  {}:{} ({})",
                (pos.0 + 1).to_string().green(),
                (pos.1 + 1).to_string().green(),
                offset.description().dimmed()
            );
        }
    }

    if successful_positions.len() > 1 {
        println!();
        println!(
            "{} Multiple positions work - this indicates the LSP server is flexible",
            "💡".blue()
        );
        println!(
            "  Consider using the first successful position: {}:{}",
            (successful_positions[0].0 .0 + 1).to_string().green(),
            (successful_positions[0].0 .1 + 1).to_string().green()
        );
    } else if successful_positions.len() == 1 {
        println!();
        println!(
            "{} Single working position found - this LSP server is position-sensitive",
            "⚡".yellow()
        );
        println!(
            "  Use exactly: {}:{}",
            (successful_positions[0].0 .0 + 1).to_string().green(),
            (successful_positions[0].0 .1 + 1).to_string().green()
        );
    } else {
        println!();
        println!(
            "{} No working positions found - possible issues:",
            "⚠️".red()
        );
        println!("  • Symbol not found or not supported by LSP server");
        println!("  • LSP server not properly initialized");
        println!("  • Symbol type not supported for this operation");
        println!("  • LSP server timeout or error");
    }
}

fn show_discovered_patterns(analyzer: &PositionAnalyzer, args: &Args) {
    let stats = analyzer.get_stats();

    println!();
    println!("{} Pattern Database Statistics:", "🗄️".blue());
    println!(
        "  Total patterns: {}",
        stats.total_patterns.to_string().cyan()
    );
    println!(
        "  Reliable patterns: {}",
        stats.reliable_patterns.to_string().green()
    );
    println!(
        "  Languages covered: {}",
        stats.languages_covered.to_string().yellow()
    );
    println!(
        "  LSP servers covered: {}",
        stats.lsp_servers_covered.to_string().magenta()
    );
    println!(
        "  Overall success rate: {:.1}%",
        stats.success_rate() * 100.0
    );

    // Show patterns for the current language/server combination
    let language = args
        .file_path
        .as_ref()
        .and_then(|path| path.extension())
        .and_then(|ext| ext.to_str())
        .and_then(|ext| match ext {
            "rs" => Some("rust"),
            "go" => Some("go"),
            "py" => Some("python"),
            "js" | "jsx" => Some("javascript"),
            "ts" | "tsx" => Some("typescript"),
            _ => None,
        });

    if let Some(lang) = language {
        let patterns = analyzer.get_patterns(Some(lang), None, args.lsp_server.as_deref());

        if !patterns.is_empty() {
            println!();
            println!("{} Existing patterns for {}:", "🔍".blue(), lang.green());
            for pattern in patterns {
                let reliability = if pattern.is_reliable() { "✓" } else { "?" };
                let server = pattern.lsp_server.as_deref().unwrap_or("any");

                println!(
                    "  {} {:<12} {:<10} {} ({:.0}% confidence, {} tests)",
                    reliability.color(if pattern.is_reliable() {
                        "green"
                    } else {
                        "yellow"
                    }),
                    pattern.symbol_type.cyan(),
                    server.magenta(),
                    pattern.position_offset.description().dimmed(),
                    pattern.confidence * 100.0,
                    pattern.total_tests
                );
            }
        }
    }
}

fn show_analyzer_statistics(analyzer: &PositionAnalyzer) {
    let stats = analyzer.get_stats();

    println!("{} Position Analyzer Statistics", "📊".blue().bold());
    println!();

    println!("Overall Statistics:");
    println!(
        "  Total patterns: {}",
        stats.total_patterns.to_string().cyan()
    );
    println!(
        "  Reliable patterns: {} ({:.1}%)",
        stats.reliable_patterns.to_string().green(),
        if stats.total_patterns > 0 {
            (stats.reliable_patterns as f64 / stats.total_patterns as f64) * 100.0
        } else {
            0.0
        }
    );
    println!(
        "  Languages covered: {}",
        stats.languages_covered.to_string().yellow()
    );
    println!(
        "  LSP servers covered: {}",
        stats.lsp_servers_covered.to_string().magenta()
    );
    println!(
        "  Total tests performed: {}",
        stats.total_tests.to_string().cyan()
    );
    println!(
        "  Successful tests: {}",
        stats.successful_tests.to_string().green()
    );
    println!(
        "  Overall success rate: {:.1}%",
        stats.success_rate() * 100.0
    );

    // Show patterns by language
    for language in ["rust", "go", "python", "javascript", "typescript"] {
        let patterns = analyzer.get_patterns(Some(language), None, None);
        if !patterns.is_empty() {
            println!();
            println!("{}:", language.to_uppercase().blue());

            for pattern in patterns {
                let status = if pattern.is_reliable() { "✓" } else { "?" };
                let server = pattern.lsp_server.as_deref().unwrap_or("any");

                println!(
                    "  {} {:<10} {:<15} {:<20} ({:.0}%, {}/{})",
                    status.color(if pattern.is_reliable() {
                        "green"
                    } else {
                        "yellow"
                    }),
                    pattern.symbol_type.cyan(),
                    server.magenta(),
                    pattern.position_offset.description().dimmed(),
                    pattern.confidence * 100.0,
                    pattern.success_count,
                    pattern.total_tests
                );
            }
        }
    }

    println!();
    println!(
        "{} Use this tool to analyze specific symbols and build reliable position mappings",
        "💡".blue()
    );
}
