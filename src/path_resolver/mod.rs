//! Module for resolving special path formats to filesystem paths.
//!
//! This module provides functionality to resolve special path formats like
//! "go:github.com/user/repo", "js:express", or "rust:serde" to actual filesystem paths.

mod go;
mod javascript;
mod rust;

use std::path::{Path, PathBuf};

pub use go::GoPathResolver;
pub use javascript::JavaScriptPathResolver;
pub use rust::RustPathResolver;

/// A trait for language-specific path resolvers.
///
/// Implementations of this trait provide language-specific logic for resolving
/// package/module names to filesystem paths.
pub trait PathResolver {
    /// The prefix used to identify paths for this resolver (e.g., "go:", "js:", "rust:").
    fn prefix(&self) -> &'static str;

    /// Splits the path string (after the prefix) into the core module/package
    /// identifier and an optional subpath.
    ///
    /// For example, for Go:
    /// - "fmt" -> Ok(("fmt", None))
    /// - "net/http" -> Ok(("net/http", None)) // Stdlib multi-segment
    /// - "github.com/gin-gonic/gin" -> Ok(("github.com/gin-gonic/gin", None))
    /// - "github.com/gin-gonic/gin/examples/basic" -> Ok(("github.com/gin-gonic/gin", Some("examples/basic")))
    ///
    /// For JavaScript:
    /// - "lodash" -> Ok(("lodash", None))
    /// - "lodash/get" -> Ok(("lodash", Some("get")))
    /// - "@types/node" -> Ok(("@types/node", None))
    /// - "@types/node/fs" -> Ok(("@types/node", Some("fs")))
    ///
    /// # Arguments
    /// * `full_path_after_prefix` - The portion of the input path string that comes *after* the resolver's prefix.
    ///
    /// # Returns
    /// * `Ok((String, Option<String>))` - A tuple containing the resolved module name and an optional subpath string.
    /// * `Err(String)` - An error message if the path format is invalid for this resolver.
    fn split_module_and_subpath(
        &self,
        full_path_after_prefix: &str,
    ) -> Result<(String, Option<String>), String>;

    /// Resolves a package/module name to its filesystem location.
    ///
    /// # Arguments
    ///
    /// * `module_name` - The package/module name to resolve (without any subpath)
    ///
    /// # Returns
    ///
    /// * `Ok(PathBuf)` - The filesystem path where the package is located
    /// * `Err(String)` - An error message if resolution fails
    fn resolve(&self, module_name: &str) -> Result<PathBuf, String>;
}

/// Resolves a path that might contain special prefixes to an actual filesystem path.
///
/// Currently supported formats:
/// - "go:github.com/user/repo" - Resolves to the Go module's filesystem path
/// - "js:express" - Resolves to the JavaScript/Node.js package's filesystem path
/// - "rust:serde" - Resolves to the Rust crate's filesystem path
/// - "/dep/go/fmt" - Alternative notation for "go:fmt"
/// - "/dep/js/express" - Alternative notation for "js:express"
/// - "/dep/rust/serde" - Alternative notation for "rust:serde"
///
/// # Arguments
///
/// * `path` - The path to resolve, which might contain special prefixes
///
/// # Returns
///
/// * `Ok(PathBuf)` - The resolved filesystem path
/// * `Err(String)` - An error message if resolution fails
pub fn resolve_path(path: &str) -> Result<PathBuf, String> {
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    // Check if this is a Windows absolute path (e.g., C:\, D:\, etc.)
    // Windows paths have a drive letter followed by a colon and backslash or forward slash
    if path.len() >= 3 {
        let chars: Vec<char> = path.chars().collect();
        if chars[1] == ':'
            && chars[0].is_ascii_alphabetic()
            && (chars[2] == '\\' || chars[2] == '/')
        {
            // This is a Windows absolute path, return it as-is
            if debug_mode {
                println!(
                    "DEBUG: resolve_path - Detected Windows absolute path: {}",
                    path
                );
            }
            return Ok(PathBuf::from(path));
        }
    }

    // Create instances of all resolvers
    let resolvers: Vec<Box<dyn PathResolver>> = vec![
        Box::new(GoPathResolver::new()),
        Box::new(JavaScriptPathResolver::new()),
        Box::new(RustPathResolver::new()),
    ];

    // Check for /dep/ prefix notation
    if let Some(dep_path) = path.strip_prefix("/dep/") {
        // Extract the language identifier (e.g., "go", "js", "rust")
        let parts: Vec<&str> = dep_path.splitn(2, '/').collect();
        if parts.is_empty() {
            return Err("Invalid /dep/ path: missing language identifier".to_string());
        }

        let lang_id = parts[0];
        let remainder = parts.get(1).unwrap_or(&"");

        // Map language identifier to resolver prefix
        let prefix = match lang_id {
            "go" => "go:",
            "js" => "js:",
            "rust" => "rust:",
            _ => {
                return Err(format!(
                    "Unknown language identifier in /dep/ path: {lang_id}"
                ))
            }
        };

        // Find the appropriate resolver
        for resolver in &resolvers {
            if resolver.prefix() == prefix {
                // 1. Split the path into module name and optional subpath
                let (module_name, subpath_opt) =
                    resolver.split_module_and_subpath(remainder).map_err(|e| {
                        format!("Failed to parse path '{remainder}' for prefix '{prefix}': {e}")
                    })?;

                // 2. Resolve the base directory of the module
                let module_base_path = resolver.resolve(&module_name).map_err(|e| {
                    format!("Failed to resolve module '{module_name}' for prefix '{prefix}': {e}")
                })?;

                // 3. Combine base path with subpath if it exists
                let final_path = match subpath_opt {
                    Some(sub) if !sub.is_empty() => {
                        // Ensure subpath is treated as relative
                        let relative_subpath = Path::new(&sub)
                            .strip_prefix("/")
                            .unwrap_or_else(|_| Path::new(&sub));
                        module_base_path.join(relative_subpath)
                    }
                    _ => module_base_path, // No subpath or empty subpath
                };

                return Ok(final_path);
            }
        }

        // This should not happen if all language identifiers are properly mapped
        return Err(format!("No resolver found for language: {lang_id}"));
    }

    // Find the appropriate resolver based on the path prefix
    for resolver in resolvers {
        let prefix = resolver.prefix();
        if !prefix.ends_with(':') {
            // Internal sanity check
            eprintln!("Warning: PathResolver prefix '{prefix}' does not end with ':'");
            continue;
        }

        if let Some(full_path_after_prefix) = path.strip_prefix(prefix) {
            // 1. Split the path into module name and optional subpath
            let (module_name, subpath_opt) = resolver
                .split_module_and_subpath(full_path_after_prefix)
                .map_err(|e| {
                    format!(
                        "Failed to parse path '{full_path_after_prefix}' for prefix '{prefix}': {e}"
                    )
                })?;

            // 2. Resolve the base directory of the module
            let module_base_path = resolver.resolve(&module_name).map_err(|e| {
                format!("Failed to resolve module '{module_name}' for prefix '{prefix}': {e}")
            })?;

            // 3. Combine base path with subpath if it exists
            let final_path = match subpath_opt {
                Some(sub) if !sub.is_empty() => {
                    // Ensure subpath is treated as relative
                    let relative_subpath = Path::new(&sub)
                        .strip_prefix("/")
                        .unwrap_or_else(|_| Path::new(&sub));
                    module_base_path.join(relative_subpath)
                }
                _ => module_base_path, // No subpath or empty subpath
            };

            return Ok(final_path);
        }
    }

    // If no special prefix, return the path as is
    Ok(PathBuf::from(path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn test_resolve_path_regular() {
        let path = "/some/regular/path";
        let result = resolve_path(path);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PathBuf::from(path));
    }

    #[test]
    fn test_resolve_path_dep_prefix() {
        // Skip this test if go is not installed
        if Command::new("go").arg("version").output().is_err() {
            println!("Skipping test_resolve_path_dep_prefix: Go is not installed");
            return;
        }

        // Test with standard library package using /dep/go prefix
        let result = resolve_path("/dep/go/fmt");

        // Compare with traditional go: prefix
        let traditional_result = resolve_path("go:fmt");

        assert!(
            result.is_ok(),
            "Failed to resolve '/dep/go/fmt': {result:?}"
        );
        assert!(
            traditional_result.is_ok(),
            "Failed to resolve 'go:fmt': {traditional_result:?}"
        );

        // Both paths should resolve to the same location
        assert_eq!(result.unwrap(), traditional_result.unwrap());
    }

    #[test]
    fn test_invalid_dep_path() {
        // Test with invalid /dep/ path (missing language identifier)
        let result = resolve_path("/dep/");
        assert!(result.is_err());

        // Test with unknown language identifier
        let result = resolve_path("/dep/unknown/package");
        assert!(result.is_err());
    }
}
