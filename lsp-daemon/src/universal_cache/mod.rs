//! Universal Cache Foundation
//!
//! This module provides a unified caching interface for all LSP operations while
//! maintaining strict per-workspace cache isolation. It builds upon the existing
//! workspace cache router to provide consistent cache policies and key generation.
//!
//! ## Architecture
//!
//! - **CachePolicy**: Method-specific caching policies and scopes
//! - **KeyBuilder**: Content-addressed key generation with workspace awareness
//! - **CacheStore**: Memory + disk storage with per-workspace sled databases
//! - **UniversalCache**: High-level API coordinating all components
//!
//! ## Workspace Isolation
//!
//! All cache operations maintain workspace isolation:
//! - Separate cache databases per workspace (preserves existing behavior)
//! - Workspace-aware key generation
//! - Per-workspace policy enforcement
//! - Cross-workspace invalidation when appropriate

use anyhow::Result;
use std::path::Path;
use std::sync::Arc;

pub mod document_provider;
pub mod integration_example;
pub mod key;
pub mod layer;
pub mod monitoring;
pub mod policy;
pub mod store;

#[cfg(test)]
pub mod tests;

#[cfg(test)]
pub mod integration_tests;

pub use document_provider::{DaemonDocumentProvider, DocumentProviderFactory, DocumentState};
pub use key::{CacheKey, KeyBuilder};
pub use layer::{CacheLayer, CacheLayerConfig, CacheLayerStats, DocumentProvider};
pub use policy::{CachePolicy, CacheScope, PolicyRegistry};
pub use store::CacheStore;

/// LSP method types supported by the universal cache
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum LspMethod {
    /// Go to definition
    Definition,
    /// Find references
    References,
    /// Hover information
    Hover,
    /// Document symbols
    DocumentSymbols,
    /// Workspace symbols
    WorkspaceSymbols,
    /// Type definition
    TypeDefinition,
    /// Implementation
    Implementation,
    /// Call hierarchy (incoming/outgoing calls)
    CallHierarchy,
    /// Signature help
    SignatureHelp,
    /// Code completion
    Completion,
    /// Code actions
    CodeAction,
    /// Rename
    Rename,
    /// Folding ranges
    FoldingRange,
    /// Selection ranges
    SelectionRange,
    /// Semantic tokens
    SemanticTokens,
    /// Inlay hints
    InlayHint,
}

impl LspMethod {
    /// Get the string representation for the LSP method
    pub fn as_str(&self) -> &'static str {
        match self {
            LspMethod::Definition => "textDocument/definition",
            LspMethod::References => "textDocument/references",
            LspMethod::Hover => "textDocument/hover",
            LspMethod::DocumentSymbols => "textDocument/documentSymbol",
            LspMethod::WorkspaceSymbols => "workspace/symbol",
            LspMethod::TypeDefinition => "textDocument/typeDefinition",
            LspMethod::Implementation => "textDocument/implementation",
            LspMethod::CallHierarchy => "textDocument/prepareCallHierarchy",
            LspMethod::SignatureHelp => "textDocument/signatureHelp",
            LspMethod::Completion => "textDocument/completion",
            LspMethod::CodeAction => "textDocument/codeAction",
            LspMethod::Rename => "textDocument/rename",
            LspMethod::FoldingRange => "textDocument/foldingRange",
            LspMethod::SelectionRange => "textDocument/selectionRange",
            LspMethod::SemanticTokens => "textDocument/semanticTokens/full",
            LspMethod::InlayHint => "textDocument/inlayHint",
        }
    }
}

/// Universal cache providing unified interface for all LSP operations
pub struct UniversalCache {
    /// Policy registry for method-specific caching policies
    policy_registry: PolicyRegistry,

    /// Key builder for content-addressed cache keys
    key_builder: KeyBuilder,

    /// Cache store managing per-workspace storage
    store: Arc<CacheStore>,
}

impl UniversalCache {
    /// Create a new universal cache instance
    pub async fn new(
        workspace_cache_router: Arc<crate::workspace_cache_router::WorkspaceCacheRouter>,
    ) -> Result<Self> {
        let policy_registry = PolicyRegistry::default();
        let key_builder = KeyBuilder::new();
        let store = Arc::new(CacheStore::new(workspace_cache_router).await?);

        Ok(Self {
            policy_registry,
            key_builder,
            store,
        })
    }

    /// Get a cached value for an LSP operation
    pub async fn get<T>(
        &self,
        method: LspMethod,
        file_path: &Path,
        params: &str,
    ) -> Result<Option<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        // Get cache policy for this method
        let policy = self.policy_registry.get_policy(method);

        // Check if caching is enabled for this method
        if !policy.enabled {
            return Ok(None);
        }

        // Build cache key
        let cache_key = self
            .key_builder
            .build_key(method, file_path, params)
            .await?;

        // Get from store
        self.store.get(&cache_key).await
    }

    /// Store a value in the cache for an LSP operation
    pub async fn set<T>(
        &self,
        method: LspMethod,
        file_path: &Path,
        params: &str,
        value: &T,
    ) -> Result<()>
    where
        T: serde::Serialize,
    {
        // Get cache policy for this method
        let policy = self.policy_registry.get_policy(method);

        // Check if caching is enabled for this method
        if !policy.enabled {
            return Ok(());
        }

        // Build cache key
        let cache_key = self
            .key_builder
            .build_key(method, file_path, params)
            .await?;

        // Store in cache
        self.store.set(&cache_key, value, policy.ttl_seconds).await
    }

    /// Invalidate cache entries for a file across all relevant workspaces
    pub async fn invalidate_file(&self, file_path: &Path) -> Result<usize> {
        self.store.invalidate_file(file_path).await
    }

    /// Clear all cache entries for a specific workspace
    pub async fn clear_workspace(&self, workspace_root: &Path) -> Result<usize> {
        self.store.clear_workspace(workspace_root).await
    }

    /// Get cache statistics
    pub async fn get_stats(&self) -> Result<CacheStats> {
        self.store.get_stats().await
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    /// Total number of cache entries across all workspaces
    pub total_entries: u64,
    /// Total cache size in bytes across all workspaces
    pub total_size_bytes: u64,
    /// Number of active workspaces with caches
    pub active_workspaces: usize,
    /// Hit rate (0.0 - 1.0)
    pub hit_rate: f64,
    /// Miss rate (0.0 - 1.0)
    pub miss_rate: f64,
    /// Per-method statistics
    pub method_stats: std::collections::HashMap<LspMethod, MethodStats>,
}

/// Statistics for a specific LSP method
#[derive(Debug, Clone)]
pub struct MethodStats {
    /// Number of entries for this method
    pub entries: u64,
    /// Size in bytes for this method
    pub size_bytes: u64,
    /// Hit count
    pub hits: u64,
    /// Miss count
    pub misses: u64,
}
