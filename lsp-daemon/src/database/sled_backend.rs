//! Sled database backend implementation
//!
//! This module implements the `DatabaseBackend` trait using sled as the storage engine.
//! It provides both persistent and in-memory storage modes while maintaining compatibility
//! with existing sled usage patterns in the LSP daemon.

use super::{DatabaseBackend, DatabaseConfig, DatabaseError, DatabaseStats, DatabaseTree};
use anyhow::Context;
use async_trait::async_trait;
use sled::{Config, Db, Tree};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Sled-based implementation of DatabaseTree
pub struct SledTree {
    tree: Tree,
    name: String,
}

#[async_trait]
impl DatabaseTree for SledTree {
    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, DatabaseError> {
        match self.tree.get(key) {
            Ok(Some(value)) => Ok(Some(value.to_vec())),
            Ok(None) => Ok(None),
            Err(e) => Err(DatabaseError::OperationFailed {
                message: format!("Failed to get key from tree '{}': {e}", self.name),
            }),
        }
    }

    async fn set(&self, key: &[u8], value: &[u8]) -> Result<(), DatabaseError> {
        self.tree
            .insert(key, value)
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to set key in tree '{}': {e}", self.name),
            })?;
        Ok(())
    }

    async fn remove(&self, key: &[u8]) -> Result<bool, DatabaseError> {
        match self.tree.remove(key) {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(DatabaseError::OperationFailed {
                message: format!("Failed to remove key from tree '{}': {e}", self.name),
            }),
        }
    }

    async fn scan_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, DatabaseError> {
        let mut results = Vec::new();

        for result in self.tree.scan_prefix(prefix) {
            match result {
                Ok((key, value)) => {
                    results.push((key.to_vec(), value.to_vec()));
                }
                Err(e) => {
                    return Err(DatabaseError::OperationFailed {
                        message: format!("Failed to scan prefix in tree '{}': {e}", self.name),
                    });
                }
            }
        }

        Ok(results)
    }

    async fn clear(&self) -> Result<(), DatabaseError> {
        self.tree
            .clear()
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to clear tree '{}': {e}", self.name),
            })?;
        Ok(())
    }

    async fn len(&self) -> Result<u64, DatabaseError> {
        Ok(self.tree.len() as u64)
    }
}

/// Sled database backend implementation
pub struct SledBackend {
    db: Arc<Db>,
    config: DatabaseConfig,
}

impl SledBackend {
    /// Create a new SledBackend with explicit database instance
    /// This is useful when you already have a configured sled::Db
    pub fn from_db(db: Db, config: DatabaseConfig) -> Self {
        Self {
            db: Arc::new(db),
            config,
        }
    }

    /// Get the underlying sled database (for compatibility with existing code)
    pub fn underlying_db(&self) -> Arc<Db> {
        Arc::clone(&self.db)
    }

    /// Convert from an existing Arc<sled::Db> (common pattern in existing code)
    pub fn from_arc_db(db: Arc<Db>, config: DatabaseConfig) -> Self {
        Self { db, config }
    }
}

#[async_trait]
impl DatabaseBackend for SledBackend {
    type Tree = SledTree;

    async fn new(config: DatabaseConfig) -> Result<Self, DatabaseError>
    where
        Self: Sized,
    {
        let mut sled_config = Config::default()
            .cache_capacity(config.cache_capacity)
            .use_compression(config.compression)
            .compression_factor(config.compression_factor);

        // Set flush interval
        sled_config = match config.flush_every_ms {
            Some(ms) => sled_config.flush_every_ms(Some(ms)),
            None => sled_config.flush_every_ms(None),
        };

        let db = if config.temporary {
            debug!("Creating temporary sled database (in-memory, no disk persistence)");
            sled_config
                .temporary(true)
                .open()
                .context("Failed to create temporary sled database")
                .map_err(|e| DatabaseError::Configuration {
                    message: e.to_string(),
                })?
        } else if let Some(ref path) = config.path {
            debug!("Creating persistent sled database at: {:?}", path);

            // Ensure parent directory exists
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .context("Failed to create database directory")
                    .map_err(|e| DatabaseError::Configuration {
                        message: e.to_string(),
                    })?;
            }

            sled_config
                .path(path)
                .open()
                .context("Failed to create persistent sled database")
                .map_err(|e| DatabaseError::Configuration {
                    message: e.to_string(),
                })?
        } else {
            return Err(DatabaseError::Configuration {
                message: "Database path is required for persistent storage".to_string(),
            });
        };

        // Log database initialization
        let is_temporary = config.temporary;
        let path = config.path.clone();

        let backend = Self {
            db: Arc::new(db),
            config,
        };

        if is_temporary {
            info!("Initialized temporary sled database (no persistence)");
        } else {
            info!("Initialized persistent sled database at: {:?}", path);
        }

        Ok(backend)
    }

    async fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, DatabaseError> {
        match self.db.get(key) {
            Ok(Some(value)) => Ok(Some(value.to_vec())),
            Ok(None) => Ok(None),
            Err(e) => Err(DatabaseError::OperationFailed {
                message: format!("Failed to get key from default tree: {e}"),
            }),
        }
    }

    async fn set(&self, key: &[u8], value: &[u8]) -> Result<(), DatabaseError> {
        self.db
            .insert(key, value)
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to set key in default tree: {e}"),
            })?;
        Ok(())
    }

    async fn remove(&self, key: &[u8]) -> Result<bool, DatabaseError> {
        match self.db.remove(key) {
            Ok(Some(_)) => Ok(true),
            Ok(None) => Ok(false),
            Err(e) => Err(DatabaseError::OperationFailed {
                message: format!("Failed to remove key from default tree: {e}"),
            }),
        }
    }

    async fn scan_prefix(&self, prefix: &[u8]) -> Result<Vec<(Vec<u8>, Vec<u8>)>, DatabaseError> {
        let mut results = Vec::new();

        for result in self.db.scan_prefix(prefix) {
            match result {
                Ok((key, value)) => {
                    results.push((key.to_vec(), value.to_vec()));
                }
                Err(e) => {
                    return Err(DatabaseError::OperationFailed {
                        message: format!("Failed to scan prefix in default tree: {e}"),
                    });
                }
            }
        }

        Ok(results)
    }

    async fn open_tree(&self, name: &str) -> Result<Arc<Self::Tree>, DatabaseError> {
        let tree = self
            .db
            .open_tree(name)
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to open tree '{name}': {e}"),
            })?;

        Ok(Arc::new(SledTree {
            tree,
            name: name.to_string(),
        }))
    }

    async fn tree_names(&self) -> Result<Vec<String>, DatabaseError> {
        let names = self
            .db
            .tree_names()
            .into_iter()
            .map(|name| String::from_utf8_lossy(&name).to_string())
            .collect();
        Ok(names)
    }

    async fn clear(&self) -> Result<(), DatabaseError> {
        // Clear the default tree
        self.db
            .clear()
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to clear default tree: {e}"),
            })?;

        // Clear all named trees
        let tree_names = self.tree_names().await?;
        for tree_name in tree_names {
            if tree_name != "__sled__default" {
                // Skip sled's internal default tree name
                let tree = self.open_tree(&tree_name).await?;
                tree.clear().await?;
            }
        }

        Ok(())
    }

    async fn flush(&self) -> Result<(), DatabaseError> {
        // Force flush to disk
        self.db
            .flush()
            .map_err(|e| DatabaseError::OperationFailed {
                message: format!("Failed to flush database: {e}"),
            })?;
        Ok(())
    }

    async fn stats(&self) -> Result<DatabaseStats, DatabaseError> {
        let tree_names = self.tree_names().await?;
        let mut total_entries = 0u64;

        // Count entries in default tree
        total_entries += self.db.len() as u64;

        // Count entries in all named trees
        for tree_name in &tree_names {
            if tree_name != "__sled__default" {
                let tree = self.open_tree(tree_name).await?;
                total_entries += tree.len().await?;
            }
        }

        // Estimate size (sled doesn't provide exact size, so we estimate)
        // This is a rough estimate based on the number of entries
        let estimated_avg_entry_size = 256; // bytes per entry (key + value + overhead)
        let total_size_bytes = total_entries * estimated_avg_entry_size;

        let disk_size_bytes = if self.config.temporary {
            0 // In-memory databases don't use disk
        } else {
            self.size_on_disk().await?
        };

        Ok(DatabaseStats {
            total_entries,
            total_size_bytes,
            disk_size_bytes,
            tree_count: tree_names.len(),
            is_temporary: self.config.temporary,
        })
    }

    async fn size_on_disk(&self) -> Result<u64, DatabaseError> {
        if self.config.temporary {
            return Ok(0);
        }

        if let Some(ref path) = self.config.path {
            // Calculate total size of all files in the database directory
            let mut total_size = 0u64;

            if path.is_file() {
                // Single database file
                match std::fs::metadata(path) {
                    Ok(metadata) => total_size = metadata.len(),
                    Err(e) => {
                        warn!("Failed to get database file size for {:?}: {}", path, e);
                        return Ok(0);
                    }
                }
            } else if path.is_dir() {
                // Database directory with multiple files
                match std::fs::read_dir(path) {
                    Ok(entries) => {
                        for entry in entries.flatten() {
                            if let Ok(metadata) = entry.metadata() {
                                if metadata.is_file() {
                                    total_size += metadata.len();
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to read database directory {:?}: {}", path, e);
                        return Ok(0);
                    }
                }
            }

            Ok(total_size)
        } else {
            Ok(0)
        }
    }

    fn is_temporary(&self) -> bool {
        self.config.temporary
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_sled_backend_temporary() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let db = SledBackend::new(config)
            .await
            .expect("Failed to create temporary database");
        assert!(db.is_temporary());

        // Test basic operations
        db.set(b"key1", b"value1").await.expect("Failed to set key");
        let value = db.get(b"key1").await.expect("Failed to get key");
        assert_eq!(value, Some(b"value1".to_vec()));

        // Test removal
        let removed = db.remove(b"key1").await.expect("Failed to remove key");
        assert!(removed);

        let value = db
            .get(b"key1")
            .await
            .expect("Failed to get key after removal");
        assert_eq!(value, None);
    }

    #[tokio::test]
    async fn test_sled_backend_persistent() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test.db");

        let config = DatabaseConfig {
            path: Some(db_path.clone()),
            temporary: false,
            ..Default::default()
        };

        let db = SledBackend::new(config)
            .await
            .expect("Failed to create persistent database");
        assert!(!db.is_temporary());

        // Test basic operations
        db.set(b"persistent_key", b"persistent_value")
            .await
            .expect("Failed to set key");
        let value = db.get(b"persistent_key").await.expect("Failed to get key");
        assert_eq!(value, Some(b"persistent_value".to_vec()));
    }

    #[tokio::test]
    async fn test_sled_tree_operations() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let db = SledBackend::new(config)
            .await
            .expect("Failed to create database");

        // Test tree operations
        let tree = db
            .open_tree("test_tree")
            .await
            .expect("Failed to open tree");

        tree.set(b"tree_key", b"tree_value")
            .await
            .expect("Failed to set in tree");
        let value = tree
            .get(b"tree_key")
            .await
            .expect("Failed to get from tree");
        assert_eq!(value, Some(b"tree_value".to_vec()));

        // Test tree length
        let len = tree.len().await.expect("Failed to get tree length");
        assert_eq!(len, 1);

        // Test tree clear
        tree.clear().await.expect("Failed to clear tree");
        let len = tree
            .len()
            .await
            .expect("Failed to get tree length after clear");
        assert_eq!(len, 0);
    }

    #[tokio::test]
    async fn test_sled_prefix_scan() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let db = SledBackend::new(config)
            .await
            .expect("Failed to create database");

        // Insert test data
        db.set(b"prefix:key1", b"value1")
            .await
            .expect("Failed to set key1");
        db.set(b"prefix:key2", b"value2")
            .await
            .expect("Failed to set key2");
        db.set(b"other:key3", b"value3")
            .await
            .expect("Failed to set key3");

        // Test prefix scan
        let results = db
            .scan_prefix(b"prefix:")
            .await
            .expect("Failed to scan prefix");
        assert_eq!(results.len(), 2);

        // Check that results contain expected keys
        let keys: Vec<&[u8]> = results.iter().map(|(k, _)| k.as_slice()).collect();
        assert!(keys.contains(&b"prefix:key1".as_slice()));
        assert!(keys.contains(&b"prefix:key2".as_slice()));
    }

    #[tokio::test]
    async fn test_sled_serialization_helpers() {
        use crate::database::DatabaseBackendExt;
        use serde::{Deserialize, Serialize};

        #[derive(Debug, Serialize, Deserialize, PartialEq)]
        struct TestData {
            id: u64,
            name: String,
        }

        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let db = SledBackend::new(config)
            .await
            .expect("Failed to create database");

        let test_data = TestData {
            id: 42,
            name: "test".to_string(),
        };

        // Test serialized set/get
        db.set_serialized(b"test_key", &test_data)
            .await
            .expect("Failed to set serialized data");
        let retrieved: Option<TestData> = db
            .get_serialized(b"test_key")
            .await
            .expect("Failed to get serialized data");

        assert_eq!(retrieved, Some(test_data));
    }

    #[tokio::test]
    async fn test_sled_stats() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let db = SledBackend::new(config)
            .await
            .expect("Failed to create database");

        // Insert some test data
        db.set(b"key1", b"value1")
            .await
            .expect("Failed to set key1");
        db.set(b"key2", b"value2")
            .await
            .expect("Failed to set key2");

        let tree = db
            .open_tree("test_tree")
            .await
            .expect("Failed to open tree");
        tree.set(b"tree_key", b"tree_value")
            .await
            .expect("Failed to set in tree");

        let stats = db.stats().await.expect("Failed to get stats");
        assert!(stats.total_entries >= 3); // At least 2 in default + 1 in tree
        assert!(stats.is_temporary);
        assert_eq!(stats.disk_size_bytes, 0); // Temporary database
    }
}
