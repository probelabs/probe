use std::path::PathBuf;

#[cfg(any(target_os = "linux", target_os = "android"))]
fn abstract_socket_disabled() -> bool {
    std::env::var("PROBE_DISABLE_ABSTRACT_SOCKET").is_ok()
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn custom_socket_override() -> Option<String> {
    std::env::var("PROBE_LSP_SOCKET_PATH").ok()
}

/// Get the default socket/pipe path for the current platform
pub fn get_default_socket_path() -> String {
    // Check for environment variable override first
    if let Ok(path) = std::env::var("PROBE_LSP_SOCKET_PATH") {
        return path;
    }

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
        if unix_abstract_name(path).is_some() {
            return false;
        }
        std::path::Path::new(path).exists()
    }

    #[cfg(windows)]
    {
        // On Windows, check if we can connect to the named pipe
        use tokio::net::windows::named_pipe::ClientOptions;

        // Try to connect with a short timeout to check if pipe exists
        let _client =
            ClientOptions::new().pipe_mode(tokio::net::windows::named_pipe::PipeMode::Message);

        // Use blocking I/O for the existence check (quick operation)
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
        {
            Ok(_) => {
                tracing::trace!("Named pipe exists and is accessible: {}", path);
                true
            }
            Err(e) => {
                tracing::trace!(
                    "Named pipe does not exist or is not accessible: {} (error: {})",
                    path,
                    e
                );
                false
            }
        }
    }
}

/// Remove a socket file (Unix only, no-op on Windows)
pub fn remove_socket_file(path: &str) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        if unix_abstract_name(path).is_some() {
            return Ok(());
        }
        if std::path::Path::new(path).exists() {
            std::fs::remove_file(path)?;
        }
    }

    #[cfg(windows)]
    {
        // Named pipes don't leave files on Windows, so this is a no-op
        tracing::trace!("Socket removal is no-op on Windows for path: {}", path);
    }

    Ok(())
}

/// Get the parent directory for socket file (Unix only)
pub fn get_socket_parent_dir(path: &str) -> Option<PathBuf> {
    #[cfg(unix)]
    {
        if unix_abstract_name(path).is_some() {
            return None;
        }
        std::path::Path::new(path).parent().map(|p| p.to_path_buf())
    }

    #[cfg(windows)]
    {
        // Named pipes don't need parent directory creation on Windows
        tracing::trace!(
            "Parent directory creation is not needed on Windows for path: {}",
            path
        );
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

/// Determine the abstract socket name for the provided path, if enabled on this platform.
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn unix_abstract_name(path: &str) -> Option<Vec<u8>> {
    if abstract_socket_disabled() {
        return None;
    }

    if path.starts_with("unix:@") {
        return Some(path[6..].as_bytes().to_vec());
    }
    if path.starts_with('@') {
        return Some(path[1..].as_bytes().to_vec());
    }

    if let Some(ref override_path) = custom_socket_override() {
        if override_path.starts_with("unix:@") {
            return Some(override_path[6..].as_bytes().to_vec());
        }
        if override_path.starts_with('@') {
            return Some(override_path[1..].as_bytes().to_vec());
        }
        // Respect explicit filesystem override
        return None;
    }

    // Generate deterministic abstract name based on requested path
    let hash = blake3::hash(path.as_bytes());
    let name = format!("probe-lsp-{}", &hash.to_hex()[..16]);
    Some(name.as_bytes().to_vec())
}

#[cfg(not(any(target_os = "linux", target_os = "android")))]
pub fn unix_abstract_name(_path: &str) -> Option<Vec<u8>> {
    None
}
