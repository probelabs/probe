//! JavaScript/Node.js-specific path resolver implementation.

use super::PathResolver;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A path resolver for JavaScript/Node.js packages.
pub struct JavaScriptPathResolver;

impl Default for JavaScriptPathResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl JavaScriptPathResolver {
    /// Creates a new JavaScript path resolver.
    pub fn new() -> Self {
        JavaScriptPathResolver
    }

    /// Finds the nearest node_modules directory from the current directory upwards.
    fn find_node_modules(&self) -> Result<PathBuf, String> {
        // Start from the current directory
        let mut current_dir = std::env::current_dir()
            .map_err(|e| format!("Failed to get current directory: {}", e))?;

        // Look for node_modules in the current directory and its parents
        loop {
            let node_modules = current_dir.join("node_modules");
            if node_modules.exists() && node_modules.is_dir() {
                return Ok(node_modules);
            }

            // Go up one directory
            if !current_dir.pop() {
                // We've reached the root directory without finding node_modules
                return Err("Could not find node_modules directory".to_string());
            }
        }
    }

    /// Resolves a package using npm's resolve functionality.
    fn resolve_with_npm(&self, package_name: &str) -> Result<PathBuf, String> {
        // Use npm to resolve the package
        let output = Command::new("npm")
            .args(["root", "-g"])
            .output()
            .map_err(|e| format!("Failed to execute 'npm root -g': {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "Error running 'npm root -g': {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        // Get the global node_modules path
        let global_node_modules = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let global_package_path = Path::new(&global_node_modules).join(package_name);

        if global_package_path.exists() {
            return Ok(global_package_path);
        }

        // Try to find in local node_modules
        if let Ok(node_modules) = self.find_node_modules() {
            let local_package_path = node_modules.join(package_name);
            if local_package_path.exists() {
                return Ok(local_package_path);
            }
        }

        // If we couldn't find it, try using require.resolve
        let script = format!(
            "try {{ console.log(require.resolve('{}')) }} catch(e) {{ process.exit(1) }}",
            package_name
        );

        let output = Command::new("node")
            .args(["-e", &script])
            .output()
            .map_err(|e| format!("Failed to execute Node.js: {}", e))?;

        if output.status.success() {
            let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();

            // If the resolved path is a file, get its directory
            let path = PathBuf::from(&path_str);
            if path.is_file() {
                if let Some(parent) = path.parent() {
                    return Ok(parent.to_path_buf());
                }
            }

            return Ok(path);
        }

        Err(format!(
            "Could not resolve JavaScript package: {}",
            package_name
        ))
    }
}

impl PathResolver for JavaScriptPathResolver {
    fn prefix(&self) -> &'static str {
        "js:"
    }

    fn split_module_and_subpath(
        &self,
        full_path_after_prefix: &str,
    ) -> Result<(String, Option<String>), String> {
        if full_path_after_prefix.is_empty() {
            return Err("JavaScript path cannot be empty".to_string());
        }
        if full_path_after_prefix.contains("..") {
            return Err("JavaScript path cannot contain '..'".to_string());
        }

        // Trim potential trailing slash
        let path = full_path_after_prefix.trim_end_matches('/');

        if path.starts_with('@') {
            // Scoped: @scope/package/maybe/subpath
            let parts: Vec<&str> = path.splitn(3, '/').collect();
            match parts.len() {
                1 => Err(format!(
                    "Invalid scoped package format (missing package name): {}",
                    path
                )), // e.g., "@scope"
                2 => {
                    // e.g., "@scope/package"
                    let scope = parts[0];
                    let pkg = parts[1];
                    if scope.len() <= 1 || pkg.is_empty() || pkg.contains('/') {
                        Err(format!("Invalid scoped package format: {}", path))
                    } else {
                        let module_name = format!("{}/{}", scope, pkg);
                        Ok((module_name, None))
                    }
                }
                3 => {
                    // e.g., "@scope/package/subpath" or "@scope/package/"
                    let scope = parts[0];
                    let pkg = parts[1];
                    let sub = parts[2];
                    if scope.len() <= 1 || pkg.is_empty() || pkg.contains('/') {
                        Err(format!("Invalid scoped package format: {}", path))
                    } else {
                        let module_name = format!("{}/{}", scope, pkg);
                        // Handle trailing slash case "@scope/pkg/" -> subpath should be None
                        let subpath_opt = if sub.is_empty() {
                            None
                        } else {
                            Some(sub.to_string())
                        };
                        Ok((module_name, subpath_opt))
                    }
                }
                _ => unreachable!("splitn(3) limits len to 3"),
            }
        } else {
            // Regular: package/maybe/subpath
            let mut parts = path.splitn(2, '/');
            let module_name = parts.next().unwrap().to_string(); // Cannot fail on non-empty string
            if module_name.is_empty() || module_name.starts_with('/') {
                // Basic validation
                Err(format!("Invalid package format: {}", path))
            } else {
                let subpath_opt = parts.next().filter(|s| !s.is_empty()).map(String::from); // Handle trailing slash "pkg/"
                Ok((module_name, subpath_opt))
            }
        }
    }

    fn resolve(&self, module_name: &str) -> Result<PathBuf, String> {
        // First, check if this is a path to a package.json file
        let path = PathBuf::from(module_name);
        if path.exists()
            && path.is_file()
            && path.file_name().is_some_and(|name| name == "package.json")
        {
            // If it's a package.json file, return its directory
            return path.parent().map_or(
                Err("Could not determine parent directory of package.json".to_string()),
                |parent| Ok(parent.to_path_buf()),
            );
        }

        // If it's a directory containing package.json, return the directory
        let package_dir = PathBuf::from(module_name);
        let package_json = package_dir.join("package.json");
        if package_dir.exists() && package_dir.is_dir() && package_json.exists() {
            return Ok(package_dir);
        }

        // Otherwise, try to resolve it as a package name
        self.resolve_with_npm(module_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_js_path_resolver_with_directory() {
        // Create a temporary directory with a package.json file
        let temp_dir = tempfile::tempdir().unwrap();
        let package_json_path = temp_dir.path().join("package.json");

        // Write a minimal package.json
        fs::write(
            &package_json_path,
            r#"{"name": "test-package", "version": "1.0.0"}"#,
        )
        .expect("Failed to write package.json");

        let resolver = JavaScriptPathResolver::new();
        let result = resolver.resolve(temp_dir.path().to_str().unwrap());

        assert!(
            result.is_ok(),
            "Failed to resolve directory with package.json: {:?}",
            result
        );
        assert_eq!(result.unwrap(), temp_dir.path());
    }

    #[test]
    fn test_js_path_resolver_with_package_json() {
        // Create a temporary directory with a package.json file
        let temp_dir = tempfile::tempdir().unwrap();
        let package_json_path = temp_dir.path().join("package.json");

        // Write a minimal package.json
        fs::write(
            &package_json_path,
            r#"{"name": "test-package", "version": "1.0.0"}"#,
        )
        .expect("Failed to write package.json");

        let resolver = JavaScriptPathResolver::new();
        let result = resolver.resolve(package_json_path.to_str().unwrap());

        assert!(
            result.is_ok(),
            "Failed to resolve package.json: {:?}",
            result
        );
        assert_eq!(result.unwrap(), temp_dir.path());
    }

    #[test]
    fn test_js_path_resolver_npm_package() {
        // Skip this test if npm is not installed
        if Command::new("npm").arg("--version").output().is_err() {
            println!("Skipping test_js_path_resolver_npm_package: npm is not installed");
            return;
        }

        // This test is more complex as it requires npm to be installed
        // We'll try to resolve a common package that might be installed globally
        let resolver = JavaScriptPathResolver::new();

        // Try to find the node_modules directory first
        if resolver.find_node_modules().is_err() {
            println!("Skipping test_js_path_resolver_npm_package: node_modules not found");
            return;
        }

        // Try to resolve a common package like 'lodash' if it exists
        let result = resolver.resolve("lodash");
        if result.is_ok() {
            let path = result.unwrap();
            assert!(path.exists(), "Path does not exist: {:?}", path);

            // Check if it contains a package.json
            let package_json = path.join("package.json");
            assert!(
                package_json.exists(),
                "package.json not found: {:?}",
                package_json
            );
        } else {
            println!("Skipping assertion for 'lodash': Package not found");
        }
    }
}
