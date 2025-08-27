//! Integration Example for Cache Layer Middleware
//!
//! This module demonstrates how to integrate the Cache Layer middleware
//! with the LSP daemon request handling pipeline.
//!
//! Note: This is a documentation module showing the integration pattern.
//! The actual integration would require modifications to the daemon's public API.

#[cfg(test)]
mod tests {
    use crate::universal_cache::{
        CacheLayer, CacheLayerConfig, DocumentProvider, DocumentProviderFactory, UniversalCache,
    };
    use std::sync::Arc;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_cache_layer_creation() {
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

        let workspace_router = Arc::new(crate::workspace_cache_router::WorkspaceCacheRouter::new(
            config,
            server_manager,
        ));

        // Create universal cache
        let universal_cache = Arc::new(UniversalCache::new(workspace_router).await.unwrap());

        // Create document provider
        let document_provider = DocumentProviderFactory::create_filesystem_provider();

        // Create cache layer
        let cache_layer = CacheLayer::new(universal_cache, Some(document_provider), None);

        // Verify cache layer can provide stats
        let stats = cache_layer.get_stats().await.unwrap();
        assert_eq!(stats.cache_stats.total_entries, 0);
        assert!(!stats.cache_warming_enabled); // Default config should have warming disabled
    }

    #[tokio::test]
    async fn test_document_provider_integration() {
        let document_provider = DocumentProviderFactory::create_daemon_provider(None);

        let uri = "file:///test/example.rs";
        let content = "fn main() {}";

        // Test document lifecycle
        document_provider
            .document_opened(uri, content.to_string(), 1)
            .await;
        assert!(document_provider.is_document_open(uri).await);
        // Use DocumentProvider trait
        let provider_trait: &dyn DocumentProvider = document_provider.as_ref();
        assert!(!provider_trait.has_unsaved_changes(uri).await.unwrap());

        // Modify document
        document_provider
            .document_changed(uri, "fn main() { println!(\"test\"); }".to_string(), 2)
            .await;
        assert!(provider_trait.has_unsaved_changes(uri).await.unwrap());

        // Save document
        document_provider.document_saved(uri, None, 3).await;
        assert!(!provider_trait.has_unsaved_changes(uri).await.unwrap());

        // Close document
        document_provider.document_closed(uri).await;
        assert!(!document_provider.is_document_open(uri).await);
    }

    #[tokio::test]
    async fn test_cache_configuration() {
        // Test different cache layer configurations

        let temp_dir = TempDir::new().unwrap();
        let config = crate::workspace_cache_router::WorkspaceCacheRouterConfig {
            base_cache_dir: temp_dir.path().join("caches"),
            ..Default::default()
        };

        let registry = Arc::new(crate::lsp_registry::LspRegistry::new().unwrap());
        let child_processes = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let server_manager = Arc::new(
            crate::server_manager::SingleServerManager::new_with_tracker(registry, child_processes),
        );
        let workspace_router = Arc::new(crate::workspace_cache_router::WorkspaceCacheRouter::new(
            config,
            server_manager,
        ));
        let universal_cache = Arc::new(UniversalCache::new(workspace_router).await.unwrap());

        // Test with cache warming enabled
        let config_with_warming = CacheLayerConfig {
            cache_warming_enabled: true,
            cache_warming_concurrency: 8,
            singleflight_timeout: std::time::Duration::from_secs(60),
            detailed_metrics: true,
            workspace_revision_ttl: std::time::Duration::from_secs(120),
        };

        let cache_layer = CacheLayer::new(universal_cache.clone(), None, Some(config_with_warming));
        let stats = cache_layer.get_stats().await.unwrap();
        assert!(stats.cache_warming_enabled);

        // Test with cache warming disabled
        let config_no_warming = CacheLayerConfig {
            cache_warming_enabled: false,
            ..Default::default()
        };

        let cache_layer = CacheLayer::new(universal_cache, None, Some(config_no_warming));
        let stats = cache_layer.get_stats().await.unwrap();
        assert!(!stats.cache_warming_enabled);
    }
}

pub mod usage_examples {
    //! Usage documentation and examples
    //!
    //! # Usage Examples for Cache Layer Middleware
    //!
    //! ## Basic Integration
    //!
    //! ```rust,ignore
    //! use lsp_daemon::universal_cache::{CacheLayer, UniversalCache, DocumentProviderFactory};
    //! use std::sync::Arc;
    //!
    //! // 1. Create universal cache with workspace router
    //! let universal_cache = Arc::new(UniversalCache::new(workspace_router).await?);
    //!
    //! // 2. Create document provider
    //! let document_provider = DocumentProviderFactory::create_daemon_provider(Some(workspace_resolver));
    //!
    //! // 3. Create cache layer
    //! let cache_layer = CacheLayer::new(universal_cache, Some(document_provider), None);
    //!
    //! // 4. Handle requests with caching
    //! let response = cache_layer.handle_request(request, |req| async move {
    //!     // Your original LSP request handler here
    //!     handle_lsp_request(req).await
    //! }).await?;
    //! ```
    //!
    //! ## Advanced Configuration
    //!
    //! ```rust,ignore
    //! use lsp_daemon::universal_cache::{CacheLayer, CacheLayerConfig};
    //! use std::time::Duration;
    //!
    //! let config = CacheLayerConfig {
    //!     cache_warming_enabled: true,
    //!     cache_warming_concurrency: 8,
    //!     singleflight_timeout: Duration::from_secs(30),
    //!     detailed_metrics: true,
    //!     workspace_revision_ttl: Duration::from_secs(60),
    //! };
    //!
    //! let cache_layer = CacheLayer::new(universal_cache, document_provider, Some(config));
    //! ```
    //!
    //! ## Cache Management
    //!
    //! ```rust,ignore
    //! // Get cache statistics
    //! let stats = cache_layer.get_stats().await?;
    //! println!("Cache hit rate: {:.2}%", stats.cache_stats.hit_rate * 100.0);
    //!
    //! // Invalidate cache for a workspace
    //! let invalidated = cache_layer.invalidate_workspace(&workspace_path).await?;
    //! println!("Invalidated {} cache entries", invalidated);
    //!
    //! // Warm cache for frequently accessed files
    //! let warmed = cache_layer.warm_cache(&workspace_path, file_list).await?;
    //! println!("Warmed {} cache entries", warmed);
    //! ```
    //!
    //! ## Document Provider Integration
    //!
    //! ```rust,ignore
    //! // Notify document provider of document changes
    //! document_provider.document_opened("file:///path/to/file.rs", content, 1).await;
    //! document_provider.document_changed("file:///path/to/file.rs", new_content, 2).await;
    //! document_provider.document_saved("file:///path/to/file.rs", None, 3).await;
    //! document_provider.document_closed("file:///path/to/file.rs").await;
    //!
    //! // Cache layer will automatically check for unsaved changes
    //! // and bypass cache for modified files
    //! ```
}
