//! Path safety utilities for LSP daemon to avoid junction point cycles on Windows CI
//!
//! This module provides safe alternatives to canonicalize() and other path operations
//! that can trigger stack overflow when encountering Windows junction points.

use std::path::{Path, PathBuf};

/// Safely canonicalize a path, avoiding junction point cycles on Windows CI
pub fn safe_canonicalize(path: &Path) -> PathBuf {
    // On Windows CI, avoid canonicalize() completely to prevent junction point traversal
    #[cfg(target_os = "windows")]
    {
        if std::env::var("CI").is_ok() {
            // In CI, just convert to absolute path without following junctions
            if path.is_absolute() {
                return path.to_path_buf();
            } else {
                // Make relative paths absolute using current directory
                return std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(path);
            }
        }
    }

    // On non-Windows or non-CI, use regular canonicalize with fallback
    path.canonicalize().unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(path)
        }
    })
}
