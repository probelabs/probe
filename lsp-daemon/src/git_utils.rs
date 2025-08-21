//! Git utilities for cache-aware version tracking
//!
//! This module provides git context tracking and change detection capabilities
//! for maintaining cache consistency across branches and commits.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{debug, warn};

/// Git context information for cache versioning
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitContext {
    /// Current commit hash (full SHA)
    pub commit_hash: String,
    /// Current branch name
    pub branch: String,
    /// Whether the working directory has uncommitted changes
    pub is_dirty: bool,
    /// Remote origin URL if available
    pub remote_url: Option<String>,
    /// Repository root path
    pub repo_root: PathBuf,
}

impl GitContext {
    /// Capture current git context for the given repository path
    pub fn capture(repo_path: &Path) -> Result<Option<Self>> {
        // Skip git operations entirely in CI to prevent hanging
        if std::env::var("CI").is_ok() || std::env::var("GITHUB_ACTIONS").is_ok() {
            debug!("CI environment detected - skipping git operations to prevent hanging");
            return Ok(None);
        }

        // First check if this is a git repository
        if !Self::is_git_repo(repo_path)? {
            debug!("Path is not a git repository: {}", repo_path.display());
            return Ok(None);
        }

        // Get repo root to work with
        let repo_root = Self::get_repo_root(repo_path)?;

        // Capture all git metadata
        let commit_hash = Self::get_current_commit_hash(&repo_root)
            .context("Failed to get current commit hash")?;

        let branch =
            Self::get_current_branch(&repo_root).context("Failed to get current branch")?;

        let is_dirty = Self::is_working_directory_dirty(&repo_root)
            .context("Failed to check working directory status")?;

        let remote_url = Self::get_remote_url(&repo_root).ok();

        Ok(Some(GitContext {
            commit_hash,
            branch,
            is_dirty,
            remote_url,
            repo_root,
        }))
    }

    /// Check if the git context has changed compared to another context
    pub fn has_changed(&self, other: &GitContext) -> bool {
        self.commit_hash != other.commit_hash
            || self.branch != other.branch
            || self.is_dirty != other.is_dirty
    }

    /// Check if only the branch has changed (same commit)
    pub fn has_branch_changed(&self, other: &GitContext) -> bool {
        self.branch != other.branch && self.commit_hash == other.commit_hash
    }

    /// Check if there are new commits (different commit, same branch)
    pub fn has_new_commits(&self, other: &GitContext) -> bool {
        self.branch == other.branch && self.commit_hash != other.commit_hash
    }

    /// Get list of files changed between two commits
    pub fn get_changed_files_between_commits(
        repo_root: &Path,
        from_commit: &str,
        to_commit: &str,
    ) -> Result<HashSet<PathBuf>> {
        let output = Command::new("git")
            .args([
                "diff",
                "--name-only",
                &format!("{from_commit}..{to_commit}"),
            ])
            .current_dir(repo_root)
            .output()
            .context("Failed to execute git diff")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("git diff failed: {}", stderr));
        }

        let stdout =
            String::from_utf8(output.stdout).context("Invalid UTF-8 in git diff output")?;

        let changed_files: HashSet<PathBuf> = stdout
            .lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| repo_root.join(line.trim()))
            .collect();

        debug!(
            "Found {} changed files between {} and {}",
            changed_files.len(),
            from_commit,
            to_commit
        );

        Ok(changed_files)
    }

    /// Get list of files changed since the last commit (including staged and unstaged)
    pub fn get_changed_files_since_commit(
        repo_root: &Path,
        commit: &str,
    ) -> Result<HashSet<PathBuf>> {
        // Get both staged and unstaged changes
        let mut changed_files = HashSet::new();

        // Changes between commit and index (staged)
        let staged_output = Command::new("git")
            .args(["diff", "--name-only", "--cached", commit])
            .current_dir(repo_root)
            .output()
            .context("Failed to get staged changes")?;

        if staged_output.status.success() {
            let stdout = String::from_utf8_lossy(&staged_output.stdout);
            for line in stdout.lines() {
                if !line.trim().is_empty() {
                    changed_files.insert(repo_root.join(line.trim()));
                }
            }
        }

        // Changes between index and working tree (unstaged)
        let unstaged_output = Command::new("git")
            .args(["diff", "--name-only"])
            .current_dir(repo_root)
            .output()
            .context("Failed to get unstaged changes")?;

        if unstaged_output.status.success() {
            let stdout = String::from_utf8_lossy(&unstaged_output.stdout);
            for line in stdout.lines() {
                if !line.trim().is_empty() {
                    changed_files.insert(repo_root.join(line.trim()));
                }
            }
        }

        // Untracked files
        let untracked_output = Command::new("git")
            .args(["ls-files", "--others", "--exclude-standard"])
            .current_dir(repo_root)
            .output()
            .context("Failed to get untracked files")?;

        if untracked_output.status.success() {
            let stdout = String::from_utf8_lossy(&untracked_output.stdout);
            for line in stdout.lines() {
                if !line.trim().is_empty() {
                    changed_files.insert(repo_root.join(line.trim()));
                }
            }
        }

        debug!(
            "Found {} changed files since commit {}",
            changed_files.len(),
            commit
        );

        Ok(changed_files)
    }

    /// Check if a path is within a git repository
    fn is_git_repo(path: &Path) -> Result<bool> {
        let output = Command::new("git")
            .args(["rev-parse", "--git-dir"])
            .current_dir(path)
            .output()
            .context("Failed to check if path is git repository")?;

        Ok(output.status.success())
    }

    /// Get the root directory of the git repository
    fn get_repo_root(path: &Path) -> Result<PathBuf> {
        let output = Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .current_dir(path)
            .output()
            .context("Failed to get git repository root")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to get git root: {}", stderr));
        }

        let stdout =
            String::from_utf8(output.stdout).context("Invalid UTF-8 in git root output")?;

        let root_path = stdout.trim();
        Ok(PathBuf::from(root_path))
    }

    /// Get the current commit hash (full SHA)
    fn get_current_commit_hash(repo_root: &Path) -> Result<String> {
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(repo_root)
            .output()
            .context("Failed to get current commit hash")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to get commit hash: {}", stderr));
        }

        let hash = String::from_utf8(output.stdout)
            .context("Invalid UTF-8 in commit hash")?
            .trim()
            .to_string();

        if hash.is_empty() {
            return Err(anyhow::anyhow!("Empty commit hash"));
        }

        Ok(hash)
    }

    /// Get the current branch name
    fn get_current_branch(repo_root: &Path) -> Result<String> {
        // Try modern git first
        let output = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(repo_root)
            .output()
            .context("Failed to get current branch")?;

        if output.status.success() {
            let branch = String::from_utf8(output.stdout)
                .context("Invalid UTF-8 in branch name")?
                .trim()
                .to_string();

            if !branch.is_empty() {
                return Ok(branch);
            }
        }

        // Fallback for older git versions
        let output = Command::new("git")
            .args(["symbolic-ref", "--short", "HEAD"])
            .current_dir(repo_root)
            .output()
            .context("Failed to get current branch (fallback)")?;

        if output.status.success() {
            let branch = String::from_utf8(output.stdout)
                .context("Invalid UTF-8 in branch name (fallback)")?
                .trim()
                .to_string();

            if !branch.is_empty() {
                return Ok(branch);
            }
        }

        // Final fallback: we might be in detached HEAD state
        warn!("Could not determine branch name, possibly in detached HEAD state");
        Ok("HEAD".to_string())
    }

    /// Check if the working directory has uncommitted changes
    fn is_working_directory_dirty(repo_root: &Path) -> Result<bool> {
        let output = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(repo_root)
            .output()
            .context("Failed to check working directory status")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Failed to check git status: {}", stderr);
            return Ok(false); // Assume clean on error
        }

        let stdout =
            String::from_utf8(output.stdout).context("Invalid UTF-8 in git status output")?;

        // If there's any output from git status --porcelain, there are changes
        Ok(!stdout.trim().is_empty())
    }

    /// Get the remote origin URL
    fn get_remote_url(repo_root: &Path) -> Result<String> {
        let output = Command::new("git")
            .args(["remote", "get-url", "origin"])
            .current_dir(repo_root)
            .output()
            .context("Failed to get remote URL")?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("No remote origin configured"));
        }

        let url = String::from_utf8(output.stdout)
            .context("Invalid UTF-8 in remote URL")?
            .trim()
            .to_string();

        if url.is_empty() {
            return Err(anyhow::anyhow!("Empty remote URL"));
        }

        Ok(url)
    }

    /// Get a short identifier for logging/display purposes
    pub fn short_id(&self) -> String {
        format!(
            "{}@{}{}",
            self.branch,
            &self.commit_hash[..8.min(self.commit_hash.len())],
            if self.is_dirty { "*" } else { "" }
        )
    }

    /// Check if a file path is within this git repository
    pub fn contains_path(&self, path: &Path) -> bool {
        path.canonicalize()
            .map(|canonical_path| canonical_path.starts_with(&self.repo_root))
            .unwrap_or(false)
    }

    /// Convert a path to be relative to the repository root
    pub fn relative_path(&self, path: &Path) -> Result<PathBuf> {
        let canonical_path = path.canonicalize().context("Failed to canonicalize path")?;

        canonical_path
            .strip_prefix(&self.repo_root)
            .map(|rel| rel.to_path_buf())
            .context("Path is not within git repository")
    }
}

/// Configuration for git-aware features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitConfig {
    /// Whether to track git commits for cache versioning
    pub track_commits: bool,
    /// Whether to preserve cache entries across branch switches
    pub preserve_across_branches: bool,
    /// Whether to namespace cache entries by branch
    pub namespace_by_branch: bool,
    /// Whether to automatically detect file changes for invalidation
    pub auto_detect_changes: bool,
    /// Maximum number of git contexts to keep in history
    pub max_history_depth: usize,
    /// Whether to check for changes when serving requests
    pub check_changes_on_request: bool,
    /// Interval for periodic git status checks (in seconds)
    pub periodic_check_interval_secs: u64,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            track_commits: true,
            preserve_across_branches: false,
            namespace_by_branch: false,
            auto_detect_changes: true,
            max_history_depth: 10,
            check_changes_on_request: false,
            periodic_check_interval_secs: 30,
        }
    }
}

impl GitConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(value) = std::env::var("PROBE_GIT_TRACK_COMMITS") {
            config.track_commits = value.to_lowercase() == "true";
        }

        if let Ok(value) = std::env::var("PROBE_GIT_PRESERVE_ACROSS_BRANCHES") {
            config.preserve_across_branches = value.to_lowercase() == "true";
        }

        if let Ok(value) = std::env::var("PROBE_GIT_NAMESPACE_BY_BRANCH") {
            config.namespace_by_branch = value.to_lowercase() == "true";
        }

        if let Ok(value) = std::env::var("PROBE_GIT_AUTO_DETECT_CHANGES") {
            config.auto_detect_changes = value.to_lowercase() == "true";
        }

        if let Ok(value) = std::env::var("PROBE_GIT_MAX_HISTORY_DEPTH") {
            if let Ok(depth) = value.parse::<usize>() {
                config.max_history_depth = depth;
            }
        }

        if let Ok(value) = std::env::var("PROBE_GIT_CHECK_CHANGES_ON_REQUEST") {
            config.check_changes_on_request = value.to_lowercase() == "true";
        }

        if let Ok(value) = std::env::var("PROBE_GIT_PERIODIC_CHECK_INTERVAL") {
            if let Ok(interval) = value.parse::<u64>() {
                config.periodic_check_interval_secs = interval;
            }
        }

        config
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<()> {
        if self.max_history_depth == 0 {
            return Err(anyhow::anyhow!("max_history_depth must be greater than 0"));
        }

        if self.periodic_check_interval_secs == 0 {
            return Err(anyhow::anyhow!(
                "periodic_check_interval_secs must be greater than 0"
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn setup_test_repo() -> Result<PathBuf> {
        let temp_dir = tempdir()?;
        let repo_path = temp_dir.path().to_path_buf();

        // Initialize git repo
        Command::new("git")
            .args(&["init"])
            .current_dir(&repo_path)
            .output()?;

        // Configure git user
        Command::new("git")
            .args(&["config", "user.name", "Test User"])
            .current_dir(&repo_path)
            .output()?;

        Command::new("git")
            .args(&["config", "user.email", "test@example.com"])
            .current_dir(&repo_path)
            .output()?;

        // Create and commit a file
        fs::write(repo_path.join("test.txt"), "initial content")?;
        Command::new("git")
            .args(&["add", "test.txt"])
            .current_dir(&repo_path)
            .output()?;
        Command::new("git")
            .args(&["commit", "-m", "Initial commit"])
            .current_dir(&repo_path)
            .output()?;

        Ok(repo_path)
    }

    #[test]
    fn test_git_context_capture() {
        let repo_path = setup_test_repo().expect("Failed to setup test repo");

        let context = GitContext::capture(&repo_path).unwrap();
        assert!(context.is_some());

        let ctx = context.unwrap();
        assert!(!ctx.commit_hash.is_empty());
        assert_eq!(ctx.branch, "master"); // git init creates master branch
        assert!(!ctx.is_dirty); // Clean after commit
        assert_eq!(ctx.repo_root, repo_path);
    }

    #[test]
    fn test_git_context_dirty_detection() {
        let repo_path = setup_test_repo().expect("Failed to setup test repo");

        // Modify file to make repo dirty
        fs::write(repo_path.join("test.txt"), "modified content").unwrap();

        let context = GitContext::capture(&repo_path).unwrap().unwrap();
        assert!(context.is_dirty);
    }

    #[test]
    fn test_git_context_comparison() {
        let repo_path = setup_test_repo().expect("Failed to setup test repo");

        let ctx1 = GitContext::capture(&repo_path).unwrap().unwrap();
        let ctx2 = GitContext::capture(&repo_path).unwrap().unwrap();

        // Should be identical
        assert!(!ctx1.has_changed(&ctx2));

        // Modify to make dirty
        fs::write(repo_path.join("test.txt"), "modified").unwrap();
        let ctx3 = GitContext::capture(&repo_path).unwrap().unwrap();

        // Should detect change
        assert!(ctx1.has_changed(&ctx3));
    }

    #[test]
    fn test_non_git_directory() {
        let temp_dir = tempdir().unwrap();
        let context = GitContext::capture(temp_dir.path()).unwrap();
        assert!(context.is_none());
    }

    #[test]
    fn test_git_config_from_env() {
        // Set some env vars
        std::env::set_var("PROBE_GIT_TRACK_COMMITS", "false");
        std::env::set_var("PROBE_GIT_PRESERVE_ACROSS_BRANCHES", "true");
        std::env::set_var("PROBE_GIT_MAX_HISTORY_DEPTH", "20");

        let config = GitConfig::from_env();
        assert!(!config.track_commits);
        assert!(config.preserve_across_branches);
        assert_eq!(config.max_history_depth, 20);

        // Cleanup
        std::env::remove_var("PROBE_GIT_TRACK_COMMITS");
        std::env::remove_var("PROBE_GIT_PRESERVE_ACROSS_BRANCHES");
        std::env::remove_var("PROBE_GIT_MAX_HISTORY_DEPTH");
    }

    #[test]
    fn test_git_config_validation() {
        let mut config = GitConfig::default();
        assert!(config.validate().is_ok());

        config.max_history_depth = 0;
        assert!(config.validate().is_err());

        config.max_history_depth = 5;
        config.periodic_check_interval_secs = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_short_id_generation() {
        let ctx = GitContext {
            commit_hash: "abcdef1234567890".to_string(),
            branch: "main".to_string(),
            is_dirty: false,
            remote_url: None,
            repo_root: PathBuf::from("/test"),
        };

        assert_eq!(ctx.short_id(), "main@abcdef12");

        let dirty_ctx = GitContext {
            is_dirty: true,
            ..ctx
        };
        assert_eq!(dirty_ctx.short_id(), "main@abcdef12*");
    }
}
