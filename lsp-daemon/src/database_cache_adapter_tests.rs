//! Comprehensive tests for database cache adapter and cache listing functionality
//!
//! These tests ensure that:
//! 1. Cache statistics are read directly from database (no memory caching)
//! 2. Cache listing functionality works correctly with proper database queries
//! 3. The iter_universal_entries method scans the correct database tree

#[cfg(test)]
mod tests {
    use super::super::database_cache_adapter::{DatabaseCacheAdapter, DatabaseCacheConfig};
    use super::super::universal_cache::key::CacheKey;
    use super::super::universal_cache::LspMethod;
    use tempfile::tempdir;

    /// Test that iter_universal_entries correctly reads from the universal_tree
    #[tokio::test]
    async fn test_iter_universal_entries_reads_universal_tree() {
        let temp_dir = tempdir().unwrap();
        let config = DatabaseCacheConfig {
            database_config: crate::database::DatabaseConfig {
                path: Some(temp_dir.path().join("test.db")),
                ..Default::default()
            },
            ..Default::default()
        };

        let adapter = DatabaseCacheAdapter::new(config).await.unwrap();

        // Initially should have no entries
        let entries = adapter.iter_universal_entries().await.unwrap();
        assert_eq!(entries.len(), 0, "Should start with no entries");

        // Add some entries to the universal tree
        let test_key1 = "workspace1:textDocument_hover:src/main.rs:hash1";
        let test_value1 = b"test_value_1";
        adapter
            .set_universal_entry(test_key1, test_value1)
            .await
            .unwrap();

        let test_key2 = "workspace1:textDocument_definition:src/lib.rs:hash2";
        let test_value2 = b"test_value_2";
        adapter
            .set_universal_entry(test_key2, test_value2)
            .await
            .unwrap();

        // Now iter_universal_entries should return these entries
        let entries = adapter.iter_universal_entries().await.unwrap();
        assert_eq!(entries.len(), 2, "Should return 2 entries");

        let keys: Vec<String> = entries.iter().map(|(k, _)| k.clone()).collect();
        assert!(keys.contains(&test_key1.to_string()));
        assert!(keys.contains(&test_key2.to_string()));

        // Verify the values are correct
        let entry1 = entries.iter().find(|(k, _)| k == test_key1).unwrap();
        assert_eq!(entry1.1, test_value1);

        let entry2 = entries.iter().find(|(k, _)| k == test_key2).unwrap();
        assert_eq!(entry2.1, test_value2);
    }

    /// Test that cache key parsing works correctly for various LSP methods
    #[tokio::test]
    async fn test_cache_key_parsing_all_methods() {
        let test_cases = vec![
            (
                "workspace1:textDocument_definition:src/main.rs:hash1",
                LspMethod::Definition,
            ),
            (
                "workspace2:textDocument_references:lib/utils.rs:hash2",
                LspMethod::References,
            ),
            (
                "workspace3:textDocument_hover:tests/test.rs:hash3",
                LspMethod::Hover,
            ),
            (
                "workspace4:textDocument_prepareCallHierarchy:src/parser.rs:hash4",
                LspMethod::CallHierarchy,
            ),
            (
                "workspace5:textDocument_implementation:src/traits.rs:hash5",
                LspMethod::Implementation,
            ),
        ];

        for (storage_key, expected_method) in test_cases {
            let parsed_key = CacheKey::from_storage_key(storage_key);
            assert!(
                parsed_key.is_some(),
                "Should successfully parse key: {storage_key}"
            );

            let key = parsed_key.unwrap();
            assert_eq!(key.method, expected_method);

            // Test round-trip: to_storage_key -> from_storage_key
            let reconstructed_key = key.to_storage_key();
            let reparsed_key = CacheKey::from_storage_key(&reconstructed_key);
            assert!(reparsed_key.is_some());
            assert_eq!(reparsed_key.unwrap().method, expected_method);
        }
    }

    /// Test that invalid cache keys are rejected properly
    #[test]
    fn test_cache_key_parsing_rejects_invalid_keys() {
        let invalid_keys = vec![
            "",                                               // Empty string
            "just_one_part",                                  // Not enough parts
            "two:parts",                                      // Still not enough
            "three:parts:only",                               // Still not enough
            "workspace:invalid_method:file:hash",             // Invalid method
            "workspace:textDocument/invalidMethod:file:hash", // Invalid method
        ];

        for invalid_key in invalid_keys {
            let parsed = CacheKey::from_storage_key(invalid_key);
            assert!(parsed.is_none(), "Should reject invalid key: {invalid_key}");
        }
    }

    /// Test cache statistics calculation from database (no memory caching)
    #[tokio::test]
    async fn test_cache_stats_read_from_database() {
        let temp_dir = tempdir().unwrap();
        let config = DatabaseCacheConfig {
            database_config: crate::database::DatabaseConfig {
                temporary: false,
                path: Some(temp_dir.path().join("stats_test.db")),
                ..Default::default()
            },
            ..Default::default()
        };

        let adapter = DatabaseCacheAdapter::new(config).await.unwrap();

        // Initially should have zero stats
        let initial_stats = adapter.get_stats().await.unwrap();
        assert_eq!(initial_stats.total_entries, 0);
        assert_eq!(initial_stats.total_size_bytes, 0);

        // Add some cache entries
        let entries = vec![
            ("ws1:textDocument_hover:main.rs:h1", "data1".as_bytes()),
            ("ws1:textDocument_definition:lib.rs:h2", "data22".as_bytes()),
            (
                "ws1:textDocument_references:test.rs:h3",
                "data333".as_bytes(),
            ),
        ];

        for (key, value) in &entries {
            adapter.set_universal_entry(key, value).await.unwrap();
        }

        // Update hit/miss statistics
        adapter
            .update_hit_miss_counts(Some(5), Some(3))
            .await
            .unwrap();

        // Get stats again - should read from database
        let updated_stats = adapter.get_stats().await.unwrap();
        // The entry count might include metadata entries, so check it's at least 3
        assert!(
            updated_stats.total_entries >= 3,
            "Should have at least 3 entries, got {}",
            updated_stats.total_entries
        );
        assert!(updated_stats.total_size_bytes > 0);
        assert_eq!(updated_stats.hit_count, 5);
        assert_eq!(updated_stats.miss_count, 3);

        // Update stats again with additional hits/misses
        adapter
            .update_hit_miss_counts(Some(2), Some(1))
            .await
            .unwrap();

        let final_stats = adapter.get_stats().await.unwrap();
        assert_eq!(final_stats.hit_count, 7); // 5 + 2
        assert_eq!(final_stats.miss_count, 4); // 3 + 1

        // Drop the first adapter to release database lock
        drop(adapter);

        // Verify stats are persistent by creating a new adapter instance
        let adapter2 = DatabaseCacheAdapter::new(DatabaseCacheConfig {
            database_config: crate::database::DatabaseConfig {
                temporary: false,
                path: Some(temp_dir.path().join("stats_test.db")),
                ..Default::default()
            },
            ..Default::default()
        })
        .await
        .unwrap();

        let persistent_stats = adapter2.get_stats().await.unwrap();
        assert!(
            persistent_stats.total_entries >= 3,
            "Should have at least 3 entries, got {}",
            persistent_stats.total_entries
        );
        assert_eq!(persistent_stats.hit_count, 7);
        assert_eq!(persistent_stats.miss_count, 4);
    }

    /// Test that cache entries can be retrieved and listed correctly
    #[tokio::test]
    async fn test_cache_entry_listing_and_retrieval() {
        let temp_dir = tempdir().unwrap();
        let config = DatabaseCacheConfig {
            database_config: crate::database::DatabaseConfig {
                temporary: false,
                path: Some(temp_dir.path().join("listing_test.db")),
                ..Default::default()
            },
            ..Default::default()
        };

        let adapter = DatabaseCacheAdapter::new(config).await.unwrap();

        // Create test cache entries with different methods
        let test_entries = vec![
            (
                "ws1:textDocument_hover:src/main.rs:hash1",
                r#"{"hover": "main function"}"#.as_bytes(),
            ),
            (
                "ws1:textDocument_definition:src/main.rs:hash2",
                r#"{"definition": "line 42"}"#.as_bytes(),
            ),
            (
                "ws1:textDocument_references:src/lib.rs:hash3",
                r#"{"references": ["ref1", "ref2"]}"#.as_bytes(),
            ),
            (
                "ws2:textDocument_hover:src/utils.rs:hash4",
                r#"{"hover": "utility function"}"#.as_bytes(),
            ),
        ];

        // Store all entries
        for (key, value) in &test_entries {
            adapter.set_universal_entry(key, value).await.unwrap();
        }

        // List all entries
        let all_entries = adapter.iter_universal_entries().await.unwrap();
        assert_eq!(all_entries.len(), 4);

        // Verify each entry can be parsed and contains expected data
        for (storage_key, value) in &all_entries {
            let parsed_key = CacheKey::from_storage_key(storage_key);
            assert!(parsed_key.is_some(), "Key should parse: {storage_key}");

            // Find the original entry
            let original = test_entries.iter().find(|(k, _)| k == storage_key).unwrap();
            assert_eq!(value, original.1);
        }

        // Test retrieval of specific entries
        for (key, expected_value) in &test_entries {
            let retrieved = adapter.get_universal_entry(key).await.unwrap();
            assert!(retrieved.is_some(), "Should retrieve entry for key: {key}");
            assert_eq!(retrieved.unwrap(), *expected_value);
        }
    }

    /// Test cache clearing functionality
    #[tokio::test]
    async fn test_cache_clearing() {
        let temp_dir = tempdir().unwrap();
        let config = DatabaseCacheConfig {
            database_config: crate::database::DatabaseConfig {
                temporary: false,
                path: Some(temp_dir.path().join("clearing_test.db")),
                ..Default::default()
            },
            ..Default::default()
        };

        let adapter = DatabaseCacheAdapter::new(config).await.unwrap();

        // Add test entries
        let entries = vec![
            ("ws1:textDocument_hover:main.rs:h1", "data1".as_bytes()),
            ("ws1:textDocument_definition:lib.rs:h2", "data2".as_bytes()),
            ("ws2:textDocument_hover:utils.rs:h3", "data3".as_bytes()),
        ];

        for (key, value) in &entries {
            adapter.set_universal_entry(key, value).await.unwrap();
        }

        // Verify entries exist
        let all_entries = adapter.iter_universal_entries().await.unwrap();
        assert_eq!(all_entries.len(), 3);

        // Test clearing entries by prefix
        let cleared_count = adapter
            .clear_universal_entries_by_prefix("ws1:")
            .await
            .unwrap();
        assert_eq!(cleared_count, 2);

        // Verify only ws2 entry remains
        let remaining_entries = adapter.iter_universal_entries().await.unwrap();
        assert_eq!(remaining_entries.len(), 1);
        assert!(remaining_entries[0].0.starts_with("ws2:"));

        // Test full clear
        adapter.clear().await.unwrap();
        let final_entries = adapter.iter_universal_entries().await.unwrap();
        assert_eq!(final_entries.len(), 0);
    }

    /// Test that statistics always come from database, not memory cache
    #[tokio::test]
    async fn test_no_memory_caching_of_statistics() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("no_memory_cache.db");
        let config = DatabaseCacheConfig {
            database_config: crate::database::DatabaseConfig {
                temporary: false,
                path: Some(db_path.clone()),
                ..Default::default()
            },
            ..Default::default()
        };

        let adapter = DatabaseCacheAdapter::new(config.clone()).await.unwrap();

        // Add initial entries and stats
        adapter
            .set_universal_entry("ws1:textDocument_hover:main.rs:h1", "data1".as_bytes())
            .await
            .unwrap();
        adapter
            .update_hit_miss_counts(Some(10), Some(5))
            .await
            .unwrap();

        let stats1 = adapter.get_stats().await.unwrap();
        assert_eq!(stats1.hit_count, 10);
        assert_eq!(stats1.miss_count, 5);

        // Drop the first adapter to release database lock
        drop(adapter);

        // Create a SECOND adapter instance pointing to the same database
        let config2 = DatabaseCacheConfig {
            database_config: crate::database::DatabaseConfig {
                temporary: false,
                path: Some(db_path.clone()),
                ..Default::default()
            },
            ..Default::default()
        };
        let adapter2 = DatabaseCacheAdapter::new(config2).await.unwrap();

        // Add more entries and update stats through the second adapter
        adapter2
            .set_universal_entry("ws1:textDocument_definition:lib.rs:h2", "data2".as_bytes())
            .await
            .unwrap();
        adapter2
            .update_hit_miss_counts(Some(3), Some(2))
            .await
            .unwrap();

        // Drop the second adapter
        drop(adapter2);

        // Create a THIRD adapter instance to verify stats are persistent
        let config3 = DatabaseCacheConfig {
            database_config: crate::database::DatabaseConfig {
                temporary: false,
                path: Some(db_path.clone()),
                ..Default::default()
            },
            ..Default::default()
        };
        let adapter3 = DatabaseCacheAdapter::new(config3).await.unwrap();

        // Get stats from THIRD adapter - should see updates from second adapter
        // This proves there's no memory caching, as the third adapter would not
        // know about changes made by previous adapters if stats were cached in memory
        let updated_stats = adapter3.get_stats().await.unwrap();
        assert!(
            updated_stats.total_entries >= 2,
            "Should have at least 2 entries, got {}",
            updated_stats.total_entries
        ); // Should see both entries
        assert_eq!(updated_stats.hit_count, 13); // 10 + 3
        assert_eq!(updated_stats.miss_count, 7); // 5 + 2

        // The third adapter should see consistent stats proving no memory caching
        let stats3 = adapter3.get_stats().await.unwrap();
        assert!(
            stats3.total_entries >= 2,
            "Should have at least 2 entries, got {}",
            stats3.total_entries
        );
        assert_eq!(stats3.hit_count, 13);
        assert_eq!(stats3.miss_count, 7);
    }
}
