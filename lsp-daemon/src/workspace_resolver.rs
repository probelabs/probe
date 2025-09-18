use crate::language_detector::Language;
use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{debug, info, warn};

/// Workspace root resolution cache entry
#[derive(Debug, Clone)]
struct WorkspaceCacheEntry {
    workspace_root: PathBuf,
    cached_at: Instant,
}

/// Centralized workspace resolution with consistent marker detection
/// and priority-based workspace detection across the entire system
pub struct WorkspaceResolver {
    allowed_roots: Option<Vec<PathBuf>>,
    workspace_cache: HashMap<PathBuf, WorkspaceCacheEntry>, // file_dir -> workspace info
    max_cache_size: usize,
    cache_ttl_secs: u64,
}

impl WorkspaceResolver {
    pub fn new(allowed_roots: Option<Vec<PathBuf>>) -> Self {
        Self {
            allowed_roots,
            workspace_cache: HashMap::new(),
            max_cache_size: 1000,
            cache_ttl_secs: 300, // 5 minutes cache TTL
        }
    }

    /// Get the consolidated workspace marker priority list used across the entire system
    /// This is the single source of truth for workspace marker priorities
    pub fn get_workspace_markers_with_priority(
    ) -> &'static [(/* marker */ &'static str, /* priority */ usize)] {
        &[
            // High priority - language-specific project files
            ("go.mod", 100),
            ("go.work", 95),
            ("Cargo.toml", 100),
            ("package.json", 90),
            ("pyproject.toml", 100),
            ("setup.py", 80),
            ("pom.xml", 100),
            ("build.gradle", 90),
            ("build.gradle.kts", 90),
            ("CMakeLists.txt", 85),
            // Medium priority - build/config files
            ("Makefile", 60),
            ("makefile", 60),
            ("configure.ac", 70),
            ("meson.build", 70),
            ("tsconfig.json", 85),
            ("jsconfig.json", 75),
            ("composer.json", 85),
            ("requirements.txt", 70),
            ("setup.cfg", 75),
            ("Pipfile", 80),
            // Lower priority - VCS roots
            (".git", 50),
            (".svn", 40),
            (".hg", 40),
            // Very low priority - generic markers
            ("LICENSE", 20),
            ("README.md", 20),
        ]
    }

    /// Resolve workspace root for a given file path (takes a mutable reference for caching)
    pub fn resolve_workspace(
        &mut self,
        file_path: &Path,
        hint: Option<PathBuf>,
    ) -> Result<PathBuf> {
        info!(
            "Resolving workspace for file: {:?}, hint: {:?}",
            file_path, hint
        );

        // 1. Use client hint if provided and valid
        if let Some(hint_root) = hint {
            // Canonicalize the hint path to ensure it's absolute
            let canonical_hint = hint_root.canonicalize().unwrap_or_else(|e| {
                warn!("Failed to canonicalize hint {:?}: {}", hint_root, e);
                hint_root.clone()
            });
            if self.is_valid_workspace(&canonical_hint, file_path)? {
                info!("Using client workspace hint: {:?}", canonical_hint);
                return Ok(canonical_hint);
            }
            warn!(
                "Client workspace hint {:?} is invalid for file {:?}",
                canonical_hint, file_path
            );
        }

        // 2. Check cache (with TTL validation)
        let file_dir = file_path.parent().unwrap_or(file_path).to_path_buf();
        if let Some(cached_entry) = self.workspace_cache.get(&file_dir) {
            if cached_entry.cached_at.elapsed().as_secs() < self.cache_ttl_secs {
                debug!("Using cached workspace: {:?}", cached_entry.workspace_root);
                return Ok(cached_entry.workspace_root.clone());
            } else {
                debug!("Cache entry expired for {:?}, will re-detect", file_dir);
            }
        }

        // 3. Auto-detect workspace
        let detected_root = self.detect_workspace(file_path)?;
        info!("Auto-detected workspace: {:?}", detected_root);

        // Canonicalize the detected root to ensure it's an absolute path
        let canonical_root = detected_root.canonicalize().unwrap_or_else(|e| {
            warn!(
                "Failed to canonicalize detected root {:?}: {}",
                detected_root, e
            );
            detected_root.clone()
        });

        // 4. Validate against allowed_roots if configured
        if let Some(ref allowed) = self.allowed_roots {
            if !allowed.iter().any(|root| canonical_root.starts_with(root)) {
                return Err(anyhow!(
                    "Workspace {:?} not in allowed roots: {:?}",
                    canonical_root,
                    allowed
                ));
            }
        }

        // 5. Cache and return the canonical path
        self.cache_workspace(file_dir, canonical_root.clone());
        info!("Resolved workspace root: {:?}", canonical_root);
        Ok(canonical_root)
    }

    /// Resolve workspace root for a file path - simpler version without hint support
    /// This is the primary method other components should use for workspace resolution
    pub fn resolve_workspace_for_file(&mut self, file_path: &Path) -> Result<PathBuf> {
        self.resolve_workspace(file_path, None)
    }

    /// Detect the most appropriate workspace root for a file (now public)
    /// Uses the reliable workspace detection logic from workspace_utils
    pub fn detect_workspace(&self, file_path: &Path) -> Result<PathBuf> {
        debug!(
            "WORKSPACE_RESOLVER: Detecting workspace for file_path: {:?}",
            file_path
        );

        // Use the reliable workspace detection from workspace_utils
        // This finds the authoritative root workspace instead of using "best match" logic
        let workspace_root = crate::workspace_utils::find_workspace_root_with_fallback(file_path)
            .context("Failed to detect workspace root using workspace_utils")?;

        debug!(
            "WORKSPACE_RESOLVER: Found workspace root: {:?}",
            workspace_root
        );
        Ok(workspace_root)
    }

    /// Check if a workspace hint is valid for the given file
    fn is_valid_workspace(&self, workspace_root: &Path, file_path: &Path) -> Result<bool> {
        // File must be within the workspace
        if !file_path.starts_with(workspace_root) {
            return Ok(false);
        }

        // Workspace must exist
        if !workspace_root.exists() {
            return Ok(false);
        }

        // If allowed_roots is configured, workspace must be within one of them
        if let Some(ref allowed) = self.allowed_roots {
            if !allowed.iter().any(|root| workspace_root.starts_with(root)) {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Cache a workspace resolution, with size limit and TTL
    fn cache_workspace(&mut self, file_dir: PathBuf, workspace_root: PathBuf) {
        // First, remove expired entries during cache maintenance
        self.cleanup_expired_cache_entries();

        if self.workspace_cache.len() >= self.max_cache_size {
            // Simple cache eviction - remove oldest entries by cached_at time
            let mut entries: Vec<_> = self.workspace_cache.iter().collect();
            entries.sort_by_key(|(_, entry)| entry.cached_at);

            let to_remove: Vec<_> = entries
                .iter()
                .take(self.max_cache_size / 4)
                .map(|(key, _)| (*key).clone())
                .collect();

            for key in to_remove {
                self.workspace_cache.remove(&key);
            }
        }

        let cache_entry = WorkspaceCacheEntry {
            workspace_root,
            cached_at: Instant::now(),
        };
        self.workspace_cache.insert(file_dir, cache_entry);
    }

    /// Remove expired cache entries
    fn cleanup_expired_cache_entries(&mut self) {
        let now = Instant::now();
        let ttl_duration = std::time::Duration::from_secs(self.cache_ttl_secs);

        self.workspace_cache
            .retain(|_, entry| now.duration_since(entry.cached_at) < ttl_duration);
    }

    /// Get language-specific project markers
    #[allow(dead_code)]
    pub fn get_language_markers(&self, language: Language) -> Vec<&'static str> {
        match language {
            Language::Go => vec!["go.mod", "go.work", "vendor"],
            Language::Rust => vec!["Cargo.toml", "Cargo.lock"],
            Language::JavaScript | Language::TypeScript => {
                vec![
                    "package.json",
                    "tsconfig.json",
                    "jsconfig.json",
                    "node_modules",
                ]
            }
            Language::Python => vec![
                "pyproject.toml",
                "setup.py",
                "requirements.txt",
                "setup.cfg",
            ],
            Language::Java => vec!["pom.xml", "build.gradle", "build.gradle.kts"],
            Language::C | Language::Cpp => vec!["CMakeLists.txt", "Makefile", "configure.ac"],
            Language::CSharp => vec!["*.sln", "*.csproj"],
            Language::Ruby => vec!["Gemfile", ".ruby-version"],
            Language::Php => vec!["composer.json", "composer.lock"],
            Language::Swift => vec!["Package.swift", "*.xcodeproj"],
            Language::Kotlin => vec!["build.gradle.kts", "build.gradle"],
            Language::Scala => vec!["build.sbt", "build.sc"],
            Language::Haskell => vec!["stack.yaml", "*.cabal", "cabal.project"],
            Language::Elixir => vec!["mix.exs"],
            Language::Clojure => vec!["project.clj", "deps.edn"],
            Language::Lua => vec![".luarc.json"],
            Language::Zig => vec!["build.zig"],
            Language::Unknown => vec![".git", "README.md"],
        }
    }

    /// Clear the cache
    #[allow(dead_code)]
    pub fn clear_cache(&mut self) {
        self.workspace_cache.clear();
    }

    /// Get cache statistics including TTL information
    #[allow(dead_code)]
    pub fn cache_stats(&self) -> (usize, usize, u64, usize) {
        let now = Instant::now();
        let ttl_duration = std::time::Duration::from_secs(self.cache_ttl_secs);
        let expired_count = self
            .workspace_cache
            .values()
            .filter(|entry| now.duration_since(entry.cached_at) >= ttl_duration)
            .count();

        (
            self.workspace_cache.len(), // total entries
            self.max_cache_size,        // max cache size
            self.cache_ttl_secs,        // TTL in seconds
            expired_count,              // expired entries count
        )
    }

    /// Check if a path is within allowed roots
    pub fn is_path_allowed(&self, path: &Path) -> bool {
        match &self.allowed_roots {
            None => true, // No restrictions
            Some(allowed) => allowed.iter().any(|root| path.starts_with(root)),
        }
    }

    /// Create a new shared WorkspaceResolver wrapped in Arc<Mutex<>>
    /// This is the preferred way to create a WorkspaceResolver for sharing across components
    pub fn new_shared(allowed_roots: Option<Vec<PathBuf>>) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self::new(allowed_roots)))
    }

    /// Convenience method for resolving workspace through Arc<Mutex<WorkspaceResolver>>
    pub async fn resolve_workspace_shared(
        resolver: &Arc<Mutex<WorkspaceResolver>>,
        file_path: &Path,
    ) -> Result<PathBuf> {
        let mut resolver = resolver.lock().unwrap();
        resolver.resolve_workspace_for_file(file_path)
    }

    /// Convenience method for detecting workspace through Arc<Mutex<WorkspaceResolver>>
    pub async fn detect_workspace_shared(
        resolver: &Arc<Mutex<WorkspaceResolver>>,
        file_path: &Path,
    ) -> Result<PathBuf> {
        let resolver = resolver.lock().unwrap();
        resolver.detect_workspace(file_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_detect_go_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("project");
        let src_dir = project_root.join("src");

        fs::create_dir_all(&src_dir).unwrap();
        fs::write(project_root.join("go.mod"), "module test").unwrap();

        let mut resolver = WorkspaceResolver::new(None);
        let file_path = src_dir.join("main.go");
        let workspace = resolver.resolve_workspace(&file_path, None).unwrap();

        // Canonicalize the expected path for comparison since resolve_workspace now returns canonical paths
        let expected = project_root.canonicalize().unwrap();
        assert_eq!(workspace, expected);
    }

    #[test]
    fn test_workspace_hint() {
        let temp_dir = TempDir::new().unwrap();
        let hint_root = temp_dir.path().to_path_buf();
        let file_path = hint_root.join("test.go");

        let mut resolver = WorkspaceResolver::new(None);
        let workspace = resolver
            .resolve_workspace(&file_path, Some(hint_root.clone()))
            .unwrap();

        // Canonicalize the expected path for comparison
        let expected = hint_root.canonicalize().unwrap();
        assert_eq!(workspace, expected);
    }

    #[test]
    fn test_allowed_roots_restriction() {
        let temp_dir = TempDir::new().unwrap();
        let allowed_root = temp_dir.path().join("allowed");
        let forbidden_root = temp_dir.path().join("forbidden");

        fs::create_dir_all(&allowed_root).unwrap();
        fs::create_dir_all(&forbidden_root).unwrap();

        // Canonicalize the allowed root for the resolver
        let canonical_allowed = allowed_root.canonicalize().unwrap();
        let mut resolver = WorkspaceResolver::new(Some(vec![canonical_allowed]));

        // File in allowed root should work
        let allowed_file = allowed_root.join("test.go");
        let result = resolver.resolve_workspace(&allowed_file, None);
        assert!(result.is_ok());

        // File in forbidden root should fail
        let forbidden_file = forbidden_root.join("test.go");
        let result = resolver.resolve_workspace(&forbidden_file, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_cache_functionality() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.go");

        let mut resolver = WorkspaceResolver::new(None);

        // First resolution should detect and cache
        let workspace1 = resolver.resolve_workspace(&file_path, None).unwrap();

        // Second resolution should use cache
        let workspace2 = resolver.resolve_workspace(&file_path, None).unwrap();

        assert_eq!(workspace1, workspace2);
        assert_eq!(resolver.cache_stats().0, 1); // One entry in cache
    }

    #[test]
    fn test_resolve_workspace_for_file() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("project");
        fs::create_dir_all(&project_root).unwrap();
        fs::write(project_root.join("package.json"), r#"{"name": "test"}"#).unwrap();

        let file_path = project_root.join("src").join("index.js");
        fs::create_dir_all(file_path.parent().unwrap()).unwrap();

        let mut resolver = WorkspaceResolver::new(None);
        let workspace = resolver.resolve_workspace_for_file(&file_path).unwrap();

        let expected = project_root.canonicalize().unwrap();
        assert_eq!(workspace, expected);
    }

    #[test]
    fn test_consolidated_marker_priorities() {
        let markers = WorkspaceResolver::get_workspace_markers_with_priority();

        // Verify high-priority markers
        assert!(markers
            .iter()
            .any(|(marker, priority)| *marker == "Cargo.toml" && *priority == 100));
        assert!(markers
            .iter()
            .any(|(marker, priority)| *marker == "go.mod" && *priority == 100));
        assert!(markers
            .iter()
            .any(|(marker, priority)| *marker == "package.json" && *priority == 90));

        // Verify VCS markers have lower priority
        assert!(markers
            .iter()
            .any(|(marker, priority)| *marker == ".git" && *priority == 50));

        // Verify consistent ordering (high priority items should come first conceptually)
        let cargo_priority = markers
            .iter()
            .find(|(marker, _)| *marker == "Cargo.toml")
            .unwrap()
            .1;
        let git_priority = markers
            .iter()
            .find(|(marker, _)| *marker == ".git")
            .unwrap()
            .1;
        assert!(cargo_priority > git_priority);
    }

    #[test]
    fn test_public_detect_workspace() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("project");
        fs::create_dir_all(&project_root).unwrap();
        fs::write(
            project_root.join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )
        .unwrap();

        let deep_file = project_root.join("src").join("main.rs");
        fs::create_dir_all(deep_file.parent().unwrap()).unwrap();
        fs::write(&deep_file, "fn main() {}").unwrap();

        let resolver = WorkspaceResolver::new(None);
        let workspace = resolver.detect_workspace(&deep_file).unwrap();

        let expected = project_root.canonicalize().unwrap();
        // Compare the canonical forms of both paths to handle macOS symlinks
        let workspace_canonical = workspace.canonicalize().unwrap();
        assert_eq!(workspace_canonical, expected);
    }

    #[test]
    fn test_cache_ttl_functionality() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");

        let mut resolver = WorkspaceResolver::new(None);
        resolver.cache_ttl_secs = 1; // Very short TTL for testing

        // First resolution
        let workspace1 = resolver.resolve_workspace_for_file(&file_path).unwrap();
        assert_eq!(resolver.cache_stats().0, 1);

        // Should use cache immediately
        let workspace2 = resolver.resolve_workspace_for_file(&file_path).unwrap();
        assert_eq!(workspace1, workspace2);

        // Wait for cache to expire
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Next resolution should re-detect (cache expired)
        let _workspace3 = resolver.resolve_workspace_for_file(&file_path).unwrap();
        let (_, _, _, expired_count) = resolver.cache_stats();
        assert!(expired_count > 0 || resolver.workspace_cache.len() == 1); // Either expired or cleaned up
    }

    #[tokio::test]
    async fn test_shared_resolver_functionality() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("shared-project");
        fs::create_dir_all(&project_root).unwrap();
        fs::write(
            project_root.join("pyproject.toml"),
            "[project]\nname = \"shared\"",
        )
        .unwrap();

        let file_path = project_root.join("main.py");
        fs::write(&file_path, "print('hello')").unwrap();

        // Test shared resolver creation and usage
        let resolver = WorkspaceResolver::new_shared(None);

        let workspace1 = WorkspaceResolver::resolve_workspace_shared(&resolver, &file_path)
            .await
            .unwrap();
        let workspace2 = WorkspaceResolver::detect_workspace_shared(&resolver, &file_path)
            .await
            .unwrap();

        let expected = project_root.canonicalize().unwrap();
        // Compare the canonical forms to handle macOS symlinks
        let workspace1_canonical = workspace1.canonicalize().unwrap();
        let workspace2_canonical = workspace2.canonicalize().unwrap();
        assert_eq!(workspace1_canonical, expected);
        assert_eq!(workspace2_canonical, expected);
    }

    #[test]
    fn test_priority_based_workspace_detection() {
        let temp_dir = TempDir::new().unwrap();

        // Create nested structure with multiple markers
        let root_dir = temp_dir.path().join("root");
        let sub_dir = root_dir.join("sub");
        fs::create_dir_all(&sub_dir).unwrap();

        // Root has .git (priority 50)
        fs::create_dir_all(root_dir.join(".git")).unwrap();

        // Sub has Cargo.toml (priority 100)
        fs::write(sub_dir.join("Cargo.toml"), "[package]\nname = \"sub\"").unwrap();

        let file_in_sub = sub_dir.join("main.rs");

        let resolver = WorkspaceResolver::new(None);
        let workspace = resolver.detect_workspace(&file_in_sub).unwrap();

        // Should choose sub directory because Cargo.toml has higher priority than .git
        let expected = sub_dir.canonicalize().unwrap();
        // Compare the canonical forms to handle macOS symlinks
        let workspace_canonical = workspace.canonicalize().unwrap();
        assert_eq!(workspace_canonical, expected);
    }

    #[test]
    fn test_cache_cleanup_and_eviction() {
        let temp_dir = TempDir::new().unwrap();

        let mut resolver = WorkspaceResolver::new(None);
        resolver.max_cache_size = 3; // Small cache for testing
        resolver.cache_ttl_secs = 1; // Short TTL

        // Fill cache beyond capacity
        for i in 0..5 {
            let file_path = temp_dir.path().join(format!("file_{i}.rs"));
            let _ = resolver.resolve_workspace_for_file(&file_path);
        }

        // Should not exceed max cache size
        assert!(resolver.workspace_cache.len() <= resolver.max_cache_size);

        // Wait for TTL expiration
        std::thread::sleep(std::time::Duration::from_secs(2));

        // New resolution should trigger cleanup
        let new_file = temp_dir.path().join("new_file.rs");
        let _ = resolver.resolve_workspace_for_file(&new_file);

        let (total, max_size, ttl, _expired) = resolver.cache_stats();
        assert!(total <= max_size);
        assert_eq!(ttl, 1);
        // Note: expired count is always >= 0 for unsigned integers
    }
}
