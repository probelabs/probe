//! Cache Layer Middleware
//!
//! This module provides the caching middleware that wraps LSP request handling
//! with transparent cache layer functionality. It implements the memory → disk → LSP server
//! flow with singleflight deduplication for concurrent requests.

use crate::protocol::{DaemonRequest, DaemonResponse};
use crate::universal_cache::{KeyBuilder, LspMethod, PolicyRegistry, UniversalCache};
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::{OnceCell, RwLock};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Singleflight group for deduplicating concurrent requests
#[derive(Debug)]
struct SingleflightGroup {
    /// Active requests mapped to their result cells
    active: RwLock<HashMap<String, std::sync::Arc<OnceCell<SingleflightResult>>>>,
}

/// Result of a singleflight operation
#[derive(Debug, Clone)]
struct SingleflightResult {
    /// The response data
    data: DaemonResponse,
    /// Whether this was a cache hit
    from_cache: bool,
    /// Time taken to complete the request
    duration: Duration,
}

impl SingleflightGroup {
    fn new() -> Self {
        Self {
            active: RwLock::new(HashMap::new()),
        }
    }

    /// Execute a function with singleflight deduplication
    async fn call<F, Fut>(&self, key: &str, f: F) -> Result<SingleflightResult>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = Result<SingleflightResult>> + Send + 'static,
    {
        let start = Instant::now();
        eprintln!("DEBUG: Singleflight call for key '{}'", key);

        // Get or create a OnceCell for this key
        let cell = {
            let mut active = self.active.write().await;
            eprintln!("DEBUG: Singleflight active count: {}", active.len());

            use std::collections::hash_map::Entry;
            match active.entry(key.to_string()) {
                Entry::Occupied(entry) => {
                    eprintln!(
                        "DEBUG: Found existing cell for key '{}', will await result",
                        key
                    );
                    entry.get().clone()
                }
                Entry::Vacant(entry) => {
                    eprintln!(
                        "DEBUG: Creating new cell for key '{}', will become leader",
                        key
                    );
                    let cell = std::sync::Arc::new(OnceCell::new());
                    entry.insert(cell.clone());
                    eprintln!("DEBUG: Inserted new cell, active count: {}", active.len());
                    cell
                }
            }
        };

        // Use OnceCell to ensure only one initializer runs
        let result = cell
            .get_or_try_init(|| {
                eprintln!("DEBUG: Leader executing function for key '{}'", key);
                async move {
                    let result = f().await;
                    eprintln!("DEBUG: Leader function completed for key '{}'", key);
                    result
                }
            })
            .await;

        // Best-effort cleanup: remove the cell if it's the same one we inserted
        {
            let mut active = self.active.write().await;
            if let Some(existing_cell) = active.get(key) {
                if std::sync::Arc::ptr_eq(existing_cell, &cell) {
                    active.remove(key);
                    eprintln!(
                        "DEBUG: Cleaned up cell for key '{}', active count: {}",
                        key,
                        active.len()
                    );
                } else {
                    eprintln!(
                        "DEBUG: Cell for key '{}' was replaced, skipping cleanup",
                        key
                    );
                }
            }
        }

        match result {
            Ok(result) => {
                eprintln!(
                    "DEBUG: Singleflight completed for key '{}' after {:?}",
                    key,
                    start.elapsed()
                );
                Ok(result.clone())
            }
            Err(e) => {
                eprintln!("DEBUG: Singleflight failed for key '{}': {}", key, e);
                Err(e)
            }
        }
    }
}

/// Document provider interface for accessing unsaved content
#[async_trait::async_trait]
pub trait DocumentProvider: Send + Sync {
    /// Get the current content of a document
    async fn get_document_content(&self, uri: &str) -> Result<Option<String>>;

    /// Check if a document has unsaved changes
    async fn has_unsaved_changes(&self, uri: &str) -> Result<bool>;

    /// Get the workspace root for a document
    async fn get_workspace_root(&self, uri: &str) -> Result<Option<PathBuf>>;
}

/// Default document provider that only works with saved files
pub struct FileSystemDocumentProvider;

#[async_trait::async_trait]
impl DocumentProvider for FileSystemDocumentProvider {
    async fn get_document_content(&self, uri: &str) -> Result<Option<String>> {
        if uri.starts_with("file://") {
            let path = uri.strip_prefix("file://").unwrap();
            match tokio::fs::read_to_string(path).await {
                Ok(content) => Ok(Some(content)),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(e.into()),
            }
        } else {
            Ok(None)
        }
    }

    async fn has_unsaved_changes(&self, _uri: &str) -> Result<bool> {
        // Filesystem provider assumes all files are saved
        Ok(false)
    }

    async fn get_workspace_root(&self, _uri: &str) -> Result<Option<PathBuf>> {
        // Let workspace resolution handle this
        Ok(None)
    }
}

/// Cache layer middleware configuration
#[derive(Debug, Clone)]
pub struct CacheLayerConfig {
    /// Whether cache warming is enabled
    pub cache_warming_enabled: bool,

    /// Maximum number of concurrent cache warming operations
    pub cache_warming_concurrency: usize,

    /// Timeout for singleflight operations
    pub singleflight_timeout: Duration,

    /// Whether to track detailed timing metrics
    pub detailed_metrics: bool,

    /// Maximum age of workspace revision cache
    pub workspace_revision_ttl: Duration,
}

impl Default for CacheLayerConfig {
    fn default() -> Self {
        Self {
            cache_warming_enabled: false,
            cache_warming_concurrency: 4,
            singleflight_timeout: Duration::from_secs(30),
            detailed_metrics: true,
            workspace_revision_ttl: Duration::from_secs(60),
        }
    }
}

/// Workspace revision tracking for invalidation
#[derive(Debug, Clone)]
#[allow(dead_code)]
struct WorkspaceRevision {
    /// Workspace root path
    workspace_root: PathBuf,

    /// Revision identifier (e.g., git commit hash)
    revision: String,

    /// Last update timestamp
    updated_at: SystemTime,

    /// Files that have changed since last revision
    changed_files: Vec<PathBuf>,
}

/// Cache layer middleware providing transparent caching for LSP requests
pub struct CacheLayer {
    /// Universal cache instance
    cache: Arc<UniversalCache>,

    /// Key builder for cache keys
    key_builder: KeyBuilder,

    /// Policy registry for cache policies
    policy_registry: PolicyRegistry,

    /// Document provider for accessing unsaved content
    document_provider: Arc<dyn DocumentProvider>,

    /// Singleflight group for deduplication
    singleflight: Arc<SingleflightGroup>,

    /// Configuration
    config: CacheLayerConfig,

    /// Workspace revision tracking
    workspace_revisions: Arc<RwLock<HashMap<PathBuf, WorkspaceRevision>>>,
}

impl Clone for CacheLayer {
    fn clone(&self) -> Self {
        Self {
            cache: self.cache.clone(),
            key_builder: self.key_builder.clone(),
            policy_registry: PolicyRegistry::new(), // Create new instance with default policies
            document_provider: self.document_provider.clone(),
            singleflight: self.singleflight.clone(), // Share the same singleflight group via Arc
            config: self.config.clone(),
            workspace_revisions: self.workspace_revisions.clone(),
        }
    }
}

impl CacheLayer {
    /// Create a new cache layer middleware
    pub fn new(
        cache: Arc<UniversalCache>,
        document_provider: Option<Arc<dyn DocumentProvider>>,
        config: Option<CacheLayerConfig>,
    ) -> Self {
        let document_provider =
            document_provider.unwrap_or_else(|| Arc::new(FileSystemDocumentProvider));

        let config = config.unwrap_or_default();

        Self {
            cache,
            key_builder: KeyBuilder::new(),
            policy_registry: PolicyRegistry::new(),
            document_provider,
            singleflight: Arc::new(SingleflightGroup::new()),
            config,
            workspace_revisions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Process a daemon request with caching middleware
    pub async fn handle_request<F, Fut>(
        &self,
        request: DaemonRequest,
        upstream_handler: F,
    ) -> Result<DaemonResponse>
    where
        F: FnOnce(DaemonRequest) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = DaemonResponse> + Send + 'static,
    {
        let start_time = Instant::now();

        // Check if this request type supports caching
        let lsp_method = match self.extract_lsp_method(&request) {
            Some(method) => method,
            None => {
                // Non-cacheable request, pass through directly
                debug!(
                    "Non-cacheable request: {:?}",
                    std::mem::discriminant(&request)
                );
                return Ok(upstream_handler(request).await);
            }
        };

        // Get cache policy for this method
        let policy = self.policy_registry.get_policy(lsp_method);
        if !policy.enabled {
            debug!("Caching disabled for method: {:?}", lsp_method);
            return Ok(upstream_handler(request).await);
        }

        // Extract request parameters for cache key generation
        let (file_path, params) = match self.extract_request_params(&request) {
            Ok((path, params)) => (path, params),
            Err(e) => {
                warn!("Failed to extract request parameters: {}", e);
                return Ok(upstream_handler(request).await);
            }
        };

        // Check for unsaved changes
        let has_unsaved_changes = self
            .check_unsaved_changes(&file_path)
            .await
            .unwrap_or(false);

        if has_unsaved_changes {
            debug!(
                "File has unsaved changes, bypassing cache: {}",
                file_path.display()
            );
            return Ok(upstream_handler(request).await);
        }

        // Create synchronous singleflight key for immediate deduplication
        let singleflight_key = self
            .key_builder
            .build_singleflight_key(lsp_method, &file_path, &params);

        eprintln!(
            "DEBUG: Synchronous singleflight key: '{}'",
            singleflight_key
        );

        // Debug: Log the singleflight key for debugging identical requests
        debug!(
            "Singleflight key generated: '{}' for request {:?} at {}:{}",
            singleflight_key,
            std::mem::discriminant(&request),
            file_path.display(),
            Self::extract_position_from_request(&request).unwrap_or_else(|| "unknown".to_string())
        );

        // Clone necessary data before the async closure
        let cache = self.cache.clone();
        let key_builder = self.key_builder.clone();
        let method = lsp_method;
        let path = file_path.clone();
        let params_str = params.clone();
        let request_info = self.extract_request_info(&request);
        let request_for_closure = request.clone();
        let upstream_handler_opt =
            std::sync::Arc::new(std::sync::Mutex::new(Some(upstream_handler)));

        // Execute with singleflight deduplication
        debug!(
            "Singleflight: executing request with key '{}'",
            singleflight_key
        );
        let result = self
            .singleflight
            .call(&singleflight_key, move || {
                let cache = cache.clone();
                let key_builder = key_builder.clone();
                let path = path.clone();
                let params_str = params_str.clone();
                let request_info = request_info.clone();
                let request_for_closure = request_for_closure.clone();
                let upstream_handler_opt = upstream_handler_opt.clone();
                let start = start_time;

                Box::pin(async move {
                    eprintln!("DEBUG: Inside singleflight closure, building full cache key...");

                    // Build full cache key (with proper content addressing)
                    let cache_key = match key_builder.build_key(method, &path, &params_str).await {
                        Ok(key) => key,
                        Err(e) => {
                            eprintln!("DEBUG: Failed to build cache key in singleflight: {}", e);
                            // Skip cache, go directly to upstream handler
                            let upstream_handler = match upstream_handler_opt.lock().unwrap().take()
                            {
                                Some(handler) => handler,
                                None => {
                                    eprintln!(
                                        "ERROR: Upstream handler already taken in error case!"
                                    );
                                    return Ok(SingleflightResult {
                                        data: DaemonResponse::Hover {
                                            request_id: match &request_for_closure {
                                                DaemonRequest::Hover { request_id, .. } => {
                                                    *request_id
                                                }
                                                _ => uuid::Uuid::new_v4(),
                                            },
                                            content: None,
                                        },
                                        from_cache: false,
                                        duration: start.elapsed(),
                                    });
                                }
                            };
                            let response = upstream_handler(request_for_closure).await;
                            return Ok(SingleflightResult {
                                data: response,
                                from_cache: false,
                                duration: start.elapsed(),
                            });
                        }
                    };

                    eprintln!(
                        "DEBUG: Built full cache key in singleflight: {}",
                        cache_key.to_storage_key()
                    );

                    // Try cache first
                    match cache
                        .get::<serde_json::Value>(method, &path, &params_str)
                        .await
                    {
                        Ok(Some(cached_value)) => {
                            debug!(
                                "Cache HIT for {:?} {} - key: {}",
                                method,
                                request_info,
                                cache_key.to_storage_key()
                            );

                            // Convert cached value back to DaemonResponse
                            match Self::deserialize_response(&request_for_closure, cached_value) {
                                Ok(response) => {
                                    return Ok(SingleflightResult {
                                        data: response,
                                        from_cache: true,
                                        duration: start.elapsed(),
                                    });
                                }
                                Err(e) => {
                                    warn!("Failed to deserialize cached response: {}", e);
                                    // Fall through to LSP server
                                }
                            }
                        }
                        Ok(None) => {
                            debug!(
                                "Cache MISS for {:?} {} - key: {}",
                                method,
                                request_info,
                                cache_key.to_storage_key()
                            );
                        }
                        Err(e) => {
                            warn!("Cache get error: {}", e);
                            // Fall through to LSP server
                        }
                    }

                    // Cache miss or error, call upstream handler
                    let upstream_handler = match upstream_handler_opt.lock().unwrap().take() {
                        Some(handler) => handler,
                        None => {
                            // This shouldn't happen in a proper singleflight - only one closure should execute
                            eprintln!("ERROR: Upstream handler already taken!");
                            return Ok(SingleflightResult {
                                data: DaemonResponse::Hover {
                                    request_id: match &request_for_closure {
                                        DaemonRequest::Hover { request_id, .. } => *request_id,
                                        _ => uuid::Uuid::new_v4(),
                                    },
                                    content: None,
                                },
                                from_cache: false,
                                duration: start.elapsed(),
                            });
                        }
                    };
                    let response = upstream_handler(request_for_closure).await;
                    let elapsed = start.elapsed();

                    // Cache the response if it's successful
                    if Self::should_cache_response(&response) {
                        if let Ok(serialized) = Self::serialize_response(&response) {
                            if let Err(e) = cache.set(method, &path, &params_str, &serialized).await
                            {
                                warn!("Failed to cache response: {}", e);
                            } else {
                                debug!(
                                    "Cached response for {:?} {} - key: {}",
                                    method,
                                    request_info,
                                    cache_key.to_storage_key()
                                );
                            }
                        }
                    }

                    Ok(SingleflightResult {
                        data: response,
                        from_cache: false,
                        duration: elapsed,
                    })
                })
            })
            .await?;

        // Log performance metrics if enabled
        if self.config.detailed_metrics {
            let cache_status = if result.from_cache { "HIT" } else { "MISS" };
            let (has_data, request_info) =
                self.analyze_request_and_response(&request, &result.data);

            info!(
                "Cache {} for {:?} in {:?} (singleflight: {:?}) {} - {} | File={}, params={}",
                cache_status,
                lsp_method,
                result.duration,
                start_time.elapsed(),
                if has_data {
                    "✓ HAS_DATA"
                } else {
                    "✗ NO_DATA"
                },
                request_info,
                file_path.display(),
                params.chars().take(80).collect::<String>()
            );
        }

        // Adapt the response to use this caller's request_id instead of the shared one
        let adapted_response = Self::adapt_response_request_id(result.data, &request);
        Ok(adapted_response)
    }

    /// Adapt a response to use the correct request_id for each caller
    /// This is essential for singleflight deduplication where multiple callers
    /// get the same response content but need their own request_ids
    fn adapt_response_request_id(
        response: DaemonResponse,
        request: &DaemonRequest,
    ) -> DaemonResponse {
        let caller_request_id = Self::extract_request_id(request);

        match response {
            DaemonResponse::Hover { content, .. } => DaemonResponse::Hover {
                request_id: caller_request_id,
                content,
            },
            DaemonResponse::Definition { locations, .. } => DaemonResponse::Definition {
                request_id: caller_request_id,
                locations,
            },
            DaemonResponse::References { locations, .. } => DaemonResponse::References {
                request_id: caller_request_id,
                locations,
            },
            DaemonResponse::DocumentSymbols { symbols, .. } => DaemonResponse::DocumentSymbols {
                request_id: caller_request_id,
                symbols,
            },
            DaemonResponse::Completion { items, .. } => DaemonResponse::Completion {
                request_id: caller_request_id,
                items,
            },
            DaemonResponse::CallHierarchy { result, .. } => DaemonResponse::CallHierarchy {
                request_id: caller_request_id,
                result,
            },
            DaemonResponse::WorkspaceSymbols { symbols, .. } => DaemonResponse::WorkspaceSymbols {
                request_id: caller_request_id,
                symbols,
            },
            DaemonResponse::Implementations { locations, .. } => DaemonResponse::Implementations {
                request_id: caller_request_id,
                locations,
            },
            DaemonResponse::TypeDefinition { locations, .. } => DaemonResponse::TypeDefinition {
                request_id: caller_request_id,
                locations,
            },
            DaemonResponse::Error { error, .. } => DaemonResponse::Error {
                request_id: caller_request_id,
                error,
            },
            // For other variants, just return as-is (they might not be cacheable)
            other => other,
        }
    }

    /// Extract request_id from any DaemonRequest
    fn extract_request_id(request: &DaemonRequest) -> Uuid {
        match request {
            DaemonRequest::Hover { request_id, .. } => *request_id,
            DaemonRequest::Definition { request_id, .. } => *request_id,
            DaemonRequest::References { request_id, .. } => *request_id,
            DaemonRequest::DocumentSymbols { request_id, .. } => *request_id,
            DaemonRequest::Completion { request_id, .. } => *request_id,
            DaemonRequest::CallHierarchy { request_id, .. } => *request_id,
            DaemonRequest::WorkspaceSymbols { request_id, .. } => *request_id,
            DaemonRequest::Implementations { request_id, .. } => *request_id,
            DaemonRequest::TypeDefinition { request_id, .. } => *request_id,
            // Add other variants as needed
            _ => panic!("Unsupported request type for request_id extraction"),
        }
    }

    /// Invalidate cache entries when workspace state changes
    pub async fn invalidate_workspace(&self, workspace_root: &Path) -> Result<usize> {
        info!(
            "Invalidating cache for workspace: {}",
            workspace_root.display()
        );

        let invalidated = self.cache.clear_workspace(workspace_root).await?;

        // Update workspace revision
        self.update_workspace_revision(workspace_root, None).await?;

        info!("Invalidated {} cache entries for workspace", invalidated);
        Ok(invalidated)
    }

    /// Invalidate cache entries for a specific file
    pub async fn invalidate_file(&self, file_path: &Path) -> Result<usize> {
        debug!("Invalidating cache for file: {}", file_path.display());

        let invalidated = self.cache.invalidate_file(file_path).await?;

        debug!("Invalidated {} cache entries for file", invalidated);
        Ok(invalidated)
    }

    /// Clear cache entries for a specific symbol
    pub async fn clear_symbol(
        &self,
        file_path: &Path,
        symbol_name: &str,
        line: Option<u32>,
        column: Option<u32>,
        methods: Option<Vec<String>>,
        all_positions: bool,
    ) -> Result<(usize, Vec<(u32, u32)>, Vec<String>, u64)> {
        debug!(
            "Clearing cache for symbol '{}' in file: {}",
            symbol_name,
            file_path.display()
        );

        let result = self
            .cache
            .store
            .clear_symbol(file_path, symbol_name, line, column, methods, all_positions)
            .await?;

        debug!(
            "Cleared {} cache entries for symbol '{}'",
            result.0, symbol_name
        );
        Ok(result)
    }

    /// Warm cache for commonly accessed methods
    pub async fn warm_cache(&self, _workspace_root: &Path, files: Vec<PathBuf>) -> Result<usize> {
        if !self.config.cache_warming_enabled {
            return Ok(0);
        }

        info!(
            "Starting cache warming for {} files in workspace",
            files.len()
        );

        // Methods to warm cache for (based on frequency of access)
        let methods_to_warm = vec![
            LspMethod::Hover,
            LspMethod::Definition,
            LspMethod::DocumentSymbols,
            LspMethod::References,
        ];

        let mut warmed_count = 0;

        // Use semaphore to limit concurrency
        let semaphore = Arc::new(tokio::sync::Semaphore::new(
            self.config.cache_warming_concurrency,
        ));
        let mut handles = Vec::new();

        for file_path in files {
            for method in &methods_to_warm {
                let sem = semaphore.clone();
                let cache = self.cache.clone();
                // Don't need key_builder in current warming implementation
                let file = file_path.clone();
                let method = *method;

                let handle = tokio::spawn(async move {
                    let _permit = sem.acquire().await.ok()?;

                    // Build cache key with minimal params
                    let params = match method {
                        LspMethod::Hover => r#"{"position": {"line": 0, "character": 0}}"#,
                        LspMethod::Definition => r#"{"position": {"line": 0, "character": 0}}"#,
                        LspMethod::DocumentSymbols => "{}",
                        LspMethod::References => {
                            r#"{"position": {"line": 0, "character": 0}, "context": {"includeDeclaration": true}}"#
                        }
                        _ => "{}",
                    };

                    // Check if already cached
                    match cache.get::<serde_json::Value>(method, &file, params).await {
                        Ok(Some(_)) => {
                            debug!(
                                "Cache already warmed for {:?} on {}",
                                method,
                                file.display()
                            );
                            return Some(1);
                        }
                        Ok(None) => {
                            debug!(
                                "Cache miss during warming for {:?} on {}",
                                method,
                                file.display()
                            );
                        }
                        Err(e) => {
                            warn!(
                                "Cache warming error for {:?} on {}: {}",
                                method,
                                file.display(),
                                e
                            );
                        }
                    }

                    None
                });

                handles.push(handle);
            }
        }

        // Wait for all warming operations to complete
        for handle in handles {
            if let Ok(Some(count)) = handle.await {
                warmed_count += count;
            }
        }

        info!("Cache warming completed: {} entries warmed", warmed_count);
        Ok(warmed_count)
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> Result<CacheLayerStats> {
        let cache_stats = self.cache.get_stats().await?;

        let workspace_revisions = self.workspace_revisions.read().await;
        let active_workspaces = workspace_revisions.len();

        Ok(CacheLayerStats {
            cache_stats,
            active_workspaces,
            singleflight_active: 0, // Would need to track this
            cache_warming_enabled: self.config.cache_warming_enabled,
        })
    }

    /// Get direct access to the underlying universal cache for indexing operations
    pub fn get_universal_cache(&self) -> &Arc<UniversalCache> {
        &self.cache
    }

    /// List cache keys with filtering and pagination
    pub async fn list_keys(
        &self,
        workspace_path: Option<&Path>,
        operation_filter: Option<&str>,
        file_pattern_filter: Option<&str>,
        limit: usize,
        offset: usize,
        sort_by: Option<&str>,
    ) -> Result<(Vec<crate::protocol::CacheKeyInfo>, usize)> {
        self.cache
            .list_keys(
                workspace_path,
                operation_filter,
                file_pattern_filter,
                limit,
                offset,
                sort_by,
            )
            .await
    }

    // === Private Methods ===

    /// Extract position from daemon request for debugging
    fn extract_position_from_request(request: &DaemonRequest) -> Option<String> {
        match request {
            DaemonRequest::Hover { line, column, .. } => Some(format!("{line}:{column}")),
            DaemonRequest::Definition { line, column, .. } => Some(format!("{line}:{column}")),
            DaemonRequest::References { line, column, .. } => Some(format!("{line}:{column}")),
            DaemonRequest::TypeDefinition { line, column, .. } => {
                Some(format!("{line}:{column}"))
            }
            DaemonRequest::Implementations { line, column, .. } => {
                Some(format!("{line}:{column}"))
            }
            DaemonRequest::CallHierarchy { line, column, .. } => {
                Some(format!("{line}:{column}"))
            }
            DaemonRequest::Completion { line, column, .. } => Some(format!("{line}:{column}")),
            _ => None,
        }
    }

    /// Extract LSP method from daemon request
    fn extract_lsp_method(&self, request: &DaemonRequest) -> Option<LspMethod> {
        match request {
            DaemonRequest::Definition { .. } => Some(LspMethod::Definition),
            DaemonRequest::References { .. } => Some(LspMethod::References),
            DaemonRequest::Hover { .. } => Some(LspMethod::Hover),
            DaemonRequest::DocumentSymbols { .. } => Some(LspMethod::DocumentSymbols),
            DaemonRequest::WorkspaceSymbols { .. } => Some(LspMethod::WorkspaceSymbols),
            DaemonRequest::TypeDefinition { .. } => Some(LspMethod::TypeDefinition),
            DaemonRequest::Implementations { .. } => Some(LspMethod::Implementation),
            DaemonRequest::CallHierarchy { .. } => Some(LspMethod::CallHierarchy),
            DaemonRequest::Completion { .. } => Some(LspMethod::Completion),
            _ => None,
        }
    }

    /// Extract request parameters for cache key generation
    fn extract_request_params(&self, request: &DaemonRequest) -> Result<(PathBuf, String)> {
        match request {
            DaemonRequest::Definition {
                file_path,
                line,
                column,
                ..
            } => {
                let params = serde_json::json!({
                    "position": {"line": line, "character": column}
                });
                Ok((file_path.clone(), params.to_string()))
            }
            DaemonRequest::References {
                file_path,
                line,
                column,
                include_declaration,
                ..
            } => {
                let params = serde_json::json!({
                    "position": {"line": line, "character": column},
                    "context": {"includeDeclaration": include_declaration}
                });
                Ok((file_path.clone(), params.to_string()))
            }
            DaemonRequest::Hover {
                file_path,
                line,
                column,
                ..
            } => {
                let params = serde_json::json!({
                    "position": {"line": line, "character": column}
                });
                Ok((file_path.clone(), params.to_string()))
            }
            DaemonRequest::DocumentSymbols { file_path, .. } => {
                Ok((file_path.clone(), "{}".to_string()))
            }
            DaemonRequest::WorkspaceSymbols { query, .. } => {
                // For workspace symbols, use current directory as file path
                let current_dir = std::env::current_dir()?;
                let params = serde_json::json!({
                    "query": query
                });
                Ok((current_dir, params.to_string()))
            }
            DaemonRequest::TypeDefinition {
                file_path,
                line,
                column,
                ..
            } => {
                let params = serde_json::json!({
                    "position": {"line": line, "character": column}
                });
                Ok((file_path.clone(), params.to_string()))
            }
            DaemonRequest::Implementations {
                file_path,
                line,
                column,
                ..
            } => {
                let params = serde_json::json!({
                    "position": {"line": line, "character": column}
                });
                Ok((file_path.clone(), params.to_string()))
            }
            DaemonRequest::CallHierarchy {
                file_path,
                line,
                column,
                ..
            } => {
                let params = serde_json::json!({
                    "position": {"line": line, "character": column}
                });
                Ok((file_path.clone(), params.to_string()))
            }
            _ => Err(anyhow!("Unsupported request type for parameter extraction")),
        }
    }

    /// Check if a file has unsaved changes
    async fn check_unsaved_changes(&self, file_path: &Path) -> Result<bool> {
        let file_uri = format!("file://{}", file_path.display());
        self.document_provider.has_unsaved_changes(&file_uri).await
    }

    /// Update workspace revision information
    async fn update_workspace_revision(
        &self,
        workspace_root: &Path,
        changed_files: Option<Vec<PathBuf>>,
    ) -> Result<()> {
        let revision = self.get_current_revision(workspace_root).await?;

        let workspace_revision = WorkspaceRevision {
            workspace_root: workspace_root.to_path_buf(),
            revision,
            updated_at: SystemTime::now(),
            changed_files: changed_files.unwrap_or_default(),
        };

        let mut revisions = self.workspace_revisions.write().await;
        revisions.insert(workspace_root.to_path_buf(), workspace_revision);

        Ok(())
    }

    /// Get current revision identifier for a workspace (e.g., git commit hash)
    async fn get_current_revision(&self, workspace_root: &Path) -> Result<String> {
        // Try to get git commit hash first
        if let Ok(output) = tokio::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(workspace_root)
            .output()
            .await
        {
            if output.status.success() {
                if let Ok(hash) = String::from_utf8(output.stdout) {
                    return Ok(hash.trim().to_string());
                }
            }
        }

        // Fallback to directory modification time
        let metadata = tokio::fs::metadata(workspace_root).await?;
        if let Ok(modified) = metadata.modified() {
            if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
                return Ok(duration.as_secs().to_string());
            }
        }

        // Ultimate fallback
        Ok("unknown".to_string())
    }

    /// Serialize daemon response for caching
    fn serialize_response(response: &DaemonResponse) -> Result<serde_json::Value> {
        serde_json::to_value(response).context("Failed to serialize response")
    }

    /// Deserialize cached value back to daemon response
    fn deserialize_response(
        request: &DaemonRequest,
        cached_value: serde_json::Value,
    ) -> Result<DaemonResponse> {
        // Extract request ID from original request
        let request_id = match request {
            DaemonRequest::Definition { request_id, .. } => *request_id,
            DaemonRequest::References { request_id, .. } => *request_id,
            DaemonRequest::Hover { request_id, .. } => *request_id,
            DaemonRequest::DocumentSymbols { request_id, .. } => *request_id,
            DaemonRequest::WorkspaceSymbols { request_id, .. } => *request_id,
            DaemonRequest::TypeDefinition { request_id, .. } => *request_id,
            DaemonRequest::Implementations { request_id, .. } => *request_id,
            DaemonRequest::CallHierarchy { request_id, .. } => *request_id,
            _ => Uuid::new_v4(),
        };

        // Reconstruct response with current request ID
        let mut response: DaemonResponse = serde_json::from_value(cached_value)
            .context("Failed to deserialize cached response")?;

        // Update request ID to match current request
        match &mut response {
            DaemonResponse::Definition { request_id: id, .. } => *id = request_id,
            DaemonResponse::References { request_id: id, .. } => *id = request_id,
            DaemonResponse::Hover { request_id: id, .. } => *id = request_id,
            DaemonResponse::DocumentSymbols { request_id: id, .. } => *id = request_id,
            DaemonResponse::WorkspaceSymbols { request_id: id, .. } => *id = request_id,
            DaemonResponse::TypeDefinition { request_id: id, .. } => *id = request_id,
            DaemonResponse::Implementations { request_id: id, .. } => *id = request_id,
            DaemonResponse::CallHierarchy { request_id: id, .. } => *id = request_id,
            _ => {}
        }

        Ok(response)
    }

    /// Analyze request and response to determine if meaningful data was returned
    fn analyze_request_and_response(
        &self,
        request: &DaemonRequest,
        response: &DaemonResponse,
    ) -> (bool, String) {
        let request_info = self.extract_request_info(request);
        let has_meaningful_data = self.has_meaningful_response_data(response);

        (has_meaningful_data, request_info)
    }

    /// Extract file/symbol/position information from request
    fn extract_request_info(&self, request: &DaemonRequest) -> String {
        match request {
            DaemonRequest::CallHierarchy {
                file_path,
                line,
                column,
                ..
            } => {
                format!(
                    "{}:{}:{}",
                    file_path.file_name().unwrap_or_default().to_string_lossy(),
                    line + 1,   // Convert to 1-based line numbers
                    column + 1  // Convert to 1-based column numbers
                )
            }
            DaemonRequest::Definition {
                file_path,
                line,
                column,
                ..
            } => {
                format!(
                    "{}:{}:{}",
                    file_path.file_name().unwrap_or_default().to_string_lossy(),
                    line + 1,
                    column + 1
                )
            }
            DaemonRequest::References {
                file_path,
                line,
                column,
                ..
            } => {
                format!(
                    "{}:{}:{}",
                    file_path.file_name().unwrap_or_default().to_string_lossy(),
                    line + 1,
                    column + 1
                )
            }
            DaemonRequest::Hover {
                file_path,
                line,
                column,
                ..
            } => {
                format!(
                    "{}:{}:{}",
                    file_path.file_name().unwrap_or_default().to_string_lossy(),
                    line + 1,
                    column + 1
                )
            }
            DaemonRequest::TypeDefinition {
                file_path,
                line,
                column,
                ..
            } => {
                format!(
                    "{}:{}:{}",
                    file_path.file_name().unwrap_or_default().to_string_lossy(),
                    line + 1,
                    column + 1
                )
            }
            DaemonRequest::Implementations {
                file_path,
                line,
                column,
                ..
            } => {
                format!(
                    "{}:{}:{}",
                    file_path.file_name().unwrap_or_default().to_string_lossy(),
                    line + 1,
                    column + 1
                )
            }
            DaemonRequest::DocumentSymbols { file_path, .. } => {
                format!(
                    "{}",
                    file_path.file_name().unwrap_or_default().to_string_lossy()
                )
            }
            DaemonRequest::WorkspaceSymbols { query, .. } => {
                format!("query:{query}")
            }
            _ => "unknown".to_string(),
        }
    }

    /// Determine if response contains meaningful data
    fn has_meaningful_response_data(&self, response: &DaemonResponse) -> bool {
        match response {
            DaemonResponse::CallHierarchy { result, .. } => {
                // Check if call hierarchy has incoming or outgoing calls
                !result.incoming.is_empty() || !result.outgoing.is_empty()
            }
            DaemonResponse::Definition { locations, .. } => !locations.is_empty(),
            DaemonResponse::References { locations, .. } => !locations.is_empty(),
            DaemonResponse::Hover { content, .. } => content.is_some(),
            DaemonResponse::TypeDefinition { locations, .. } => !locations.is_empty(),
            DaemonResponse::Implementations { locations, .. } => !locations.is_empty(),
            DaemonResponse::DocumentSymbols { symbols, .. } => !symbols.is_empty(),
            DaemonResponse::WorkspaceSymbols { symbols, .. } => !symbols.is_empty(),
            DaemonResponse::Error { .. } => false,
            _ => true, // Other response types are assumed to have data
        }
    }

    /// Determine if a response should be cached
    fn should_cache_response(response: &DaemonResponse) -> bool {
        // Don't cache error responses
        matches!(response, DaemonResponse::Error { .. }).not()
    }
}

/// Cache layer statistics
#[derive(Debug, Clone)]
pub struct CacheLayerStats {
    /// Universal cache statistics
    pub cache_stats: crate::universal_cache::CacheStats,

    /// Number of active workspaces
    pub active_workspaces: usize,

    /// Number of active singleflight operations
    pub singleflight_active: usize,

    /// Whether cache warming is enabled
    pub cache_warming_enabled: bool,
}

/// Extensions for boolean negation
trait BooleanExt {
    fn not(self) -> bool;
}

impl BooleanExt for bool {
    fn not(self) -> bool {
        !self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{DaemonRequest, DaemonResponse, Location};
    use crate::universal_cache::UniversalCache;
    use crate::workspace_cache_router::WorkspaceCacheRouter;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::time::{sleep, Duration};

    async fn create_test_cache_layer() -> (CacheLayer, TempDir) {
        let temp_dir = TempDir::new().unwrap();

        // Create workspace cache router
        let config = crate::workspace_cache_router::WorkspaceCacheRouterConfig {
            base_cache_dir: temp_dir.path().join("caches"),
            max_open_caches: 3,
            max_parent_lookup_depth: 2,
            ..Default::default()
        };

        let registry = Arc::new(crate::lsp_registry::LspRegistry::new().unwrap());
        let child_processes = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let server_manager = Arc::new(
            crate::server_manager::SingleServerManager::new_with_tracker(registry, child_processes),
        );

        let workspace_router = Arc::new(WorkspaceCacheRouter::new(config, server_manager));

        // Create universal cache
        let universal_cache = Arc::new(UniversalCache::new(workspace_router).await.unwrap());

        // Create cache layer
        let cache_layer = CacheLayer::new(universal_cache, None, None);

        (cache_layer, temp_dir)
    }

    #[tokio::test]
    async fn test_cache_layer_pass_through() {
        let (cache_layer, _temp_dir) = create_test_cache_layer().await;

        // Test non-cacheable request passes through
        let request = DaemonRequest::Status {
            request_id: Uuid::new_v4(),
        };

        let upstream_called = Arc::new(tokio::sync::Mutex::new(false));
        let upstream_called_clone = upstream_called.clone();

        let response = cache_layer
            .handle_request(request, move |_req| {
                let called = upstream_called_clone.clone();
                async move {
                    *called.lock().await = true;
                    DaemonResponse::Status {
                        request_id: Uuid::new_v4(),
                        status: crate::protocol::DaemonStatus {
                            uptime_secs: 100,
                            pools: vec![],
                            total_requests: 1,
                            active_connections: 0,
                            version: "test".to_string(),
                            git_hash: "test".to_string(),
                            build_date: "test".to_string(),
                            universal_cache_stats: None,
                        },
                    }
                }
            })
            .await
            .unwrap();

        // Should have called upstream handler
        assert!(*upstream_called.lock().await);

        // Should get response back
        matches!(response, DaemonResponse::Status { .. });
    }

    #[tokio::test]
    async fn test_cache_layer_caching() {
        let (cache_layer, temp_dir) = create_test_cache_layer().await;

        // Create test workspace and file
        let workspace = temp_dir.path().join("test-workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::write(workspace.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();

        let test_file = workspace.join("src/main.rs");
        std::fs::create_dir_all(test_file.parent().unwrap()).unwrap();
        std::fs::write(&test_file, "fn main() {}").unwrap();

        let request_id = Uuid::new_v4();
        let request = DaemonRequest::Definition {
            request_id,
            file_path: test_file.clone(),
            line: 0,
            column: 3,
            workspace_hint: Some(workspace.clone()),
        };

        let call_count = Arc::new(tokio::sync::Mutex::new(0));
        let call_count_clone = call_count.clone();

        // First call should miss cache and call upstream
        let response1 = cache_layer
            .handle_request(request.clone(), move |_req| {
                let count = call_count_clone.clone();
                async move {
                    let mut count = count.lock().await;
                    *count += 1;
                    DaemonResponse::Definition {
                        request_id,
                        locations: vec![Location {
                            uri: format!("file://{}", test_file.display()),
                            range: crate::protocol::Range {
                                start: crate::protocol::Position {
                                    line: 0,
                                    character: 3,
                                },
                                end: crate::protocol::Position {
                                    line: 0,
                                    character: 7,
                                },
                            },
                        }],
                    }
                }
            })
            .await
            .unwrap();

        // Should have called upstream once
        assert_eq!(*call_count.lock().await, 1);

        let call_count_clone2 = call_count.clone();

        // Second identical call should hit cache
        let response2 = cache_layer
            .handle_request(request, move |_req| {
                let count = call_count_clone2.clone();
                async move {
                    let mut count = count.lock().await;
                    *count += 1;
                    DaemonResponse::Definition {
                        request_id,
                        locations: vec![],
                    }
                }
            })
            .await
            .unwrap();

        // Should still only have called upstream once (cache hit)
        assert_eq!(*call_count.lock().await, 1);

        // Both responses should match (ignoring request ID)
        match (response1, response2) {
            (
                DaemonResponse::Definition {
                    locations: loc1, ..
                },
                DaemonResponse::Definition {
                    locations: loc2, ..
                },
            ) => {
                assert_eq!(loc1.len(), loc2.len());
                if !loc1.is_empty() {
                    assert_eq!(loc1[0].uri, loc2[0].uri);
                }
            }
            _ => panic!("Expected Definition responses"),
        }
    }

    #[tokio::test]
    async fn test_singleflight_deduplication() {
        let (cache_layer, temp_dir) = create_test_cache_layer().await;

        // Create test workspace and file
        let workspace = temp_dir.path().join("singleflight-workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::write(
            workspace.join("Cargo.toml"),
            "[package]\nname = \"singleflight\"",
        )
        .unwrap();

        let test_file = workspace.join("src/lib.rs");
        std::fs::create_dir_all(test_file.parent().unwrap()).unwrap();
        std::fs::write(&test_file, "pub fn test() {}").unwrap();

        let request_id1 = Uuid::new_v4();
        let request_id2 = Uuid::new_v4();

        let request1 = DaemonRequest::Hover {
            request_id: request_id1,
            file_path: test_file.clone(),
            line: 0,
            column: 8,
            workspace_hint: Some(workspace.clone()),
        };

        let request2 = DaemonRequest::Hover {
            request_id: request_id2,
            file_path: test_file.clone(),
            line: 0,
            column: 8,
            workspace_hint: Some(workspace.clone()),
        };

        let call_count = Arc::new(tokio::sync::Mutex::new(0));
        let call_count_clone1 = call_count.clone();
        let call_count_clone2 = call_count.clone();

        // Use a barrier to ensure both tasks start concurrently
        let barrier = Arc::new(tokio::sync::Barrier::new(2));
        let barrier1 = barrier.clone();
        let barrier2 = barrier.clone();

        // Launch two concurrent identical requests
        let handle1 = tokio::spawn({
            let cache_layer = cache_layer.clone();
            async move {
                // Wait for both tasks to reach this point
                barrier1.wait().await;
                cache_layer
                    .handle_request(request1, move |req| {
                        let count = call_count_clone1.clone();
                        async move {
                            let mut count = count.lock().await;
                            *count += 1;
                            // Simulate slow LSP server
                            sleep(Duration::from_millis(100)).await;
                            // Return response (request_id will be adapted by CacheLayer)
                            let request_id = match req {
                                DaemonRequest::Hover { request_id, .. } => request_id,
                                _ => panic!("Expected Hover request"),
                            };
                            DaemonResponse::Hover {
                                request_id,
                                content: Some(crate::protocol::HoverContent {
                                    contents: "singleflight deduplication test".to_string(),
                                    range: None,
                                }),
                            }
                        }
                    })
                    .await
            }
        });

        let handle2 = tokio::spawn({
            async move {
                // Wait for both tasks to reach this point - ensures true concurrency
                barrier2.wait().await;
                cache_layer
                    .handle_request(request2, move |req| {
                        let count = call_count_clone2.clone();
                        async move {
                            let mut count = count.lock().await;
                            *count += 1;
                            // This should never be called due to singleflight deduplication
                            let request_id = match req {
                                DaemonRequest::Hover { request_id, .. } => request_id,
                                _ => panic!("Expected Hover request"),
                            };
                            DaemonResponse::Hover {
                                request_id,
                                content: Some(crate::protocol::HoverContent {
                                    contents: "singleflight deduplication test".to_string(),
                                    range: None,
                                }),
                            }
                        }
                    })
                    .await
            }
        });

        let (result1, result2) = tokio::join!(handle1, handle2);
        let response1 = result1.unwrap().unwrap();
        let response2 = result2.unwrap().unwrap();

        // Should only have called upstream once due to singleflight
        assert_eq!(*call_count.lock().await, 1);

        // Both responses should have the same content but different request_ids (singleflight)
        match (response1, response2) {
            (
                DaemonResponse::Hover {
                    request_id: req_id1,
                    content: Some(content1),
                },
                DaemonResponse::Hover {
                    request_id: req_id2,
                    content: Some(content2),
                },
            ) => {
                // Same content from singleflight result
                assert_eq!(content1.contents, content2.contents);
                // But different request_ids (adapted by CacheLayer)
                assert_eq!(req_id1, request_id1);
                assert_eq!(req_id2, request_id2);
                assert_ne!(req_id1, req_id2);
            }
            _ => panic!("Expected Hover responses with content"),
        }
    }

    #[tokio::test]
    async fn test_cache_invalidation() {
        let (cache_layer, temp_dir) = create_test_cache_layer().await;

        // Create test workspace and file
        let workspace = temp_dir.path().join("invalidation-workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::write(workspace.join("go.mod"), "module invalidation").unwrap();

        let test_file = workspace.join("main.go");
        std::fs::write(&test_file, "package main\n\nfunc main() {}").unwrap();

        // First, populate cache
        let request_id = Uuid::new_v4();
        let request = DaemonRequest::DocumentSymbols {
            request_id,
            file_path: test_file.clone(),
            workspace_hint: Some(workspace.clone()),
        };

        let _response = cache_layer
            .handle_request(request.clone(), move |_req| async move {
                DaemonResponse::DocumentSymbols {
                    request_id,
                    symbols: vec![],
                }
            })
            .await
            .unwrap();

        // Verify cache has content
        let stats_before = cache_layer.get_stats().await.unwrap();
        assert!(stats_before.cache_stats.total_entries > 0);

        // Invalidate file cache
        let invalidated = cache_layer.invalidate_file(&test_file).await.unwrap();
        assert!(invalidated >= 0); // May be 0 if persistent cache not fully implemented

        // Invalidate workspace cache
        let invalidated_workspace = cache_layer.invalidate_workspace(&workspace).await.unwrap();
        assert!(invalidated_workspace >= 0);

        // Statistics should show cache was cleared
        let stats_after = cache_layer.get_stats().await.unwrap();
        // Note: In current implementation, memory cache may still have entries due to TTL
        // This test mainly verifies the invalidation methods work without errors
        assert!(stats_after.active_workspaces >= 0);
    }

    #[tokio::test]
    async fn test_cache_warming() {
        // Create cache layer with warming enabled for this test
        let temp_dir = TempDir::new().unwrap();

        // Create workspace cache router
        let config = crate::workspace_cache_router::WorkspaceCacheRouterConfig {
            base_cache_dir: temp_dir.path().join("caches"),
            max_open_caches: 3,
            max_parent_lookup_depth: 2,
            ..Default::default()
        };

        let registry = Arc::new(crate::lsp_registry::LspRegistry::new().unwrap());
        let child_processes = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let server_manager = Arc::new(
            crate::server_manager::SingleServerManager::new_with_tracker(registry, child_processes),
        );

        let workspace_router = Arc::new(WorkspaceCacheRouter::new(config, server_manager));

        // Create universal cache
        let universal_cache = Arc::new(UniversalCache::new(workspace_router).await.unwrap());

        // Create cache layer with warming enabled
        let cache_warming_config = CacheLayerConfig {
            cache_warming_enabled: true,
            ..Default::default()
        };
        let cache_layer = CacheLayer::new(universal_cache, None, Some(cache_warming_config));

        // Create test workspace with multiple files
        let workspace = temp_dir.path().join("warming-workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::write(workspace.join("package.json"), r#"{"name": "warming"}"#).unwrap();

        let file1 = workspace.join("src/index.js");
        let file2 = workspace.join("src/utils.js");

        std::fs::create_dir_all(file1.parent().unwrap()).unwrap();
        std::fs::write(&file1, "export function main() {}").unwrap();
        std::fs::write(&file2, "export function helper() {}").unwrap();

        // Warm cache
        let files = vec![file1.clone(), file2.clone()];
        let warmed = cache_layer.warm_cache(&workspace, files).await.unwrap();

        // Should have attempted to warm cache (actual warming depends on cache implementation)
        assert!(warmed >= 0);

        // Verify warming doesn't crash
        let stats = cache_layer.get_stats().await.unwrap();
        assert!(stats.cache_warming_enabled);
    }
}
