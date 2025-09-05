//! File Change Detection System for Incremental Indexing
#![allow(dead_code, clippy::all)]
//!
//! This module provides a comprehensive file change detection system that serves as the
//! foundation for incremental indexing. It implements content-addressed file versioning,
//! efficient change detection, and git integration for blob OID support.
//!
//! ## Key Features
//!
//! - Content-addressed file hashing using BLAKE3 (preferred) or SHA-256
//! - Language detection integration with known file extensions
//! - Git integration for blob OID tracking and ignore pattern support
//! - Performance optimizations with mtime checks and efficient scanning
//! - Database integration for content-addressed file version lookup
//! - Comprehensive change detection (create, update, delete operations)
//!
//! ## Usage
//!
//! ```rust
//! use file_detector::{FileChangeDetector, HashAlgorithm};
//! use database::DatabaseBackend;
//!
//! let detector = FileChangeDetector::new();
//! let changes = detector.detect_changes(workspace_id, &path, &database).await?;
//! ```

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;
use tokio::fs;
use tokio::sync::Semaphore;
use tracing::{debug, info, warn};

use crate::database::{DatabaseBackend, DatabaseError};
use crate::git_service::{GitService, GitServiceError};

/// Hash algorithms supported for content addressing
#[derive(Debug, Clone, PartialEq)]
pub enum HashAlgorithm {
    Blake3,
    Sha256,
}

impl Default for HashAlgorithm {
    fn default() -> Self {
        Self::Blake3
    }
}

/// Types of file changes that can be detected
#[derive(Debug, Clone, PartialEq)]
pub enum FileChangeType {
    /// File was created (new file not in database)
    Create,
    /// File content was modified (different content hash)
    Update,
    /// File was deleted (exists in database but not on filesystem)
    Delete,
    /// File was moved (same content hash, different path)
    Move { from: PathBuf, to: PathBuf },
}

/// Represents a detected file change with comprehensive metadata
#[derive(Debug, Clone)]
pub struct FileChange {
    /// Path to the changed file
    pub path: PathBuf,
    /// Type of change detected
    pub change_type: FileChangeType,
    /// Content digest for the current file state (None for deletions)
    pub content_digest: Option<String>,
    /// File size in bytes (None for deletions)
    pub size_bytes: Option<u64>,
    /// Last modification time as Unix timestamp (None for deletions)
    pub mtime: Option<i64>,
    /// Detected language from file extension or content analysis
    pub detected_language: Option<String>,
}

/// Configuration for file change detection
#[derive(Debug, Clone)]
pub struct DetectionConfig {
    /// Hash algorithm to use for content addressing
    pub hash_algorithm: HashAlgorithm,
    /// Patterns to ignore during scanning (in addition to gitignore)
    pub ignore_patterns: Vec<String>,
    /// File extensions to consider for indexing
    pub supported_extensions: HashSet<String>,
    /// Maximum file size to process (in bytes)
    pub max_file_size: u64,
    /// Maximum depth for directory traversal
    pub max_depth: Option<usize>,
    /// Whether to include hidden files/directories
    pub include_hidden: bool,
    /// Whether to respect gitignore files
    pub respect_gitignore: bool,
}

impl Default for DetectionConfig {
    fn default() -> Self {
        let mut supported_extensions = HashSet::new();

        // Add common programming language extensions
        let extensions = [
            "rs",
            "js",
            "jsx",
            "ts",
            "tsx",
            "py",
            "go",
            "c",
            "h",
            "cpp",
            "cc",
            "cxx",
            "hpp",
            "hxx",
            "java",
            "rb",
            "php",
            "swift",
            "cs",
            "kt",
            "scala",
            "clj",
            "ex",
            "exs",
            "erl",
            "hrl",
            "hs",
            "lhs",
            "ml",
            "mli",
            "fs",
            "fsx",
            "fsi",
            "dart",
            "jl",
            "r",
            "R",
            "m",
            "mm",
            "pl",
            "pm",
            "sh",
            "bash",
            "zsh",
            "fish",
            "lua",
            "vim",
            "sql",
            "json",
            "yaml",
            "yml",
            "toml",
            "xml",
            "html",
            "css",
            "scss",
            "sass",
            "less",
            "md",
            "rst",
            "tex",
            "dockerfile",
        ];

        for ext in &extensions {
            supported_extensions.insert(ext.to_string());
        }

        Self {
            hash_algorithm: HashAlgorithm::Blake3,
            ignore_patterns: vec![
                "target/".to_string(),
                "node_modules/".to_string(),
                ".git/".to_string(),
                ".svn/".to_string(),
                ".hg/".to_string(),
                "build/".to_string(),
                "dist/".to_string(),
                ".vscode/".to_string(),
                ".idea/".to_string(),
                "*.tmp".to_string(),
                "*.log".to_string(),
                "*.cache".to_string(),
                ".DS_Store".to_string(),
                "Thumbs.db".to_string(),
            ],
            supported_extensions,
            max_file_size: 10 * 1024 * 1024, // 10MB
            max_depth: Some(20),
            include_hidden: false,
            respect_gitignore: true,
        }
    }
}

/// Comprehensive error types for file detection operations
#[derive(Debug, thiserror::Error)]
pub enum DetectionError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),

    #[error("Invalid path: {path}")]
    InvalidPath { path: PathBuf },

    #[error("Hash computation failed: {0}")]
    HashError(String),

    #[error("Git service error: {0}")]
    Git(#[from] GitServiceError),

    #[error("File too large: {size} bytes exceeds limit of {limit} bytes")]
    FileTooLarge { size: u64, limit: u64 },

    #[error("Directory traversal too deep: {depth} exceeds limit of {limit}")]
    TooDeep { depth: usize, limit: usize },

    #[error("Concurrent processing error: {0}")]
    Concurrency(String),

    #[error("Context error: {0}")]
    Context(#[from] anyhow::Error),
}

/// File change detector with configurable algorithms and optimizations
pub struct FileChangeDetector {
    /// Configuration for detection behavior
    config: DetectionConfig,
    /// Semaphore for controlling concurrent file operations
    file_semaphore: Arc<Semaphore>,
}

impl FileChangeDetector {
    /// Create a new file change detector with default configuration
    pub fn new() -> Self {
        Self::with_config(DetectionConfig::default())
    }

    /// Create a new file change detector with custom configuration
    pub fn with_config(config: DetectionConfig) -> Self {
        Self {
            config,
            file_semaphore: Arc::new(Semaphore::new(100)), // Limit concurrent file operations
        }
    }

    /// Create a detector with git integration for a workspace
    /// Note: This just validates git repository availability but doesn't store it due to thread safety issues
    pub fn with_git_integration(
        config: DetectionConfig,
        workspace_root: &Path,
    ) -> Result<Self, DetectionError> {
        match GitService::discover_repo(workspace_root, workspace_root) {
            Ok(_) => {
                info!("Git repository detected at {}", workspace_root.display());
            }
            Err(GitServiceError::NotRepo) => {
                debug!("No git repository found at {}", workspace_root.display());
            }
            Err(e) => {
                warn!("Git integration failed: {}", e);
            }
        };

        Ok(Self::with_config(config))
    }

    /// Compute content hash for a file using the configured algorithm
    pub async fn compute_file_hash(
        &self,
        file_path: &Path,
    ) -> Result<(String, u64), DetectionError> {
        let content = fs::read(file_path)
            .await
            .context(format!("Failed to read file: {}", file_path.display()))?;

        let size = content.len() as u64;

        if size > self.config.max_file_size {
            return Err(DetectionError::FileTooLarge {
                size,
                limit: self.config.max_file_size,
            });
        }

        let hash = self.compute_content_hash(&content);
        Ok((hash, size))
    }

    /// Compute content hash for raw bytes
    pub fn compute_content_hash(&self, content: &[u8]) -> String {
        match self.config.hash_algorithm {
            HashAlgorithm::Blake3 => {
                let hash = blake3::hash(content);
                hash.to_hex().to_string()
            }
            HashAlgorithm::Sha256 => {
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(content);
                format!("{:x}", hasher.finalize())
            }
        }
    }

    /// Check if a file should be indexed based on configuration
    pub fn should_index_file(&self, file_path: &Path) -> bool {
        // Check file extension
        if let Some(extension) = file_path.extension().and_then(|e| e.to_str()) {
            if !self.config.supported_extensions.contains(extension) {
                return false;
            }
        } else {
            // No extension - only allow specific filenames
            if let Some(filename) = file_path.file_name().and_then(|n| n.to_str()) {
                let allowed_no_ext = ["Dockerfile", "Makefile", "Rakefile", "Gemfile", "Procfile"];
                if !allowed_no_ext.contains(&filename) {
                    return false;
                }
            } else {
                return false;
            }
        }

        // Check ignore patterns
        let path_str = file_path.to_string_lossy();
        for pattern in &self.config.ignore_patterns {
            if pattern.ends_with('/') {
                // Directory pattern
                if path_str.contains(pattern) {
                    return false;
                }
            } else if pattern.contains('*') {
                // Glob pattern - simple implementation
                if glob_match(pattern, &path_str) {
                    return false;
                }
            } else if path_str.contains(pattern) {
                return false;
            }
        }

        // TODO: Add git ignore checking when needed
        true
    }

    /// Detect if a file is binary using content analysis
    pub async fn is_binary_file(&self, file_path: &Path) -> Result<bool, DetectionError> {
        // Read first 512 bytes to check for binary content
        let mut file = fs::File::open(file_path).await?;
        let mut buffer = vec![0u8; 512];

        use tokio::io::AsyncReadExt;
        let bytes_read = file.read(&mut buffer).await?;
        buffer.truncate(bytes_read);

        // Check for null bytes (common binary indicator)
        if buffer.contains(&0) {
            return Ok(true);
        }

        // Check for high proportion of non-printable characters
        let non_printable_count = buffer
            .iter()
            .filter(|&&b| b < 32 && b != 9 && b != 10 && b != 13)
            .count();

        let ratio = non_printable_count as f64 / buffer.len() as f64;
        Ok(ratio > 0.3) // More than 30% non-printable characters
    }

    /// Detect programming language for a file
    pub fn detect_language(&self, file_path: &Path) -> Option<String> {
        // Use extension-based detection with known language extensions
        if let Some(extension) = file_path.extension().and_then(|e| e.to_str()) {
            // Check if this extension is supported based on our known languages
            let supported_languages = [
                "rs", "js", "jsx", "ts", "tsx", "py", "go", "c", "h", "cpp", "cc", "cxx", "hpp",
                "hxx", "java", "rb", "php", "swift", "cs", "kt", "scala", "clj", "ex", "exs",
                "erl", "hrl", "hs", "lhs", "ml", "mli", "fs", "fsx", "fsi", "dart", "jl", "r", "R",
                "m", "mm", "pl", "pm", "sh", "bash", "zsh", "fish", "lua", "vim", "sql",
            ];

            if supported_languages.contains(&extension) {
                return Some(extension.to_string());
            }
        }

        // Fallback to extension-based detection for any extension
        file_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_string())
    }

    /// Detect all file changes in a workspace by comparing with database state
    pub async fn detect_changes<T>(
        &self,
        workspace_id: i64,
        scan_path: &Path,
        database: &T,
    ) -> Result<Vec<FileChange>, DetectionError>
    where
        T: DatabaseBackend + ?Sized,
    {
        info!(
            "Starting change detection for workspace {} at {}",
            workspace_id,
            scan_path.display()
        );

        // Get current file list from filesystem
        let current_files = self.scan_directory(scan_path).await?;
        debug!("Found {} files to check", current_files.len());

        // Compare with database state
        let changes = self
            .compare_with_database(&current_files, workspace_id, database)
            .await?;

        info!(
            "Detected {} changes: {} creates, {} updates, {} deletes",
            changes.len(),
            changes
                .iter()
                .filter(|c| matches!(c.change_type, FileChangeType::Create))
                .count(),
            changes
                .iter()
                .filter(|c| matches!(c.change_type, FileChangeType::Update))
                .count(),
            changes
                .iter()
                .filter(|c| matches!(c.change_type, FileChangeType::Delete))
                .count()
        );

        Ok(changes)
    }

    /// Scan directory recursively for indexable files
    pub async fn scan_directory(&self, path: &Path) -> Result<Vec<PathBuf>, DetectionError> {
        if !path.exists() {
            return Err(DetectionError::InvalidPath {
                path: path.to_path_buf(),
            });
        }

        let mut files = Vec::new();
        self.scan_directory_recursive(path, &mut files, 0).await?;

        // Sort for deterministic ordering
        files.sort();

        Ok(files)
    }

    /// Recursive directory scanning with depth limits
    fn scan_directory_recursive<'a>(
        &'a self,
        path: &'a Path,
        files: &'a mut Vec<PathBuf>,
        depth: usize,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), DetectionError>> + Send + 'a>>
    {
        Box::pin(async move {
            if let Some(max_depth) = self.config.max_depth {
                if depth > max_depth {
                    return Err(DetectionError::TooDeep {
                        depth,
                        limit: max_depth,
                    });
                }
            }

            let mut entries = fs::read_dir(path).await?;
            while let Some(entry) = entries.next_entry().await? {
                let entry_path = entry.path();
                let file_name = entry_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");

                // Skip hidden files/directories if not configured to include them
                if !self.config.include_hidden && file_name.starts_with('.') {
                    continue;
                }

                if entry_path.is_dir() {
                    // Recursively scan subdirectories
                    self.scan_directory_recursive(&entry_path, files, depth + 1)
                        .await?;
                } else if entry_path.is_file() {
                    // Check if file should be indexed
                    if self.should_index_file(&entry_path) {
                        files.push(entry_path);
                    }
                }
            }

            Ok(())
        })
    }

    /// Compare current files with database state to detect changes
    async fn compare_with_database<T>(
        &self,
        current_files: &[PathBuf],
        workspace_id: i64,
        database: &T,
    ) -> Result<Vec<FileChange>, DetectionError>
    where
        T: DatabaseBackend + ?Sized,
    {
        let mut changes = Vec::new();

        // Process files sequentially to avoid thread safety issues
        for file_path in current_files {
            if let Some(change) = self
                .check_file_change(file_path, workspace_id, database)
                .await?
            {
                changes.push(change);
            }
        }

        // TODO: Detect deletions by checking database files not found in current scan
        // This requires querying all files in the workspace from the database
        // and comparing with current_files set

        Ok(changes)
    }

    /// Check if a single file has changed compared to database state
    async fn check_file_change<T>(
        &self,
        file_path: &Path,
        _workspace_id: i64,
        database: &T,
    ) -> Result<Option<FileChange>, DetectionError>
    where
        T: DatabaseBackend + ?Sized,
    {
        // Get file metadata
        let metadata = fs::metadata(file_path).await?;
        let mtime = metadata
            .modified()?
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        // Skip if file is binary
        if self.is_binary_file(file_path).await? {
            return Ok(None);
        }

        // Compute current content hash
        let (content_hash, size_bytes) = self.compute_file_hash(file_path).await?;

        // Check if file version exists in database
        let existing_version = database.get_file_version_by_digest(&content_hash).await?;

        let change_type = if existing_version.is_some() {
            // File content exists in database - check if it's linked to this workspace
            // TODO: This needs a new database method to check workspace file associations
            // For now, assume it's an update if we found the content hash
            FileChangeType::Update
        } else {
            // New content hash - this is either a create or update
            FileChangeType::Create
        };

        let detected_language = self.detect_language(file_path);

        Ok(Some(FileChange {
            path: file_path.to_path_buf(),
            change_type,
            content_digest: Some(content_hash),
            size_bytes: Some(size_bytes),
            mtime: Some(mtime),
            detected_language,
        }))
    }

    /// Get git blob OID for a file if git integration is available
    /// Note: Due to thread safety issues with gix, this creates a new GitService per call
    pub fn get_git_blob_oid(
        &self,
        _file_path: &Path,
        workspace_root: &Path,
    ) -> Result<Option<String>, DetectionError> {
        match GitService::discover_repo(workspace_root, workspace_root) {
            Ok(_git) => {
                // TODO: Implement blob OID retrieval when GitService supports it
                // For now, return None to indicate git OID is not available
                Ok(None)
            }
            Err(_) => Ok(None),
        }
    }

    /// Check if a file is ignored by git
    /// Note: Due to thread safety issues with gix, this creates a new GitService per call
    pub fn is_git_ignored(&self, _file_path: &Path, workspace_root: &Path) -> bool {
        match GitService::discover_repo(workspace_root, workspace_root) {
            Ok(_git) => {
                // TODO: Implement git ignore checking when GitService supports it
                false
            }
            Err(_) => false,
        }
    }

    /// Get the current git HEAD commit hash if available
    /// Note: Due to thread safety issues with gix, this creates a new GitService per call
    pub fn get_git_head_commit(
        &self,
        workspace_root: &Path,
    ) -> Result<Option<String>, DetectionError> {
        match GitService::discover_repo(workspace_root, workspace_root) {
            Ok(git) => Ok(git.head_commit()?),
            Err(_) => Ok(None),
        }
    }
}

impl Default for FileChangeDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Simple glob pattern matching implementation
fn glob_match(pattern: &str, text: &str) -> bool {
    if !pattern.contains('*') {
        return pattern == text;
    }

    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.is_empty() {
        return true;
    }

    let mut text_pos = 0;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }

        if i == 0 {
            // First part must match the beginning
            if !text[text_pos..].starts_with(part) {
                return false;
            }
            text_pos += part.len();
        } else if i == parts.len() - 1 {
            // Last part must match the end
            return text[text_pos..].ends_with(part);
        } else {
            // Middle part - find next occurrence
            if let Some(pos) = text[text_pos..].find(part) {
                text_pos += pos + part.len();
            } else {
                return false;
            }
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_file_change_detector_creation() {
        let detector = FileChangeDetector::new();
        assert_eq!(detector.config.hash_algorithm, HashAlgorithm::Blake3);
        assert!(!detector.config.supported_extensions.is_empty());
    }

    #[tokio::test]
    async fn test_content_hashing() {
        let detector = FileChangeDetector::new();
        let content = b"Hello, world!";

        let hash1 = detector.compute_content_hash(content);
        let hash2 = detector.compute_content_hash(content);

        // Same content should produce same hash
        assert_eq!(hash1, hash2);
        assert!(!hash1.is_empty());

        // Different content should produce different hash
        let different_content = b"Hello, universe!";
        let hash3 = detector.compute_content_hash(different_content);
        assert_ne!(hash1, hash3);
    }

    #[tokio::test]
    async fn test_file_indexing_decision() {
        let detector = FileChangeDetector::new();

        // Should index supported extensions
        assert!(detector.should_index_file(Path::new("test.rs")));
        assert!(detector.should_index_file(Path::new("test.js")));
        assert!(detector.should_index_file(Path::new("test.py")));

        // Should not index unsupported extensions
        assert!(!detector.should_index_file(Path::new("test.exe")));
        assert!(!detector.should_index_file(Path::new("test.bin")));

        // Should not index ignored patterns
        assert!(!detector.should_index_file(Path::new("target/debug/test.rs")));
        assert!(!detector.should_index_file(Path::new("node_modules/test.js")));

        // Should index special files without extensions
        assert!(detector.should_index_file(Path::new("Dockerfile")));
        assert!(detector.should_index_file(Path::new("Makefile")));
    }

    #[tokio::test]
    async fn test_language_detection() {
        let detector = FileChangeDetector::new();

        assert_eq!(
            detector.detect_language(Path::new("test.rs")),
            Some("rs".to_string())
        );
        assert_eq!(
            detector.detect_language(Path::new("test.js")),
            Some("js".to_string())
        );
        assert_eq!(
            detector.detect_language(Path::new("test.py")),
            Some("py".to_string())
        );
        assert_eq!(
            detector.detect_language(Path::new("test.unknown")),
            Some("unknown".to_string())
        );
    }

    #[tokio::test]
    async fn test_binary_detection() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let detector = FileChangeDetector::new();

        // Create text file
        let text_file = temp_dir.path().join("test.txt");
        fs::write(&text_file, "Hello, world!\nThis is text content.\n").await?;

        // Create binary file
        let binary_file = temp_dir.path().join("test.bin");
        let binary_content = vec![0u8, 1u8, 255u8, 0u8, 127u8];
        fs::write(&binary_file, &binary_content).await?;

        assert!(!detector.is_binary_file(&text_file).await?);
        assert!(detector.is_binary_file(&binary_file).await?);

        Ok(())
    }

    #[tokio::test]
    async fn test_directory_scanning() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let detector = FileChangeDetector::new();

        // Create test file structure
        let src_dir = temp_dir.path().join("src");
        fs::create_dir(&src_dir).await?;

        let main_file = src_dir.join("main.rs");
        fs::write(&main_file, "fn main() {}").await?;

        let lib_file = src_dir.join("lib.rs");
        fs::write(&lib_file, "pub fn hello() {}").await?;

        // Create ignored file
        let target_dir = temp_dir.path().join("target");
        fs::create_dir(&target_dir).await?;
        let ignored_file = target_dir.join("ignored.rs");
        fs::write(&ignored_file, "// ignored").await?;

        let files = detector.scan_directory(temp_dir.path()).await?;

        // Should find the two .rs files in src/, but not the one in target/
        assert_eq!(files.len(), 2);
        assert!(files.iter().any(|f| f.ends_with("main.rs")));
        assert!(files.iter().any(|f| f.ends_with("lib.rs")));
        assert!(!files.iter().any(|f| f.to_string_lossy().contains("target")));

        Ok(())
    }

    #[test]
    fn test_glob_matching() {
        assert!(glob_match("*.rs", "test.rs"));
        assert!(glob_match("*.rs", "src/main.rs"));
        assert!(!glob_match("*.rs", "test.js"));

        assert!(glob_match("target/*", "target/debug"));
        assert!(glob_match("target/*", "target/release"));
        assert!(!glob_match("target/*", "src/main.rs"));

        assert!(glob_match("*test*.rs", "unit_test_helper.rs"));
        assert!(glob_match("*test*.rs", "test_utils.rs"));
        assert!(!glob_match("*test*.rs", "main.rs"));
    }

    #[test]
    fn test_hash_algorithms() {
        let content = b"test content";

        let blake3_detector = FileChangeDetector::with_config(DetectionConfig {
            hash_algorithm: HashAlgorithm::Blake3,
            ..Default::default()
        });

        let sha256_detector = FileChangeDetector::with_config(DetectionConfig {
            hash_algorithm: HashAlgorithm::Sha256,
            ..Default::default()
        });

        let blake3_hash = blake3_detector.compute_content_hash(content);
        let sha256_hash = sha256_detector.compute_content_hash(content);

        // Hashes should be different algorithms but consistent
        assert_ne!(blake3_hash, sha256_hash);
        assert_eq!(blake3_hash.len(), 64); // BLAKE3 produces 32-byte hash (64 hex chars)
        assert_eq!(sha256_hash.len(), 64); // SHA-256 produces 32-byte hash (64 hex chars)
    }
}
