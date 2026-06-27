use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Maximum file size that search-like text paths will read into memory.
pub const MAX_SEARCHABLE_TEXT_FILE_SIZE_BYTES: u64 = 1024 * 1024;

const HARD_DENY_FILENAMES: &[&str] = &[".ds_store", "thumbs.db"];

const HARD_DENY_COMPOUND_SUFFIXES: &[&str] =
    &[".tar.gz", ".tar.bz2", ".tar.xz", ".tar.zst", ".min.js.map"];

/// Extensions that are not useful as source/text search targets and should
/// stay denied even when users request custom text extensions.
pub const HARD_DENY_EXTENSIONS: &[&str] = &[
    "7z", "a", "app", "avi", "bak", "bin", "bmp", "bz2", "class", "db", "db3", "dll", "dmg",
    "duckdb", "dylib", "ear", "eot", "exe", "gif", "gz", "ico", "jar", "jpeg", "jpg", "lib", "m4a",
    "m4v", "mdb", "mkv", "mov", "mp3", "mp4", "o", "obj", "otf", "out", "parquet", "pdf", "png",
    "pyc", "pyo", "rar", "sqlite", "sqlite3", "so", "swp", "swo", "tar", "tgz", "ttf", "wasm",
    "war", "webm", "webp", "woff", "woff2", "xz", "zst",
];

pub fn hard_deny_globs() -> Vec<String> {
    let mut globs = Vec::with_capacity(
        HARD_DENY_EXTENSIONS.len() + HARD_DENY_FILENAMES.len() + HARD_DENY_COMPOUND_SUFFIXES.len(),
    );
    globs.extend(HARD_DENY_EXTENSIONS.iter().map(|ext| format!("*.{ext}")));
    globs.extend(HARD_DENY_FILENAMES.iter().map(|name| (*name).to_string()));
    globs.extend(
        HARD_DENY_COMPOUND_SUFFIXES
            .iter()
            .map(|suffix| format!("*{suffix}")),
    );
    globs
}

pub fn is_hard_denied_path(path: &Path) -> bool {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_lowercase();

    if HARD_DENY_FILENAMES.contains(&file_name.as_str()) {
        return true;
    }

    if HARD_DENY_COMPOUND_SUFFIXES
        .iter()
        .any(|suffix| file_name.ends_with(suffix))
    {
        return true;
    }

    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| HARD_DENY_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
}

pub fn resolve_searchable_path(path: &Path) -> Result<PathBuf> {
    if crate::path_safety::is_symlink_or_junction(path) {
        anyhow::bail!("Skipping symlink/junction: {}", path.display());
    }

    if crate::path_safety::is_ci_environment() {
        Ok(path.to_path_buf())
    } else {
        std::fs::canonicalize(path)
            .with_context(|| format!("Failed to resolve file path: {}", path.display()))
    }
}

pub fn validate_searchable_text_file(path: &Path) -> Result<PathBuf> {
    if is_hard_denied_path(path) {
        anyhow::bail!(
            "File extension is hard-denied for text search: {}",
            path.display()
        );
    }

    let resolved_path = resolve_searchable_path(path)?;
    if is_hard_denied_path(&resolved_path) {
        anyhow::bail!(
            "File extension is hard-denied for text search: {}",
            resolved_path.display()
        );
    }

    let metadata = std::fs::metadata(&resolved_path)
        .with_context(|| format!("Failed to get file metadata: {}", resolved_path.display()))?;

    if !metadata.is_file() {
        anyhow::bail!("Path is not a regular file: {}", resolved_path.display());
    }

    if metadata.len() > MAX_SEARCHABLE_TEXT_FILE_SIZE_BYTES {
        anyhow::bail!(
            "File too large: {} bytes (limit: {} bytes)",
            metadata.len(),
            MAX_SEARCHABLE_TEXT_FILE_SIZE_BYTES
        );
    }

    Ok(resolved_path)
}

pub fn read_searchable_text_file(path: &Path) -> Result<String> {
    let resolved_path = validate_searchable_text_file(path)?;
    let content = std::fs::read_to_string(&resolved_path)
        .with_context(|| format!("Failed to read file: {}", resolved_path.display()))?;

    if content.as_bytes().contains(&0) {
        anyhow::bail!(
            "File appears to contain binary data: {}",
            resolved_path.display()
        );
    }

    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn hard_denies_binary_extensions_case_insensitively() {
        assert!(is_hard_denied_path(Path::new("image.PNG")));
        assert!(is_hard_denied_path(Path::new("archive.tar.gz")));
        assert!(is_hard_denied_path(Path::new("library.so")));
        assert!(!is_hard_denied_path(Path::new("config.json")));
        assert!(!is_hard_denied_path(Path::new("settings.conf")));
    }

    #[test]
    fn read_rejects_nul_bytes() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"hello\0world").unwrap();

        let err = read_searchable_text_file(file.path()).unwrap_err();
        assert!(err.to_string().contains("binary data"));
    }

    #[test]
    fn read_rejects_oversized_files() {
        let mut file = NamedTempFile::new().unwrap();
        let content = vec![b'a'; MAX_SEARCHABLE_TEXT_FILE_SIZE_BYTES as usize + 1];
        file.write_all(&content).unwrap();

        let err = read_searchable_text_file(file.path()).unwrap_err();
        assert!(err.to_string().contains("File too large"));
    }
}
