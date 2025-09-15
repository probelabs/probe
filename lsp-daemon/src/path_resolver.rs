//! Git-aware path resolution utility
//!
//! This module provides utilities for resolving file paths relative to git repositories,
//! handling regular git repos, worktrees, submodules, and falling back to workspace-relative paths.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tracing::{debug, warn};

/// Maximum number of directories to traverse when looking for git root
const MAX_TRAVERSAL_DEPTH: usize = 20;

/// Timeout for filesystem operations to prevent hanging on slow filesystems
const FILESYSTEM_TIMEOUT: Duration = Duration::from_secs(5);

/// Git-aware path resolution utility
pub struct PathResolver {
    /// Maximum depth to traverse when looking for git root
    max_depth: usize,
    /// Timeout for filesystem operations
    timeout: Duration,
}

impl Default for PathResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl PathResolver {
    /// Create a new path resolver with default settings
    pub fn new() -> Self {
        Self {
            max_depth: MAX_TRAVERSAL_DEPTH,
            timeout: FILESYSTEM_TIMEOUT,
        }
    }

    /// Create a new path resolver with custom settings
    pub fn with_config(max_depth: usize, timeout: Duration) -> Self {
        Self { max_depth, timeout }
    }

    /// Get the relative path for a file, using git root when available, workspace root as fallback
    pub fn get_relative_path(&self, file_path: &Path, workspace_path: &Path) -> String {
        // Try to find git root first
        if let Some(git_root) = self.find_git_root(file_path) {
            // Ensure the file is within the git root
            if file_path.starts_with(&git_root) {
                return file_path
                    .strip_prefix(&git_root)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| file_path.to_string_lossy().to_string());
            }
        }

        // Fallback to workspace-relative path
        if file_path.starts_with(workspace_path) {
            file_path
                .strip_prefix(workspace_path)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| file_path.to_string_lossy().to_string())
        } else {
            // Return absolute path if file is not within workspace
            file_path.to_string_lossy().to_string()
        }
    }

    /// Find the git repository root by traversing up directories
    pub fn find_git_root(&self, path: &Path) -> Option<PathBuf> {
        let start_time = Instant::now();

        // Start from the file's directory
        let mut current = if path.is_file() { path.parent()? } else { path };

        // Get user home directory for boundary checking
        let home_dir = self.get_home_directory();

        let mut depth = 0;

        while depth < self.max_depth {
            // Check timeout
            if start_time.elapsed() > self.timeout {
                warn!("Git root search timed out after {:?}", self.timeout);
                return None;
            }

            // Safety check: don't traverse above home directory
            if let Some(ref home) = home_dir {
                if current == home.as_path() {
                    break;
                }
            }

            // Look for .git directory or file
            let git_path = current.join(".git");

            if self.path_exists_safe(&git_path) {
                if git_path.is_dir() {
                    // Regular git repository
                    return Some(current.to_path_buf());
                } else if git_path.is_file() {
                    // Worktree or submodule - check if it's valid
                    if self.is_git_worktree(&git_path) {
                        return Some(current.to_path_buf());
                    }
                }
            }

            // Move up one directory
            current = current.parent()?;
            depth += 1;
        }

        None
    }

    /// Find the workspace root as a fallback when no git root is found
    pub fn find_workspace_root(&self, path: &Path) -> PathBuf {
        // Use the existing workspace resolver from the codebase
        // This is a simple fallback implementation that looks for common workspace markers
        let start_dir = if path.is_file() {
            path.parent().unwrap_or(path)
        } else {
            path
        };

        // Common workspace markers in priority order
        let markers = [
            "Cargo.toml",     // Rust
            "package.json",   // Node.js/JavaScript
            "go.mod",         // Go
            "pyproject.toml", // Python
            "setup.py",       // Python
            "pom.xml",        // Java Maven
            "build.gradle",   // Java Gradle
            "CMakeLists.txt", // C/C++
            "tsconfig.json",  // TypeScript
            ".git",           // Git repository
            "README.md",      // Generic project root
        ];

        let mut current = start_dir;
        let mut depth = 0;

        while depth < self.max_depth {
            for marker in &markers {
                let marker_path = current.join(marker);
                if self.path_exists_safe(&marker_path) {
                    return current.to_path_buf();
                }
            }

            // Move up one directory
            if let Some(parent) = current.parent() {
                current = parent;
                depth += 1;
            } else {
                break;
            }
        }

        // Fallback to the starting directory
        start_dir.to_path_buf()
    }

    /// Check if a .git file represents a git worktree
    pub fn is_git_worktree(&self, git_path: &Path) -> bool {
        if !git_path.is_file() {
            return false;
        }

        match fs::read_to_string(git_path) {
            Ok(content) => {
                let content = content.trim();
                // Git worktrees have a .git file containing "gitdir: /path/to/repo"
                content.starts_with("gitdir: ") && content.len() > 8
            }
            Err(_) => false,
        }
    }

    /// Safely check if a path exists, handling permission errors gracefully
    fn path_exists_safe(&self, path: &Path) -> bool {
        match fs::metadata(path) {
            Ok(_) => true,
            Err(e) => {
                // Log permission errors but don't fail
                if e.kind() == std::io::ErrorKind::PermissionDenied {
                    debug!("Permission denied accessing path: {:?}", path);
                }
                false
            }
        }
    }

    /// Get the user's home directory for boundary checking
    fn get_home_directory(&self) -> Option<PathBuf> {
        env::var_os("HOME")
            .or_else(|| env::var_os("USERPROFILE"))
            .map(PathBuf::from)
    }
}

/// Convenience functions for common use cases

/// Get relative path using default resolver
pub fn get_relative_path(file_path: &Path, workspace_path: &Path) -> String {
    let resolver = PathResolver::new();
    resolver.get_relative_path(file_path, workspace_path)
}

/// Find git root using default resolver
pub fn find_git_root(path: &Path) -> Option<PathBuf> {
    let resolver = PathResolver::new();
    resolver.find_git_root(path)
}

/// Find workspace root using default resolver
pub fn find_workspace_root(path: &Path) -> PathBuf {
    let resolver = PathResolver::new();
    resolver.find_workspace_root(path)
}

/// Check if path is git worktree using default resolver
pub fn is_git_worktree(git_path: &Path) -> bool {
    let resolver = PathResolver::new();
    resolver.is_git_worktree(git_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_regular_git_repo() {
        let temp_dir = tempdir().unwrap();
        let repo_root = temp_dir.path();

        // Create a .git directory
        let git_dir = repo_root.join(".git");
        fs::create_dir_all(&git_dir).unwrap();

        // Create a file in a subdirectory
        let subdir = repo_root.join("src");
        fs::create_dir_all(&subdir).unwrap();
        let file_path = subdir.join("main.rs");
        fs::write(&file_path, "fn main() {}").unwrap();

        let resolver = PathResolver::new();
        let git_root = resolver.find_git_root(&file_path);

        assert_eq!(git_root, Some(repo_root.to_path_buf()));
    }

    #[test]
    fn test_git_worktree() {
        let temp_dir = tempdir().unwrap();
        let repo_root = temp_dir.path();

        // Create a .git file (worktree)
        let git_file = repo_root.join(".git");
        fs::write(
            &git_file,
            "gitdir: /path/to/main/repo/.git/worktrees/feature-branch",
        )
        .unwrap();

        let resolver = PathResolver::new();

        // Test worktree detection
        assert!(resolver.is_git_worktree(&git_file));

        // Test git root finding
        let file_path = repo_root.join("src").join("main.rs");
        let git_root = resolver.find_git_root(&file_path);
        assert_eq!(git_root, Some(repo_root.to_path_buf()));
    }

    #[test]
    fn test_workspace_fallback() {
        let temp_dir = tempdir().unwrap();
        let workspace_root = temp_dir.path();

        // Create a Cargo.toml (workspace marker)
        let cargo_toml = workspace_root.join("Cargo.toml");
        fs::write(&cargo_toml, "[package]\nname = \"test\"").unwrap();

        // Create a file in a subdirectory
        let subdir = workspace_root.join("src");
        fs::create_dir_all(&subdir).unwrap();
        let file_path = subdir.join("lib.rs");
        fs::write(&file_path, "// lib").unwrap();

        let resolver = PathResolver::new();
        let workspace_root_found = resolver.find_workspace_root(&file_path);

        assert_eq!(workspace_root_found, workspace_root.to_path_buf());
    }

    #[test]
    fn test_relative_path_calculation() {
        let temp_dir = tempdir().unwrap();
        let repo_root = temp_dir.path();

        // Create a .git directory
        let git_dir = repo_root.join(".git");
        fs::create_dir_all(&git_dir).unwrap();

        // Create nested file
        let nested_path = repo_root.join("src").join("module").join("file.rs");
        fs::create_dir_all(nested_path.parent().unwrap()).unwrap();
        fs::write(&nested_path, "// content").unwrap();

        let resolver = PathResolver::new();
        let relative = resolver.get_relative_path(&nested_path, repo_root);

        // On Windows, use forward slashes in the expected result
        let expected = if cfg!(windows) {
            "src\\module\\file.rs"
        } else {
            "src/module/file.rs"
        };

        assert_eq!(relative, expected);
    }

    #[test]
    fn test_max_depth_limit() {
        let resolver = PathResolver::with_config(2, Duration::from_secs(1));

        // Create a deep path that exceeds max depth
        let deep_path = PathBuf::from("/a/b/c/d/e/f/g/file.txt");

        // This should return None due to depth limit (and non-existent path)
        let result = resolver.find_git_root(&deep_path);
        assert_eq!(result, None);
    }

    #[test]
    fn test_invalid_git_file() {
        let temp_dir = tempdir().unwrap();
        let repo_root = temp_dir.path();

        // Create an invalid .git file
        let git_file = repo_root.join(".git");
        fs::write(&git_file, "invalid content").unwrap();

        let resolver = PathResolver::new();
        assert!(!resolver.is_git_worktree(&git_file));
    }

    #[test]
    fn test_permission_error_handling() {
        let resolver = PathResolver::new();

        // Test with a non-existent path
        let non_existent = PathBuf::from("/this/path/does/not/exist");
        assert!(!resolver.path_exists_safe(&non_existent));
    }
}
