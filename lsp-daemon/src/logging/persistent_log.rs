//! Persistent log storage for LSP daemon logs
//!
//! Stores recent log entries to disk for persistence across daemon restarts.
//! Similar to crash logs, maintains a rotating buffer of the last N entries.

use crate::protocol::LogEntry;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Maximum number of log entries to persist to disk
const MAX_PERSISTENT_ENTRIES: usize = 1000;

/// File name for the persistent log file
const LOG_FILE_NAME: &str = "lsp-daemon.log.json";

/// File name for the previous session's logs
const PREVIOUS_LOG_FILE_NAME: &str = "lsp-daemon.previous.log.json";

/// Persistent log storage that writes to disk
#[derive(Clone)]
pub struct PersistentLogStorage {
    log_dir: PathBuf,
    entries: Arc<RwLock<Vec<LogEntry>>>,
    max_entries: usize,
    persistence_disabled: Arc<AtomicBool>,
}

impl PersistentLogStorage {
    /// Create a new persistent log storage
    pub fn new(log_dir: PathBuf) -> Result<Self> {
        // Ensure log directory exists
        fs::create_dir_all(&log_dir)
            .with_context(|| format!("Failed to create log directory: {:?}", log_dir))?;

        let storage = Self {
            log_dir,
            entries: Arc::new(RwLock::new(Vec::new())),
            max_entries: MAX_PERSISTENT_ENTRIES,
            persistence_disabled: Arc::new(AtomicBool::new(false)),
        };

        // Load existing logs if available
        storage.load_previous_logs()?;

        Ok(storage)
    }

    /// Get the path to the current log file
    fn current_log_path(&self) -> PathBuf {
        self.log_dir.join(LOG_FILE_NAME)
    }

    /// Get the path to the previous session's log file
    fn previous_log_path(&self) -> PathBuf {
        self.log_dir.join(PREVIOUS_LOG_FILE_NAME)
    }

    /// Load logs from the previous session
    pub fn load_previous_logs(&self) -> Result<Vec<LogEntry>> {
        let current_path = self.current_log_path();
        let previous_path = self.previous_log_path();

        // Move current log to previous if it exists
        if current_path.exists() {
            // Attempt to rename, ignore errors if file is in use
            let _ = fs::rename(&current_path, &previous_path);
        }

        // Try to load from previous log file
        if previous_path.exists() {
            match self.load_from_file(&previous_path) {
                Ok(entries) => {
                    debug!(
                        "Loaded {} previous log entries from {:?}",
                        entries.len(),
                        previous_path
                    );
                    Ok(entries)
                }
                Err(e) => {
                    warn!("Failed to load previous logs: {}", e);
                    Ok(Vec::new())
                }
            }
        } else {
            Ok(Vec::new())
        }
    }

    /// Load log entries from a file
    fn load_from_file(&self, path: &Path) -> Result<Vec<LogEntry>> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("Failed to read log file: {:?}", path))?;

        let log_file: PersistentLogFile = serde_json::from_str(&contents)
            .with_context(|| format!("Failed to parse log file: {:?}", path))?;

        Ok(log_file.entries)
    }

    /// Add a log entry and persist to disk
    pub async fn add_entry(&self, entry: LogEntry) -> Result<()> {
        let mut entries = self.entries.write().await;

        entries.push(entry);

        // Maintain max entries limit
        if entries.len() > self.max_entries {
            let remove_count = entries.len() - self.max_entries;
            entries.drain(0..remove_count);
        }

        // Clone entries for persistence
        let entries_to_save = entries.clone();
        drop(entries); // Release lock before I/O

        if self.persistence_disabled.load(Ordering::Relaxed) {
            return Ok(());
        }

        let log_path = self.current_log_path();
        let disabled_flag = self.persistence_disabled.clone();
        tokio::task::spawn_blocking(move || {
            if let Err(e) = Self::persist_to_disk(&log_path, entries_to_save) {
                if !disabled_flag.swap(true, Ordering::Relaxed) {
                    warn!(
                        "Disabling persistent log writes after error: {}. Logs will remain in-memory only.",
                        e
                    );
                }
            }
        });

        Ok(())
    }

    /// Persist entries to disk
    fn persist_to_disk(path: &Path, entries: Vec<LogEntry>) -> Result<()> {
        let log_file = PersistentLogFile {
            version: 1,
            entries,
            metadata: LogMetadata {
                daemon_version: env!("CARGO_PKG_VERSION").to_string(),
                created_at: chrono::Utc::now().to_rfc3339(),
            },
        };

        let json = serde_json::to_string_pretty(&log_file)?;

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let temp_dir = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| std::env::temp_dir());
        let mut temp_file = tempfile::NamedTempFile::new_in(&temp_dir)?;
        temp_file.write_all(json.as_bytes())?;
        temp_file.flush()?;
        temp_file.persist(path)?;

        Ok(())
    }

    /// Get all current session entries
    pub async fn get_current_entries(&self) -> Vec<LogEntry> {
        self.entries.read().await.clone()
    }

    /// Get entries from previous session
    pub fn get_previous_entries(&self) -> Result<Vec<LogEntry>> {
        let previous_path = self.previous_log_path();
        if previous_path.exists() {
            self.load_from_file(&previous_path)
        } else {
            Ok(Vec::new())
        }
    }

    /// Get combined entries (previous + current)
    pub async fn get_all_entries(&self, limit: Option<usize>) -> Result<Vec<LogEntry>> {
        let mut all_entries = Vec::new();

        // Add previous session entries
        if let Ok(previous) = self.get_previous_entries() {
            all_entries.extend(previous);
        }

        // Add current session entries
        let current = self.get_current_entries().await;
        all_entries.extend(current);

        // Apply limit if specified
        if let Some(limit) = limit {
            let start = all_entries.len().saturating_sub(limit);
            all_entries = all_entries[start..].to_vec();
        }

        Ok(all_entries)
    }

    /// Clear current session logs
    pub async fn clear_current(&self) -> Result<()> {
        self.entries.write().await.clear();

        // Remove current log file
        let current_path = self.current_log_path();
        if current_path.exists() {
            fs::remove_file(current_path)?;
        }

        Ok(())
    }

    /// Clear all logs (current and previous)
    pub async fn clear_all(&self) -> Result<()> {
        self.clear_current().await?;

        // Remove previous log file
        let previous_path = self.previous_log_path();
        if previous_path.exists() {
            fs::remove_file(previous_path)?;
        }

        Ok(())
    }

    /// Flush current entries to disk immediately
    pub async fn flush(&self) -> Result<()> {
        let entries = self.entries.read().await.clone();
        Self::persist_to_disk(&self.current_log_path(), entries)?;
        Ok(())
    }
}

/// Structure for persisted log file
#[derive(Debug, Serialize, Deserialize)]
struct PersistentLogFile {
    version: u32,
    entries: Vec<LogEntry>,
    metadata: LogMetadata,
}

/// Metadata for log file
#[derive(Debug, Serialize, Deserialize)]
struct LogMetadata {
    daemon_version: String,
    created_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LogLevel;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_persistent_storage_basic() {
        let temp_dir = TempDir::new().unwrap();
        let storage = PersistentLogStorage::new(temp_dir.path().to_path_buf()).unwrap();

        let entry = LogEntry {
            sequence: 1,
            timestamp: "2024-01-01 12:00:00.000 UTC".to_string(),
            level: LogLevel::Info,
            target: "test".to_string(),
            message: "Test message".to_string(),
            file: Some("test.rs".to_string()),
            line: Some(42),
        };

        storage.add_entry(entry.clone()).await.unwrap();

        let entries = storage.get_current_entries().await;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message, "Test message");
    }

    #[tokio::test]
    async fn test_persistence_across_sessions() {
        let temp_dir = TempDir::new().unwrap();
        let log_dir = temp_dir.path().to_path_buf();

        // First session
        {
            let storage = PersistentLogStorage::new(log_dir.clone()).unwrap();

            for i in 0..5 {
                let entry = LogEntry {
                    sequence: i,
                    timestamp: format!("2024-01-01 12:00:{:02}.000 UTC", i),
                    level: LogLevel::Info,
                    target: "test".to_string(),
                    message: format!("Message {}", i),
                    file: None,
                    line: None,
                };
                storage.add_entry(entry).await.unwrap();
            }

            // Force flush to disk
            storage.flush().await.unwrap();
        }

        // Second session - should load previous logs
        {
            let storage = PersistentLogStorage::new(log_dir.clone()).unwrap();

            let previous = storage.get_previous_entries().unwrap();
            assert_eq!(previous.len(), 5);
            assert_eq!(previous[0].message, "Message 0");
            assert_eq!(previous[4].message, "Message 4");
        }
    }

    #[tokio::test]
    async fn test_max_entries_limit() {
        let temp_dir = TempDir::new().unwrap();
        let mut storage = PersistentLogStorage::new(temp_dir.path().to_path_buf()).unwrap();
        storage.max_entries = 10; // Set lower limit for testing

        // Add more than max entries
        for i in 0..15 {
            let entry = LogEntry {
                sequence: i,
                timestamp: format!("2024-01-01 12:00:{:02}.000 UTC", i),
                level: LogLevel::Info,
                target: "test".to_string(),
                message: format!("Message {}", i),
                file: None,
                line: None,
            };
            storage.add_entry(entry).await.unwrap();
        }

        let entries = storage.get_current_entries().await;
        assert_eq!(entries.len(), 10);
        // Should have kept the last 10 entries (5-14)
        assert_eq!(entries[0].message, "Message 5");
        assert_eq!(entries[9].message, "Message 14");
    }

    #[tokio::test]
    async fn test_clear_operations() {
        let temp_dir = TempDir::new().unwrap();
        let storage = PersistentLogStorage::new(temp_dir.path().to_path_buf()).unwrap();

        // Add some entries
        for i in 0..3 {
            let entry = LogEntry {
                sequence: i,
                timestamp: format!("2024-01-01 12:00:{:02}.000 UTC", i),
                level: LogLevel::Info,
                target: "test".to_string(),
                message: format!("Message {}", i),
                file: None,
                line: None,
            };
            storage.add_entry(entry).await.unwrap();
        }

        assert_eq!(storage.get_current_entries().await.len(), 3);

        // Clear current
        storage.clear_current().await.unwrap();
        assert_eq!(storage.get_current_entries().await.len(), 0);
    }
}
