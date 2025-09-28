use pathdiff::diff_paths;
use std::path::{Component, Path, PathBuf};

use crate::workspace_utils;

/// Normalize the path component of a version-aware UID.
///
/// * `uid` – UID in the format `path:hash:name:line`
/// * `workspace_hint` – Optional workspace root that should be treated as the anchor for
///   relative paths. When `None`, the workspace root is inferred using
///   `workspace_utils::find_workspace_root_with_fallback`.
pub fn normalize_uid_with_hint(uid: &str, workspace_hint: Option<&Path>) -> String {
    if uid.is_empty()
        || uid.starts_with("EXTERNAL:")
        || uid.starts_with("UNRESOLVED:")
        || uid.starts_with("fallback_")
    {
        return uid.to_string();
    }

    let mut parts = uid.splitn(4, ':');
    let path_part = match parts.next() {
        Some(part) => part,
        None => return uid.to_string(),
    };
    let hash_part = match parts.next() {
        Some(part) => part,
        None => return uid.to_string(),
    };
    let name_part = match parts.next() {
        Some(part) => part,
        None => return uid.to_string(),
    };
    let line_part = match parts.next() {
        Some(part) => part,
        None => return uid.to_string(),
    };

    if !is_absolute_like(path_part) {
        return uid.to_string();
    }

    let absolute_path = Path::new(path_part);
    let canonical_file = absolute_path
        .canonicalize()
        .unwrap_or_else(|_| absolute_path.to_path_buf());

    if !canonical_file.is_absolute() {
        return uid.to_string();
    }

    let workspace_root = workspace_hint
        .map(Path::to_path_buf)
        .or_else(|| infer_workspace_root(&canonical_file))
        .unwrap_or_else(|| {
            canonical_file
                .parent()
                .unwrap_or_else(|| Path::new("/"))
                .to_path_buf()
        });

    let canonical_root = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.clone());

    if canonical_file == canonical_root {
        return uid.to_string();
    }

    if let Some(relative_path) = diff_paths(&canonical_file, &canonical_root) {
        if relative_path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return uid.to_string();
        }

        let mut normalized = relative_path.to_string_lossy().replace('\\', "/");
        while normalized.starts_with('/') {
            normalized.remove(0);
        }

        if normalized.is_empty() {
            return uid.to_string();
        }

        return format!("{}:{}:{}:{}", normalized, hash_part, name_part, line_part);
    }

    uid.to_string()
}

/// Returns true if the provided path string looks like an absolute path.
pub fn is_absolute_like(path: &str) -> bool {
    if path.is_empty() {
        return false;
    }

    if path.starts_with('/') || path.starts_with('\\') {
        return true;
    }

    if path.len() >= 2 {
        let bytes = path.as_bytes();
        return bytes[1] == b':' && (bytes[0].is_ascii_alphabetic());
    }

    false
}

fn infer_workspace_root(file_path: &Path) -> Option<PathBuf> {
    workspace_utils::find_workspace_root_with_fallback(file_path).ok()
}
