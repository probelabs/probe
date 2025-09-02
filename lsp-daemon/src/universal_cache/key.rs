//! Content-Addressed Cache Key Generation
//!
//! This module provides workspace-aware cache key generation using Blake3 hashing
//! for consistent, collision-resistant cache keys.

use crate::universal_cache::LspMethod;
use anyhow::{Context, Result};
use blake3::Hasher;
use dashmap::DashMap;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tokio::fs;

/// Global workspace resolution cache to eliminate race conditions
/// Key: Canonical file path, Value: (workspace_root, workspace_id)
static WORKSPACE_RESOLUTION_CACHE: Lazy<DashMap<PathBuf, (PathBuf, String)>> =
    Lazy::new(|| DashMap::new());

/// A content-addressed cache key
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CacheKey {
    /// Workspace-relative file path for portability
    pub workspace_relative_path: PathBuf,

    /// LSP method being cached
    pub method: LspMethod,

    /// Blake3 hash of the cache key components
    pub content_hash: String,

    /// Workspace identifier for routing
    pub workspace_id: String,

    /// File modification time (for quick staleness checks)
    pub file_mtime: u64,

    /// Optional symbol name extracted from response (for display purposes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_name: Option<String>,

    /// Optional position info (line:column) for display
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<String>,
}

impl CacheKey {
    /// Create a cache key string representation for storage
    pub fn to_storage_key(&self) -> String {
        // Include symbol name in the key if available for easier debugging/display
        if let Some(ref symbol_name) = self.symbol_name {
            format!(
                "{}:{}:{}:{}:{}",
                self.workspace_id,
                self.method.as_str().replace('/', "_"),
                self.workspace_relative_path.to_string_lossy(),
                self.content_hash,
                symbol_name
            )
        } else {
            format!(
                "{}:{}:{}:{}",
                self.workspace_id,
                self.method.as_str().replace('/', "_"),
                self.workspace_relative_path.to_string_lossy(),
                self.content_hash
            )
        }
    }

    /// Parse a cache key from its storage representation
    pub fn from_storage_key(key: &str) -> Option<Self> {
        let parts: Vec<&str> = key.splitn(5, ':').collect();
        if parts.len() < 4 {
            return None;
        }

        let workspace_id = parts[0].to_string();
        let method_str = parts[1].replace('_', "/");
        let workspace_relative_path = PathBuf::from(parts[2]);
        let content_hash = parts[3].to_string();
        let symbol_name = if parts.len() == 5 {
            Some(parts[4].to_string())
        } else {
            None
        };

        // Parse method from string
        let method = match method_str.as_str() {
            "textDocument/definition" => LspMethod::Definition,
            "textDocument/references" => LspMethod::References,
            "textDocument/hover" => LspMethod::Hover,
            "textDocument/documentSymbol" => LspMethod::DocumentSymbols,
            "workspace/symbol" => LspMethod::WorkspaceSymbols,
            "textDocument/typeDefinition" => LspMethod::TypeDefinition,
            "textDocument/implementation" => LspMethod::Implementation,
            "textDocument/prepareCallHierarchy" => LspMethod::CallHierarchy,
            "textDocument/signatureHelp" => LspMethod::SignatureHelp,
            "textDocument/completion" => LspMethod::Completion,
            "textDocument/codeAction" => LspMethod::CodeAction,
            "textDocument/rename" => LspMethod::Rename,
            "textDocument/foldingRange" => LspMethod::FoldingRange,
            "textDocument/selectionRange" => LspMethod::SelectionRange,
            "textDocument/semanticTokens/full" => LspMethod::SemanticTokens,
            "textDocument/inlayHint" => LspMethod::InlayHint,
            _ => return None,
        };

        Some(Self {
            workspace_relative_path,
            method,
            content_hash,
            workspace_id,
            file_mtime: 0, // Will need to be populated separately
            symbol_name,
            position: None, // Will need to be populated separately
        })
    }
}

/// Cache key builder with workspace awareness
pub struct KeyBuilder {
    /// Hasher instance for key generation
    hasher_pool: std::sync::Mutex<Vec<Hasher>>,
    /// Centralized workspace resolver for consistent workspace detection
    workspace_resolver:
        Option<std::sync::Arc<tokio::sync::Mutex<crate::workspace_resolver::WorkspaceResolver>>>,
}

impl Clone for KeyBuilder {
    fn clone(&self) -> Self {
        Self {
            hasher_pool: std::sync::Mutex::new(Vec::new()),
            workspace_resolver: self.workspace_resolver.clone(),
        }
    }
}

impl KeyBuilder {
    /// Create a new key builder without workspace resolver (for testing)
    pub fn new() -> Self {
        Self {
            hasher_pool: std::sync::Mutex::new(Vec::new()),
            workspace_resolver: None,
        }
    }

    /// Build a synchronous singleflight key for immediate deduplication (no async I/O)
    pub fn build_singleflight_key(
        &self,
        method: LspMethod,
        file_path: &Path,
        params: &str,
    ) -> String {
        // Use synchronous operations only for immediate deduplication
        let canonical_path = file_path
            .canonicalize()
            .unwrap_or_else(|_| file_path.to_path_buf());

        // Create a simple hash without file I/O
        format!(
            "sf_{}:{}:{}",
            method.as_str().replace('/', "_"),
            canonical_path.display(),
            blake3::hash(params.as_bytes()).to_hex()
        )
    }

    /// Create a new key builder with workspace resolver integration
    pub fn new_with_workspace_resolver(
        workspace_resolver: std::sync::Arc<
            tokio::sync::Mutex<crate::workspace_resolver::WorkspaceResolver>,
        >,
    ) -> Self {
        Self {
            hasher_pool: std::sync::Mutex::new(Vec::new()),
            workspace_resolver: Some(workspace_resolver),
        }
    }

    /// Build a content-addressed cache key
    pub async fn build_key(
        &self,
        method: LspMethod,
        file_path: &Path,
        params: &str,
    ) -> Result<CacheKey> {
        // Canonicalize the file path
        let canonical_file_path = self.canonicalize_file_path(file_path)?;

        // Get file modification time
        let file_mtime = self.get_file_mtime(&canonical_file_path).await?;

        // Resolve workspace for this file deterministically
        let (workspace_root, workspace_id) = self
            .resolve_workspace_deterministic(&canonical_file_path)
            .await?;

        // Calculate workspace-relative path
        let workspace_relative_path =
            self.get_workspace_relative_path(&canonical_file_path, &workspace_root)?;

        // Generate content hash based on file metadata (no file content reading)
        // This makes cache key generation much faster for singleflight deduplication
        let content_hash = self
            .generate_fast_content_hash(
                method,
                &workspace_relative_path,
                params,
                file_mtime,
                &canonical_file_path,
            )
            .await?;

        // Extract position from params for display
        let position = Self::extract_position_from_params(params);

        let cache_key = CacheKey {
            workspace_relative_path,
            method,
            content_hash: content_hash.clone(),
            workspace_id: workspace_id.clone(),
            file_mtime,
            symbol_name: None, // Will be populated when we have symbol info
            position,
        };

        eprintln!(
            "DEBUG: Generated cache key for {}: storage_key={}, content_hash={}, mtime={}",
            file_path.display(),
            cache_key.to_storage_key(),
            content_hash,
            file_mtime
        );

        Ok(cache_key)
    }

    /// Generate server fingerprint for LSP server state
    pub async fn generate_server_fingerprint(
        &self,
        language: &str,
        server_version: &str,
        workspace_root: &Path,
    ) -> Result<String> {
        let mut hasher = self.get_hasher().await;

        hasher.update(b"server_fingerprint:");
        hasher.update(language.as_bytes());
        hasher.update(b":");
        hasher.update(server_version.as_bytes());
        hasher.update(b":");
        hasher.update(workspace_root.to_string_lossy().as_bytes());

        // Make fingerprint commit-aware if this is a Git workspace.
        // This keeps the same signature but yields better isolation between commits.
        // Non-git workspaces will simply skip this and produce the same value as before.
        if let Ok(svc) =
            crate::git_service::GitService::discover_repo(workspace_root, workspace_root)
        {
            if let Ok(Some(head)) = svc.head_commit() {
                hasher.update(b":");
                hasher.update(head.as_bytes());
            }
        }

        let hash = hasher.finalize();
        self.return_hasher(hasher).await;

        Ok(hash.to_hex().to_string())
    }

    /// Check if a cache key is still valid (not stale)
    pub async fn is_key_valid(&self, key: &CacheKey, file_path: &Path) -> Result<bool> {
        // Check if file still exists
        if !file_path.exists() {
            return Ok(false);
        }

        // Check file modification time
        let current_mtime = self.get_file_mtime(file_path).await?;
        if current_mtime != key.file_mtime {
            return Ok(false);
        }

        // If we get here, the key is still valid
        Ok(true)
    }

    /// Extract position information from LSP params for display
    fn extract_position_from_params(params: &str) -> Option<String> {
        // Try to parse JSON params and extract position
        if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(params) {
            if let Some(position) = parsed.get("position") {
                let line = position.get("line").and_then(|l| l.as_u64()).unwrap_or(0);
                let character = position
                    .get("character")
                    .and_then(|c| c.as_u64())
                    .unwrap_or(0);
                return Some(format!("{}:{}", line + 1, character + 1)); // Convert to 1-based for display
            }
        }
        None
    }

    // === Private Implementation ===

    /// Canonicalize file path with error handling
    fn canonicalize_file_path(&self, file_path: &Path) -> Result<PathBuf> {
        file_path
            .canonicalize()
            .or_else(|_| -> Result<PathBuf> { Ok(file_path.to_path_buf()) })
            .context("Failed to canonicalize file path")
    }

    /// Get file modification time as Unix timestamp
    async fn get_file_mtime(&self, file_path: &Path) -> Result<u64> {
        let metadata = fs::metadata(file_path)
            .await
            .context("Failed to get file metadata")?;

        let mtime = metadata
            .modified()
            .context("Failed to get file modification time")?;

        let duration = mtime
            .duration_since(SystemTime::UNIX_EPOCH)
            .context("Invalid file modification time")?;

        // Use nanosecond precision to detect rapid file changes
        Ok(duration.as_nanos() as u64)
    }

    /// Resolve workspace root and ID for a file
    async fn resolve_workspace(&self, file_path: &Path) -> Result<(PathBuf, String)> {
        let workspace_root = if let Some(ref resolver) = self.workspace_resolver {
            // Use centralized workspace resolver for consistent detection
            let mut resolver = resolver.lock().await;
            resolver.resolve_workspace_for_file(file_path)?
        } else {
            // Fallback to local implementation for backward compatibility
            self.find_workspace_root_fallback(file_path).await?
        };
        let workspace_id = self.generate_workspace_id(&workspace_root).await?;

        Ok((workspace_root, workspace_id))
    }

    /// Resolve workspace deterministically with caching to eliminate race conditions
    async fn resolve_workspace_deterministic(&self, file_path: &Path) -> Result<(PathBuf, String)> {
        // Check cache first to avoid async races
        if let Some(cached) = WORKSPACE_RESOLUTION_CACHE.get(file_path) {
            eprintln!(
                "DEBUG: Workspace cache HIT for {}: {:?}",
                file_path.display(),
                cached.value()
            );
            return Ok(cached.clone());
        }

        eprintln!("DEBUG: Workspace cache MISS for {}", file_path.display());

        // Resolve workspace using existing logic
        let result = self.resolve_workspace(file_path).await?;

        eprintln!(
            "DEBUG: Resolved workspace for {}: {:?}",
            file_path.display(),
            result
        );

        // Cache the result for future requests
        WORKSPACE_RESOLUTION_CACHE.insert(file_path.to_path_buf(), result.clone());

        Ok(result)
    }

    /// Find workspace root by walking up directory tree (fallback implementation)
    async fn find_workspace_root_fallback(&self, file_path: &Path) -> Result<PathBuf> {
        let start_dir = if file_path.is_file() {
            file_path.parent().unwrap_or(file_path)
        } else {
            file_path
        };

        let mut current_dir = Some(start_dir);

        while let Some(dir) = current_dir {
            // Check for common workspace markers
            let markers = [
                "Cargo.toml",
                "package.json",
                "go.mod",
                "pyproject.toml",
                "setup.py",
                "requirements.txt",
                "tsconfig.json",
                ".git",
                "pom.xml",
                "build.gradle",
                "CMakeLists.txt",
            ];

            for marker in &markers {
                if dir.join(marker).exists() {
                    return Ok(dir.to_path_buf());
                }
            }

            current_dir = dir.parent();
        }

        // Fallback to current directory
        std::env::current_dir().context("Failed to get current directory")
    }

    /// Generate workspace ID from workspace root path
    async fn generate_workspace_id(&self, workspace_root: &Path) -> Result<String> {
        let mut hasher = self.get_hasher().await;

        hasher.update(b"workspace_id:");
        hasher.update(workspace_root.to_string_lossy().as_bytes());

        let hash = hasher.finalize();
        self.return_hasher(hasher).await;

        // Use first 8 characters of hash + folder name
        let folder_name = workspace_root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");

        Ok(format!(
            "{}_{}",
            &hash.to_hex().to_string()[..8],
            folder_name
        ))
    }

    /// Calculate workspace-relative path
    fn get_workspace_relative_path(
        &self,
        file_path: &Path,
        workspace_root: &Path,
    ) -> Result<PathBuf> {
        file_path
            .strip_prefix(workspace_root)
            .map(|p| p.to_path_buf())
            .or_else(|_| -> Result<PathBuf> { Ok(file_path.to_path_buf()) })
            .context("Failed to calculate workspace-relative path")
    }

    /// Read file content with error handling
    #[allow(dead_code)]
    async fn read_file_content(&self, file_path: &Path) -> Result<String> {
        fs::read_to_string(file_path)
            .await
            .context("Failed to read file content")
    }

    /// Generate content-addressed hash (with file content)
    #[allow(dead_code)]
    async fn generate_content_hash(
        &self,
        method: LspMethod,
        workspace_relative_path: &Path,
        file_content: &str,
        params: &str,
        file_mtime: u64,
    ) -> Result<String> {
        let mut hasher = self.get_hasher().await;

        // Hash all components that affect the cache entry
        hasher.update(b"cache_key:");
        hasher.update(method.as_str().as_bytes());
        hasher.update(b":");
        hasher.update(workspace_relative_path.to_string_lossy().as_bytes());
        hasher.update(b":");
        hasher.update(file_content.as_bytes());
        hasher.update(b":");
        hasher.update(params.as_bytes());
        hasher.update(b":");
        hasher.update(&file_mtime.to_le_bytes());

        let hash = hasher.finalize();
        self.return_hasher(hasher).await;

        Ok(hash.to_hex().to_string())
    }

    /// Generate fast content hash (without reading file content) for singleflight deduplication
    async fn generate_fast_content_hash(
        &self,
        method: LspMethod,
        workspace_relative_path: &Path,
        params: &str,
        file_mtime: u64,
        file_path: &Path,
    ) -> Result<String> {
        let mut hasher = self.get_hasher().await;

        // Hash components without reading file content (much faster)
        hasher.update(b"fast_cache_key:");
        hasher.update(method.as_str().as_bytes());
        hasher.update(b":");
        hasher.update(workspace_relative_path.to_string_lossy().as_bytes());
        hasher.update(b":");
        hasher.update(params.as_bytes());
        hasher.update(b":");
        hasher.update(&file_mtime.to_le_bytes());

        // Add file size as additional distinguisher (fast to get)
        if let Ok(metadata) = tokio::fs::metadata(file_path).await {
            hasher.update(b":");
            hasher.update(&metadata.len().to_le_bytes());
        }

        let hash = hasher.finalize();
        self.return_hasher(hasher).await;

        Ok(hash.to_hex().to_string())
    }

    /// Get a hasher from the pool or create a new one
    async fn get_hasher(&self) -> Hasher {
        let mut pool = self.hasher_pool.lock().unwrap();
        pool.pop().unwrap_or_default()
    }

    /// Return a hasher to the pool for reuse
    async fn return_hasher(&self, mut hasher: Hasher) {
        hasher.reset();
        let mut pool = self.hasher_pool.lock().unwrap();
        if pool.len() < 10 {
            // Limit pool size
            pool.push(hasher);
        }
    }
}

impl Default for KeyBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    use tokio;

    #[tokio::test]
    async fn test_key_generation() {
        let temp_dir = TempDir::new().unwrap();
        let workspace = temp_dir.path().join("test-workspace");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();

        let test_file = workspace.join("src/main.rs");
        fs::create_dir_all(test_file.parent().unwrap()).unwrap();
        fs::write(&test_file, "fn main() {}").unwrap();

        let key_builder = KeyBuilder::new();
        let key = key_builder
            .build_key(
                LspMethod::Definition,
                &test_file,
                r#"{"position": {"line": 0, "character": 3}}"#,
            )
            .await
            .unwrap();

        // Verify key components
        assert_eq!(key.method, LspMethod::Definition);
        assert_eq!(key.workspace_relative_path, PathBuf::from("src/main.rs"));
        assert!(key.workspace_id.contains("test-workspace"));
        assert!(!key.content_hash.is_empty());
        assert!(key.file_mtime > 0);
    }

    #[tokio::test]
    async fn test_key_stability() {
        let temp_dir = TempDir::new().unwrap();
        let workspace = temp_dir.path().join("stable-workspace");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("package.json"), r#"{"name": "stable"}"#).unwrap();

        let test_file = workspace.join("index.js");
        fs::write(&test_file, "console.log('hello');").unwrap();

        let key_builder = KeyBuilder::new();
        let params = r#"{"position": {"line": 0, "character": 0}}"#;

        // Generate key twice
        let key1 = key_builder
            .build_key(LspMethod::Hover, &test_file, params)
            .await
            .unwrap();
        let key2 = key_builder
            .build_key(LspMethod::Hover, &test_file, params)
            .await
            .unwrap();

        // Keys should be identical
        assert_eq!(key1, key2);
        assert_eq!(key1.to_storage_key(), key2.to_storage_key());
    }

    #[tokio::test]
    async fn test_key_invalidation() {
        let temp_dir = TempDir::new().unwrap();
        let workspace = temp_dir.path().join("invalidation-workspace");
        fs::create_dir_all(&workspace).unwrap();
        fs::write(workspace.join("go.mod"), "module invalidation").unwrap();

        let test_file = workspace.join("main.go");
        fs::write(&test_file, "package main\n\nfunc main() {}").unwrap();

        let key_builder = KeyBuilder::new();
        let params = r#"{"position": {"line": 2, "character": 5}}"#;

        // Generate initial key
        let key1 = key_builder
            .build_key(LspMethod::References, &test_file, params)
            .await
            .unwrap();

        // Modify file with robust timing
        tokio::time::sleep(std::time::Duration::from_millis(100)).await; // Ensure different mtime
        fs::write(
            &test_file,
            "package main\n\nfunc main() {\n    // Modified\n}",
        )
        .unwrap();

        // Additional sleep to ensure filesystem timestamp resolution
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Generate new key
        let key2 = key_builder
            .build_key(LspMethod::References, &test_file, params)
            .await
            .unwrap();

        // Keys should be different
        assert_ne!(key1, key2);
        assert_ne!(key1.content_hash, key2.content_hash);
        assert_ne!(key1.file_mtime, key2.file_mtime);
    }

    #[tokio::test]
    async fn test_server_fingerprint() {
        let temp_dir = TempDir::new().unwrap();
        let workspace = temp_dir.path();

        let key_builder = KeyBuilder::new();

        // Generate fingerprints
        let fp1 = key_builder
            .generate_server_fingerprint("rust", "1.0.0", workspace)
            .await
            .unwrap();
        let fp2 = key_builder
            .generate_server_fingerprint("rust", "1.0.0", workspace)
            .await
            .unwrap();
        let fp3 = key_builder
            .generate_server_fingerprint("rust", "1.1.0", workspace)
            .await
            .unwrap();

        // Same inputs should produce same fingerprint
        assert_eq!(fp1, fp2);

        // Different inputs should produce different fingerprints
        assert_ne!(fp1, fp3);
    }

    #[tokio::test]
    async fn test_storage_key_round_trip() {
        let original_key = CacheKey {
            workspace_relative_path: PathBuf::from("src/lib.rs"),
            method: LspMethod::Definition,
            content_hash: "abc123def456".to_string(),
            workspace_id: "12345678_my-project".to_string(),
            file_mtime: 1234567890,
            symbol_name: None,
            position: None,
        };

        let storage_key = original_key.to_storage_key();
        let parsed_key = CacheKey::from_storage_key(&storage_key).unwrap();

        assert_eq!(
            parsed_key.workspace_relative_path,
            original_key.workspace_relative_path
        );
        assert_eq!(parsed_key.method, original_key.method);
        assert_eq!(parsed_key.content_hash, original_key.content_hash);
        assert_eq!(parsed_key.workspace_id, original_key.workspace_id);
    }

    #[tokio::test]
    async fn test_workspace_detection() {
        let temp_dir = TempDir::new().unwrap();

        // Create nested workspace structure
        let root_workspace = temp_dir.path().join("root");
        let sub_workspace = root_workspace.join("sub");
        fs::create_dir_all(&root_workspace).unwrap();
        fs::create_dir_all(&sub_workspace).unwrap();

        // Root has git repo
        fs::create_dir_all(root_workspace.join(".git")).unwrap();

        // Sub has Cargo.toml (should take precedence)
        fs::write(
            sub_workspace.join("Cargo.toml"),
            "[package]\nname = \"sub\"",
        )
        .unwrap();

        let test_file = sub_workspace.join("src/main.rs");
        fs::create_dir_all(test_file.parent().unwrap()).unwrap();
        fs::write(&test_file, "fn main() {}").unwrap();

        let key_builder = KeyBuilder::new();
        let key = key_builder
            .build_key(LspMethod::Definition, &test_file, "{}")
            .await
            .unwrap();

        // Should use sub workspace (nearest workspace marker)
        assert_eq!(key.workspace_relative_path, PathBuf::from("src/main.rs"));
        assert!(key.workspace_id.contains("sub"));
    }

    #[tokio::test]
    async fn test_key_validation() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.txt");
        fs::write(&test_file, "initial content").unwrap();

        let key_builder = KeyBuilder::new();
        let key = key_builder
            .build_key(LspMethod::Hover, &test_file, "{}")
            .await
            .unwrap();

        // Key should be valid initially
        assert!(key_builder.is_key_valid(&key, &test_file).await.unwrap());

        // Modify file with robust timing
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        fs::write(&test_file, "modified content").unwrap();

        // Additional sleep to ensure filesystem timestamp resolution
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Key should now be invalid
        assert!(!key_builder.is_key_valid(&key, &test_file).await.unwrap());

        // Remove file
        fs::remove_file(&test_file).unwrap();

        // Key should be invalid for non-existent file
        assert!(!key_builder.is_key_valid(&key, &test_file).await.unwrap());
    }
}
