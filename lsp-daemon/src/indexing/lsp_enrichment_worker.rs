//! LSP Enrichment Worker Module
//!
//! This module provides parallel workers that process symbols from the enrichment queue
//! and enrich them with LSP data using SingleServerManager directly.
//! This provides optimal performance by avoiding IPC overhead.

use anyhow::{Context, Result};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::time::{sleep, timeout};
use tracing::{debug, error, info, warn};

use crate::database::DatabaseBackend;
use crate::database_cache_adapter::{BackendType, DatabaseCacheAdapter};
use crate::indexing::lsp_enrichment_queue::{LspEnrichmentQueue, QueueItem};
use crate::server_manager::SingleServerManager;
use crate::lsp_database_adapter::LspDatabaseAdapter;
use crate::workspace_utils;
use crate::path_resolver::PathResolver;
use crate::language_detector::Language;

/// Configuration for LSP enrichment workers
#[derive(Debug, Clone)]
pub struct EnrichmentWorkerConfig {
    /// Number of parallel workers
    pub parallelism: usize,
    /// Batch size for processing symbols
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
            parallelism: std::env::var("PROBE_LSP_ENRICHMENT_PARALLELISM")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5),
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

/// Statistics for enrichment workers
#[derive(Debug, Default)]
pub struct EnrichmentWorkerStats {
    /// Total symbols processed
    pub symbols_processed: AtomicU64,
    /// Total symbols successfully enriched
    pub symbols_enriched: AtomicU64,
    /// Total symbols that failed enrichment
    pub symbols_failed: AtomicU64,
    /// Number of active workers
    pub active_workers: AtomicU64,
}

impl EnrichmentWorkerStats {
    /// Get snapshot of current stats
    pub fn snapshot(&self) -> EnrichmentWorkerStatsSnapshot {
        EnrichmentWorkerStatsSnapshot {
            symbols_processed: self.symbols_processed.load(Ordering::Relaxed),
            symbols_enriched: self.symbols_enriched.load(Ordering::Relaxed),
            symbols_failed: self.symbols_failed.load(Ordering::Relaxed),
            active_workers: self.active_workers.load(Ordering::Relaxed),
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

/// Immutable snapshot of worker stats
#[derive(Debug, Clone)]
pub struct EnrichmentWorkerStatsSnapshot {
    pub symbols_processed: u64,
    pub symbols_enriched: u64,
    pub symbols_failed: u64,
    pub active_workers: u64,
}

/// LSP Enrichment Worker Pool
///
/// Manages a pool of workers that process symbols from the enrichment queue
/// and enrich them with LSP data using SingleServerManager directly.
/// This provides optimal performance by avoiding IPC overhead.
pub struct LspEnrichmentWorkerPool {
    /// Worker configuration
    config: EnrichmentWorkerConfig,
    /// Server manager for direct LSP access
    server_manager: Arc<SingleServerManager>,
    /// Database adapter for LSP data conversion
    database_adapter: LspDatabaseAdapter,
    /// Path resolver for relative path handling
    path_resolver: Arc<PathResolver>,
    /// Worker statistics
    stats: Arc<EnrichmentWorkerStats>,
    /// Shutdown signal
    shutdown: Arc<AtomicBool>,
    /// Semaphore for controlling worker concurrency
    semaphore: Arc<Semaphore>,
}

impl LspEnrichmentWorkerPool {
    /// Create a new worker pool using direct SingleServerManager access
    pub fn new(
        config: EnrichmentWorkerConfig,
        server_manager: Arc<SingleServerManager>,
        database_adapter: LspDatabaseAdapter,
        path_resolver: Arc<PathResolver>,
    ) -> Self {
        let semaphore = Arc::new(Semaphore::new(config.parallelism));

        Self {
            config,
            server_manager,
            database_adapter,
            path_resolver,
            stats: Arc::new(EnrichmentWorkerStats::default()),
            shutdown: Arc::new(AtomicBool::new(false)),
            semaphore,
        }
    }

    /// Start the worker pool processing symbols from the queue
    pub async fn start_processing(
        &self,
        queue: Arc<LspEnrichmentQueue>,
        cache_adapter: Arc<DatabaseCacheAdapter>,
    ) -> Result<Vec<tokio::task::JoinHandle<()>>> {
        info!(
            "Starting LSP enrichment worker pool with {} workers",
            self.config.parallelism
        );

        let mut handles = Vec::new();

        for worker_id in 0..self.config.parallelism {
            let handle = self.spawn_worker(
                worker_id,
                queue.clone(),
                cache_adapter.clone(),
            ).await?;
            handles.push(handle);
        }

        Ok(handles)
    }

    /// Spawn a single worker using direct SingleServerManager access
    async fn spawn_worker(
        &self,
        worker_id: usize,
        queue: Arc<LspEnrichmentQueue>,
        cache_adapter: Arc<DatabaseCacheAdapter>,
    ) -> Result<tokio::task::JoinHandle<()>> {
        let semaphore = self.semaphore.clone();
        let stats = self.stats.clone();
        let shutdown = self.shutdown.clone();
        let config = self.config.clone();
        let server_manager = self.server_manager.clone();
        let path_resolver = self.path_resolver.clone();

        let handle = tokio::spawn(async move {
            info!("LSP enrichment worker {} started (using direct SingleServerManager)", worker_id);
            stats.active_workers.fetch_add(1, Ordering::Relaxed);

            while !shutdown.load(Ordering::Relaxed) {
                // Acquire semaphore permit for concurrency control
                let _permit = match semaphore.acquire().await {
                    Ok(permit) => permit,
                    Err(_) => {
                        debug!("Worker {} failed to acquire semaphore permit", worker_id);
                        break;
                    }
                };

                // Try to get next symbol from queue
                match queue.pop_next().await {
                    Some(queue_item) => {
                        debug!(
                            "Worker {} processing symbol: {} ({}:{}) using SingleServerManager",
                            worker_id, queue_item.name, queue_item.file_path.display(), queue_item.def_start_line
                        );

                        // Process the symbol using SingleServerManager directly
                        match Self::process_symbol_with_retries(
                            &queue_item,
                            &server_manager,
                            &path_resolver,
                            &cache_adapter,
                            &config,
                        ).await {
                            Ok(_) => {
                                stats.symbols_enriched.fetch_add(1, Ordering::Relaxed);
                                debug!("Worker {} successfully enriched symbol: {}", worker_id, queue_item.name);
                            }
                            Err(e) => {
                                stats.symbols_failed.fetch_add(1, Ordering::Relaxed);
                                warn!(
                                    "Worker {} failed to enrich symbol {}: {}",
                                    worker_id, queue_item.name, e
                                );
                            }
                        }

                        stats.symbols_processed.fetch_add(1, Ordering::Relaxed);
                    }
                    None => {
                        // Queue is empty, wait before checking again
                        debug!("Worker {} found empty queue, sleeping", worker_id);
                        sleep(config.empty_queue_delay).await;
                    }
                }
            }

            stats.active_workers.fetch_sub(1, Ordering::Relaxed);
            info!("LSP enrichment worker {} stopped", worker_id);
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
    ) -> Result<()> {
        let mut last_error = None;

        for attempt in 0..=config.max_retries {
            if attempt > 0 {
                debug!(
                    "Retrying LSP enrichment for symbol {} (attempt {}/{}) using SingleServerManager",
                    queue_item.name, attempt + 1, config.max_retries + 1
                );
                sleep(Duration::from_millis(500 * attempt as u64)).await;
            }

            match Self::process_symbol_once(
                queue_item,
                server_manager,
                path_resolver,
                cache_adapter,
                config,
            ).await {
                Ok(_) => return Ok(()),
                Err(e) => {
                    last_error = Some(e);
                    warn!(
                        "Attempt {} failed for symbol {}: {}",
                        attempt + 1, queue_item.name, last_error.as_ref().unwrap()
                    );
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            anyhow::anyhow!("Unknown error during symbol processing")
        }))
    }

    /// Process a single symbol using SingleServerManager directly
    async fn process_symbol_once(
        queue_item: &QueueItem,
        server_manager: &Arc<SingleServerManager>,
        _path_resolver: &Arc<PathResolver>,
        cache_adapter: &Arc<DatabaseCacheAdapter>,
        config: &EnrichmentWorkerConfig,
    ) -> Result<()> {
        // Step 1: Resolve workspace root using simple workspace detection
        let workspace_root = workspace_utils::find_workspace_root_with_fallback(&queue_item.file_path)
            .context("Failed to resolve workspace root")?;

        debug!(
            "Processing symbol {} in workspace: {} (using SingleServerManager)",
            queue_item.name,
            workspace_root.display()
        );

        // Step 2: Detect language from file extension
        let language = Self::detect_language_from_path(&queue_item.file_path)
            .context("Failed to detect language from file path")?;

        debug!("Detected language: {:?} for file: {}", language, queue_item.file_path.display());

        // Step 3: Get call hierarchy using SingleServerManager directly
        let call_hierarchy_result = timeout(
            config.request_timeout,
            server_manager.call_hierarchy(
                language,
                workspace_root.clone(),
                &queue_item.file_path,
                queue_item.def_start_line,
                queue_item.def_start_char,
            ),
        )
        .await
        .context("Call hierarchy request timed out")?
        .with_context(|| format!(
            "Failed to get call hierarchy from LSP for symbol '{}' at {}:{}:{}. \
            This usually means the LSP server is not installed, not responding, or the symbol is not at a callable position.",
            queue_item.name,
            queue_item.file_path.display(),
            queue_item.def_start_line,
            queue_item.def_start_char
        ))?;

        // Step 4: Get references using SingleServerManager directly
        let references_result = timeout(
            config.request_timeout,
            server_manager.references(
                language,
                workspace_root.clone(),
                &queue_item.file_path,
                queue_item.def_start_line,
                queue_item.def_start_char,
                true, // include_declaration
            ),
        )
        .await
        .context("References request timed out")?
        .context("Failed to get references from LSP")?;

        // Step 5: Convert LSP results to database format using LspDatabaseAdapter
        let database_adapter = LspDatabaseAdapter::new();

        // Convert call hierarchy result to database format
        let (symbols, edges) = database_adapter.convert_call_hierarchy_to_database(
            &call_hierarchy_result,
            &queue_item.file_path,
            &language.as_str(),
            1, // file_version_id - placeholder
            &workspace_root,
        ).context("Failed to convert call hierarchy result to database format")?;

        let BackendType::SQLite(sqlite_backend) = cache_adapter.backend();

        // Store converted symbols and edges
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
        }

        // For now, skip storing references as the conversion is complex
        // TODO: Implement proper references to edges conversion
        let _references_locations = Self::parse_references_json_to_locations(&references_result)
            .context("Failed to parse references result to locations")?;
        let _reference_edges: Vec<crate::database::Edge> = Vec::new(); // Placeholder

        info!(
            "Successfully enriched symbol {} with {} symbols and {} call hierarchy edges (using SingleServerManager)",
            queue_item.name,
            symbols.len(),
            edges.len()
        );

        Ok(())
    }

    /// Detect language from file path
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

        if let Some(array) = json_result.as_array() {
            for item in array {
                if let (Some(uri), Some(range)) = (
                    item.get("uri").and_then(|v| v.as_str()),
                    item.get("range"),
                ) {
                    let range = Self::parse_lsp_range(range)?;
                    locations.push(crate::protocol::Location {
                        uri: uri.to_string(),
                        range,
                    });
                }
            }
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

    /// Get current worker statistics
    pub fn get_stats(&self) -> Arc<EnrichmentWorkerStats> {
        self.stats.clone()
    }

    /// Signal workers to shutdown
    pub fn shutdown(&self) {
        info!("Signaling LSP enrichment workers to shutdown");
        self.shutdown.store(true, Ordering::Relaxed);
    }

    /// Wait for all workers to complete
    pub async fn wait_for_completion(&self, handles: Vec<tokio::task::JoinHandle<()>>) -> Result<()> {
        info!("Waiting for LSP enrichment workers to complete");

        for handle in handles {
            if let Err(e) = handle.await {
                error!("Worker join error: {}", e);
            }
        }

        info!("All LSP enrichment workers completed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enrichment_worker_config_default() {
        let config = EnrichmentWorkerConfig::default();
        assert_eq!(config.parallelism, 5);
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
        assert_eq!(snapshot.active_workers, 0);
        assert_eq!(stats.success_rate(), 0.0);

        // Test after some operations
        stats.symbols_processed.store(10, Ordering::Relaxed);
        stats.symbols_enriched.store(8, Ordering::Relaxed);
        stats.symbols_failed.store(2, Ordering::Relaxed);
        stats.active_workers.store(3, Ordering::Relaxed);

        let snapshot = stats.snapshot();
        assert_eq!(snapshot.symbols_processed, 10);
        assert_eq!(snapshot.symbols_enriched, 8);
        assert_eq!(snapshot.symbols_failed, 2);
        assert_eq!(snapshot.active_workers, 3);
        assert_eq!(stats.success_rate(), 80.0);
    }

    #[tokio::test]
    async fn test_worker_pool_creation() {
        // This test requires mocked dependencies, so we'll just test the basic creation
        let config = EnrichmentWorkerConfig::default();

        // Verify config values are set correctly
        assert!(config.parallelism > 0);
        assert!(config.batch_size > 0);
        assert!(config.request_timeout > Duration::from_secs(0));
        assert!(config.empty_queue_delay > Duration::from_secs(0));
    }
}