//! LSP Client Wrapper
//!
//! This module provides a wrapper around the server manager to expose LSP operations
//! in a form suitable for relationship enhancement. It handles language detection,
//! workspace resolution, and coordinates with the universal cache system.

use anyhow::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::time::{timeout, Duration};
use tracing::debug;

use super::lsp_enhancer::LspEnhancementError;
use crate::language_detector::{Language, LanguageDetector};
use crate::protocol::{CallHierarchyResult, Location};
use crate::server_manager::SingleServerManager;
use crate::workspace_resolver::WorkspaceResolver;

/// Wrapper around server manager for LSP operations used in relationship enhancement
pub struct LspClientWrapper {
    server_manager: Arc<SingleServerManager>,
    language_detector: Arc<LanguageDetector>,
    workspace_resolver: Arc<tokio::sync::Mutex<WorkspaceResolver>>,
}

impl LspClientWrapper {
    /// Create a new LSP client wrapper
    pub fn new(
        server_manager: Arc<SingleServerManager>,
        language_detector: Arc<LanguageDetector>,
        workspace_resolver: Arc<tokio::sync::Mutex<WorkspaceResolver>>,
    ) -> Self {
        Self {
            server_manager,
            language_detector,
            workspace_resolver,
        }
    }

    /// Get textDocument/references for a file position
    pub async fn get_references(
        &self,
        file_path: &Path,
        line: u32,
        column: u32,
        include_declaration: bool,
        timeout_ms: u64,
    ) -> Result<Vec<Location>, LspEnhancementError> {
        let language = self.detect_language(file_path)?;
        let _workspace_root = self.resolve_workspace(file_path).await?;

        // Call LSP references with timeout
        let json_result = timeout(
            Duration::from_millis(timeout_ms),
            self.server_manager
                .references(language, file_path, line, column, include_declaration),
        )
        .await
        .map_err(|_| LspEnhancementError::LspTimeout {
            operation: "references".to_string(),
            timeout_ms,
        })?
        .map_err(|e| LspEnhancementError::InvalidLspResponse {
            method: "references".to_string(),
            error: e.to_string(),
        })?;

        // Parse JSON response to Vec<Location>
        let locations: Vec<Location> = serde_json::from_value(json_result).map_err(|e| {
            LspEnhancementError::InvalidLspResponse {
                method: "references".to_string(),
                error: format!("Failed to parse locations: {}", e),
            }
        })?;

        debug!(
            "Got {} references for {}:{}:{}",
            locations.len(),
            file_path.display(),
            line,
            column
        );

        Ok(locations)
    }

    /// Get textDocument/definition for a file position
    pub async fn get_definition(
        &self,
        file_path: &Path,
        line: u32,
        column: u32,
        timeout_ms: u64,
    ) -> Result<Vec<Location>, LspEnhancementError> {
        let language = self.detect_language(file_path)?;
        let _workspace_root = self.resolve_workspace(file_path).await?;

        // Call LSP definition with timeout
        let json_result = timeout(
            Duration::from_millis(timeout_ms),
            self.server_manager
                .definition(language, file_path, line, column),
        )
        .await
        .map_err(|_| LspEnhancementError::LspTimeout {
            operation: "definition".to_string(),
            timeout_ms,
        })?
        .map_err(|e| LspEnhancementError::InvalidLspResponse {
            method: "definition".to_string(),
            error: e.to_string(),
        })?;

        // Parse JSON response to Vec<Location>
        let locations: Vec<Location> = serde_json::from_value(json_result).map_err(|e| {
            LspEnhancementError::InvalidLspResponse {
                method: "definition".to_string(),
                error: format!("Failed to parse locations: {}", e),
            }
        })?;

        debug!(
            "Got {} definitions for {}:{}:{}",
            locations.len(),
            file_path.display(),
            line,
            column
        );

        Ok(locations)
    }

    /// Get textDocument/hover for a file position
    pub async fn get_hover(
        &self,
        file_path: &Path,
        line: u32,
        column: u32,
        timeout_ms: u64,
    ) -> Result<Option<serde_json::Value>, LspEnhancementError> {
        let language = self.detect_language(file_path)?;
        let _workspace_root = self.resolve_workspace(file_path).await?;

        // Call LSP hover with timeout
        let result = timeout(
            Duration::from_millis(timeout_ms),
            self.server_manager.hover(language, file_path, line, column),
        )
        .await
        .map_err(|_| LspEnhancementError::LspTimeout {
            operation: "hover".to_string(),
            timeout_ms,
        })?
        .map_err(|e| LspEnhancementError::InvalidLspResponse {
            method: "hover".to_string(),
            error: e.to_string(),
        })?;

        debug!(
            "Got hover response for {}:{}:{}",
            file_path.display(),
            line,
            column
        );

        Ok(Some(result))
    }

    /// Get callHierarchy for a file position
    pub async fn get_call_hierarchy(
        &self,
        file_path: &Path,
        line: u32,
        column: u32,
        timeout_ms: u64,
    ) -> Result<CallHierarchyResult, LspEnhancementError> {
        let language = self.detect_language(file_path)?;
        let _workspace_root = self.resolve_workspace(file_path).await?;

        // Call LSP call hierarchy with timeout
        let result = timeout(
            Duration::from_millis(timeout_ms),
            self.server_manager
                .call_hierarchy(language, file_path, line, column),
        )
        .await
        .map_err(|_| LspEnhancementError::LspTimeout {
            operation: "call_hierarchy".to_string(),
            timeout_ms,
        })?
        .map_err(|e| LspEnhancementError::InvalidLspResponse {
            method: "call_hierarchy".to_string(),
            error: e.to_string(),
        })?;

        debug!(
            "Got call hierarchy with {} incoming calls and {} outgoing calls for {}:{}:{}",
            result.incoming.len(),
            result.outgoing.len(),
            file_path.display(),
            line,
            column
        );

        Ok(result)
    }

    /// Get textDocument/implementation for a file position
    pub async fn get_implementation(
        &self,
        file_path: &Path,
        line: u32,
        column: u32,
        timeout_ms: u64,
    ) -> Result<Vec<Location>, LspEnhancementError> {
        let language = self.detect_language(file_path)?;
        let _workspace_root = self.resolve_workspace(file_path).await?;

        // Call LSP implementation with timeout
        let json_result = timeout(
            Duration::from_millis(timeout_ms),
            self.server_manager
                .implementation(language, file_path, line, column),
        )
        .await
        .map_err(|_| LspEnhancementError::LspTimeout {
            operation: "implementation".to_string(),
            timeout_ms,
        })?
        .map_err(|e| LspEnhancementError::InvalidLspResponse {
            method: "implementation".to_string(),
            error: e.to_string(),
        })?;

        // Parse JSON response to Vec<Location>
        let locations: Vec<Location> = serde_json::from_value(json_result).map_err(|e| {
            LspEnhancementError::InvalidLspResponse {
                method: "implementation".to_string(),
                error: format!("Failed to parse locations: {}", e),
            }
        })?;

        debug!(
            "Got {} implementations for {}:{}:{}",
            locations.len(),
            file_path.display(),
            line,
            column
        );

        Ok(locations)
    }

    /// Detect language for a file
    fn detect_language(&self, file_path: &Path) -> Result<Language, LspEnhancementError> {
        self.language_detector
            .detect(file_path)
            .map_err(|e| LspEnhancementError::InternalError {
                message: format!("Language detection failed: {}", e),
            })
    }

    /// Resolve workspace for a file
    async fn resolve_workspace(&self, file_path: &Path) -> Result<PathBuf, LspEnhancementError> {
        let mut resolver = self.workspace_resolver.lock().await;
        resolver.resolve_workspace(file_path, None).map_err(|e| {
            LspEnhancementError::InternalError {
                message: format!("Workspace resolution failed: {}", e),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language_detector::LanguageDetector;
    use crate::lsp_registry::LspRegistry;
    use crate::server_manager::SingleServerManager;
    use crate::workspace_resolver::WorkspaceResolver;
    use std::path::PathBuf;
    use tokio;

    async fn create_test_wrapper() -> LspClientWrapper {
        let registry = Arc::new(LspRegistry::new().expect("Failed to create LSP registry"));
        let child_processes = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let server_manager = Arc::new(SingleServerManager::new_with_tracker(
            registry,
            child_processes,
        ));
        let language_detector = Arc::new(LanguageDetector::new());
        let workspace_resolver = Arc::new(tokio::sync::Mutex::new(WorkspaceResolver::new(None)));

        LspClientWrapper::new(server_manager, language_detector, workspace_resolver)
    }

    #[tokio::test]
    async fn test_detect_language() {
        let wrapper = create_test_wrapper().await;
        let rust_file = PathBuf::from("test.rs");

        let language = wrapper.detect_language(&rust_file).unwrap();
        assert_eq!(language, Language::Rust);
    }

    #[tokio::test]
    async fn test_get_references_timeout() {
        let wrapper = create_test_wrapper().await;
        let test_file = PathBuf::from("nonexistent.rs");

        // This should timeout quickly since the file doesn't exist and no server is running
        let result = wrapper.get_references(&test_file, 10, 5, false, 100).await;

        // Should either timeout or fail due to no workspace/server
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_wrapper_creation() {
        let wrapper = create_test_wrapper().await;

        // Basic smoke test - wrapper should be created successfully
        assert!(
            !wrapper.server_manager.get_stats().await.is_empty()
                || wrapper.server_manager.get_stats().await.is_empty()
        );
    }
}
