#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn init_test_repo() -> (TempDir, git2::Repository) {
        let dir = TempDir::new().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();

        // Configure git user for commits
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        (dir, repo)
    }

    #[test]
    fn test_git_service_non_git_directory() {
        let temp_dir = TempDir::new().unwrap();
        let result =
            crate::git_service::GitService::discover_repo(temp_dir.path(), temp_dir.path());

        assert!(result.is_err());
        match result {
            Err(crate::git_service::GitServiceError::NotRepo) => {}
            _ => panic!("Expected NotRepo error"),
        }
    }

    #[test]
    fn test_git_service_empty_repo() {
        let (temp_dir, _repo) = init_test_repo();
        let service =
            crate::git_service::GitService::discover_repo(temp_dir.path(), temp_dir.path())
                .unwrap();

        // Empty repo has no HEAD commit
        let head = service.head_commit().unwrap();
        assert_eq!(head, None);

        // No modified files in empty repo
        let modified = service.modified_files().unwrap();
        assert!(modified.is_empty());
    }

    #[test]
    fn test_git_service_with_commit() {
        let (temp_dir, repo) = init_test_repo();

        // Create a file and commit it
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "initial content").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("test.txt")).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();

        repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
            .unwrap();

        let service =
            crate::git_service::GitService::discover_repo(temp_dir.path(), temp_dir.path())
                .unwrap();

        // Should have a HEAD commit now
        let head = service.head_commit().unwrap();
        assert!(head.is_some());

        // No modified files (everything is committed)
        let modified = service.modified_files().unwrap();
        assert!(modified.is_empty());
    }

    #[test]
    fn test_git_service_modified_files() {
        let (temp_dir, repo) = init_test_repo();

        // Create and commit initial file
        let file_path = temp_dir.path().join("committed.txt");
        fs::write(&file_path, "committed content").unwrap();

        let mut index = repo.index().unwrap();
        index
            .add_path(std::path::Path::new("committed.txt"))
            .unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();

        repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
            .unwrap();

        // Now modify the committed file
        fs::write(&file_path, "modified content").unwrap();

        // Add a new untracked file
        let new_file = temp_dir.path().join("new.txt");
        fs::write(&new_file, "new content").unwrap();

        let service =
            crate::git_service::GitService::discover_repo(temp_dir.path(), temp_dir.path())
                .unwrap();

        let modified = service.modified_files().unwrap();
        println!("Modified files: {:?}", modified);

        // Files should be detected as modified
        assert!(modified.len() > 0, "Should have modified files");
        assert!(
            modified.contains(&"committed.txt".to_string())
                || modified.contains(&"new.txt".to_string()),
            "Should detect at least one modified file, got: {:?}",
            modified
        );
    }

    #[test]
    fn test_workspace_context_with_git() {
        let (temp_dir, repo) = init_test_repo();

        // Create and commit a file
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "content").unwrap();

        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("test.txt")).unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();

        let commit_oid = repo
            .commit(Some("HEAD"), &sig, &sig, "Test commit", &tree, &[])
            .unwrap();

        // Modify the file
        fs::write(&file_path, "modified").unwrap();

        // Test WorkspaceContext integration
        let ctx = crate::database::duckdb_backend::WorkspaceContext::new_with_git(
            "test_workspace",
            temp_dir.path().to_str().unwrap(),
        );

        assert_eq!(ctx.workspace_id, "test_workspace");
        assert!(ctx.current_commit.is_some(), "Should have a commit hash");

        // The commit hash should match
        let commit_hash = ctx.current_commit.unwrap();
        assert_eq!(commit_hash, commit_oid.to_string());

        // Should detect the modified file
        println!("Modified files in context: {:?}", ctx.modified_files);
        assert!(
            ctx.modified_files.contains(&"test.txt".to_string()),
            "Should detect test.txt as modified, got: {:?}",
            ctx.modified_files
        );
    }

    #[test]
    fn test_workspace_context_non_git() {
        let temp_dir = TempDir::new().unwrap();

        let ctx = crate::database::duckdb_backend::WorkspaceContext::new_with_git(
            "non_git",
            temp_dir.path().to_str().unwrap(),
        );

        assert_eq!(ctx.workspace_id, "non_git");
        assert!(ctx.current_commit.is_none());
        assert!(ctx.modified_files.is_empty());
    }
}
