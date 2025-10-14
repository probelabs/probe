//! Version-Aware UID Generation
//!
//! This module provides a centralized, deterministic UID generation system that creates
//! consistent identifiers for symbols across both storage and query operations.
//!
//! ## UID Format
//! `"relative/path:content_hash:symbol_name:line_number"`
//!
//! ## Examples
//! - `"src/accounting/billing.rs:7f3a9c2d:calculate_total:42"`
//! - `"lib/utils/helpers.rs:a1b2c3d4:format_currency:128"`
//!
//! ## Benefits
//! - ✅ Branch isolation (different content = different hash)
//! - ✅ Edit detection (file changes = new hash = cache invalidation)
//! - ✅ Symbol uniqueness (line number prevents collisions)
//! - ✅ Workspace portability (relative paths work across clones)
//! - ✅ Deterministic generation (both paths create identical UIDs)

use crate::symbol::dependency_path::classify_absolute_path;
use anyhow::{Context, Result};
use blake3::Hasher as Blake3Hasher;
use std::path::Path;
use tracing::debug;

/// Generate a version-aware UID for a symbol
///
/// This function creates a deterministic UID that includes:
/// - Workspace-relative file path
/// - Content hash (first 8 chars of Blake3 hash)
/// - Symbol name
/// - Line number
///
/// # Arguments
/// * `workspace_root` - The root path of the workspace
/// * `file_path` - The absolute path to the file containing the symbol
/// * `file_content` - The content of the file (for hashing)
/// * `symbol_name` - The name of the symbol
/// * `line_number` - The line number where the symbol is located
///
/// # Returns
/// A Result containing the version-aware UID string
///
/// # Examples
/// ```rust
/// use std::path::Path;
/// use version_aware_uid::generate_version_aware_uid;
///
/// let workspace_root = Path::new("/home/user/project");
/// let file_path = Path::new("/home/user/project/src/main.rs");
/// let file_content = "fn main() { println!(\"Hello\"); }";
/// let symbol_name = "main";
/// let line_number = 1;
///
/// let uid = generate_version_aware_uid(
///     workspace_root,
///     file_path,
///     file_content,
///     symbol_name,
///     line_number
/// ).unwrap();
///
/// // Result: "src/main.rs:a1b2c3d4:main:1"
/// ```
pub fn generate_version_aware_uid(
    workspace_root: &Path,
    file_path: &Path,
    file_content: &str,
    symbol_name: &str,
    line_number: u32,
) -> Result<String> {
    // Input validation
    if symbol_name.is_empty() {
        return Err(anyhow::anyhow!("Symbol name cannot be empty"));
    }

    if line_number == 0 {
        return Err(anyhow::anyhow!("Line number must be greater than 0"));
    }

    // Get workspace-relative path using the provided anchor workspace root.
    // If the file is outside this workspace, this helper will classify it under
    // a stable /dep/... namespace (or EXTERNAL: as a last resort).
    let relative_path =
        get_workspace_relative_path(file_path, workspace_root).with_context(|| {
            format!(
                "Failed to get relative path for file: {} (workspace: {})",
                file_path.display(),
                workspace_root.display()
            )
        })?;

    // Generate content hash
    let content_hash = blake3_hash_file_content(file_content)
        .with_context(|| "Failed to generate content hash")?;

    // Construct the UID
    let uid = format!(
        "{}:{}:{}:{}",
        relative_path, content_hash, symbol_name, line_number
    );

    debug!(
        "[VERSION_AWARE_UID] Generated UID for '{}' at line {}: {}",
        symbol_name, line_number, uid
    );

    Ok(uid)
}

/// Get the relative path of a file within a workspace
///
/// # Arguments
/// * `file_path` - The absolute path to the file
/// * `workspace_root` - The root path of the workspace
///
/// # Returns
/// A Result containing the relative path as a string
///
/// # Edge Cases
/// - If file is outside workspace, uses absolute path with "EXTERNAL:" prefix
/// - If paths cannot be resolved, uses filename only with "UNRESOLVED:" prefix
pub fn get_workspace_relative_path(file_path: &Path, workspace_root: &Path) -> Result<String> {
    // Try to canonicalize paths for accurate comparison
    let canonical_file = file_path
        .canonicalize()
        .unwrap_or_else(|_| file_path.to_path_buf());
    let canonical_workspace = workspace_root
        .canonicalize()
        .unwrap_or_else(|_| workspace_root.to_path_buf());

    // Check if file is within workspace
    if let Ok(relative) = canonical_file.strip_prefix(&canonical_workspace) {
        Ok(relative.to_string_lossy().to_string())
    } else {
        // Fallback: attempt non-canonical strip_prefix in case canonicalization changed roots (e.g., symlinks)
        if let Ok(relative) = file_path.strip_prefix(workspace_root) {
            return Ok(relative.to_string_lossy().to_string());
        }

        // Last resort: try string-based prefix if paths are on the same drive but canonicalization differed
        let file_str = canonical_file.to_string_lossy();
        let ws_str = canonical_workspace.to_string_lossy();
        if file_str.starts_with(&*ws_str) {
            // Safe because starts_with guarantees ws_str length <= file_str length
            let mut rel = file_str[ws_str.len()..].to_string();
            // Trim any leading path separator
            if rel.starts_with('/') || rel.starts_with('\\') {
                rel.remove(0);
            }
            if !rel.is_empty() {
                return Ok(rel);
            }
        }

        // File is outside workspace — try to convert to canonical /dep/* path first
        if let Some(dep_path) = classify_absolute_path(&canonical_file) {
            debug!(
                "[VERSION_AWARE_UID] External file mapped to dependency path: {} -> {}",
                canonical_file.display(),
                dep_path
            );
            return Ok(dep_path);
        }

        // Fall back to explicit EXTERNAL prefix when we can't classify the ecosystem
        debug!(
            "[VERSION_AWARE_UID] File {} is outside workspace {}, using EXTERNAL path",
            file_path.display(),
            workspace_root.display()
        );
        Ok(format!("EXTERNAL:{}", file_path.to_string_lossy()))
    }
}

/// Generate a Blake3 hash of file content and return first 8 characters
///
/// # Arguments
/// * `content` - The file content to hash
///
/// # Returns
/// A Result containing the first 8 characters of the Blake3 hash as hex string
///
/// # Examples
/// ```rust
/// let content = "fn main() {}";
/// let hash = blake3_hash_file_content(content).unwrap();
/// assert_eq!(hash.len(), 8);
/// ```
pub fn blake3_hash_file_content(content: &str) -> Result<String> {
    if content.is_empty() {
        // Use a consistent hash for empty files
        return Ok("00000000".to_string());
    }

    let mut hasher = Blake3Hasher::new();
    hasher.update(content.as_bytes());
    let hash = hasher.finalize();

    // Take first 8 characters of hex representation
    let hash_hex = hash.to_hex().to_string();
    Ok(hash_hex.chars().take(8).collect())
}

/// Validate a version-aware UID format
///
/// # Arguments
/// * `uid` - The UID string to validate
///
/// # Returns
/// True if the UID matches the expected format, false otherwise
pub fn validate_version_aware_uid(uid: &str) -> bool {
    if uid.is_empty() {
        return false;
    }

    let parts: Vec<&str> = uid.split(':').collect();

    // Should have exactly 4 parts: path:hash:symbol:line
    if parts.len() != 4 {
        return false;
    }

    let (path_part, hash_part, symbol_part, line_part) = (parts[0], parts[1], parts[2], parts[3]);

    // Path part should not be empty
    if path_part.is_empty() {
        return false;
    }

    // Hash part should be exactly 8 hex characters
    if hash_part.len() != 8 || !hash_part.chars().all(|c| c.is_ascii_hexdigit()) {
        return false;
    }

    // Symbol part should not be empty
    if symbol_part.is_empty() {
        return false;
    }

    // Line part should be a positive integer
    if let Ok(line_num) = line_part.parse::<u32>() {
        line_num > 0
    } else {
        false
    }
}

/// Extract components from a version-aware UID
///
/// # Arguments
/// * `uid` - The UID string to parse
///
/// # Returns
/// A Result containing a tuple of (relative_path, content_hash, symbol_name, line_number)
pub fn parse_version_aware_uid(uid: &str) -> Result<(String, String, String, u32)> {
    if !validate_version_aware_uid(uid) {
        return Err(anyhow::anyhow!("Invalid UID format: {}", uid));
    }

    let parts: Vec<&str> = uid.split(':').collect();
    let relative_path = parts[0].to_string();
    let content_hash = parts[1].to_string();
    let symbol_name = parts[2].to_string();
    let line_number = parts[3]
        .parse::<u32>()
        .with_context(|| format!("Invalid line number in UID: {}", parts[3]))?;

    Ok((relative_path, content_hash, symbol_name, line_number))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_generate_version_aware_uid() {
        let workspace_root = PathBuf::from("/home/user/project");
        let file_path = PathBuf::from("/home/user/project/src/main.rs");
        let file_content = "fn main() { println!(\"Hello, world!\"); }";
        let symbol_name = "main";
        let line_number = 1;

        let uid = generate_version_aware_uid(
            &workspace_root,
            &file_path,
            file_content,
            symbol_name,
            line_number,
        )
        .unwrap();

        // Should have the expected format
        assert!(uid.starts_with("src/main.rs:"));
        assert!(uid.contains(":main:1"));
        assert_eq!(uid.split(':').count(), 4);
    }

    #[test]
    fn test_get_workspace_relative_path() {
        let workspace_root = PathBuf::from("/home/user/project");

        // File within workspace
        let file_path = PathBuf::from("/home/user/project/src/lib.rs");
        let relative = get_workspace_relative_path(&file_path, &workspace_root).unwrap();
        assert_eq!(relative, "src/lib.rs");

        // File outside workspace
        let external_file = PathBuf::from("/tmp/external.rs");
        let external_relative =
            get_workspace_relative_path(&external_file, &workspace_root).unwrap();
        assert!(external_relative.starts_with("EXTERNAL:"));
    }

    #[test]
    fn test_blake3_hash_file_content() {
        let content = "fn main() {}";
        let hash = blake3_hash_file_content(content).unwrap();

        // Should be exactly 8 hex characters
        assert_eq!(hash.len(), 8);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));

        // Empty content should return consistent hash
        let empty_hash = blake3_hash_file_content("").unwrap();
        assert_eq!(empty_hash, "00000000");

        // Same content should produce same hash
        let hash2 = blake3_hash_file_content(content).unwrap();
        assert_eq!(hash, hash2);

        // Different content should produce different hash
        let different_content = "fn other() {}";
        let different_hash = blake3_hash_file_content(different_content).unwrap();
        assert_ne!(hash, different_hash);
    }

    #[test]
    fn test_validate_version_aware_uid() {
        // Valid UIDs
        assert!(validate_version_aware_uid("src/main.rs:a1b2c3d4:main:1"));
        assert!(validate_version_aware_uid(
            "lib/utils.rs:12345678:helper:42"
        ));
        assert!(validate_version_aware_uid(
            "EXTERNAL:/tmp/file.rs:abcdef12:func:100"
        ));

        // Invalid UIDs
        assert!(!validate_version_aware_uid(""));
        assert!(!validate_version_aware_uid("invalid"));
        assert!(!validate_version_aware_uid("a:b:c")); // too few parts
        assert!(!validate_version_aware_uid("a:b:c:d:e")); // too many parts
        assert!(!validate_version_aware_uid(":hash:symbol:1")); // empty path
        assert!(!validate_version_aware_uid("path::symbol:1")); // empty hash
        assert!(!validate_version_aware_uid("path:hash::1")); // empty symbol
        assert!(!validate_version_aware_uid("path:hash:symbol:0")); // invalid line number
        assert!(!validate_version_aware_uid("path:hash:symbol:abc")); // non-numeric line
        assert!(!validate_version_aware_uid("path:1234567:symbol:1")); // hash too short
        assert!(!validate_version_aware_uid("path:123456789:symbol:1")); // hash too long
        assert!(!validate_version_aware_uid("path:1234567g:symbol:1")); // non-hex in hash
    }

    #[test]
    fn test_parse_version_aware_uid() {
        let uid = "src/main.rs:a1b2c3d4:main:42";
        let (path, hash, symbol, line) = parse_version_aware_uid(uid).unwrap();

        assert_eq!(path, "src/main.rs");
        assert_eq!(hash, "a1b2c3d4");
        assert_eq!(symbol, "main");
        assert_eq!(line, 42);

        // Invalid UID should fail
        assert!(parse_version_aware_uid("invalid:uid").is_err());
    }

    #[test]
    fn test_edge_cases() {
        let workspace_root = PathBuf::from("/project");
        let file_content = "fn test() {}";

        // Test with empty symbol name
        let result = generate_version_aware_uid(
            &workspace_root,
            &PathBuf::from("/project/main.rs"),
            file_content,
            "",
            1,
        );
        assert!(result.is_err());

        // Test with zero line number
        let result = generate_version_aware_uid(
            &workspace_root,
            &PathBuf::from("/project/main.rs"),
            file_content,
            "test",
            0,
        );
        assert!(result.is_err());

        // Test with special characters in symbol name
        let uid = generate_version_aware_uid(
            &workspace_root,
            &PathBuf::from("/project/main.rs"),
            file_content,
            "operator+",
            10,
        )
        .unwrap();
        assert!(uid.contains("operator+"));
    }

    #[test]
    fn test_content_hash_consistency() {
        let workspace_root = PathBuf::from("/project");
        let file_path = PathBuf::from("/project/src/test.rs");
        let symbol_name = "test_func";
        let line_number = 10;

        // Same content should produce same UID
        let content1 = "fn test_func() { return 42; }";
        let uid1 = generate_version_aware_uid(
            &workspace_root,
            &file_path,
            content1,
            symbol_name,
            line_number,
        )
        .unwrap();

        let uid2 = generate_version_aware_uid(
            &workspace_root,
            &file_path,
            content1,
            symbol_name,
            line_number,
        )
        .unwrap();

        assert_eq!(uid1, uid2);

        // Different content should produce different UID
        let content2 = "fn test_func() { return 43; }";
        let uid3 = generate_version_aware_uid(
            &workspace_root,
            &file_path,
            content2,
            symbol_name,
            line_number,
        )
        .unwrap();

        assert_ne!(uid1, uid3);

        // Only the hash part should be different
        let parts1: Vec<&str> = uid1.split(':').collect();
        let parts3: Vec<&str> = uid3.split(':').collect();

        assert_eq!(parts1[0], parts3[0]); // same path
        assert_ne!(parts1[1], parts3[1]); // different hash
        assert_eq!(parts1[2], parts3[2]); // same symbol
        assert_eq!(parts1[3], parts3[3]); // same line
    }
}
