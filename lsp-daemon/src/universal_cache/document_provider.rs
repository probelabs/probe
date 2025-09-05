//! Document Provider Implementation for LSP Daemon
//!
//! This module provides document providers that integrate with the LSP daemon's
//! document management system to handle unsaved buffers and workspace resolution.

use super::{layer::FileSystemDocumentProvider, DocumentProvider};
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Document state tracking unsaved changes and content
#[derive(Debug, Clone)]
pub struct DocumentState {
    /// Current document content (if different from file)
    pub content: Option<String>,
    /// Whether the document has unsaved changes
    pub has_unsaved_changes: bool,
    /// Document version number (for change tracking)
    pub version: u64,
    /// Last modified timestamp
    pub last_modified: std::time::SystemTime,
}

impl Default for DocumentState {
    fn default() -> Self {
        Self {
            content: None,
            has_unsaved_changes: false,
            version: 0,
            last_modified: std::time::SystemTime::now(),
        }
    }
}

/// Document provider that tracks open documents and unsaved changes
/// This is designed to integrate with LSP document synchronization
pub struct DaemonDocumentProvider {
    /// Map of document URIs to their current state
    documents: Arc<RwLock<HashMap<String, DocumentState>>>,

    /// Workspace resolver for finding workspace roots
    workspace_resolver:
        Option<Arc<tokio::sync::Mutex<crate::workspace_resolver::WorkspaceResolver>>>,
}

impl DaemonDocumentProvider {
    /// Create a new daemon document provider
    pub fn new(
        workspace_resolver: Option<
            Arc<tokio::sync::Mutex<crate::workspace_resolver::WorkspaceResolver>>,
        >,
    ) -> Self {
        Self {
            documents: Arc::new(RwLock::new(HashMap::new())),
            workspace_resolver,
        }
    }

    /// Notify that a document was opened
    pub async fn document_opened(&self, uri: &str, content: String, version: u64) {
        let mut documents = self.documents.write().await;
        documents.insert(
            uri.to_string(),
            DocumentState {
                content: Some(content),
                has_unsaved_changes: false,
                version,
                last_modified: std::time::SystemTime::now(),
            },
        );
    }

    /// Notify that a document was changed
    pub async fn document_changed(&self, uri: &str, content: String, version: u64) {
        let mut documents = self.documents.write().await;
        let doc_state = documents.entry(uri.to_string()).or_default();
        doc_state.content = Some(content);
        doc_state.has_unsaved_changes = true;
        doc_state.version = version;
        doc_state.last_modified = std::time::SystemTime::now();
    }

    /// Notify that a document was saved
    pub async fn document_saved(&self, uri: &str, content: Option<String>, version: u64) {
        let mut documents = self.documents.write().await;
        if let Some(doc_state) = documents.get_mut(uri) {
            if let Some(content) = content {
                doc_state.content = Some(content);
            }
            doc_state.has_unsaved_changes = false;
            doc_state.version = version;
            doc_state.last_modified = std::time::SystemTime::now();
        }
    }

    /// Notify that a document was closed
    pub async fn document_closed(&self, uri: &str) {
        let mut documents = self.documents.write().await;
        documents.remove(uri);
    }

    /// Get all open documents
    pub async fn get_open_documents(&self) -> HashMap<String, DocumentState> {
        let documents = self.documents.read().await;
        documents.clone()
    }

    /// Check if a document is open
    pub async fn is_document_open(&self, uri: &str) -> bool {
        let documents = self.documents.read().await;
        documents.contains_key(uri)
    }

    /// Get document version
    pub async fn get_document_version(&self, uri: &str) -> Option<u64> {
        let documents = self.documents.read().await;
        documents.get(uri).map(|doc| doc.version)
    }

    /// Clear all document states (for testing or reset)
    pub async fn clear_all_documents(&self) {
        let mut documents = self.documents.write().await;
        documents.clear();
    }

    /// Get documents with unsaved changes
    pub async fn get_dirty_documents(&self) -> Vec<String> {
        let documents = self.documents.read().await;
        documents
            .iter()
            .filter_map(|(uri, state)| {
                if state.has_unsaved_changes {
                    Some(uri.clone())
                } else {
                    None
                }
            })
            .collect()
    }
}

#[async_trait]
impl DocumentProvider for DaemonDocumentProvider {
    async fn get_document_content(&self, uri: &str) -> Result<Option<String>> {
        let documents = self.documents.read().await;

        if let Some(doc_state) = documents.get(uri) {
            if let Some(content) = &doc_state.content {
                return Ok(Some(content.clone()));
            }
        }

        // Fall back to reading from filesystem
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

    async fn has_unsaved_changes(&self, uri: &str) -> Result<bool> {
        let documents = self.documents.read().await;

        if let Some(doc_state) = documents.get(uri) {
            Ok(doc_state.has_unsaved_changes)
        } else {
            // Document not open, assume no unsaved changes
            Ok(false)
        }
    }

    async fn get_workspace_root(&self, uri: &str) -> Result<Option<PathBuf>> {
        // Convert URI to path
        let file_path = if uri.starts_with("file://") {
            PathBuf::from(uri.strip_prefix("file://").unwrap())
        } else {
            return Ok(None);
        };

        // Use workspace resolver if available
        if let Some(resolver) = &self.workspace_resolver {
            let mut resolver = resolver.lock().await;
            match resolver.resolve_workspace(&file_path, None) {
                Ok(workspace_root) => Ok(Some(workspace_root)),
                Err(_) => Ok(None),
            }
        } else {
            Ok(None)
        }
    }
}

/// Factory for creating document providers
pub struct DocumentProviderFactory;

impl DocumentProviderFactory {
    /// Create a daemon document provider with workspace resolver integration
    pub fn create_daemon_provider(
        workspace_resolver: Option<
            Arc<tokio::sync::Mutex<crate::workspace_resolver::WorkspaceResolver>>,
        >,
    ) -> Arc<DaemonDocumentProvider> {
        Arc::new(DaemonDocumentProvider::new(workspace_resolver))
    }

    /// Create a filesystem-only document provider (for testing or simple cases)
    pub fn create_filesystem_provider() -> Arc<dyn DocumentProvider> {
        Arc::new(FileSystemDocumentProvider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_document_lifecycle() {
        let provider = DaemonDocumentProvider::new(None);

        let uri = "file:///test/document.rs";
        let initial_content = "fn main() {}";
        let modified_content = "fn main() { println!(\"Hello!\"); }";

        // Document not open initially
        assert!(!provider.is_document_open(uri).await);
        assert!(!provider.has_unsaved_changes(uri).await.unwrap());

        // Open document
        provider
            .document_opened(uri, initial_content.to_string(), 1)
            .await;
        assert!(provider.is_document_open(uri).await);
        assert!(!provider.has_unsaved_changes(uri).await.unwrap());
        assert_eq!(provider.get_document_version(uri).await, Some(1));

        // Verify content
        let content = provider.get_document_content(uri).await.unwrap();
        assert_eq!(content, Some(initial_content.to_string()));

        // Modify document
        provider
            .document_changed(uri, modified_content.to_string(), 2)
            .await;
        assert!(provider.has_unsaved_changes(uri).await.unwrap());
        assert_eq!(provider.get_document_version(uri).await, Some(2));

        // Verify modified content
        let content = provider.get_document_content(uri).await.unwrap();
        assert_eq!(content, Some(modified_content.to_string()));

        // Check dirty documents
        let dirty_docs = provider.get_dirty_documents().await;
        assert_eq!(dirty_docs.len(), 1);
        assert_eq!(dirty_docs[0], uri);

        // Save document
        provider.document_saved(uri, None, 3).await;
        assert!(!provider.has_unsaved_changes(uri).await.unwrap());
        assert_eq!(provider.get_document_version(uri).await, Some(3));

        // No more dirty documents
        let dirty_docs = provider.get_dirty_documents().await;
        assert_eq!(dirty_docs.len(), 0);

        // Close document
        provider.document_closed(uri).await;
        assert!(!provider.is_document_open(uri).await);
        assert_eq!(provider.get_document_version(uri).await, None);
    }

    #[tokio::test]
    async fn test_multiple_documents() {
        let provider = DaemonDocumentProvider::new(None);

        let uri1 = "file:///test/doc1.rs";
        let uri2 = "file:///test/doc2.rs";
        let uri3 = "file:///test/doc3.rs";

        // Open multiple documents
        provider
            .document_opened(uri1, "content1".to_string(), 1)
            .await;
        provider
            .document_opened(uri2, "content2".to_string(), 1)
            .await;
        provider
            .document_opened(uri3, "content3".to_string(), 1)
            .await;

        // Verify all are open
        assert_eq!(provider.get_open_documents().await.len(), 3);

        // Modify some documents
        provider
            .document_changed(uri1, "modified1".to_string(), 2)
            .await;
        provider
            .document_changed(uri3, "modified3".to_string(), 2)
            .await;

        // Check dirty documents
        let mut dirty_docs = provider.get_dirty_documents().await;
        dirty_docs.sort();
        assert_eq!(dirty_docs.len(), 2);
        assert!(dirty_docs.contains(&uri1.to_string()));
        assert!(dirty_docs.contains(&uri3.to_string()));

        // Save one document
        provider.document_saved(uri1, None, 3).await;

        // Should have one fewer dirty document
        let dirty_docs = provider.get_dirty_documents().await;
        assert_eq!(dirty_docs.len(), 1);
        assert_eq!(dirty_docs[0], uri3);

        // Clear all documents
        provider.clear_all_documents().await;
        assert_eq!(provider.get_open_documents().await.len(), 0);
        assert_eq!(provider.get_dirty_documents().await.len(), 0);
    }

    #[tokio::test]
    async fn test_document_versions() {
        let provider = DaemonDocumentProvider::new(None);

        let uri = "file:///test/versioned.rs";

        // Open document with version 1
        provider.document_opened(uri, "v1".to_string(), 1).await;
        assert_eq!(provider.get_document_version(uri).await, Some(1));

        // Multiple changes with increasing versions
        provider.document_changed(uri, "v2".to_string(), 2).await;
        assert_eq!(provider.get_document_version(uri).await, Some(2));

        provider.document_changed(uri, "v3".to_string(), 3).await;
        assert_eq!(provider.get_document_version(uri).await, Some(3));

        // Save with version 4
        provider
            .document_saved(uri, Some("v4".to_string()), 4)
            .await;
        assert_eq!(provider.get_document_version(uri).await, Some(4));

        // Content should be v4
        let content = provider.get_document_content(uri).await.unwrap();
        assert_eq!(content, Some("v4".to_string()));
    }

    #[tokio::test]
    async fn test_timestamp_tracking() {
        let provider = DaemonDocumentProvider::new(None);

        let uri = "file:///test/timestamped.rs";

        // Open document
        let before_open = std::time::SystemTime::now();
        provider
            .document_opened(uri, "initial".to_string(), 1)
            .await;

        // Small delay
        sleep(Duration::from_millis(10)).await;

        // Modify document
        let before_change = std::time::SystemTime::now();
        provider
            .document_changed(uri, "changed".to_string(), 2)
            .await;

        // Verify timestamps are reasonable (within last few seconds)
        let documents = provider.get_open_documents().await;
        let doc_state = documents.get(uri).unwrap();

        let now = std::time::SystemTime::now();
        let age = now.duration_since(doc_state.last_modified).unwrap();
        assert!(age.as_secs() < 5); // Should be very recent

        // Last modified should be after the change
        assert!(doc_state.last_modified >= before_change);
        assert!(doc_state.last_modified > before_open);
    }

    #[tokio::test]
    async fn test_document_provider_factory() {
        // Test filesystem provider creation
        let fs_provider = DocumentProviderFactory::create_filesystem_provider();

        // Should work with non-existent files (returns None)
        let content = fs_provider
            .get_document_content("file:///nonexistent.rs")
            .await
            .unwrap();
        assert_eq!(content, None);

        // Should report no unsaved changes for filesystem provider
        let unsaved = fs_provider
            .has_unsaved_changes("file:///any.rs")
            .await
            .unwrap();
        assert!(!unsaved);

        // Test daemon provider creation
        let daemon_provider = DocumentProviderFactory::create_daemon_provider(None);

        // Should start with no open documents
        assert_eq!(daemon_provider.get_open_documents().await.len(), 0);
    }

    #[tokio::test]
    async fn test_uri_parsing() {
        let provider = DaemonDocumentProvider::new(None);

        // Test file URI
        let file_uri = "file:///usr/local/test.rs";
        let content = provider.get_document_content(file_uri).await.unwrap();
        // Should return None for non-existent file without error
        assert_eq!(content, None);

        // Test non-file URI
        let http_uri = "http://example.com/test.rs";
        let content = provider.get_document_content(http_uri).await.unwrap();
        assert_eq!(content, None);
    }
}
