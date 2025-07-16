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

    fn split_module_and_subpath(
        &self,
        full_path_after_prefix: &str,
    ) -> Result<(String, Option<String>), String> {
        if full_path_after_prefix.is_empty() {
            return Err("Go path cannot be empty".to_string());
        }
        if full_path_after_prefix.contains("..") {
            return Err("Go path cannot contain '..'".to_string());
        }

        // Trim potential trailing slash
        let path = full_path_after_prefix.trim_end_matches('/');

        // Common external host heuristic
        let parts: Vec<&str> = path.split('/').collect();
        let is_common_external = parts.len() >= 3
            && (parts[0] == "github.com"
                || parts[0] == "gitlab.com"
                || parts[0] == "bitbucket.org"
                || (parts[0] == "golang.org" && parts[1] == "x"));

        if is_common_external {
            // Assume module is host/user_or_x/repo_or_pkg (first 3 parts)
            let module_name = parts[..3].join("/");
            let subpath = if parts.len() > 3 {
                Some(parts[3..].join("/")).filter(|s| !s.is_empty()) // Ensure subpath isn't just ""
            } else {
                None
            };
            Ok((module_name, subpath))
        } else {
            // For standard library packages, we need to handle file paths specially
            // Check if the last part looks like a file (has an extension)
            if parts.len() > 1 && parts.last().unwrap().contains('.') {
                // Assume the last part is a file and everything before is the module
                let file_part = parts.last().unwrap();
                let module_parts = &parts[..parts.len() - 1];
                let module_name = module_parts.join("/");
                Ok((module_name, Some(file_part.to_string())))
            } else {
                // Fallback: Assume the *entire* path is the module identifier
                // This covers:
                // - Simple stdlib ("fmt")
                // - Stdlib with slashes ("net/http", "net/http/pprof")
                // - Less common external paths ("mycorp.com/internal/pkg")
                Ok((path.to_string(), None))
            }
        }
    }

    fn resolve(&self, module_name: &str) -> Result<PathBuf, String> {
        // Check if Go is installed before trying to run it
        if Command::new("go").arg("version").output().is_err() {
            return Err(
                "Go command not found. Please ensure Go is installed and in your PATH.".to_string(),
            );
        }

        // Run `go list -json <import-path>`
        let output = Command::new("go")
            .args(["list", "-json", module_name])
            .output()
            .map_err(|e| format!("Failed to execute 'go list': {e}"))?;

        if !output.status.success() {
            return Err(format!(
                "Error running 'go list': {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let json_str = String::from_utf8_lossy(&output.stdout);
        let json: Value = serde_json::from_str(&json_str)
            .map_err(|e| format!("Failed to parse JSON output from 'go list': {e}"))?;

        // Extract the directory path
        if let Some(dir) = json["Dir"].as_str() {
            Ok(PathBuf::from(dir))
        } else {
            Err(format!("No directory found for Go package: {module_name}"))
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
