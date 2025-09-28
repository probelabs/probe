//! Simple workspace detection utilities
//!
//! This module provides reliable workspace detection logic copied from the working
//! manual LSP commands. It replaces the complex WorkspaceResolver that was causing
//! empty workspace paths in the enrichment workers.

use anyhow::{Context, Result};
use dashmap::DashSet;
use once_cell::sync::Lazy;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use toml_edit::{Array, DocumentMut, Item, Table, Value};
use tracing::{debug, info, warn};

use crate::language_detector::Language;
use crate::path_safety;
use crate::path_safety::safe_canonicalize;

static RUST_MEMBERSHIP_CACHE: Lazy<DashSet<PathBuf>> = Lazy::new(|| DashSet::new());

/// Find workspace root by looking for common project markers
///
/// This function searches upward from the given file path looking for workspace markers.
/// For Cargo workspaces, it specifically looks for a root Cargo.toml with [workspace] section.
/// For PHP projects, it prioritizes the nearest composer.json over parent git repositories.
/// For other projects, it returns the topmost directory containing a workspace marker.
///
/// This approach consolidates all files in a workspace under a single LSP workspace registration.
pub fn find_workspace_root(file_path: &Path) -> Option<PathBuf> {
    let mut current = file_path.parent()?;

    // Check if this is a PHP file to apply special workspace detection
    let is_php_file = file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase() == "php")
        .unwrap_or(false);

    debug!(
        "WORKSPACE_UTILS: Processing file {:?}, is_php_file: {}",
        file_path, is_php_file
    );

    // Look for common project root markers in priority order
    let markers = [
        "Cargo.toml",     // Rust
        "package.json",   // JavaScript/TypeScript
        "go.mod",         // Go
        "pyproject.toml", // Python
        "setup.py",       // Python
        "composer.json",  // PHP - prioritized before .git for PHP files
        "tsconfig.json",  // TypeScript
        ".git",           // Generic VCS
        "pom.xml",        // Java
        "build.gradle",   // Java/Gradle
        "CMakeLists.txt", // C/C++
    ];

    let mut found_workspace: Option<PathBuf> = None;
    let mut depth = 0;

    // Search upward and keep the topmost workspace found
    while current.parent().is_some() && depth < 10 {
        for marker in &markers {
            let marker_path = current.join(marker);
            if marker_path.exists() {
                debug!(
                    "Found workspace marker '{}' at: {}",
                    marker,
                    current.display()
                );

                // Special handling for Cargo.toml: check if it's a workspace root
                if *marker == "Cargo.toml" {
                    if is_cargo_workspace_root(&marker_path) {
                        debug!("Found Cargo workspace root at: {}", current.display());
                        return Some(current.to_path_buf());
                    }
                }

                // Special handling for PHP files: prefer composer.json over .git
                if is_php_file && *marker == "composer.json" {
                    debug!(
                        "Found PHP project root with composer.json at: {}",
                        current.display()
                    );
                    return Some(current.to_path_buf());
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
        debug!(
            "No workspace markers found for file: {}",
            file_path.display()
        );
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
    let fallback = file_path.parent().unwrap_or(file_path).to_path_buf();

    debug!("Using fallback workspace root: {}", fallback.display());
    Ok(fallback)
}

/// Check if a path looks like a workspace root by checking for common markers
pub fn is_workspace_root(path: &Path) -> bool {
    let markers = [
        "Cargo.toml",
        "package.json",
        "go.mod",
        "pyproject.toml",
        "setup.py",
        ".git",
        "tsconfig.json",
        "composer.json",
        "pom.xml",
        "build.gradle",
        "CMakeLists.txt",
    ];

    markers.iter().any(|marker| path.join(marker).exists())
}

/// Resolve the workspace directory that should be used when talking to an LSP server.
///
/// For most languages this is equivalent to `find_workspace_root_with_fallback`, but
/// Rust workspaces require additional handling so that nested crates that are not
/// explicitly listed in the parent `[workspace]` are still analyzable. When such a
/// crate is detected, this helper automatically amends the parent workspace manifest
/// to include the crate as a member.
pub fn resolve_lsp_workspace_root(language: Language, file_path: &Path) -> Result<PathBuf> {
    let canonical_file = safe_canonicalize(file_path);

    match language {
        Language::Rust => {
            if let Some(crate_root) = find_nearest_with_marker(&canonical_file, "Cargo.toml") {
                let crate_manifest = crate_root.join("Cargo.toml");
                if path_safety::exists_no_follow(&crate_manifest) {
                    // Look for a parent workspace manifest that owns this crate.
                    if let Some(workspace_root) = find_rust_workspace_root(&crate_root)? {
                        ensure_rust_workspace_membership(&crate_root, &workspace_root)?;
                        return Ok(workspace_root);
                    }

                    return Ok(crate_root);
                }
            }

            // Fallback to the generic detection if we couldn't find a crate manifest.
            find_workspace_root_with_fallback(&canonical_file)
        }
        _ => find_workspace_root_with_fallback(&canonical_file),
    }
}

fn find_nearest_with_marker(file_path: &Path, marker: &str) -> Option<PathBuf> {
    let mut current = file_path.parent();

    while let Some(dir) = current {
        let marker_path = dir.join(marker);
        if path_safety::exists_no_follow(&marker_path) {
            return Some(dir.to_path_buf());
        }
        current = dir.parent();
    }

    None
}

fn find_rust_workspace_root(crate_root: &Path) -> Result<Option<PathBuf>> {
    let mut current = crate_root.parent();

    while let Some(dir) = current {
        let manifest_path = dir.join("Cargo.toml");
        if path_safety::exists_no_follow(&manifest_path) {
            if has_workspace_section(&manifest_path)? {
                return Ok(Some(dir.to_path_buf()));
            }
        }
        current = dir.parent();
    }

    Ok(None)
}

fn has_workspace_section(manifest_path: &Path) -> Result<bool> {
    let content = fs::read_to_string(manifest_path)
        .with_context(|| format!("Failed to read manifest: {}", manifest_path.display()))?;

    let doc = content
        .parse::<DocumentMut>()
        .with_context(|| format!("Failed to parse manifest: {}", manifest_path.display()))?;

    Ok(doc.get("workspace").is_some())
}

fn ensure_rust_workspace_membership(crate_root: &Path, workspace_root: &Path) -> Result<()> {
    // If the crate is the workspace root, nothing to do.
    if safe_canonicalize(crate_root) == safe_canonicalize(workspace_root) {
        return Ok(());
    }

    let crate_real = safe_canonicalize(crate_root);
    if RUST_MEMBERSHIP_CACHE.contains(&crate_real) {
        return Ok(());
    }

    let workspace_real = safe_canonicalize(workspace_root);
    let manifest_path = workspace_real.join("Cargo.toml");

    let mut content = fs::read_to_string(&manifest_path).with_context(|| {
        format!(
            "Failed to read workspace manifest at {}",
            manifest_path.display()
        )
    })?;

    let mut doc = content.parse::<DocumentMut>().with_context(|| {
        format!(
            "Failed to parse workspace manifest at {}",
            manifest_path.display()
        )
    })?;

    let workspace_entry = doc.entry("workspace").or_insert(Item::Table(Table::new()));

    let members_item = workspace_entry
        .as_table_mut()
        .expect("workspace entry should be a table")
        .entry("members")
        .or_insert(Item::Value(Value::Array(Array::new())));

    let members_array = members_item
        .as_array_mut()
        .expect("workspace.members should be an array");

    let relative_path =
        pathdiff::diff_paths(&crate_real, &workspace_real).unwrap_or_else(|| PathBuf::from("."));

    let mut relative_str = relative_path.to_string_lossy().replace('\\', "/");
    if relative_str.is_empty() {
        relative_str = ".".to_string();
    }

    let already_member = members_array
        .iter()
        .any(|entry| entry.as_str().map(|s| s == relative_str).unwrap_or(false));

    let mut modified = false;
    if !already_member {
        members_array.push(Value::from(relative_str.clone()));
        modified = true;
        info!(
            "Added '{}' to workspace members in {}",
            relative_str,
            manifest_path.display()
        );
    }

    // If the path is present in workspace.exclude remove it, otherwise the
    // member we just added will still be ignored by cargo.
    if let Some(exclude_array) = workspace_entry
        .as_table_mut()
        .and_then(|table| table.get_mut("exclude"))
        .and_then(|item| item.as_array_mut())
    {
        let mut indices_to_remove = Vec::new();
        for (idx, entry) in exclude_array.iter().enumerate() {
            if entry.as_str().map(|s| s == relative_str).unwrap_or(false) {
                indices_to_remove.push(idx);
            }
        }

        if !indices_to_remove.is_empty() {
            for idx in indices_to_remove.iter().rev() {
                exclude_array.remove(*idx);
            }
            modified = true;
            info!(
                "Removed '{}' from workspace exclude list in {}",
                relative_str,
                manifest_path.display()
            );
        }
    }

    if modified {
        content = doc.to_string();
        fs::write(&manifest_path, content).with_context(|| {
            format!(
                "Failed to update workspace manifest at {}",
                manifest_path.display()
            )
        })?;

        // Run a quick cargo metadata check to ensure the manifest remains valid.
        match Command::new("cargo")
            .arg("metadata")
            .arg("--format-version")
            .arg("1")
            .arg("--manifest-path")
            .arg(&manifest_path)
            .status()
        {
            Ok(status) if status.success() => {
                debug!(
                    "cargo metadata succeeded after updating {}",
                    manifest_path.display()
                );
            }
            Ok(status) => {
                warn!(
                    "cargo metadata exited with status {} after updating {}",
                    status,
                    manifest_path.display()
                );
            }
            Err(e) => {
                warn!(
                    "Failed to run cargo metadata after updating {}: {}",
                    manifest_path.display(),
                    e
                );
            }
        }
    }

    RUST_MEMBERSHIP_CACHE.insert(crate_real);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language_detector::Language;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_find_workspace_root_with_cargo_toml() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().join("project");
        let src_dir = project_root.join("src");

        fs::create_dir_all(&src_dir).unwrap();
        fs::write(
            project_root.join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )
        .unwrap();

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
        let deep_dir = temp_dir
            .path()
            .join("isolated")
            .join("no-workspace")
            .join("deep");
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
        let deep_dir = temp_dir
            .path()
            .join("isolated")
            .join("no-workspace")
            .join("deep");
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
        fs::write(
            rust_project.join("Cargo.toml"),
            "[package]\nname = \"test\"",
        )
        .unwrap();

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
        )
        .unwrap();

        // Write member crate Cargo.toml
        fs::write(
            member_crate.join("Cargo.toml"),
            "[package]\nname = \"member\"",
        )
        .unwrap();

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

    #[test]
    fn test_resolve_lsp_workspace_root_adds_missing_member() {
        let temp_dir = TempDir::new().unwrap();
        let workspace_root = temp_dir.path().join("workspace");
        let existing_member = workspace_root.join("existing");
        let missing_member = workspace_root.join("member");
        let missing_src = missing_member.join("src");

        fs::create_dir_all(&existing_member.join("src")).unwrap();
        fs::create_dir_all(&missing_src).unwrap();

        // Workspace manifest with one existing member and exclude containing the missing member.
        fs::write(
            workspace_root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"existing\"]\nexclude = [\"member\"]\n",
        )
        .unwrap();

        // Existing member manifest (minimal crate)
        fs::write(
            existing_member.join("Cargo.toml"),
            "[package]\nname = \"existing\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        fs::write(existing_member.join("src/lib.rs"), "pub fn existing() {}\n").unwrap();

        // Missing member manifest (not yet listed in workspace)
        fs::write(
            missing_member.join("Cargo.toml"),
            "[package]\nname = \"member\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        fs::write(missing_src.join("lib.rs"), "pub fn member() {}\n").unwrap();

        // Clear membership cache to observe behavior in test
        RUST_MEMBERSHIP_CACHE.clear();

        let file_path = missing_src.join("lib.rs");
        let result_root = resolve_lsp_workspace_root(Language::Rust, &file_path)
            .expect("expected workspace resolution to succeed");

        assert_eq!(result_root, workspace_root);

        let manifest = std::fs::read_to_string(workspace_root.join("Cargo.toml")).unwrap();
        assert!(manifest.contains("\"member\""));
        assert!(!manifest.contains("exclude = [\"member\"]"));
    }
}
