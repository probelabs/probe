use std::path::PathBuf;

/// Get the default socket/pipe path for the current platform
pub fn get_default_socket_path() -> String {
    #[cfg(unix)]
    {
        std::env::temp_dir()
            .join("lsp-daemon.sock")
            .to_string_lossy()
            .to_string()
    }

    #[cfg(windows)]
    {
        r"\\.\pipe\lsp-daemon".to_string()
    }
}

/// Check if a socket/pipe path exists
pub fn socket_exists(path: &str) -> bool {
    #[cfg(unix)]
    {
        std::path::Path::new(path).exists()
    }

    #[cfg(windows)]
    {
        // On Windows, we need to try to connect to see if the pipe exists
        // For now, return false as we'll handle this properly in the IPC module
        let _ = path; // Suppress unused variable warning
        false
    }
}

/// Remove a socket file (Unix only, no-op on Windows)
pub fn remove_socket_file(path: &str) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        if std::path::Path::new(path).exists() {
            std::fs::remove_file(path)?;
        }
    }

    #[cfg(windows)]
    {
        // Named pipes don't leave files on Windows, so this is a no-op
        let _ = path;
    }

    Ok(())
}

/// Get the parent directory for socket file (Unix only)
pub fn get_socket_parent_dir(path: &str) -> Option<PathBuf> {
    #[cfg(unix)]
    {
        std::path::Path::new(path).parent().map(|p| p.to_path_buf())
    }

    #[cfg(windows)]
    {
        // Named pipes don't need parent directory creation on Windows
        let _ = path;
        None
    }
}

/// Normalize executable command for the platform
pub fn normalize_executable(command: &str) -> String {
    #[cfg(windows)]
    {
        // Add .exe extension if not present
        if !command.ends_with(".exe")
            && !command.ends_with(".bat")
            && !command.ends_with(".cmd")
            && !command.contains('.')
        {
            format!("{}.exe", command)
        } else {
            command.to_string()
        }
    }

    #[cfg(unix)]
    {
        command.to_string()
    }
}

/// Get platform-specific path separator
pub fn path_separator() -> &'static str {
    #[cfg(windows)]
    {
        "\\"
    }

    #[cfg(unix)]
    {
        "/"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_socket_path() {
        let path = get_default_socket_path();

        #[cfg(unix)]
        assert!(path.ends_with("lsp-daemon.sock"));

        #[cfg(windows)]
        assert_eq!(path, r"\\.\pipe\lsp-daemon");
    }

    #[test]
    fn test_normalize_executable() {
        #[cfg(windows)]
        {
            assert_eq!(normalize_executable("rust-analyzer"), "rust-analyzer.exe");
            assert_eq!(normalize_executable("script.bat"), "script.bat");
            assert_eq!(normalize_executable("tool.exe"), "tool.exe");
        }

        #[cfg(unix)]
        {
            assert_eq!(normalize_executable("rust-analyzer"), "rust-analyzer");
            assert_eq!(normalize_executable("script.sh"), "script.sh");
        }
    }
}
