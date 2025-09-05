//! Integration tests for workspace management
//!
//! These tests verify the end-to-end functionality of the workspace management system
//! with real database backends and file operations.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{sqlite_backend::SQLiteBackend, DatabaseBackend, DatabaseConfig};
    use crate::workspace::config::CacheConfig;
    use crate::workspace::config::{
        DatabaseSettings, EvictionStrategy, MemoryLimits, PerformanceConfig,
    };
    use crate::workspace::{FileChange, FileChangeType, WorkspaceConfig, WorkspaceManager};
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio;

    /// Create a test database configuration
    fn test_database_config() -> DatabaseConfig {
        DatabaseConfig {
            temporary: true,
            compression: false,
            cache_capacity: 10 * 1024 * 1024, // 10MB
            ..Default::default()
        }
    }

    /// Create a test workspace configuration
    fn test_workspace_config() -> WorkspaceConfig {
        WorkspaceConfig {
            max_file_size_mb: 1,
            git_integration: false, // Disabled for simpler testing
            incremental_indexing: true,
            cache_settings: CacheConfig {
                enabled: true,
                max_size_mb: 10,
                ttl_minutes: 30,
                compression: false,
                eviction_strategy: EvictionStrategy::LRU,
                persistent_storage: false,
                cache_directory: None,
            },
            performance: PerformanceConfig {
                max_concurrent_operations: 2,
                batch_size: 10,
                operation_timeout_seconds: 30,
                parallel_processing: true,
                memory_limits: MemoryLimits::default(),
                database_settings: DatabaseSettings::default(),
            },
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_workspace_manager_creation() -> Result<(), Box<dyn std::error::Error>> {
        let db_config = test_database_config();
        let database = Arc::new(SQLiteBackend::new(db_config).await?);

        let workspace_config = test_workspace_config();
        let manager = WorkspaceManager::with_config(database, workspace_config).await?;

        // Verify manager was created successfully
        let metrics = manager.get_metrics().await;
        assert_eq!(metrics.total_workspaces_managed, 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_project_creation_and_retrieval() -> Result<(), Box<dyn std::error::Error>> {
        let db_config = test_database_config();
        let database = Arc::new(SQLiteBackend::new(db_config).await?);

        let workspace_config = test_workspace_config();
        let manager = WorkspaceManager::with_config(database, workspace_config).await?;

        // Create a temporary directory for the test project
        let temp_dir = TempDir::new()?;
        let project_root = temp_dir.path();

        // Create a project
        let project_id = manager.create_project("test_project", project_root).await?;
        assert!(project_id > 0);

        // Retrieve the project
        let project = manager.get_project(project_id).await?;
        assert!(project.is_some());

        let project = project.unwrap();
        assert_eq!(project.name, "test_project");
        assert_eq!(project.root_path, project_root);
        assert!(project.is_active);

        // List projects
        let projects = manager.list_projects().await?;
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].project_id, project_id);

        Ok(())
    }

    #[tokio::test]
    async fn test_workspace_creation_and_management() -> Result<(), Box<dyn std::error::Error>> {
        let db_config = test_database_config();
        let database = Arc::new(SQLiteBackend::new(db_config).await?);

        let workspace_config = test_workspace_config();
        let manager = WorkspaceManager::with_config(database, workspace_config).await?;

        let temp_dir = TempDir::new()?;
        let project_root = temp_dir.path();

        // Create a project first
        let project_id = manager.create_project("test_project", project_root).await?;

        // Create a workspace
        let workspace_id = manager
            .create_workspace(
                project_id,
                "main_workspace",
                Some("Main workspace for testing"),
            )
            .await?;
        assert!(workspace_id > 0);

        // Retrieve the workspace
        let workspace = manager.get_workspace(workspace_id).await?;
        assert!(workspace.is_some());

        let workspace = workspace.unwrap();
        assert_eq!(workspace.name, "main_workspace");
        assert_eq!(workspace.project_id, project_id);
        assert!(workspace.is_active);
        assert_eq!(
            workspace.description,
            Some("Main workspace for testing".to_string())
        );

        // List workspaces
        let workspaces = manager.list_workspaces(Some(project_id)).await?;
        assert_eq!(workspaces.len(), 1);
        assert_eq!(workspaces[0].workspace_id, workspace_id);

        Ok(())
    }

    #[tokio::test]
    async fn test_workspace_file_indexing() -> Result<(), Box<dyn std::error::Error>> {
        let db_config = test_database_config();
        let database = Arc::new(SQLiteBackend::new(db_config).await?);

        let workspace_config = test_workspace_config();
        let manager = WorkspaceManager::with_config(database, workspace_config).await?;

        let temp_dir = TempDir::new()?;
        let project_root = temp_dir.path();

        // Create some test files
        let test_file1 = project_root.join("main.rs");
        tokio::fs::write(&test_file1, "fn main() { println!(\"Hello, world!\"); }").await?;

        let test_file2 = project_root.join("lib.rs");
        tokio::fs::write(&test_file2, "pub fn add(a: i32, b: i32) -> i32 { a + b }").await?;

        // Create project and workspace
        let project_id = manager.create_project("rust_project", project_root).await?;
        let workspace_id = manager.create_workspace(project_id, "main", None).await?;

        // Index workspace files
        let result = manager
            .index_workspace_files(workspace_id, project_root)
            .await?;

        // Verify indexing results
        assert_eq!(result.workspace_id, workspace_id);
        assert!(result.files_processed >= 2); // At least our 2 test files
        assert!(result.processing_time.as_millis() > 0);
        assert!(!result.git_integration_active); // Disabled in config

        Ok(())
    }

    #[tokio::test]
    async fn test_workspace_config_validation() -> Result<(), Box<dyn std::error::Error>> {
        // Test valid config
        let valid_config = WorkspaceConfig::builder()
            .max_file_size_mb(10)
            .git_integration(true)
            .incremental_indexing(true)
            .build();
        assert!(valid_config.is_ok());

        // Test invalid config - file size too large
        let invalid_config = WorkspaceConfig::builder()
            .max_file_size_mb(2000) // Too large
            .build();
        assert!(invalid_config.is_err());

        // Test conflicting config - cache disabled but persistent storage enabled
        let cache_config = CacheConfig {
            enabled: false,
            persistent_storage: true,
            ..Default::default()
        };

        let conflicting_config = WorkspaceConfig::builder()
            .cache_settings(cache_config)
            .build();
        assert!(conflicting_config.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_project_language_detection() -> Result<(), Box<dyn std::error::Error>> {
        let db_config = test_database_config();
        let database = Arc::new(SQLiteBackend::new(db_config).await?);

        let manager = crate::workspace::ProjectManager::new(database, false);

        let temp_dir = TempDir::new()?;
        let project_root = temp_dir.path();

        // Create files with different extensions
        tokio::fs::write(project_root.join("main.rs"), "fn main() {}").await?;
        tokio::fs::write(project_root.join("script.py"), "print('hello')").await?;
        tokio::fs::write(project_root.join("app.js"), "console.log('hello')").await?;

        let project_config = crate::workspace::project::ProjectConfig {
            name: "multi_lang_project".to_string(),
            root_path: project_root.to_path_buf(),
            auto_detect_languages: true,
            ..Default::default()
        };

        let project_id = manager.create_project(project_config).await?;
        let project = manager.get_project(project_id).await?.unwrap();

        // Should detect multiple languages
        assert!(project.supported_languages.len() >= 3);
        assert!(project.supported_languages.contains(&"rust".to_string()));
        assert!(project.supported_languages.contains(&"python".to_string()));
        assert!(project
            .supported_languages
            .contains(&"javascript".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_workspace_metrics() -> Result<(), Box<dyn std::error::Error>> {
        let db_config = test_database_config();
        let database = Arc::new(SQLiteBackend::new(db_config).await?);

        let workspace_config = test_workspace_config();
        let manager = WorkspaceManager::with_config(database, workspace_config).await?;

        // Initial metrics should be empty
        let initial_metrics = manager.get_metrics().await;
        assert_eq!(initial_metrics.total_workspaces_managed, 0);
        assert_eq!(initial_metrics.total_files_indexed, 0);

        let temp_dir = TempDir::new()?;
        let project_root = temp_dir.path();
        tokio::fs::write(project_root.join("test.rs"), "// test file").await?;

        // Create project and workspace
        let project_id = manager.create_project("metrics_test", project_root).await?;
        let workspace_id = manager.create_workspace(project_id, "main", None).await?;

        // Index files
        let _result = manager
            .index_workspace_files(workspace_id, project_root)
            .await?;

        // Check updated metrics
        let updated_metrics = manager.get_metrics().await;
        assert!(updated_metrics.total_workspaces_managed >= 1);
        assert!(updated_metrics.total_files_indexed >= 1);

        Ok(())
    }

    #[tokio::test]
    async fn test_file_change_type_conversion() {
        // FileChangeType imported at top of module
        use crate::indexing;

        // Test conversion from workspace FileChange to indexing FileChange
        let workspace_change = crate::workspace::FileChange {
            path: PathBuf::from("/test/file.rs"),
            change_type: FileChangeType::Create,
            content_digest: Some("abc123".to_string()),
            size_bytes: Some(1024),
            modified_time: Some(1234567890),
        };

        // Simulate the conversion logic from manager.rs
        let indexing_change = indexing::FileChange {
            path: workspace_change.path.clone(),
            change_type: match workspace_change.change_type {
                FileChangeType::Create => indexing::FileChangeType::Create,
                FileChangeType::Update => indexing::FileChangeType::Update,
                FileChangeType::Delete => indexing::FileChangeType::Delete,
                FileChangeType::Move { from, to } => indexing::FileChangeType::Move { from, to },
            },
            content_digest: workspace_change.content_digest,
            size_bytes: workspace_change.size_bytes,
            mtime: workspace_change.modified_time,
            detected_language: None,
        };

        assert_eq!(indexing_change.path, PathBuf::from("/test/file.rs"));
        assert_eq!(indexing_change.content_digest, Some("abc123".to_string()));
        assert_eq!(indexing_change.size_bytes, Some(1024));
        assert_eq!(indexing_change.mtime, Some(1234567890));
    }

    #[tokio::test]
    async fn test_workspace_cache_operations() -> Result<(), Box<dyn std::error::Error>> {
        let db_config = test_database_config();
        let database = Arc::new(SQLiteBackend::new(db_config).await?);

        let workspace_config = test_workspace_config();
        let manager = WorkspaceManager::with_config(database, workspace_config).await?;

        let temp_dir = TempDir::new()?;
        let project_root = temp_dir.path();

        // Create project and workspace
        let project_id = manager.create_project("cache_test", project_root).await?;
        let workspace_id = manager.create_workspace(project_id, "main", None).await?;

        // Verify workspace is cached
        let cached_workspace = manager.get_workspace(workspace_id).await?;
        assert!(cached_workspace.is_some());

        // Clear cache
        manager.clear_cache().await?;

        // Should still be able to retrieve workspace from database
        let workspace_after_clear = manager.get_workspace(workspace_id).await?;
        assert!(workspace_after_clear.is_some());

        Ok(())
    }
}
