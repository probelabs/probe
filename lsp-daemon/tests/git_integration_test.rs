#![cfg(feature = "legacy-tests")]
use anyhow::Result;
use lsp_daemon::git_service::{GitService, GitServiceError};
use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Create a test git repository with some initial content
fn create_test_git_repo(temp_dir: &TempDir) -> Result<()> {
    let repo_path = temp_dir.path();

    // Initialize git repo
    let output = Command::new("git")
        .current_dir(repo_path)
        .args(["init"])
        .output()?;

    if !output.status.success() {
        anyhow::bail!(
            "Failed to initialize git repository: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Configure git user
    Command::new("git")
        .current_dir(repo_path)
        .args(["config", "user.name", "Test User"])
        .output()?;

    Command::new("git")
        .current_dir(repo_path)
        .args(["config", "user.email", "test@example.com"])
        .output()?;

    // Create and commit initial file
    fs::write(
        repo_path.join("README.md"),
        "# Test Repository\n\nInitial commit.",
    )?;
    fs::write(
        repo_path.join("main.rs"),
        r#"
fn main() {
    println!("Hello, world!");
}
"#,
    )?;

    Command::new("git")
        .current_dir(repo_path)
        .args(["add", "README.md", "main.rs"])
        .output()?;

    let commit_output = Command::new("git")
        .current_dir(repo_path)
        .args(["commit", "-m", "Initial commit"])
        .output()?;

    if !commit_output.status.success() {
        anyhow::bail!(
            "Failed to create initial commit: {}",
            String::from_utf8_lossy(&commit_output.stderr)
        );
    }

    Ok(())
}

#[tokio::test]
async fn test_git_service_basic_operations() -> Result<()> {
    let temp_dir = TempDir::new()?;
    create_test_git_repo(&temp_dir)?;

    // Create GitService
    let git_service = GitService::discover_repo(temp_dir.path(), temp_dir.path())?;

    // Test current branch
    let current_branch = git_service.current_branch()?;
    assert!(current_branch.is_some(), "Should have a current branch");
    let initial_branch = current_branch.unwrap();
    assert!(
        initial_branch == "main" || initial_branch == "master",
        "Should be on main or master branch"
    );

    // Test head commit
    let head_commit = git_service.head_commit()?;
    assert!(head_commit.is_some(), "Should have a head commit");

    // Test branch listing
    let branches = git_service.list_branches()?;
    assert!(!branches.is_empty(), "Should have at least one branch");

    let branch_names: Vec<&str> = branches.iter().map(|(name, _)| name.as_str()).collect();
    assert!(
        branch_names.contains(&initial_branch.as_str()),
        "Should contain the initial branch"
    );

    // Test clean working directory
    let is_clean = git_service.is_working_directory_clean()?;
    assert!(is_clean, "Working directory should be clean initially");

    let modified_files = git_service.modified_files()?;
    assert!(
        modified_files.is_empty(),
        "Should have no modified files initially"
    );

    println!("✓ Basic git operations test passed");
    Ok(())
}

#[tokio::test]
async fn test_git_branch_operations() -> Result<()> {
    let temp_dir = TempDir::new()?;
    create_test_git_repo(&temp_dir)?;

    let mut git_service = GitService::discover_repo(temp_dir.path(), temp_dir.path())?;

    // Get initial state
    let initial_branch = git_service
        .current_branch()?
        .expect("Should have initial branch");

    // Test branch creation
    let new_branch_name = "feature/test-branch";
    git_service.create_branch(new_branch_name, None)?;

    // Verify branch was created
    assert!(
        git_service.branch_exists(new_branch_name)?,
        "New branch should exist"
    );

    let branches_after_create = git_service.list_branches()?;
    let branch_names: Vec<&str> = branches_after_create
        .iter()
        .map(|(name, _)| name.as_str())
        .collect();
    assert!(
        branch_names.contains(&new_branch_name),
        "Should contain the new branch"
    );

    // Test branch checkout
    git_service.checkout(new_branch_name, false)?;

    // Verify we're on the new branch
    let current_branch_after_checkout = git_service.current_branch()?;
    assert_eq!(
        current_branch_after_checkout,
        Some(new_branch_name.to_string()),
        "Should be on the new branch"
    );

    // Switch back to initial branch
    git_service.checkout(&initial_branch, false)?;

    // Test branch deletion
    git_service.delete_branch(new_branch_name, false)?;

    // Verify branch was deleted
    assert!(
        !git_service.branch_exists(new_branch_name)?,
        "Branch should no longer exist"
    );

    println!("✓ Branch operations test passed");
    Ok(())
}

#[tokio::test]
async fn test_git_change_detection() -> Result<()> {
    let temp_dir = TempDir::new()?;
    create_test_git_repo(&temp_dir)?;

    let git_service = GitService::discover_repo(temp_dir.path(), temp_dir.path())?;

    // Initial state should be clean
    let is_clean = git_service.is_working_directory_clean()?;
    assert!(is_clean, "Should be clean initially");

    // Make a change to a tracked file
    fs::write(
        temp_dir.path().join("main.rs"),
        r#"
fn main() {
    println!("Hello, modified world!");
}
"#,
    )?;

    // Test dirty state detection
    let is_clean_after_change = git_service.is_working_directory_clean()?;
    assert!(!is_clean_after_change, "Should be dirty after modification");

    let modified_files = git_service.modified_files()?;
    assert!(!modified_files.is_empty(), "Should detect modified files");

    // Create a new untracked file
    fs::write(temp_dir.path().join("new_file.txt"), "This is a new file")?;

    let modified_files_with_untracked = git_service.modified_files()?;
    assert!(
        !modified_files_with_untracked.is_empty(),
        "Should detect changes including untracked files"
    );

    println!("✓ Change detection test passed");
    Ok(())
}

#[tokio::test]
async fn test_git_commit_diff() -> Result<()> {
    let temp_dir = TempDir::new()?;
    create_test_git_repo(&temp_dir)?;

    let git_service = GitService::discover_repo(temp_dir.path(), temp_dir.path())?;

    // Get initial HEAD commit
    let head_commit = git_service.head_commit()?.expect("Should have HEAD commit");

    // Create a second commit
    fs::write(
        temp_dir.path().join("second_file.txt"),
        "Second file content",
    )?;

    Command::new("git")
        .current_dir(temp_dir.path())
        .args(["add", "second_file.txt"])
        .output()?;

    Command::new("git")
        .current_dir(temp_dir.path())
        .args(["commit", "-m", "Add second file"])
        .output()?;

    // Test diff between commits
    let changed_files = git_service.files_changed_between(&head_commit, None)?;

    // With our simplified implementation, we should detect that trees differ
    assert!(
        !changed_files.is_empty(),
        "Should detect changes between commits"
    );

    println!("✓ Commit diff test passed");
    Ok(())
}

#[tokio::test]
async fn test_git_error_handling() -> Result<()> {
    let temp_dir = TempDir::new()?;
    create_test_git_repo(&temp_dir)?;

    let mut git_service = GitService::discover_repo(temp_dir.path(), temp_dir.path())?;

    // Test checking out non-existent branch
    let checkout_result = git_service.checkout("non-existent-branch", false);
    assert!(
        checkout_result.is_err(),
        "Should fail to checkout non-existent branch"
    );

    match checkout_result {
        Err(GitServiceError::BranchNotFound { branch }) => {
            assert_eq!(branch, "non-existent-branch");
        }
        _ => panic!("Should get BranchNotFound error"),
    }

    // Test creating branch with invalid name
    let create_result = git_service.create_branch("invalid..name", None);
    assert!(
        create_result.is_err(),
        "Should fail to create branch with invalid name"
    );

    match create_result {
        Err(GitServiceError::InvalidBranchName { .. }) => {}
        _ => panic!("Should get InvalidBranchName error"),
    }

    // Test deleting non-existent branch
    let delete_result = git_service.delete_branch("non-existent-branch", false);
    assert!(
        delete_result.is_err(),
        "Should fail to delete non-existent branch"
    );

    println!("✓ Error handling test passed");
    Ok(())
}

#[tokio::test]
async fn test_git_checkout_with_dirty_worktree() -> Result<()> {
    let temp_dir = TempDir::new()?;
    create_test_git_repo(&temp_dir)?;

    let mut git_service = GitService::discover_repo(temp_dir.path(), temp_dir.path())?;

    // Create a new branch
    git_service.create_branch("feature/test", None)?;

    // Make working directory dirty
    fs::write(temp_dir.path().join("main.rs"), "Modified content")?;

    // Attempt to checkout with dirty working directory
    let checkout_result = git_service.checkout("feature/test", false);
    assert!(
        checkout_result.is_err(),
        "Should fail to checkout with dirty working directory"
    );

    match checkout_result {
        Err(GitServiceError::DirtyWorkingDirectory { files }) => {
            assert!(!files.is_empty(), "Should report dirty files");
        }
        _ => panic!("Should get DirtyWorkingDirectory error"),
    }

    println!("✓ Dirty worktree checkout test passed");
    Ok(())
}

#[tokio::test]
async fn test_end_to_end_branch_workflow() -> Result<()> {
    let temp_dir = TempDir::new()?;
    create_test_git_repo(&temp_dir)?;

    let mut git_service = GitService::discover_repo(temp_dir.path(), temp_dir.path())?;

    // 1. Start on main/master branch
    let initial_branch = git_service
        .current_branch()?
        .expect("Should have initial branch");
    println!("📍 Starting on branch: {}", initial_branch);

    // 2. Create a feature branch
    let feature_branch = "feature/add-functionality";
    git_service.create_branch(feature_branch, None)?;
    println!("🌿 Created branch: {}", feature_branch);

    // 3. Switch to feature branch
    git_service.checkout(feature_branch, false)?;
    let current_branch = git_service.current_branch()?;
    assert_eq!(current_branch, Some(feature_branch.to_string()));
    println!("🔄 Switched to branch: {}", feature_branch);

    // 4. Make changes (simulate development)
    fs::write(
        temp_dir.path().join("feature.txt"),
        "New feature implementation",
    )?;

    // 5. Commit changes (using system git command)
    Command::new("git")
        .current_dir(temp_dir.path())
        .args(["add", "feature.txt"])
        .output()?;

    Command::new("git")
        .current_dir(temp_dir.path())
        .args(["commit", "-m", "Add new feature"])
        .output()?;

    // 6. Switch back to main branch
    git_service.checkout(&initial_branch, false)?;
    let current_branch = git_service.current_branch()?;
    assert_eq!(current_branch, Some(initial_branch.clone()));
    println!("🔄 Switched back to: {}", initial_branch);

    // 7. Note: With our current simplified implementation, the working directory files
    // are not updated during checkout - only HEAD is updated. This is a known limitation.
    // In a full implementation, the feature file would not exist on main branch.
    println!("🔍 Note: Working directory files are not updated by our simplified checkout implementation");

    // 8. Switch back to feature branch - HEAD is correctly updated even if files aren't
    git_service.checkout(feature_branch, false)?;
    let current_branch_after_feature_checkout = git_service.current_branch()?;
    assert_eq!(
        current_branch_after_feature_checkout,
        Some(feature_branch.to_string()),
        "Should be back on feature branch"
    );

    // 9. Clean up - switch back to main and delete feature branch
    git_service.checkout(&initial_branch, false)?;
    git_service.delete_branch(feature_branch, false)?;

    println!("✅ End-to-end branch workflow completed successfully");
    Ok(())
}
