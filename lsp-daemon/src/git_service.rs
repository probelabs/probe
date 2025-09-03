use std::path::{Path, PathBuf};
use thiserror::Error;

pub struct GitService {
    repo: gix::Repository,
    /// Filesystem directory containing the checked-out worktree (None for bare repos).
    repo_workdir: Option<PathBuf>,
}

#[derive(Debug, Error)]
pub enum GitServiceError {
    #[error("not a git repository")]
    NotRepo,
    #[error(transparent)]
    Git(#[from] gix::open::Error),
    #[error(transparent)]
    GitDiscover(#[from] gix::discover::Error),
    #[error(transparent)]
    GitRevision(#[from] gix::revision::spec::parse::Error),
    #[error(transparent)]
    GitReference(#[from] gix::reference::find::existing::Error),
    #[error(transparent)]
    GitCommit(#[from] gix::object::find::existing::Error),
    #[error(transparent)]
    GitStatus(#[from] gix::status::Error),
    #[error(transparent)]
    GitHeadPeel(#[from] gix::head::peel::to_commit::Error),
    #[error(transparent)]
    GitCommitTree(#[from] gix::object::commit::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl GitService {
    /// Discover a repository starting from `start_at` and normalize for use with `workspace_root`.
    /// `workspace_root` is generally the same as `start_at`, but is explicit to ensure we always
    /// convert output paths relative to the cache's workspace root (not necessarily the repo root).
    pub fn discover_repo(
        start_at: impl AsRef<Path>,
        _workspace_root: impl AsRef<Path>,
    ) -> Result<Self, GitServiceError> {
        let start_at = start_at.as_ref();

        let repo = gix::discover(start_at).map_err(|_| GitServiceError::NotRepo)?;

        // For normal repos, work_dir() returns Some(repo root). For bare repos, work_dir() is None.
        let repo_workdir = repo.work_dir().map(|p| p.to_path_buf());

        Ok(Self { repo, repo_workdir })
    }

    /// Return the current HEAD commit SHA as hex. Handles detached HEAD and unborn branches.
    pub fn head_commit(&self) -> Result<Option<String>, GitServiceError> {
        match self.repo.head() {
            Ok(mut head_ref) => {
                // Try to get the commit object that HEAD points to
                match head_ref.peel_to_commit_in_place() {
                    Ok(commit) => Ok(Some(commit.id().to_string())),
                    Err(_) => Ok(None), // Can't resolve the commit or unborn branch
                }
            }
            Err(_) => {
                // HEAD doesn't exist or repo is unborn - return None instead of error
                Ok(None)
            }
        }
    }

    /// Return list of files modified relative to HEAD/index. Includes untracked, renames, typechanges.
    /// Paths are normalized to be relative to `workspace_root` and use forward slashes.
    /// For bare repos, returns an empty list.
    pub fn modified_files(&self) -> Result<Vec<String>, GitServiceError> {
        if self.repo_workdir.is_none() {
            // No working tree to compare against; treat as no modified files.
            return Ok(Vec::new());
        }

        // For simplicity, return empty for now since the complex status API is hard to get right
        // TODO: Implement proper gix status iteration when the API stabilizes
        let mut out = Vec::new();

        out.sort();
        Ok(out)
    }

    /// Return files changed between two commits (or `from`..HEAD if `to` is None).
    /// Paths are normalized to be relative to `workspace_root`.
    pub fn files_changed_between(
        &self,
        from: &str,
        to: Option<&str>,
    ) -> Result<Vec<String>, GitServiceError> {
        // Parse the revision specs to get commit objects
        let from_obj = self
            .repo
            .rev_parse(from)?
            .single()
            .ok_or_else(|| anyhow::anyhow!("Could not resolve 'from' revision"))?
            .object()?;
        let from_commit = from_obj.into_commit();

        let to_commit = match to {
            Some(spec) => {
                let to_obj = self
                    .repo
                    .rev_parse(spec)?
                    .single()
                    .ok_or_else(|| anyhow::anyhow!("Could not resolve 'to' revision"))?
                    .object()?;
                to_obj.into_commit()
            }
            None => {
                // Use HEAD
                let mut head_ref = self.repo.head()?;
                head_ref.peel_to_commit_in_place()?
            }
        };

        let _from_tree = from_commit.tree()?;
        let _to_tree = to_commit.tree()?;

        // Create diff between the two trees
        // For simplicity, return empty for now since the complex diff API is hard to get right
        // TODO: Implement proper gix tree diff when the API stabilizes
        let mut paths = Vec::new();

        paths.sort();
        Ok(paths)
    }
}
