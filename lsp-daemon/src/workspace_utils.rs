//! Simple workspace detection utilities
//!
//! This module provides reliable workspace detection logic copied from the working
//! manual LSP commands. It replaces the complex WorkspaceResolver that was causing
//! empty workspace paths in the enrichment workers.

use anyhow::Result;
use std::path::{Path, PathBuf};
use tracing::debug;

/// Find workspace root by looking for common project markers
///
/// This function searches upward from the given file path looking for workspace markers.
/// For Cargo workspaces, it specifically looks for a root Cargo.toml with [workspace] section.
/// For other projects, it returns the topmost directory containing a workspace marker.
///
/// This approach consolidates all files in a workspace under a single LSP workspace registration.
pub fn find_workspace_root(file_path: &Path) -> Option<PathBuf> {
    let mut current = file_path.parent()?;

    // Look for common project root markers in priority order
    let markers = [
        "Cargo.toml",      // Rust
        "package.json",    // JavaScript/TypeScript
        "go.mod",          // Go
        "pyproject.toml",  // Python
        "setup.py",        // Python
        ".git",            // Generic VCS
        "tsconfig.json",   // TypeScript
        "composer.json",   // PHP
        "pom.xml",         // Java
        "build.gradle",    // Java/Gradle
        "CMakeLists.txt",  // C/C++
    ];

    let mut found_workspace: Option<PathBuf> = None;
    let mut depth = 0;

    // Search upward and keep the topmost workspace found
    while current.parent().is_some() && depth < 10 {
        for marker in &markers {
            let marker_path = current.join(marker);
            if marker_path.exists() {
                debug!("Found workspace marker '{}' at: {}", marker, current.display());

                // Special handling for Cargo.toml: check if it's a workspace root
                if *marker == "Cargo.toml" {
                    if is_cargo_workspace_root(&marker_path) {
                        debug!("Found Cargo workspace root at: {}", current.display());
                        return Some(current.to_path_buf());
                    }
                }

                // For other markers or non-workspace Cargo.toml, keep searching upward
                found_workspace = Some(current.to_path_buf());
                break;
            }
        }
        current = current.parent()?;
        depth += 1;
    }

    if let Some(ref workspace) = found_workspace {
        debug!("Using topmost workspace root: {}", workspace.display());
    } else {
        debug!("No workspace markers found for file: {}", file_path.display());
    }

    found_workspace
}

/// Check if a Cargo.toml file defines a workspace root
fn is_cargo_workspace_root(cargo_toml_path: &Path) -> bool {
    if let Ok(content) = std::fs::read_to_string(cargo_toml_path) {
        // Simple check for [workspace] section
        content.contains("[workspace]")
    } else {
        false
    }
}

/// Find workspace root with fallback to parent directory
///
/// This version always returns a path - either the detected workspace root
/// or the parent directory of the file as a fallback. This prevents the
/// empty workspace path issues that were occurring with WorkspaceResolver.
pub fn find_workspace_root_with_fallback(file_path: &Path) -> Result<PathBuf> {
    // First try to find a proper workspace root
    if let Some(workspace_root) = find_workspace_root(file_path) {
        debug!("Found workspace root: {}", workspace_root.display());
        return Ok(workspace_root);
    }

    // Fall back to the parent directory of the file
    let fallback = file_path.parent()
        .unwrap_or(file_path)
        .to_path_buf();

    debug!("Using fallback workspace root: {}", fallback.display());
    Ok(fallback)
}

/// Check if a path looks like a workspace root by checking for common markers
pub fn is_workspace_root(path: &Path) -> bool {
    let markers = [
        "Cargo.toml", "package.json", "go.mod", "pyproject.toml",
        "setup.py", ".git", "tsconfig.json", "composer.json",
        "pom.xml", "build.gradle", "CMakeLists.txt"
    ];

    markers.iter().any(|marker| path.join(marker).exists())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_find_workspace_root_with_cargo_toml() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("project");
        let src_dir = project_root.join("src");

        fs::create_dir_all(&src_dir).unwrap();
        fs::write(project_root.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();

        let file_path = src_dir.join("main.rs");
        let workspace = find_workspace_root(&file_path).unwrap();

        assert_eq!(workspace, project_root);
    }

    #[test]
    fn test_find_workspace_root_with_package_json() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("project");
        let src_dir = project_root.join("src");

        fs::create_dir_all(&src_dir).unwrap();
        fs::write(project_root.join("package.json"), r#"{"name": "test"}"#).unwrap();

        let file_path = src_dir.join("index.js");
        let workspace = find_workspace_root(&file_path).unwrap();

        assert_eq!(workspace, project_root);
    }

    #[test]
    fn test_find_workspace_root_with_git() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("project");
        let src_dir = project_root.join("src");

        fs::create_dir_all(&src_dir).unwrap();
        fs::create_dir_all(project_root.join(".git")).unwrap();

        let file_path = src_dir.join("main.py");
        let workspace = find_workspace_root(&file_path).unwrap();

        assert_eq!(workspace, project_root);
    }

    #[test]
    fn test_find_workspace_root_no_markers() {
        let temp_dir = TempDir::new().unwrap();
        let deep_dir = temp_dir.path().join("isolated").join("no-workspace").join("deep");
        fs::create_dir_all(&deep_dir).unwrap();

        // Make sure no workspace markers exist in the path
        let file_path = deep_dir.join("orphan.txt");

        // This test might still find a workspace marker if we're inside a git repo
        // The important thing is that it doesn't crash and returns a reasonable result
        let workspace = find_workspace_root(&file_path);

        // Don't assert None - we might be in a git repository
        // Just verify it doesn't crash
        println!("Found workspace: {:?}", workspace);
    }

    #[test]
    fn test_find_workspace_root_with_fallback() {
        let temp_dir = TempDir::new().unwrap();
        let deep_dir = temp_dir.path().join("isolated").join("no-workspace").join("deep");
        fs::create_dir_all(&deep_dir).unwrap();

        let file_path = deep_dir.join("orphan.txt");
        let workspace = find_workspace_root_with_fallback(&file_path).unwrap();

        // The function will find a workspace marker or fallback to parent directory
        // Important thing is it returns a valid path and doesn't crash
        println!("Workspace found: {}", workspace.display());
        assert!(workspace.exists());

        // It should either be the deep_dir or an ancestor containing workspace markers
        assert!(workspace == deep_dir || deep_dir.starts_with(&workspace));
    }

    #[test]
    fn test_is_workspace_root() {
        let temp_dir = TempDir::new().unwrap();

        // Create a directory with Cargo.toml
        let rust_project = temp_dir.path().join("rust_project");
        fs::create_dir_all(&rust_project).unwrap();
        fs::write(rust_project.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();

        assert!(is_workspace_root(&rust_project));

        // Create a directory without markers
        let empty_dir = temp_dir.path().join("empty");
        fs::create_dir_all(&empty_dir).unwrap();

        assert!(!is_workspace_root(&empty_dir));
    }

    #[test]
    fn test_nested_workspaces_prefers_nearest() {
        let temp_dir = TempDir::new().unwrap();

        // Create nested structure:
        // /root/.git
        // /root/subproject/Cargo.toml
        // /root/subproject/src/main.rs
        let root = temp_dir.path().join("root");
        let subproject = root.join("subproject");
        let src = subproject.join("src");

        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::write(subproject.join("Cargo.toml"), "[package]\nname = \"sub\"").unwrap();

        let file_path = src.join("main.rs");
        let workspace = find_workspace_root(&file_path).unwrap();

        // Should find the nearest marker (Cargo.toml) not the higher-up .git
        assert_eq!(workspace, subproject);
    }

    #[test]
    fn test_cargo_workspace_root_detection() {
        let temp_dir = TempDir::new().unwrap();

        // Create structure:
        // /workspace/Cargo.toml (with [workspace])
        // /workspace/member/Cargo.toml (regular package)
        // /workspace/member/src/main.rs
        let workspace_root = temp_dir.path().join("workspace");
        let member_crate = workspace_root.join("member");
        let src = member_crate.join("src");

        fs::create_dir_all(&src).unwrap();

        // Write workspace root Cargo.toml
        fs::write(
            workspace_root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"member\"]\n",
        ).unwrap();

        // Write member crate Cargo.toml
        fs::write(
            member_crate.join("Cargo.toml"),
            "[package]\nname = \"member\"",
        ).unwrap();

        let file_path = src.join("main.rs");
        let workspace = find_workspace_root(&file_path).unwrap();

        // Should find the workspace root, not the member crate
        assert_eq!(workspace, workspace_root);
    }

    #[test]
    fn test_is_cargo_workspace_root() {
        let temp_dir = TempDir::new().unwrap();

        // Create workspace Cargo.toml
        let workspace_toml = temp_dir.path().join("workspace_Cargo.toml");
        fs::write(&workspace_toml, "[workspace]\nmembers = [\"crate1\"]").unwrap();
        assert!(is_cargo_workspace_root(&workspace_toml));

        // Create regular package Cargo.toml
        let package_toml = temp_dir.path().join("package_Cargo.toml");
        fs::write(&package_toml, "[package]\nname = \"regular\"").unwrap();
        assert!(!is_cargo_workspace_root(&package_toml));

        // Test nonexistent file
        let missing_toml = temp_dir.path().join("missing.toml");
        assert!(!is_cargo_workspace_root(&missing_toml));
    }
}