//! LSP Enrichment Worker Module
//!
//! This module provides a single worker that processes symbols from the enrichment queue
//! and enriches them with LSP data using SingleServerManager directly.
//! SingleServerManager handles all concurrency control and health tracking internally.

use anyhow::{Context, Result};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{sleep, timeout};
use tracing::{debug, error, info, warn};

use crate::database::enrichment_tracking::EnrichmentTracker;
use crate::database::DatabaseBackend;
use crate::database_cache_adapter::{BackendType, DatabaseCacheAdapter};
use crate::indexing::lsp_enrichment_queue::{EnrichmentOperation, LspEnrichmentQueue, QueueItem};
use crate::language_detector::Language;
use crate::lsp_database_adapter::LspDatabaseAdapter;
use crate::path_resolver::PathResolver;
use crate::server_manager::SingleServerManager;
use crate::symbol::uid_generator::SymbolUIDGenerator;
use crate::symbol::{SymbolContext, SymbolInfo, SymbolKind, SymbolLocation};
use crate::workspace_utils;

/// Configuration for LSP enrichment worker (single worker design)
#[derive(Debug, Clone)]
pub struct EnrichmentWorkerConfig {
    /// Batch size for processing symbols (not used yet but reserved for future batching)
    pub batch_size: usize,
    /// Timeout for individual LSP requests
    pub request_timeout: Duration,
    /// Delay between processing cycles when queue is empty
    pub empty_queue_delay: Duration,
    /// Maximum retries for failed LSP requests
    pub max_retries: u32,
}

impl Default for EnrichmentWorkerConfig {
    fn default() -> Self {
        Self {
            batch_size: std::env::var("PROBE_LSP_ENRICHMENT_BATCH_SIZE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(100),
            request_timeout: Duration::from_secs(25), // Same as existing LSP timeout
            empty_queue_delay: Duration::from_secs(5),
            max_retries: 2,
        }
    }
}

/// Statistics for enrichment worker (single worker design)
#[derive(Debug, Default)]
pub struct EnrichmentWorkerStats {
    /// Total symbols processed
    pub symbols_processed: AtomicU64,
    /// Total symbols successfully enriched
    pub symbols_enriched: AtomicU64,
    /// Total symbols that failed enrichment
    pub symbols_failed: AtomicU64,
    /// Worker status (0 = inactive, 1 = active)
    pub worker_active: AtomicBool,
    /// Positions adjusted (snapped to identifier)
    pub positions_adjusted: AtomicU64,
    /// Successful call hierarchy operations
    pub call_hierarchy_success: AtomicU64,
    /// Total references found across symbols
    pub references_found: AtomicU64,
    /// Total implementations found across symbols
    pub implementations_found: AtomicU64,
    /// Count of reference operations attempted
    pub references_attempted: AtomicU64,
    /// Count of implementation operations attempted
    pub implementations_attempted: AtomicU64,
    /// Count of call hierarchy operations attempted
    pub call_hierarchy_attempted: AtomicU64,
    /// Total edges persisted from call hierarchy
    pub edges_persisted: AtomicU64,
    /// Total edges persisted from references
    pub reference_edges_persisted: AtomicU64,
    /// Total edges persisted from implementations
    pub implementation_edges_persisted: AtomicU64,
    /// Symbols skipped due to unhealthy server
    pub symbols_skipped_unhealthy: AtomicU64,
    /// Symbols skipped due to failure tracking (in cooldown)
    pub symbols_skipped_failed: AtomicU64,
}

impl EnrichmentWorkerStats {
    /// Get snapshot of current stats
    pub fn snapshot(&self) -> EnrichmentWorkerStatsSnapshot {
        EnrichmentWorkerStatsSnapshot {
            symbols_processed: self.symbols_processed.load(Ordering::Relaxed),
            symbols_enriched: self.symbols_enriched.load(Ordering::Relaxed),
            symbols_failed: self.symbols_failed.load(Ordering::Relaxed),
            worker_active: self.worker_active.load(Ordering::Relaxed),
            positions_adjusted: self.positions_adjusted.load(Ordering::Relaxed),
            call_hierarchy_success: self.call_hierarchy_success.load(Ordering::Relaxed),
            references_found: self.references_found.load(Ordering::Relaxed),
            implementations_found: self.implementations_found.load(Ordering::Relaxed),
            references_attempted: self.references_attempted.load(Ordering::Relaxed),
            implementations_attempted: self.implementations_attempted.load(Ordering::Relaxed),
            call_hierarchy_attempted: self.call_hierarchy_attempted.load(Ordering::Relaxed),
            edges_persisted: self.edges_persisted.load(Ordering::Relaxed),
            reference_edges_persisted: self.reference_edges_persisted.load(Ordering::Relaxed),
            implementation_edges_persisted: self
                .implementation_edges_persisted
                .load(Ordering::Relaxed),
            symbols_skipped_unhealthy: self.symbols_skipped_unhealthy.load(Ordering::Relaxed),
            symbols_skipped_failed: self.symbols_skipped_failed.load(Ordering::Relaxed),
        }
    }

    /// Calculate success rate percentage
    pub fn success_rate(&self) -> f64 {
        let processed = self.symbols_processed.load(Ordering::Relaxed);
        if processed == 0 {
            0.0
        } else {
            let enriched = self.symbols_enriched.load(Ordering::Relaxed);
            (enriched as f64 / processed as f64) * 100.0
        }
    }
}

/// Immutable snapshot of worker stats (single worker design)
#[derive(Debug, Clone)]
pub struct EnrichmentWorkerStatsSnapshot {
    pub symbols_processed: u64,
    pub symbols_enriched: u64,
    pub symbols_failed: u64,
    pub worker_active: bool,
    pub positions_adjusted: u64,
    pub call_hierarchy_success: u64,
    pub references_found: u64,
    pub implementations_found: u64,
    pub references_attempted: u64,
    pub implementations_attempted: u64,
    pub call_hierarchy_attempted: u64,
    pub edges_persisted: u64,
    pub reference_edges_persisted: u64,
    pub implementation_edges_persisted: u64,
    pub symbols_skipped_unhealthy: u64,
    pub symbols_skipped_failed: u64,
}

/// LSP Enrichment Worker Pool (Single Worker Design)
///
/// Manages a single worker that processes symbols from the enrichment queue
/// and enriches them with LSP data using SingleServerManager directly.
/// SingleServerManager handles all concurrency control and health tracking internally.
pub struct LspEnrichmentWorkerPool {
    /// Worker configuration
    config: EnrichmentWorkerConfig,
    /// Server manager for direct LSP access (handles concurrency internally)
    server_manager: Arc<SingleServerManager>,
    /// Database adapter for LSP data conversion
    database_adapter: LspDatabaseAdapter,
    /// Path resolver for relative path handling
    path_resolver: Arc<PathResolver>,
    /// Worker statistics
    stats: Arc<EnrichmentWorkerStats>,
    /// Shutdown signal
    shutdown: Arc<AtomicBool>,
    /// Enrichment failure tracker
    enrichment_tracker: Arc<EnrichmentTracker>,
    /// Symbol UID generator for tracking
    uid_generator: Arc<SymbolUIDGenerator>,
}

impl LspEnrichmentWorkerPool {
    /// Create a new worker pool (single worker design) using direct SingleServerManager access
    pub fn new(
        config: EnrichmentWorkerConfig,
        server_manager: Arc<SingleServerManager>,
        database_adapter: LspDatabaseAdapter,
        path_resolver: Arc<PathResolver>,
    ) -> Self {
        Self {
            config,
            server_manager,
            database_adapter,
            path_resolver,
            stats: Arc::new(EnrichmentWorkerStats::default()),
            shutdown: Arc::new(AtomicBool::new(false)),
            enrichment_tracker: Arc::new(EnrichmentTracker::new()),
            uid_generator: Arc::new(SymbolUIDGenerator::new()),
        }
    }

    /// Start the single worker processing symbols from the queue
    pub async fn start_processing(
        &self,
        queue: Arc<LspEnrichmentQueue>,
        cache_adapter: Arc<DatabaseCacheAdapter>,
    ) -> Result<Vec<tokio::task::JoinHandle<()>>> {
        info!("Starting LSP enrichment single worker (concurrency handled by SingleServerManager)");

        let mut handles = Vec::new();

        // Start the single worker
        let handle = self
            .spawn_worker(queue.clone(), cache_adapter.clone())
            .await?;
        handles.push(handle);

        Ok(handles)
    }

    /// Spawn the single worker using direct SingleServerManager access
    async fn spawn_worker(
        &self,
        queue: Arc<LspEnrichmentQueue>,
        cache_adapter: Arc<DatabaseCacheAdapter>,
    ) -> Result<tokio::task::JoinHandle<()>> {
        let stats = self.stats.clone();
        let shutdown = self.shutdown.clone();
        let config = self.config.clone();
        let server_manager = self.server_manager.clone();
        let path_resolver = self.path_resolver.clone();

        let enrichment_tracker = self.enrichment_tracker.clone();
        let uid_generator = self.uid_generator.clone();

        let handle = tokio::spawn(async move {
            info!("LSP enrichment worker started (SingleServerManager handles concurrency)");
            stats.worker_active.store(true, Ordering::Relaxed);

            while !shutdown.load(Ordering::Relaxed) {
                // Try to get next symbol from queue
                match queue.pop_next().await {
                    Some(queue_item) => {
                        debug!(
                            "Processing symbol: {} ({}:{}) using SingleServerManager",
                            queue_item.name,
                            queue_item.file_path.display(),
                            queue_item.def_start_line
                        );

                        // Language detection and server health checking is handled
                        // internally by SingleServerManager during LSP operations

                        // Check if symbol has failed recently and is in cooldown
                        let symbol_uid =
                            Self::generate_symbol_uid(&queue_item, &uid_generator).await;

                        let should_skip = if let Ok(uid) = &symbol_uid {
                            enrichment_tracker.has_failed(uid).await
                                && !enrichment_tracker
                                    .get_symbols_ready_for_retry()
                                    .await
                                    .contains(uid)
                        } else {
                            false
                        };

                        if should_skip {
                            stats.symbols_skipped_failed.fetch_add(1, Ordering::Relaxed);
                            debug!(
                                "Skipping symbol '{}' due to failure tracking (in cooldown)",
                                queue_item.name
                            );
                        } else {
                            // Process the symbol using SingleServerManager directly
                            // SingleServerManager handles all concurrency control and health tracking
                            match Self::process_symbol_with_retries(
                                &queue_item,
                                &server_manager,
                                &path_resolver,
                                &cache_adapter,
                                &config,
                                &stats,
                                &enrichment_tracker,
                                &uid_generator,
                            )
                            .await
                            {
                                Ok(_) => {
                                    stats.symbols_enriched.fetch_add(1, Ordering::Relaxed);
                                    debug!("Successfully enriched symbol: {}", queue_item.name);

                                    // Clear failure tracking on success
                                    if let Ok(uid) = symbol_uid {
                                        enrichment_tracker.clear_failure(&uid).await;
                                    }
                                }
                                Err(e) => {
                                    // Check if this was a health-related failure
                                    let err_str = e.to_string();
                                    if err_str.contains("unhealthy")
                                        || err_str.contains("consecutive failures")
                                    {
                                        stats
                                            .symbols_skipped_unhealthy
                                            .fetch_add(1, Ordering::Relaxed);
                                        debug!(
                                            "Skipped symbol '{}' due to unhealthy server: {}",
                                            queue_item.name, e
                                        );
                                    } else {
                                        warn!(
                                            "Failed to enrich symbol '{}' ({}:{}, kind: {}, lang: {:?}): {}",
                                            queue_item.name,
                                            queue_item.file_path.display(),
                                            queue_item.def_start_line,
                                            queue_item.kind,
                                            queue_item.language,
                                            e
                                        );
                                    }
                                    stats.symbols_failed.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                        }

                        stats.symbols_processed.fetch_add(1, Ordering::Relaxed);
                    }
                    None => {
                        // Queue is empty, wait before checking again
                        debug!(
                            "Queue is empty, sleeping for {:?}",
                            config.empty_queue_delay
                        );
                        sleep(config.empty_queue_delay).await;
                    }
                }
            }

            stats.worker_active.store(false, Ordering::Relaxed);
            info!("LSP enrichment worker stopped");
        });

        Ok(handle)
    }

    /// Process a single symbol with retry logic using SingleServerManager directly
    async fn process_symbol_with_retries(
        queue_item: &QueueItem,
        server_manager: &Arc<SingleServerManager>,
        path_resolver: &Arc<PathResolver>,
        cache_adapter: &Arc<DatabaseCacheAdapter>,
        config: &EnrichmentWorkerConfig,
        stats: &Arc<EnrichmentWorkerStats>,
        enrichment_tracker: &Arc<EnrichmentTracker>,
        uid_generator: &Arc<SymbolUIDGenerator>,
    ) -> Result<()> {
        let mut last_error = None;

        for attempt in 0..=config.max_retries {
            if attempt > 0 {
                debug!(
                    "Retrying LSP enrichment for symbol {} (attempt {}/{})",
                    queue_item.name,
                    attempt + 1,
                    config.max_retries + 1
                );
                sleep(Duration::from_millis(500 * attempt as u64)).await;
            }

            match Self::process_symbol_once(
                queue_item,
                server_manager,
                path_resolver,
                cache_adapter,
                config,
                stats,
            )
            .await
            {
                Ok(_) => return Ok(()),
                Err(e) => {
                    last_error = Some(e);
                    debug!(
                        "Attempt {} failed for symbol '{}' ({}:{}, kind: {}, lang: {:?}): {}",
                        attempt + 1,
                        queue_item.name,
                        queue_item.file_path.display(),
                        queue_item.def_start_line,
                        queue_item.kind,
                        queue_item.language,
                        last_error.as_ref().unwrap()
                    );
                }
            }
        }

        // Record failure in tracker after all retries exhausted
        if let Ok(symbol_uid) = Self::generate_symbol_uid(queue_item, uid_generator).await {
            let failure_reason = last_error
                .as_ref()
                .map(|e| e.to_string())
                .unwrap_or_else(|| "Unknown error".to_string());

            enrichment_tracker
                .record_failure(
                    symbol_uid,
                    failure_reason,
                    queue_item.file_path.to_string_lossy().to_string(),
                    queue_item.def_start_line,
                    queue_item.language.as_str().to_string(),
                    queue_item.name.clone(),
                    queue_item.kind.clone(),
                )
                .await;
        }

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Unknown error during symbol processing")))
    }

    /// Process a single symbol using SingleServerManager directly
    /// SingleServerManager handles all concurrency control and health checking internally

    async fn process_symbol_once(
        queue_item: &QueueItem,
        server_manager: &Arc<SingleServerManager>,
        _path_resolver: &Arc<PathResolver>,
        cache_adapter: &Arc<DatabaseCacheAdapter>,
        config: &EnrichmentWorkerConfig,
        stats: &Arc<EnrichmentWorkerStats>,
    ) -> Result<()> {
        let workspace_root =
            workspace_utils::find_workspace_root_with_fallback(&queue_item.file_path)
                .context("Failed to resolve workspace root")?;

        debug!(
            "Processing symbol {} in workspace: {}",
            queue_item.name,
            workspace_root.display()
        );

        let need_references = queue_item
            .operations
            .contains(&EnrichmentOperation::References);
        let need_implementations = queue_item
            .operations
            .contains(&EnrichmentOperation::Implementations);
        let need_call_hierarchy = queue_item
            .operations
            .contains(&EnrichmentOperation::CallHierarchy);

        if !(need_references || need_implementations || need_call_hierarchy) {
            debug!(
                "No pending enrichment operations for symbol '{}', skipping",
                queue_item.name
            );
            return Ok(());
        }

        let language = queue_item.language;
        let language_str = language.as_str();

        let original_line = queue_item.def_start_line;
        let original_char = queue_item.def_start_char;
        let (adj_line, adj_char) = crate::position::resolve_symbol_position(
            &queue_item.file_path,
            original_line,
            original_char,
            language_str,
        )
        .unwrap_or((original_line, original_char));

        if adj_line != original_line || adj_char != original_char {
            stats.positions_adjusted.fetch_add(1, Ordering::Relaxed);
        }

        debug!(
            "Using adjusted LSP position {}:{} for {}",
            adj_line,
            adj_char,
            queue_item.file_path.display()
        );

        let BackendType::SQLite(sqlite_backend) = cache_adapter.backend();
        let database_adapter = LspDatabaseAdapter::new();

        if need_call_hierarchy {
            stats
                .call_hierarchy_attempted
                .fetch_add(1, Ordering::Relaxed);
            let call_hierarchy_result = match timeout(
                config.request_timeout,
                server_manager.call_hierarchy(language, &queue_item.file_path, adj_line, adj_char),
            )
            .await
            {
                Ok(Ok(result)) => Some(result),
                Ok(Err(e)) => {
                    debug!(
                        "Call hierarchy unavailable for '{}' ({}:{}:{}): {}",
                        queue_item.name,
                        queue_item.file_path.display(),
                        queue_item.def_start_line,
                        queue_item.def_start_char,
                        e
                    );
                    None
                }
                Err(_) => {
                    debug!(
                        "Call hierarchy request timed out for '{}' at {}:{}:{}",
                        queue_item.name,
                        queue_item.file_path.display(),
                        queue_item.def_start_line,
                        queue_item.def_start_char
                    );
                    None
                }
            };

            if let Some(call_hierarchy_result) = call_hierarchy_result {
                let (symbols, edges) = database_adapter
                    .convert_call_hierarchy_to_database(
                        &call_hierarchy_result,
                        &queue_item.file_path,
                        &language_str,
                        1,
                        &workspace_root,
                    )
                    .context("Failed to convert call hierarchy result to database format")?;

                if !symbols.is_empty() {
                    sqlite_backend
                        .store_symbols(&symbols)
                        .await
                        .context("Failed to store call hierarchy symbols in database")?;
                }

                if !edges.is_empty() {
                    sqlite_backend
                        .store_edges(&edges)
                        .await
                        .context("Failed to store call hierarchy edges in database")?;
                    stats
                        .edges_persisted
                        .fetch_add(edges.len() as u64, Ordering::Relaxed);
                }

                stats.call_hierarchy_success.fetch_add(1, Ordering::Relaxed);

                info!(
                    "Stored call hierarchy for {} ({} symbols, {} edges)",
                    queue_item.name,
                    symbols.len(),
                    edges.len()
                );
            }
        }

        if need_references {
            stats.references_attempted.fetch_add(1, Ordering::Relaxed);
            let references_result = timeout(
                config.request_timeout,
                server_manager.references(
                    language,
                    &queue_item.file_path,
                    adj_line,
                    adj_char,
                    true,
                ),
            )
            .await
            .context("References request timed out")?
            .context("Failed to get references from LSP")?;

            let references_locations = Self::parse_references_json_to_locations(&references_result)
                .context("Failed to parse references result to locations")?;
            if !references_locations.is_empty() {
                stats
                    .references_found
                    .fetch_add(references_locations.len() as u64, Ordering::Relaxed);
            }

            let (ref_symbols, ref_edges) = database_adapter
                .convert_references_to_database(
                    &references_locations,
                    &queue_item.file_path,
                    (adj_line, adj_char),
                    language_str,
                    1,
                    &workspace_root,
                )
                .await
                .context("Failed to convert references to database edges")?;

            if !ref_symbols.is_empty() {
                sqlite_backend
                    .store_symbols(&ref_symbols)
                    .await
                    .context("Failed to store reference symbols in database")?;
            }

            if !ref_edges.is_empty() {
                sqlite_backend
                    .store_edges(&ref_edges)
                    .await
                    .context("Failed to store reference edges in database")?;
                stats
                    .reference_edges_persisted
                    .fetch_add(ref_edges.len() as u64, Ordering::Relaxed);
            }
        }

        if need_implementations {
            stats
                .implementations_attempted
                .fetch_add(1, Ordering::Relaxed);
            let implementation_locations = match timeout(
                config.request_timeout,
                server_manager.implementation(language, &queue_item.file_path, adj_line, adj_char),
            )
            .await
            {
                Ok(Ok(result)) => {
                    let locations = Self::parse_references_json_to_locations(&result)
                        .context("Failed to parse implementations result to locations")?;
                    if !locations.is_empty() {
                        stats
                            .implementations_found
                            .fetch_add(locations.len() as u64, Ordering::Relaxed);
                    }
                    locations
                }
                Ok(Err(e)) => {
                    debug!(
                        "Implementations unavailable for '{}' ({}:{}:{}): {}",
                        queue_item.name,
                        queue_item.file_path.display(),
                        queue_item.def_start_line,
                        queue_item.def_start_char,
                        e
                    );
                    Vec::new()
                }
                Err(_) => {
                    debug!(
                        "Implementation request timed out for '{}' at {}:{}:{}",
                        queue_item.name,
                        queue_item.file_path.display(),
                        queue_item.def_start_line,
                        queue_item.def_start_char,
                    );
                    Vec::new()
                }
            };

            if !implementation_locations.is_empty() {
                stats
                    .implementations_found
                    .fetch_add(implementation_locations.len() as u64, Ordering::Relaxed);
            }

            let impl_edges = database_adapter
                .convert_implementations_to_database(
                    &implementation_locations,
                    &queue_item.file_path,
                    (adj_line, adj_char),
                    language_str,
                    1,
                    &workspace_root,
                )
                .context("Failed to convert implementations to database edges")?;

            if !impl_edges.is_empty() {
                sqlite_backend
                    .store_edges(&impl_edges)
                    .await
                    .context("Failed to store implementation edges in database")?;
                stats
                    .implementation_edges_persisted
                    .fetch_add(impl_edges.len() as u64, Ordering::Relaxed);
            }
        }

        Ok(())
    }

    /// Detect language from file path
    #[allow(dead_code)]
    fn detect_language_from_path(file_path: &Path) -> Result<Language> {
        let extension = file_path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("");

        let language = match extension {
            "rs" => Language::Rust,
            "py" => Language::Python,
            "js" => Language::JavaScript,
            "ts" => Language::TypeScript,
            "go" => Language::Go,
            "java" => Language::Java,
            "c" => Language::C,
            "cpp" | "cc" | "cxx" => Language::Cpp,
            "cs" => Language::CSharp,
            "rb" => Language::Ruby,
            "php" => Language::Php,
            "swift" => Language::Swift,
            "kt" => Language::Kotlin,
            "scala" => Language::Scala,
            "hs" => Language::Haskell,
            "ex" | "exs" => Language::Elixir,
            "clj" | "cljs" => Language::Clojure,
            "lua" => Language::Lua,
            "zig" => Language::Zig,
            _ => Language::Unknown,
        };

        if language == Language::Unknown {
            return Err(anyhow::anyhow!(
                "Unsupported file extension '{}' for file: {}",
                extension,
                file_path.display()
            ));
        }

        Ok(language)
    }

    /// Parse references JSON result to Location array
    fn parse_references_json_to_locations(
        json_result: &serde_json::Value,
    ) -> Result<Vec<crate::protocol::Location>> {
        let mut locations = Vec::new();

        match json_result {
            serde_json::Value::Array(array) => {
                for item in array {
                    if let (Some(uri), Some(range)) =
                        (item.get("uri").and_then(|v| v.as_str()), item.get("range"))
                    {
                        let range = Self::parse_lsp_range(range)?;
                        locations.push(crate::protocol::Location {
                            uri: uri.to_string(),
                            range,
                        });
                    }
                }
            }
            serde_json::Value::Object(obj) => {
                if let (Some(uri), Some(range)) =
                    (obj.get("uri").and_then(|v| v.as_str()), obj.get("range"))
                {
                    let range = Self::parse_lsp_range(range)?;
                    locations.push(crate::protocol::Location {
                        uri: uri.to_string(),
                        range,
                    });
                }
            }
            serde_json::Value::Null => {}
            _ => {}
        }

        Ok(locations)
    }

    /// Parse LSP range from JSON
    fn parse_lsp_range(range_json: &serde_json::Value) -> Result<crate::protocol::Range> {
        let default_start = serde_json::json!({});
        let default_end = serde_json::json!({});
        let start = range_json.get("start").unwrap_or(&default_start);
        let end = range_json.get("end").unwrap_or(&default_end);

        Ok(crate::protocol::Range {
            start: crate::protocol::Position {
                line: start.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                character: start.get("character").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            },
            end: crate::protocol::Position {
                line: end.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                character: end.get("character").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
            },
        })
    }

    /// Generate a symbol UID from a queue item for failure tracking
    async fn generate_symbol_uid(
        queue_item: &QueueItem,
        uid_generator: &Arc<SymbolUIDGenerator>,
    ) -> Result<String> {
        // Create SymbolInfo from QueueItem
        let symbol_kind = match queue_item.kind.as_str() {
            "function" => SymbolKind::Function,
            "method" => SymbolKind::Method,
            "struct" => SymbolKind::Struct,
            "class" => SymbolKind::Class,
            "variable" => SymbolKind::Variable,
            "field" => SymbolKind::Field,
            "enum" => SymbolKind::Enum,
            "interface" => SymbolKind::Interface,
            "trait" => SymbolKind::Trait,
            "module" => SymbolKind::Module,
            "namespace" => SymbolKind::Namespace,
            "constant" => SymbolKind::Constant,
            "typedef" => SymbolKind::Alias,
            "macro" => SymbolKind::Macro,
            _ => SymbolKind::Type,
        };

        let language_str = queue_item.language.as_str();

        let location = SymbolLocation {
            file_path: queue_item.file_path.clone(),
            start_line: queue_item.def_start_line,
            start_char: queue_item.def_start_char,
            end_line: queue_item.def_start_line, // Queue items don't have end positions
            end_char: queue_item.def_start_char,
        };

        let symbol_info = SymbolInfo::new(
            queue_item.name.clone(),
            symbol_kind,
            language_str.to_string(),
            location,
        );

        // Create minimal context (queue items don't have full context)
        let context = SymbolContext::new(
            1, // Default workspace ID
            language_str.to_string(),
        );

        uid_generator
            .generate_uid(&symbol_info, &context)
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to generate UID for symbol {}: {}",
                    queue_item.name,
                    e
                )
            })
    }

    /// Get current worker statistics
    pub fn get_stats(&self) -> Arc<EnrichmentWorkerStats> {
        self.stats.clone()
    }

    /// Get enrichment failure tracker
    pub fn get_enrichment_tracker(&self) -> Arc<EnrichmentTracker> {
        self.enrichment_tracker.clone()
    }

    /// Signal worker to shutdown
    pub fn shutdown(&self) {
        info!("Signaling LSP enrichment worker to shutdown");
        self.shutdown.store(true, Ordering::Relaxed);
    }

    /// Wait for worker to complete
    pub async fn wait_for_completion(
        &self,
        handles: Vec<tokio::task::JoinHandle<()>>,
    ) -> Result<()> {
        info!("Waiting for LSP enrichment worker to complete");

        for handle in handles {
            if let Err(e) = handle.await {
                error!("Worker join error: {}", e);
            }
        }

        info!("LSP enrichment worker completed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enrichment_worker_config_default() {
        let config = EnrichmentWorkerConfig::default();
        assert_eq!(config.batch_size, 100);
        assert_eq!(config.request_timeout, Duration::from_secs(25));
        assert_eq!(config.empty_queue_delay, Duration::from_secs(5));
        assert_eq!(config.max_retries, 2);
    }

    #[test]
    fn test_enrichment_worker_stats() {
        let stats = EnrichmentWorkerStats::default();

        // Test initial state
        let snapshot = stats.snapshot();
        assert_eq!(snapshot.symbols_processed, 0);
        assert_eq!(snapshot.symbols_enriched, 0);
        assert_eq!(snapshot.symbols_failed, 0);
        assert_eq!(snapshot.worker_active, false);
        assert_eq!(snapshot.symbols_skipped_unhealthy, 0);
        assert_eq!(snapshot.references_found, 0);
        assert_eq!(snapshot.implementations_found, 0);
        assert_eq!(snapshot.reference_edges_persisted, 0);
        assert_eq!(snapshot.implementation_edges_persisted, 0);
        assert_eq!(stats.success_rate(), 0.0);

        // Test after some operations
        stats.symbols_processed.store(10, Ordering::Relaxed);
        stats.symbols_enriched.store(8, Ordering::Relaxed);
        stats.symbols_failed.store(2, Ordering::Relaxed);
        stats.worker_active.store(true, Ordering::Relaxed);
        stats.symbols_skipped_unhealthy.store(1, Ordering::Relaxed);
        stats.symbols_skipped_failed.store(0, Ordering::Relaxed);
        stats.references_found.store(5, Ordering::Relaxed);
        stats.implementations_found.store(3, Ordering::Relaxed);
        stats.reference_edges_persisted.store(4, Ordering::Relaxed);
        stats
            .implementation_edges_persisted
            .store(2, Ordering::Relaxed);

        let snapshot = stats.snapshot();
        assert_eq!(snapshot.symbols_processed, 10);
        assert_eq!(snapshot.symbols_enriched, 8);
        assert_eq!(snapshot.symbols_failed, 2);
        assert_eq!(snapshot.worker_active, true);
        assert_eq!(snapshot.symbols_skipped_unhealthy, 1);
        assert_eq!(snapshot.symbols_skipped_failed, 0);
        assert_eq!(snapshot.references_found, 5);
        assert_eq!(snapshot.implementations_found, 3);
        assert_eq!(snapshot.reference_edges_persisted, 4);
        assert_eq!(snapshot.implementation_edges_persisted, 2);
        assert_eq!(stats.success_rate(), 80.0);
    }

    #[tokio::test]
    async fn test_worker_pool_creation() {
        // This test requires mocked dependencies, so we'll just test the basic creation
        let config = EnrichmentWorkerConfig::default();

        // Verify config values are set correctly
        assert!(config.batch_size > 0);
        assert!(config.request_timeout > Duration::from_secs(0));
        assert!(config.empty_queue_delay > Duration::from_secs(0));
    }
}
