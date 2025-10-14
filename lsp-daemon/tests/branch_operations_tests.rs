#![cfg(feature = "legacy-tests")]
use anyhow::Result;
use lsp_daemon::database::sqlite_backend::SQLiteBackend;
use lsp_daemon::database::{DatabaseBackend, DatabaseConfig};
use lsp_daemon::indexing::versioning::{FileVersionManager, VersioningConfig};
use lsp_daemon::workspace::branch::{BranchError, BranchManager};
use lsp_daemon::workspace::manager::WorkspaceManager;
use lsp_daemon::workspace::project::ProjectConfig;
use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;
use tokio::fs;
use tokio::sync::Mutex;

#[allow(unused_imports)] // Some imports used conditionally in tests

/// Test fixture for branch operations
struct BranchTestFixture {
    temp_dir: TempDir,
    workspace_manager: WorkspaceManager<SQLiteBackend>,
    project_id: i64,
    workspace_id: i64,
}

impl BranchTestFixture {
    async fn new() -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("test_branch_ops.db");

        let config = DatabaseConfig {
            path: Some(db_path),
            temporary: false,
            compression: false,
            cache_capacity: 1024 * 1024, // 1MB
            compression_factor: 0,
            flush_every_ms: Some(1000),
        };
        let database = Arc::new(SQLiteBackend::new(config).await?);
        let workspace_manager = WorkspaceManager::with_git_integration(
            database.clone(),
            Arc::new(Mutex::new(
                lsp_daemon::git_service::GitService::discover_repo(
                    temp_dir.path(),
                    temp_dir.path(),
                )?,
            )),
        )
        .await?;

        // Initialize git repository
        let git_output = std::process::Command::new("git")
            .current_dir(temp_dir.path())
            .args(["init"])
            .output()?;

        if !git_output.status.success() {
            anyhow::bail!("Failed to initialize git repository");
        }

        // Configure git user for tests
        std::process::Command::new("git")
            .current_dir(temp_dir.path())
            .args(["config", "user.name", "Test User"])
            .output()?;

        std::process::Command::new("git")
            .current_dir(temp_dir.path())
            .args(["config", "user.email", "test@example.com"])
            .output()?;

        // Create initial commit
        fs::write(temp_dir.path().join("README.md"), "# Test Repository").await?;

        std::process::Command::new("git")
            .current_dir(temp_dir.path())
            .args(["add", "README.md"])
            .output()?;

        std::process::Command::new("git")
            .current_dir(temp_dir.path())
            .args(["commit", "-m", "Initial commit"])
            .output()?;

        // Create project and workspace
        let project_id = workspace_manager
            .create_project("test_project", temp_dir.path())
            .await?;

        let workspace_id = workspace_manager
            .create_workspace(
                project_id,
                "main_workspace",
                Some("Test workspace for branch operations"),
            )
            .await?;

        Ok(Self {
            temp_dir,
            workspace_manager,
            project_id,
            workspace_id,
        })
    }

    fn repo_path(&self) -> &Path {
        self.temp_dir.path()
    }

    async fn create_test_file(&self, filename: &str, content: &str) -> Result<()> {
        fs::write(self.repo_path().join(filename), content).await?;

        std::process::Command::new("git")
            .current_dir(self.repo_path())
            .args(["add", filename])
            .output()?;

        let commit_output = std::process::Command::new("git")
            .current_dir(self.repo_path())
            .args(["commit", "-m", &format!("Add {}", filename)])
            .output()?;

        if !commit_output.status.success() {
            anyhow::bail!(
                "Failed to commit file: {}",
                String::from_utf8_lossy(&commit_output.stderr)
            );
        }

        Ok(())
    }
}

#[tokio::test]
async fn test_branch_creation_and_listing() -> Result<()> {
    let fixture = BranchTestFixture::new().await?;

    // Create a new branch
    fixture
        .workspace_manager
        .create_branch(fixture.workspace_id, "feature/test-branch", None)
        .await?;

    // List branches
    let branches = fixture
        .workspace_manager
        .list_branches(fixture.workspace_id)
        .await?;

    // Verify branches exist
    assert!(branches.len() >= 2); // main + feature/test-branch

    let branch_names: Vec<&str> = branches.iter().map(|b| b.branch_name.as_str()).collect();

    assert!(branch_names.contains(&"main") || branch_names.contains(&"master"));
    assert!(branch_names.contains(&"feature/test-branch"));

    // Verify current branch is still main/master
    let current_branch = fixture
        .workspace_manager
        .get_workspace_branch(fixture.workspace_id)
        .await?;

    assert!(
        current_branch == Some("main".to_string()) || current_branch == Some("master".to_string())
    );

    Ok(())
}

#[tokio::test]
async fn test_branch_switching_basic() -> Result<()> {
    let fixture = BranchTestFixture::new().await?;

    // Create test content on main branch
    fixture
        .create_test_file("main_file.txt", "Content from main branch")
        .await?;

    // Create a new branch
    fixture
        .workspace_manager
        .create_branch(fixture.workspace_id, "feature/switch-test", None)
        .await?;

    // Switch to the new branch
    let switch_result = fixture
        .workspace_manager
        .switch_branch(fixture.workspace_id, "feature/switch-test")
        .await?;

    assert_eq!(switch_result.new_branch, "feature/switch-test");
    assert!(
        switch_result.previous_branch == Some("main".to_string())
            || switch_result.previous_branch == Some("master".to_string())
    );

    // Verify current branch changed
    let current_branch = fixture
        .workspace_manager
        .get_workspace_branch(fixture.workspace_id)
        .await?;

    assert_eq!(current_branch, Some("feature/switch-test".to_string()));

    Ok(())
}

#[tokio::test]
async fn test_branch_switching_with_file_changes() -> Result<()> {
    let fixture = BranchTestFixture::new().await?;

    // Create initial file on main branch
    fixture
        .create_test_file("shared_file.txt", "Initial content")
        .await?;

    // Create and switch to feature branch
    fixture
        .workspace_manager
        .create_branch(fixture.workspace_id, "feature/file-changes", None)
        .await?;

    fixture
        .workspace_manager
        .switch_branch(fixture.workspace_id, "feature/file-changes")
        .await?;

    // Create different content on feature branch
    fixture
        .create_test_file("feature_file.txt", "Feature branch content")
        .await?;

    // Switch back to main
    let switch_result = match fixture
        .workspace_manager
        .switch_branch(fixture.workspace_id, "main")
        .await
    {
        Ok(result) => result,
        Err(_) => {
            // Try master if main doesn't exist
            fixture
                .workspace_manager
                .switch_branch(fixture.workspace_id, "master")
                .await?
        }
    };

    // Verify files changed during switch
    assert!(switch_result.files_changed > 0);
    assert!(switch_result.indexing_required);

    // Verify feature file doesn't exist on main branch
    assert!(!fixture.repo_path().join("feature_file.txt").exists());

    Ok(())
}

#[tokio::test]
async fn test_branch_deletion() -> Result<()> {
    let fixture = BranchTestFixture::new().await?;

    // Create branches for testing
    fixture
        .workspace_manager
        .create_branch(fixture.workspace_id, "temp/delete-me", None)
        .await?;

    fixture
        .workspace_manager
        .create_branch(fixture.workspace_id, "temp/keep-me", None)
        .await?;

    // Verify branches were created
    let branches_before = fixture
        .workspace_manager
        .list_branches(fixture.workspace_id)
        .await?;

    let branch_names_before: Vec<&str> = branches_before
        .iter()
        .map(|b| b.branch_name.as_str())
        .collect();

    assert!(branch_names_before.contains(&"temp/delete-me"));
    assert!(branch_names_before.contains(&"temp/keep-me"));

    // Delete one branch
    fixture
        .workspace_manager
        .delete_branch(fixture.workspace_id, "temp/delete-me", false)
        .await?;

    // Verify branch was deleted
    let branches_after = fixture
        .workspace_manager
        .list_branches(fixture.workspace_id)
        .await?;

    let branch_names_after: Vec<&str> = branches_after
        .iter()
        .map(|b| b.branch_name.as_str())
        .collect();

    assert!(!branch_names_after.contains(&"temp/delete-me"));
    assert!(branch_names_after.contains(&"temp/keep-me"));

    Ok(())
}

#[tokio::test]
async fn test_branch_switch_error_conditions() -> Result<()> {
    let fixture = BranchTestFixture::new().await?;

    // Test switching to non-existent branch
    let result = fixture
        .workspace_manager
        .switch_branch(fixture.workspace_id, "non-existent-branch")
        .await;

    assert!(result.is_err());

    // Test deleting current branch
    let current_branch = fixture
        .workspace_manager
        .get_workspace_branch(fixture.workspace_id)
        .await?
        .unwrap_or_else(|| "main".to_string());

    let result = fixture
        .workspace_manager
        .delete_branch(fixture.workspace_id, &current_branch, false)
        .await;

    assert!(result.is_err());

    Ok(())
}

#[tokio::test]
async fn test_branch_switch_with_uncommitted_changes() -> Result<()> {
    let fixture = BranchTestFixture::new().await?;

    // Create a branch to switch to
    fixture
        .workspace_manager
        .create_branch(fixture.workspace_id, "feature/uncommitted-test", None)
        .await?;

    // Create uncommitted changes
    fs::write(
        fixture.repo_path().join("uncommitted.txt"),
        "Uncommitted changes",
    )
    .await?;

    // Test that branch switch fails with uncommitted changes
    let result = fixture
        .workspace_manager
        .switch_branch(fixture.workspace_id, "feature/uncommitted-test")
        .await;

    assert!(result.is_err());

    // Verify we're still on the original branch
    let current_branch = fixture
        .workspace_manager
        .get_workspace_branch(fixture.workspace_id)
        .await?;

    assert!(
        current_branch == Some("main".to_string()) || current_branch == Some("master".to_string())
    );

    Ok(())
}

#[tokio::test]
async fn test_branch_cache_invalidation() -> Result<()> {
    let fixture = BranchTestFixture::new().await?;

    // Create test file and index it
    fixture
        .create_test_file("cached_file.txt", "Initial content")
        .await?;

    // Index the workspace to populate cache
    fixture
        .workspace_manager
        .index_workspace_files(fixture.workspace_id, fixture.repo_path())
        .await?;

    // Create and switch to feature branch
    fixture
        .workspace_manager
        .create_branch(fixture.workspace_id, "feature/cache-test", None)
        .await?;

    let switch_result = fixture
        .workspace_manager
        .switch_branch(fixture.workspace_id, "feature/cache-test")
        .await?;

    // Verify cache invalidations occurred
    assert!(switch_result.cache_invalidations > 0);

    Ok(())
}

#[tokio::test]
async fn test_branch_operations_with_git_integration_disabled() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test_no_git.db");

    let config = DatabaseConfig {
        path: Some(db_path),
        temporary: false,
        compression: false,
        cache_capacity: 1024 * 1024, // 1MB
        compression_factor: 0,
        flush_every_ms: Some(1000),
    };
    let database = Arc::new(SQLiteBackend::new(config).await?);

    // Create workspace manager without git integration
    let workspace_manager = WorkspaceManager::new(database).await?;

    let project_id = workspace_manager
        .create_project("test_project", temp_dir.path())
        .await?;

    let workspace_id = workspace_manager
        .create_workspace(project_id, "main_workspace", None)
        .await?;

    // Test that branch operations fail without git integration
    let result = workspace_manager
        .create_branch(workspace_id, "feature/test", None)
        .await;

    assert!(result.is_err());

    let result = workspace_manager
        .switch_branch(workspace_id, "feature/test")
        .await;

    assert!(result.is_err());

    Ok(())
}

#[tokio::test]
async fn test_branch_creation_with_start_point() -> Result<()> {
    let fixture = BranchTestFixture::new().await?;

    // Create some commits to have different start points
    fixture.create_test_file("file1.txt", "File 1").await?;
    fixture.create_test_file("file2.txt", "File 2").await?;

    // Get current HEAD commit
    let head_output = std::process::Command::new("git")
        .current_dir(fixture.repo_path())
        .args(["rev-parse", "HEAD"])
        .output()?;

    let _head_commit = String::from_utf8(head_output.stdout)?.trim().to_string();

    // Create branch from specific commit (HEAD~1)
    let first_commit_output = std::process::Command::new("git")
        .current_dir(fixture.repo_path())
        .args(["rev-parse", "HEAD~1"])
        .output()?;

    let first_commit = String::from_utf8(first_commit_output.stdout)?
        .trim()
        .to_string();

    // Create branch from first commit
    fixture
        .workspace_manager
        .create_branch(
            fixture.workspace_id,
            "feature/from-first-commit",
            Some(&first_commit),
        )
        .await?;

    // Switch to the branch and verify it has the expected state
    fixture
        .workspace_manager
        .switch_branch(fixture.workspace_id, "feature/from-first-commit")
        .await?;

    // file2.txt should not exist since we branched from earlier commit
    assert!(!fixture.repo_path().join("file2.txt").exists());
    assert!(fixture.repo_path().join("file1.txt").exists());

    Ok(())
}

#[tokio::test]
async fn test_concurrent_branch_operations() -> Result<()> {
    let fixture = BranchTestFixture::new().await?;

    // Create multiple branches concurrently
    let create_tasks = (0..5).map(|i| {
        let workspace_manager = &fixture.workspace_manager;
        let workspace_id = fixture.workspace_id;
        async move {
            workspace_manager
                .create_branch(workspace_id, &format!("feature/concurrent-{}", i), None)
                .await
        }
    });

    let results = futures::future::join_all(create_tasks).await;

    // All creates should succeed
    for result in results {
        result?;
    }

    // Verify all branches were created
    let branches = fixture
        .workspace_manager
        .list_branches(fixture.workspace_id)
        .await?;

    let branch_names: Vec<&str> = branches.iter().map(|b| b.branch_name.as_str()).collect();

    for i in 0..5 {
        assert!(branch_names.contains(&&format!("feature/concurrent-{}", i)[..]));
    }

    Ok(())
}

#[tokio::test]
async fn test_branch_manager_direct_operations() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test_branch_manager.db");

    // Initialize git repository
    std::process::Command::new("git")
        .current_dir(temp_dir.path())
        .args(["init"])
        .output()?;

    std::process::Command::new("git")
        .current_dir(temp_dir.path())
        .args(["config", "user.name", "Test User"])
        .output()?;

    std::process::Command::new("git")
        .current_dir(temp_dir.path())
        .args(["config", "user.email", "test@example.com"])
        .output()?;

    fs::write(temp_dir.path().join("README.md"), "# Test").await?;

    std::process::Command::new("git")
        .current_dir(temp_dir.path())
        .args(["add", "README.md"])
        .output()?;

    std::process::Command::new("git")
        .current_dir(temp_dir.path())
        .args(["commit", "-m", "Initial commit"])
        .output()?;

    let config = DatabaseConfig {
        path: Some(db_path),
        temporary: false,
        compression: false,
        cache_capacity: 1024 * 1024, // 1MB
        compression_factor: 0,
        flush_every_ms: Some(1000),
    };
    let database = Arc::new(SQLiteBackend::new(config).await?);

    let versioning_config = VersioningConfig::default();
    let file_manager = FileVersionManager::new(database.clone(), versioning_config).await?;

    let branch_manager = BranchManager::new(database, file_manager, true).await?;

    let workspace_id = 1;

    // Test direct branch manager operations
    branch_manager
        .create_branch(workspace_id, "feature/direct-test", temp_dir.path(), None)
        .await?;

    let branches = branch_manager
        .list_all_branches(workspace_id, temp_dir.path())
        .await?;

    assert!(branches.len() >= 2);

    let switch_result = branch_manager
        .switch_branch(workspace_id, "feature/direct-test", temp_dir.path())
        .await?;

    assert_eq!(switch_result.new_branch, "feature/direct-test");

    Ok(())
}
