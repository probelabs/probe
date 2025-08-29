//! Usage examples for the database abstraction layer
//!
//! This file contains examples showing how to use the database abstraction
//! layer with both persistent and in-memory storage modes.

#[cfg(test)]
mod examples {
    use super::super::{
        DatabaseBackend, DatabaseBackendExt, DatabaseConfig, DatabaseTree, SledBackend,
    };
    use serde::{Deserialize, Serialize};
    use tempfile::tempdir;

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct CacheEntry {
        pub key: String,
        pub content_hash: String,
        pub data: Vec<u8>,
        pub timestamp: u64,
    }

    /// Example: Creating a temporary (in-memory) database
    #[tokio::test]
    async fn example_temporary_database() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let db = SledBackend::new(config)
            .await
            .expect("Failed to create temporary database");

        // Basic operations
        db.set(b"key1", b"value1")
            .await
            .expect("Failed to set value");

        let value = db.get(b"key1").await.expect("Failed to get value");

        assert_eq!(value, Some(b"value1".to_vec()));
        println!("✓ Temporary database operations work correctly");
    }

    /// Example: Creating a persistent database
    #[tokio::test]
    async fn example_persistent_database() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("example.db");

        let config = DatabaseConfig {
            path: Some(db_path),
            temporary: false,
            compression: true,
            compression_factor: 5,
            ..Default::default()
        };

        let db = SledBackend::new(config)
            .await
            .expect("Failed to create persistent database");

        // Test serialized operations with complex data
        let entry = CacheEntry {
            key: "test_entry".to_string(),
            content_hash: "abc123".to_string(),
            data: vec![1, 2, 3, 4, 5],
            timestamp: 1234567890,
        };

        db.set_serialized(b"cache:entry1", &entry)
            .await
            .expect("Failed to set serialized entry");

        let retrieved: Option<CacheEntry> = db
            .get_serialized(b"cache:entry1")
            .await
            .expect("Failed to get serialized entry");

        assert_eq!(retrieved, Some(entry));
        println!("✓ Persistent database with serialization works correctly");
    }

    /// Example: Working with database trees
    #[tokio::test]
    async fn example_database_trees() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let db = SledBackend::new(config)
            .await
            .expect("Failed to create database");

        // Open different trees for different types of data
        let nodes_tree = db
            .open_tree("nodes")
            .await
            .expect("Failed to open nodes tree");

        let metadata_tree = db
            .open_tree("metadata")
            .await
            .expect("Failed to open metadata tree");

        // Store data in different trees
        nodes_tree
            .set(b"node:1", b"node_data_1")
            .await
            .expect("Failed to set in nodes tree");

        metadata_tree
            .set(b"version", b"1.0")
            .await
            .expect("Failed to set in metadata tree");

        // Retrieve data from trees
        let node_data = nodes_tree
            .get(b"node:1")
            .await
            .expect("Failed to get from nodes tree");

        let version = metadata_tree
            .get(b"version")
            .await
            .expect("Failed to get from metadata tree");

        assert_eq!(node_data, Some(b"node_data_1".to_vec()));
        assert_eq!(version, Some(b"1.0".to_vec()));

        println!("✓ Database trees work correctly for organizing data");
    }

    /// Example: Prefix scanning for cache invalidation
    #[tokio::test]
    async fn example_prefix_scanning() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let db = SledBackend::new(config)
            .await
            .expect("Failed to create database");

        // Insert multiple entries with common prefixes
        db.set(b"cache:user:123:profile", b"user_profile_data")
            .await
            .expect("Failed to set user profile");

        db.set(b"cache:user:123:settings", b"user_settings_data")
            .await
            .expect("Failed to set user settings");

        db.set(b"cache:user:456:profile", b"other_user_profile")
            .await
            .expect("Failed to set other user profile");

        db.set(b"cache:project:abc:info", b"project_info")
            .await
            .expect("Failed to set project info");

        // Scan for all user 123 entries
        let user_123_entries = db
            .scan_prefix(b"cache:user:123:")
            .await
            .expect("Failed to scan prefix");

        assert_eq!(user_123_entries.len(), 2);

        // Scan for all cache entries
        let all_cache_entries = db
            .scan_prefix(b"cache:")
            .await
            .expect("Failed to scan cache prefix");

        assert_eq!(all_cache_entries.len(), 4);

        println!("✓ Prefix scanning works for efficient cache invalidation");
    }

    /// Example: Database statistics and maintenance
    #[tokio::test]
    async fn example_database_maintenance() {
        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        let db = SledBackend::new(config)
            .await
            .expect("Failed to create database");

        // Add some test data
        for i in 0..10 {
            let key = format!("key_{}", i);
            let value = format!("value_{}", i);
            db.set(key.as_bytes(), value.as_bytes())
                .await
                .expect("Failed to set test data");
        }

        // Open a tree and add more data
        let tree = db
            .open_tree("test_tree")
            .await
            .expect("Failed to open tree");
        tree.set(b"tree_key", b"tree_value")
            .await
            .expect("Failed to set in tree");

        // Get statistics
        let stats = db.stats().await.expect("Failed to get statistics");

        println!("Database Statistics:");
        println!("  - Total entries: {}", stats.total_entries);
        println!("  - Estimated size: {} bytes", stats.total_size_bytes);
        println!("  - Tree count: {}", stats.tree_count);
        println!("  - Is temporary: {}", stats.is_temporary);
        println!("  - Disk size: {} bytes", stats.disk_size_bytes);

        assert!(stats.total_entries >= 11); // 10 + 1 in tree
        assert!(stats.tree_count >= 2); // default + test_tree
        assert!(stats.is_temporary);

        // Test flush (no-op for temporary, but should not error)
        db.flush().await.expect("Failed to flush database");

        println!("✓ Database statistics and maintenance operations work correctly");
    }

    /// Example: Converting existing sled databases to the abstraction
    #[tokio::test]
    async fn example_converting_existing_sled() {
        use sled;

        let config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };

        // Create a raw sled database (simulating existing code)
        let raw_sled = sled::Config::default()
            .temporary(true)
            .open()
            .expect("Failed to create raw sled database");

        // Insert some data using raw sled API
        raw_sled
            .insert(b"existing_key", b"existing_value")
            .expect("Failed to insert with raw sled");

        // Wrap the existing sled database with our abstraction
        let wrapped_db = SledBackend::from_db(raw_sled, config);

        // Now use the abstracted interface
        let value = wrapped_db
            .get(b"existing_key")
            .await
            .expect("Failed to get value through abstraction");

        assert_eq!(value, Some(b"existing_value".to_vec()));

        // Add new data through the abstraction
        wrapped_db
            .set(b"new_key", b"new_value")
            .await
            .expect("Failed to set through abstraction");

        println!("✓ Converting existing sled databases works correctly");
    }
}
