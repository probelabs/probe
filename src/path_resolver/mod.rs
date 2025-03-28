//! Module for resolving special path formats to filesystem paths.
//!
//! This module provides functionality to resolve special path formats like
//! "go:github.com/user/repo", "js:express", or "rust:serde" to actual filesystem paths.

mod go;
mod javascript;
mod rust;

use std::path::PathBuf;

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

    /// Resolves a package/module name to its filesystem location.
    ///
    /// # Arguments
    ///
    /// * `package_name` - The package/module name to resolve (without the prefix)
    ///
    /// # Returns
    ///
    /// * `Ok(PathBuf)` - The filesystem path where the package is located
    /// * `Err(String)` - An error message if resolution fails
    fn resolve(&self, package_name: &str) -> Result<PathBuf, String>;
}

/// Resolves a path that might contain special prefixes to an actual filesystem path.
///
/// Currently supported formats:
/// - "go:github.com/user/repo" - Resolves to the Go module's filesystem path
/// - "js:express" - Resolves to the JavaScript/Node.js package's filesystem path
/// - "rust:serde" - Resolves to the Rust crate's filesystem path
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
    // Create instances of all resolvers
    let resolvers: Vec<Box<dyn PathResolver>> = vec![
        Box::new(GoPathResolver::new()),
        Box::new(JavaScriptPathResolver::new()),
        Box::new(RustPathResolver::new()),
    ];

    // Find the appropriate resolver based on the path prefix
    for resolver in resolvers {
        let prefix = resolver.prefix();
        if let Some(package_name) = path.strip_prefix(prefix) {
            // Extract the package name (without the prefix)
            return resolver.resolve(package_name);
        }
    }

    // If no special prefix, return the path as is
    Ok(PathBuf::from(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_path_regular() {
        let path = "/some/regular/path";
        let result = resolve_path(path);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), PathBuf::from(path));
    }
}
