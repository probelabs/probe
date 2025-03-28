//! Go-specific path resolver implementation.

use super::PathResolver;
use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

/// A path resolver for Go packages.
pub struct GoPathResolver;

impl Default for GoPathResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl GoPathResolver {
    /// Creates a new Go path resolver.
    pub fn new() -> Self {
        GoPathResolver
    }
}

impl PathResolver for GoPathResolver {
    fn prefix(&self) -> &'static str {
        "go:"
    }

    fn resolve(&self, package_name: &str) -> Result<PathBuf, String> {
        // Run `go list -json <import-path>`
        let output = Command::new("go")
            .args(["list", "-json", package_name])
            .output()
            .map_err(|e| format!("Failed to execute 'go list': {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "Error running 'go list': {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let json_str = String::from_utf8_lossy(&output.stdout);
        let json: Value = serde_json::from_str(&json_str)
            .map_err(|e| format!("Failed to parse JSON output from 'go list': {}", e))?;

        // Extract the directory path
        if let Some(dir) = json["Dir"].as_str() {
            Ok(PathBuf::from(dir))
        } else {
            Err(format!(
                "No directory found for Go package: {}",
                package_name
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_go_path_resolver() {
        // Skip this test if go is not installed
        if Command::new("go").arg("version").output().is_err() {
            println!("Skipping test_go_path_resolver: Go is not installed");
            return;
        }

        let resolver = GoPathResolver::new();

        // Test with a standard library package
        let result = resolver.resolve("fmt");
        assert!(
            result.is_ok(),
            "Failed to resolve 'fmt' package: {:?}",
            result
        );

        // The path should exist and contain the package
        let path = result.unwrap();
        assert!(path.exists(), "Path does not exist: {:?}", path);
    }
}
