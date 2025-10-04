use anyhow::{Context, Result};
use clap::{CommandFactory, Parser as ClapParser};
use colored::*;
use std::path::PathBuf;
use std::time::Instant;
use tracing_subscriber::EnvFilter;

mod cli;

use cli::{Args, Commands};
use probe_code::{
    extract::{handle_extract, ExtractOptions},
    lsp_integration::management::LspManager,
    search::{format_and_print_search_results, perform_probe, SearchOptions},
};

struct SearchParams {
    pattern: String,
    paths: Vec<PathBuf>,
    files_only: bool,
    ignore: Vec<String>,
    exclude_filenames: bool,
    reranker: String,
    frequency_search: bool,
    exact: bool,
    language: Option<String>,
    max_results: Option<usize>,
    max_bytes: Option<usize>,
    max_tokens: Option<usize>,
    allow_tests: bool,
    no_merge: bool,
    merge_threshold: Option<usize>,
    dry_run: bool,
    format: String,
    session: Option<String>,
    timeout: u64,
    question: Option<String>,
    no_gitignore: bool,
    lsp: bool,
}

struct BenchmarkParams {
    bench: Option<String>,
    #[allow(dead_code)]
    sample_size: Option<usize>,
    #[allow(dead_code)]
    format: String,
    output: Option<String>,
    #[allow(dead_code)]
    compare: bool,
    #[allow(dead_code)]
    baseline: Option<String>,
    #[allow(dead_code)]
    fast: bool,
}

/// Apply configuration defaults to CLI arguments where not explicitly specified
fn apply_config_defaults(args: &mut Args, config: &probe_code::config::ResolvedConfig) {
    // Apply defaults to top-level args if not set
    if args.max_results.is_none() {
        args.max_results = config.search.max_results;
    }
    if args.max_bytes.is_none() {
        args.max_bytes = config.search.max_bytes;
    }
    if args.max_tokens.is_none() {
        args.max_tokens = config.search.max_tokens;
    }

    // Apply enable_lsp default if not explicitly set
    if !args.lsp && config.defaults.enable_lsp {
        args.lsp = true;
    }

    // Apply defaults to command-specific args
    if let Some(ref mut command) = args.command {
        match command {
            Commands::Search {
                max_results,
                max_bytes,
                max_tokens,
                allow_tests,
                no_gitignore,
                merge_threshold,
                format,
                timeout,
                lsp,
                reranker,
                frequency_search,
                ..
            } => {
                if max_results.is_none() {
                    *max_results = config.search.max_results;
                }
                if max_bytes.is_none() {
                    *max_bytes = config.search.max_bytes;
                }
                if max_tokens.is_none() {
                    *max_tokens = config.search.max_tokens;
                }
                if !*allow_tests {
                    *allow_tests = config.search.allow_tests;
                }
                if !*no_gitignore {
                    *no_gitignore = config.search.no_gitignore;
                }
                if merge_threshold.is_none() {
                    *merge_threshold = Some(config.search.merge_threshold);
                }
                if format == "color" {
                    // Only override if using default
                    *format = config.defaults.format.clone();
                }
                if *timeout == 30 {
                    // Only override if using default
                    *timeout = config.defaults.timeout;
                }
                if !*lsp && config.defaults.enable_lsp {
                    *lsp = true;
                }
                if reranker == "bm25" {
                    // Only override if using default
                    *reranker = config.search.reranker.clone();
                }
                if *frequency_search && !config.search.frequency {
                    *frequency_search = false;
                }
            }
            Commands::Extract {
                context_lines,
                allow_tests,
                format,
                lsp,
                include_stdlib,
                ..
            } => {
                if *context_lines == 0 {
                    // Only override if using default
                    *context_lines = config.extract.context_lines;
                }
                if !*allow_tests {
                    *allow_tests = config.extract.allow_tests;
                }
                if format == "color" {
                    // Only override if using default
                    *format = config.defaults.format.clone();
                }
                if !*lsp && config.defaults.enable_lsp {
                    *lsp = true;
                }
                if !*include_stdlib {
                    *include_stdlib = config.lsp.include_stdlib;
                }
            }
            Commands::Query {
                max_results,
                allow_tests,
                format,
                ..
            } => {
                if max_results.is_none() {
                    *max_results = config.query.max_results;
                }
                if !*allow_tests {
                    *allow_tests = config.query.allow_tests;
                }
                if format == "color" {
                    // Only override if using default
                    *format = config.defaults.format.clone();
                }
            }
            _ => {}
        }
    }
}

fn handle_search(params: SearchParams) -> Result<()> {
    // Print version at the start for text-based formats
    if params.format != "json" && params.format != "xml" {
        println!("Probe version: {}", probe_code::version::get_version());
    }

    let use_frequency = params.frequency_search;

    println!("{} {}", "Pattern:".bold().green(), params.pattern);
    // Normalize the search root early. Some downstream code paths are stricter about absolute paths.
    let raw_root = params.paths.first().unwrap();

    // Use safe path operations to avoid following symlinks/junctions
    use probe_code::path_safety;

    // On Windows CI, avoid canonicalize() which can trigger stack overflow with junction points
    #[cfg(target_os = "windows")]
    let canonical_root = if path_safety::is_ci_environment() {
        // In CI, just use the path as-is or make it absolute without canonicalize
        if raw_root.is_absolute() {
            raw_root.clone()
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(raw_root)
        }
    } else if path_safety::exists_no_follow(raw_root) {
        match raw_root.canonicalize() {
            Ok(p) => p,
            Err(_) => raw_root.clone(),
        }
    } else {
        raw_root.clone()
    };

    #[cfg(not(target_os = "windows"))]
    let canonical_root = if path_safety::exists_no_follow(raw_root) {
        match raw_root.canonicalize() {
            Ok(p) => p,
            Err(_) => raw_root.clone(),
        }
    } else {
        raw_root.clone()
    };
    println!("{} {}", "Path:".bold().green(), canonical_root.display());

    // Show advanced options if they differ from defaults
    let mut advanced_options = Vec::<String>::new();
    if params.files_only {
        advanced_options.push("Files only".to_string());
    }
    if params.exclude_filenames {
        advanced_options.push("Exclude filenames".to_string());
    }
    if params.reranker != "hybrid" {
        advanced_options.push(format!("Reranker: {}", params.reranker));
    }
    if !use_frequency {
        advanced_options.push("Frequency search disabled".to_string());
    }
    if let Some(lang) = &params.language {
        advanced_options.push(format!("Language: {lang}"));
    }
    if params.allow_tests {
        advanced_options.push("Including tests".to_string());
    }
    if params.no_gitignore {
        advanced_options.push("Ignoring .gitignore".to_string());
    }
    if params.no_merge {
        advanced_options.push("No block merging".to_string());
    }
    if let Some(threshold) = params.merge_threshold {
        advanced_options.push(format!("Merge threshold: {threshold}"));
    }
    if params.dry_run {
        advanced_options.push("Dry run (file names and lines only)".to_string());
    }
    if let Some(session) = &params.session {
        advanced_options.push(format!("Session: {session}"));
    }

    // Show timeout if it's not the default value of 30 seconds
    if params.timeout != 30 {
        advanced_options.push(format!("Timeout: {} seconds", params.timeout));
    }

    if !advanced_options.is_empty() {
        println!(
            "{} {}",
            "Options:".bold().green(),
            advanced_options.join(", ")
        );
    }

    let start_time = Instant::now();

    // Create a vector with the pattern
    let query = vec![params.pattern.clone()];

    let search_options = SearchOptions {
        // Pass a normalized path so directory roots are always accepted.
        path: &canonical_root,
        queries: &query,
        files_only: params.files_only,
        custom_ignores: &params.ignore,
        exclude_filenames: params.exclude_filenames,
        reranker: &params.reranker,
        frequency_search: use_frequency,
        exact: params.exact,
        language: params.language.as_deref(),
        max_results: params.max_results,
        max_bytes: params.max_bytes,
        max_tokens: params.max_tokens,
        allow_tests: params.allow_tests,
        no_merge: params.no_merge,
        merge_threshold: params.merge_threshold,
        dry_run: params.dry_run,
        session: params.session.as_deref(),
        timeout: params.timeout,
        question: params.question.as_deref(),
        no_gitignore: params.no_gitignore,
        lsp: params.lsp,
    };

    let limited_results = perform_probe(&search_options)?;

    // Calculate search time
    let duration = start_time.elapsed();

    // Create the query plan regardless of whether we have results
    let query_plan = if search_options.queries.len() > 1 {
        // Join multiple queries with AND
        let combined_query = search_options.queries.join(" AND ");
        probe_code::search::query::create_query_plan(&combined_query, false).ok()
    } else {
        probe_code::search::query::create_query_plan(&search_options.queries[0], false).ok()
    };

    if limited_results.results.is_empty() {
        // For JSON and XML formats, still call format_and_print_search_results
        if params.format == "json" || params.format == "xml" {
            format_and_print_search_results(
                &limited_results.results,
                search_options.dry_run,
                &params.format,
                query_plan.as_ref(),
            );
        } else {
            // For other formats, print the "No results found" message
            println!("{}", "No results found.".yellow().bold());
            println!("Search completed in {duration:.2?}");
        }
    } else {
        // For non-JSON/XML formats, print search time
        if params.format != "json" && params.format != "xml" {
            println!("Search completed in {duration:.2?}");
            println!();
        }

        format_and_print_search_results(
            &limited_results.results,
            search_options.dry_run,
            &params.format,
            query_plan.as_ref(),
        );

        if !limited_results.skipped_files.is_empty() {
            if let Some(limits) = &limited_results.limits_applied {
                println!();
                println!("{}", "Limits applied:".yellow().bold());
                if let Some(max_results) = limits.max_results {
                    println!("  {} {max_results}", "Max results:".yellow());
                }
                if let Some(max_bytes) = limits.max_bytes {
                    println!("  {} {max_bytes}", "Max bytes:".yellow());
                }
                if let Some(max_tokens) = limits.max_tokens {
                    println!("  {} {max_tokens}", "Max tokens:".yellow());
                }

                println!();

                // Calculate total skipped files (results skipped + files not processed)
                let results_skipped = limited_results.skipped_files.len();
                let files_not_processed =
                    limited_results.files_skipped_early_termination.unwrap_or(0);
                let total_skipped = results_skipped + files_not_processed;

                println!(
                    "{} {}",
                    "Skipped files due to limits:".yellow().bold(),
                    total_skipped
                );

                // Show guidance message to get more results
                if total_skipped > 0 {
                    if let Some(session_id) = search_options.session {
                        if !session_id.is_empty() && session_id != "new" {
                            println!("💡 To get more results from this search query, repeat it with the same params and session ID: {session_id}");
                        } else {
                            println!("💡 To get more results from this search query, repeat it with the same params and session ID (see above)");
                        }
                    } else {
                        println!("💡 To get more results from this search query, repeat it with the same params and use --session with the session ID shown above");
                    }
                }

                // Show breakdown in debug mode
                if std::env::var("PROBE_DEBUG").is_ok() && total_skipped > 0 {
                    if results_skipped > 0 {
                        println!(
                            "  {} {}",
                            "Results skipped after processing:".yellow(),
                            results_skipped
                        );
                    }
                    if files_not_processed > 0 {
                        println!(
                            "  {} {}",
                            "Files not processed (early termination):".yellow(),
                            files_not_processed
                        );
                    }
                }
            }
        }

        // Display information about cached blocks
        if let Some(cached_skipped) = limited_results.cached_blocks_skipped {
            if cached_skipped > 0 {
                println!();
                println!(
                    "{} {}",
                    "Skipped blocks due to session cache:".yellow().bold(),
                    cached_skipped
                );
            }
        }
    }

    // Add helpful tip at the very bottom of output
    println!();
    println!("💡 Tip: Use --exact flag when searching for specific function names or variables for more precise results");

    Ok(())
}

fn handle_benchmark(params: BenchmarkParams) -> Result<()> {
    use std::process::Command;

    println!("{}", "Running performance benchmarks...".bold().green());

    let bench_type = params.bench.as_deref().unwrap_or("all");

    // Build the cargo bench command
    let mut cmd = Command::new("cargo");
    cmd.arg("bench");

    // Add specific benchmark if requested
    match bench_type {
        "search" => {
            cmd.arg("--bench").arg("search_benchmarks");
        }
        "timing" => {
            cmd.arg("--bench").arg("timing_benchmarks");
        }
        "parsing" => {
            cmd.arg("--bench").arg("parsing_benchmarks");
        }
        "all" => {
            // Run all benchmarks (default)
        }
        _ => {
            eprintln!("Unknown benchmark type: {bench_type}");
            return Ok(());
        }
    }

    // Add criterion options after --
    let criterion_args: Vec<String> = Vec::new();

    // Note: Criterion benchmarks don't support --sample-size from command line
    // Sample size is configured in the benchmark code itself

    // For now, keep it simple and just run the benchmarks
    // Advanced features like baseline comparison can be added later

    if !criterion_args.is_empty() {
        cmd.arg("--");
        cmd.args(criterion_args);
    }

    // Execute the benchmark
    let output = cmd.output()?;

    if !output.status.success() {
        eprintln!("Benchmark failed:");
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        return Ok(());
    }

    // Print benchmark output
    println!("{}", String::from_utf8_lossy(&output.stdout));

    // Save output to file if requested
    if let Some(output_file) = &params.output {
        use std::fs;
        fs::write(output_file, &output.stdout)?;
        println!("Benchmark results saved to: {output_file}");
    }

    println!();
    println!("{}", "Benchmark completed successfully!".bold().green());
    println!(
        "Results are also available in: {}",
        "target/criterion/".yellow()
    );

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments first so we can decide whether to initialize a global
    // tracing subscriber here or let the embedded daemon install its own layers.
    // This avoids clobbering the daemon's in‑memory log layer.
    let mut args = Args::parse();

    // Decide if this process is going to run the embedded daemon in foreground.
    // When true, skip installing a global subscriber here so the daemon can
    // attach its memory/persistent layers and expose logs via `probe lsp logs`.
    use probe_code::lsp_integration::LspSubcommands;
    let is_daemon_foreground = matches!(&args.command,
        Some(Commands::Lsp { subcommand: LspSubcommands::Start { foreground, .. } }) if *foreground
    );

    if !is_daemon_foreground {
        // Initialize logging from RUST_LOG (or fallback to info). Use try_init to avoid double init.
        let _ = tracing_subscriber::fmt()
            .with_env_filter(
                EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
            )
            .with_target(true)
            .with_writer(std::io::stderr)
            .try_init();
    }
    // Set CI/Windows safety guards BEFORE any config/filesystem operations
    // This prevents stack overflow from junction point cycles on Windows CI
    #[cfg(target_os = "windows")]
    {
        if std::env::var("CI").is_ok() {
            // Disable project config discovery to avoid junction traversal
            std::env::set_var("PROBE_SKIP_PROJECT_CONFIG", "1");
            // Disable parent .gitignore discovery to avoid climbing into junction cycles
            std::env::set_var("PROBE_NO_GITIGNORE", "1");
        }
    }

    // Load global configuration
    let config = probe_code::config::get_config();

    // Args already parsed above; apply config defaults now (CLI flags win)

    // Apply config defaults to CLI args where not specified
    apply_config_defaults(&mut args, config);

    // Set/clear global autostart guard to prevent unwanted LSP daemon spawning
    // Only disable autostart for specific LSP subcommands that could cause recursion
    let _should_disable_autostart = match &args.command {
        Some(Commands::Lsp { subcommand }) => {
            use probe_code::lsp_integration::LspSubcommands;
            // Only these specific commands should disable autostart to prevent recursion
            // Note: Shutdown should NOT disable autostart as it needs to connect first
            // Note: Restart should NOT disable autostart as it needs to start a new daemon
            matches!(subcommand, LspSubcommands::Start { .. })
        }
        _ => false, // Never disable autostart for regular commands - let daemon handle lazy init
    } || config.lsp.disable_autostart; // Also check config setting

    // No more eager LSP initialization - let the daemon handle cache-first approach
    // The daemon will:
    // 1. Check cache first (fast, <100ms)
    // 2. Only initialize LSP servers on cache miss
    // 3. Auto-start daemon if needed during actual LSP operations

    match args.command {
        // When no subcommand provided and no pattern, show help
        None if args.pattern.is_none() || args.pattern.as_ref().unwrap().is_empty() => {
            Args::command().print_help()?;
            return Ok(());
        }
        // When no subcommand but pattern is provided, fallback to search mode
        None => {
            // Use provided pattern
            let pattern = args.pattern.unwrap();

            // Use provided paths or default to current directory
            let paths = if args.paths.is_empty() {
                vec![std::path::PathBuf::from(".")]
            } else {
                args.paths
            };

            handle_search(SearchParams {
                pattern,
                paths,
                files_only: args.files_only,
                ignore: args.ignore,
                exclude_filenames: args.exclude_filenames,
                reranker: args.reranker,
                frequency_search: args.frequency_search,
                exact: args.exact,
                language: None, // Default to None for the no-subcommand case
                max_results: args.max_results,
                max_bytes: args.max_bytes,
                max_tokens: args.max_tokens,
                allow_tests: args.allow_tests,
                no_merge: args.no_merge,
                merge_threshold: args.merge_threshold,
                dry_run: args.dry_run,
                format: args.format,
                session: args.session,
                timeout: args.timeout,
                question: args.question,
                no_gitignore: args.no_gitignore
                    || std::env::var("PROBE_NO_GITIGNORE")
                        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                        .unwrap_or(false),
                lsp: args.lsp,
            })?
        }
        Some(Commands::Search {
            pattern,
            paths,
            files_only,
            ignore,
            exclude_filenames,
            reranker,
            frequency_search,
            exact,
            language,
            max_results,
            max_bytes,
            max_tokens,
            allow_tests,
            no_merge,
            merge_threshold,
            dry_run,
            format,
            session,
            timeout,
            question,
            no_gitignore,
            lsp,
        }) => handle_search(SearchParams {
            pattern,
            paths,
            files_only,
            ignore,
            exclude_filenames,
            reranker,
            frequency_search,
            exact,
            language,
            max_results,
            max_bytes,
            max_tokens,
            allow_tests,
            no_merge,
            merge_threshold,
            dry_run,
            format,
            session,
            timeout,
            question,
            no_gitignore: no_gitignore
                || std::env::var("PROBE_NO_GITIGNORE")
                    .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                    .unwrap_or(false),
            lsp,
        })?,
        Some(Commands::Extract {
            files,
            ignore,
            context_lines,
            format,
            from_clipboard,
            input_file,
            to_clipboard,
            dry_run,
            diff,
            allow_tests,
            keep_input,
            prompt,
            instructions,
            no_gitignore,
            lsp,
            include_stdlib: _,
        }) => handle_extract(ExtractOptions {
            files,
            custom_ignores: ignore,
            context_lines,
            format,
            from_clipboard,
            input_file,
            to_clipboard,
            dry_run,
            diff,
            allow_tests,
            keep_input,
            prompt: prompt.map(|p| {
                probe_code::extract::PromptTemplate::from_str(&p).unwrap_or_else(|e| {
                    eprintln!("Warning: {e}");
                    probe_code::extract::PromptTemplate::Engineer
                })
            }),
            instructions,
            no_gitignore: no_gitignore
                || std::env::var("PROBE_NO_GITIGNORE")
                    .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                    .unwrap_or(false),
            lsp,
        })?,
        Some(Commands::Query {
            pattern,
            path,
            language,
            ignore,
            allow_tests,
            max_results,
            format,
            no_gitignore,
        }) => probe_code::query::handle_query(
            &pattern,
            &path,
            language.as_deref().map(|lang| {
                // Normalize language aliases
                match lang.to_lowercase().as_str() {
                    "rs" => "rust",
                    "js" | "jsx" => "javascript",
                    "ts" | "tsx" => "typescript",
                    "py" => "python",
                    "h" => "c",
                    "cc" | "cxx" | "hpp" | "hxx" => "cpp",
                    "rb" => "ruby",
                    "cs" => "csharp",
                    _ => lang, // Return the original language if no alias is found
                }
            }),
            &ignore,
            allow_tests,
            max_results,
            &format,
            no_gitignore || std::env::var("PROBE_NO_GITIGNORE").unwrap_or_default() == "1",
        )?,
        Some(Commands::Benchmark {
            bench,
            sample_size,
            format,
            output,
            compare,
            baseline,
            fast,
        }) => handle_benchmark(BenchmarkParams {
            bench,
            sample_size,
            format,
            output,
            compare,
            baseline,
            fast,
        })?,
        Some(Commands::Lsp { subcommand }) => {
            LspManager::handle_command(&subcommand, "color").await?;
        }
        Some(Commands::Config { subcommand }) => {
            handle_config_command(&subcommand)?;
        }
    }

    Ok(())
}

fn handle_config_command(subcommand: &cli::ConfigSubcommands) -> Result<()> {
    use cli::ConfigSubcommands;

    match subcommand {
        ConfigSubcommands::Show { format } => {
            let config = probe_code::config::get_config();

            match format.as_str() {
                "json" => {
                    println!("{}", config.to_json_string()?);
                }
                "env" => {
                    // Show as environment variables
                    println!("# Probe configuration as environment variables");
                    println!("# Set these in your shell to override config file settings");
                    println!();
                    println!("# General settings");
                    println!(
                        "export PROBE_DEBUG={}",
                        if config.defaults.debug { "1" } else { "0" }
                    );
                    println!("export PROBE_LOG_LEVEL={}", config.defaults.log_level);
                    println!(
                        "export PROBE_ENABLE_LSP={}",
                        if config.defaults.enable_lsp {
                            "true"
                        } else {
                            "false"
                        }
                    );
                    println!("export PROBE_FORMAT={}", config.defaults.format);
                    println!("export PROBE_TIMEOUT={}", config.defaults.timeout);
                    println!();
                    println!("# Search settings");
                    if let Some(max) = config.search.max_results {
                        println!("export PROBE_MAX_RESULTS={max}");
                    }
                    if let Some(max) = config.search.max_tokens {
                        println!("export PROBE_MAX_TOKENS={max}");
                    }
                    if let Some(max) = config.search.max_bytes {
                        println!("export PROBE_MAX_BYTES={max}");
                    }
                    println!(
                        "export PROBE_SEARCH_FREQUENCY={}",
                        if config.search.frequency {
                            "true"
                        } else {
                            "false"
                        }
                    );
                    println!("export PROBE_SEARCH_RERANKER={}", config.search.reranker);
                    println!(
                        "export PROBE_SEARCH_MERGE_THRESHOLD={}",
                        config.search.merge_threshold
                    );
                    println!(
                        "export PROBE_ALLOW_TESTS={}",
                        if config.search.allow_tests {
                            "true"
                        } else {
                            "false"
                        }
                    );
                    println!(
                        "export PROBE_NO_GITIGNORE={}",
                        if config.search.no_gitignore {
                            "true"
                        } else {
                            "false"
                        }
                    );
                    println!();
                    println!("# Extract settings");
                    println!(
                        "export PROBE_EXTRACT_CONTEXT_LINES={}",
                        config.extract.context_lines
                    );
                    println!();
                    println!("# LSP settings");
                    println!(
                        "export PROBE_LSP_INCLUDE_STDLIB={}",
                        if config.lsp.include_stdlib {
                            "true"
                        } else {
                            "false"
                        }
                    );
                    if let Some(ref path) = config.lsp.socket_path {
                        println!("export PROBE_LSP_SOCKET_PATH={path}");
                    }
                    println!(
                        "export PROBE_LSP_WORKSPACE_CACHE_MAX={}",
                        config.lsp.workspace_cache.max_open_caches
                    );
                    println!(
                        "export PROBE_LSP_WORKSPACE_CACHE_SIZE_MB={}",
                        config.lsp.workspace_cache.size_mb_per_workspace
                    );
                    println!(
                        "export PROBE_LSP_WORKSPACE_LOOKUP_DEPTH={}",
                        config.lsp.workspace_cache.lookup_depth
                    );
                    if let Some(ref dir) = config.lsp.workspace_cache.base_dir {
                        println!("export PROBE_LSP_WORKSPACE_CACHE_DIR={dir}");
                    }
                    println!();
                    println!("# Performance settings");
                    println!(
                        "export PROBE_TREE_CACHE_SIZE={}",
                        config.performance.tree_cache_size
                    );
                    println!(
                        "export PROBE_OPTIMIZE_BLOCKS={}",
                        if config.performance.optimize_blocks {
                            "1"
                        } else {
                            "0"
                        }
                    );
                    println!();
                    println!("# Indexing settings");
                    println!(
                        "export PROBE_INDEXING_ENABLED={}",
                        if config.indexing.enabled {
                            "true"
                        } else {
                            "false"
                        }
                    );
                    println!(
                        "export PROBE_INDEXING_AUTO_INDEX={}",
                        if config.indexing.auto_index {
                            "true"
                        } else {
                            "false"
                        }
                    );
                    println!(
                        "export PROBE_INDEXING_WATCH_FILES={}",
                        if config.indexing.watch_files {
                            "true"
                        } else {
                            "false"
                        }
                    );
                    println!(
                        "export PROBE_INDEXING_DEFAULT_DEPTH={}",
                        config.indexing.default_depth
                    );
                    println!(
                        "export PROBE_INDEXING_MAX_WORKERS={}",
                        config.indexing.max_workers
                    );
                    println!(
                        "export PROBE_INDEXING_MEMORY_BUDGET_MB={}",
                        config.indexing.memory_budget_mb
                    );
                }
                _ => {
                    eprintln!("Unknown format: {format}");
                }
            }
        }
        ConfigSubcommands::Validate { file } => {
            let config_path = if let Some(path) = file {
                std::path::PathBuf::from(path)
            } else {
                // Use default config path with robust cross-platform support
                if let Some(home_dir) = dirs::home_dir() {
                    home_dir.join(".probe").join("settings.json")
                } else {
                    // Fallback to environment variables if dirs crate fails
                    #[cfg(target_os = "windows")]
                    {
                        let userprofile = std::env::var("USERPROFILE")
                            .unwrap_or_else(|_| "C:\\Users\\Default".to_string());
                        std::path::PathBuf::from(userprofile)
                            .join(".probe")
                            .join("settings.json")
                    }
                    #[cfg(not(target_os = "windows"))]
                    {
                        let home = std::env::var("HOME").unwrap_or_else(|_| "~".to_string());
                        std::path::PathBuf::from(home)
                            .join(".probe")
                            .join("settings.json")
                    }
                }
            };

            if !config_path.exists() {
                eprintln!("Config file not found: {}", config_path.display());
                return Ok(());
            }

            let contents = std::fs::read_to_string(&config_path)
                .context(format!("Failed to read config file: {config_path:?}"))?;

            // Parse as ProbeConfig (with Options) for validation
            match serde_json::from_str::<probe_code::config::ProbeConfig>(&contents) {
                Ok(config) => {
                    // Also validate the values
                    config.validate()?;
                    println!("✓ Configuration is valid");
                }
                Err(e) => {
                    eprintln!("✗ Configuration is invalid: {e}");
                    std::process::exit(1);
                }
            }
        }
        ConfigSubcommands::Set {
            key,
            value,
            scope,
            force,
        } => {
            probe_code::config::config_ops::set_config_value(key, value, scope, *force)?;
        }
        ConfigSubcommands::Get { key, show_source } => {
            probe_code::config::config_ops::get_config_value(key, *show_source)?;
        }
        ConfigSubcommands::Reset { scope, force } => {
            probe_code::config::config_ops::reset_config(scope, *force)?;
        }
    }

    Ok(())
}
// Test comment
