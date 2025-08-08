//! Version utilities for probe
//!
//! This module provides utilities for getting version information at runtime.

/// Get the version string from Cargo.toml
pub fn get_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Get the package name from Cargo.toml
pub fn get_package_name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

/// Get a formatted version string with package name
pub fn get_version_info() -> String {
    format!("{} {}", get_package_name(), get_version())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_version() {
        let version = get_version();
        assert!(!version.is_empty());
        // Should follow semantic versioning pattern
        assert!(version.contains('.'));
    }

    #[test]
    fn test_get_package_name() {
        let name = get_package_name();
        assert_eq!(name, "probe-code");
    }

    #[test]
    fn test_get_version_info() {
        let info = get_version_info();
        assert!(info.contains("probe-code"));
        assert!(info.contains('.'));
    }
}