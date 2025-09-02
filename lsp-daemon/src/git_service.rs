use std::path::{Path, PathBuf};
use thiserror::Error;

pub struct GitService {
    repo: git2::Repository,
    /// Where file paths from Git (which are repo-root-relative) should be converted to.
    workspace_root: PathBuf,
    /// Filesystem directory containing the checked-out worktree (None for bare repos).
    repo_workdir: Option<PathBuf>,
    /// Repo root for path resolution. For normal repos this equals workdir; for bare repos we
    /// approximate from `.git` directory parent if available.
    repo_root: PathBuf,
}

#[derive(Debug, Error)]
pub enum GitServiceError {
    #[error("not a git repository")]
    NotRepo,
    #[error(transparent)]
    Git(#[from] git2::Error),
}

impl GitService {
    /// Discover a repository starting from `start_at` and normalize for use with `workspace_root`.
    /// `workspace_root` is generally the same as `start_at`, but is explicit to ensure we always
    /// convert output paths relative to the cache's workspace root (not necessarily the repo root).
    pub fn discover_repo(
        start_at: impl AsRef<Path>,
        workspace_root: impl AsRef<Path>,
    ) -> Result<Self, GitServiceError> {
        let start_at = start_at.as_ref();
        let workspace_root = workspace_root.as_ref().to_path_buf();
        let repo = git2::Repository::discover(start_at).map_err(|e| {
            if e.code() == git2::ErrorCode::NotFound {
                GitServiceError::NotRepo
            } else {
                GitServiceError::Git(e)
            }
        })?;

        // For normal repos, workdir is Some(repo root). For bare repos, workdir is None and
        // repo.path() points to the .git directory. Use its parent if available.
        let repo_workdir = repo.workdir().map(|p| p.to_path_buf());
        let repo_root = if let Some(wd) = &repo_workdir {
            wd.clone()
        } else {
            repo.path()
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| workspace_root.clone())
        };

        Ok(Self {
            repo,
            workspace_root,
            repo_workdir,
            repo_root,
        })
    }

    /// Return the current HEAD commit SHA as hex. Handles detached HEAD and unborn branches.
    pub fn head_commit(&self) -> Result<Option<String>, GitServiceError> {
        match self.repo.head() {
            Ok(head) => {
                // Peel to a commit regardless of symbolic/detached
                let commit = head.peel_to_commit()?;
                Ok(Some(commit.id().to_string()))
            }
            Err(e)
                if e.code() == git2::ErrorCode::UnbornBranch
                    || e.code() == git2::ErrorCode::NotFound =>
            {
                // e.g., repo just initialized without a commit
                Ok(None)
            }
            Err(e) => Err(GitServiceError::Git(e)),
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

        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true)
            .recurse_untracked_dirs(true)
            .renames_head_to_index(true)
            .renames_index_to_workdir(true)
            .exclude_submodules(false)
            .show(git2::StatusShow::IndexAndWorkdir);

        let statuses = self.repo.statuses(Some(&mut opts))?;
        let mut out = Vec::new();

        for e in statuses.iter() {
            let s = e.status();
            let relevant = s.is_wt_new()
                || s.is_wt_modified()
                || s.is_wt_deleted()
                || s.is_index_new()
                || s.is_index_modified()
                || s.is_index_deleted()
                || s.contains(git2::Status::WT_RENAMED)
                || s.contains(git2::Status::INDEX_RENAMED)
                || s.contains(git2::Status::WT_TYPECHANGE)
                || s.contains(git2::Status::INDEX_TYPECHANGE);
            if !relevant {
                continue;
            }

            let path = e
                .head_to_index()
                .and_then(|d| d.new_file().path())
                .or_else(|| e.index_to_workdir().and_then(|d| d.new_file().path()))
                .or_else(|| e.head_to_index().and_then(|d| d.old_file().path()))
                .or_else(|| e.index_to_workdir().and_then(|d| d.old_file().path()))
                .or_else(|| e.path().map(Path::new));

            if let Some(p) = path {
                if let Some(rel) = self.repo_relative_to_workspace(p) {
                    if !out.contains(&rel) {
                        out.push(rel);
                    }
                }
            }
        }

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
        let from_commit = self.repo.revparse_single(from)?.peel_to_commit()?;
        let to_commit = match to {
            Some(spec) => self.repo.revparse_single(spec)?.peel_to_commit()?,
            None => self.repo.head()?.peel_to_commit()?,
        };

        let from_tree = from_commit.tree()?;
        let to_tree = to_commit.tree()?;

        let mut opts = git2::DiffOptions::new();
        opts.include_typechange(true).recurse_untracked_dirs(true);

        let diff =
            self.repo
                .diff_tree_to_tree(Some(&from_tree), Some(&to_tree), Some(&mut opts))?;

        let mut paths = Vec::new();
        diff.foreach(
            &mut |delta, _| {
                if let Some(p) = delta.new_file().path().or_else(|| delta.old_file().path()) {
                    if let Some(rel) = self.repo_relative_to_workspace(p) {
                        if !paths.contains(&rel) {
                            paths.push(rel);
                        }
                    }
                }
                true
            },
            None,
            None,
            None,
        )?;

        paths.sort();
        Ok(paths)
    }

    fn repo_relative_to_workspace(&self, repo_relative: &Path) -> Option<String> {
        // Convert repo-relative path to absolute by joining with repo_root, then diff to workspace_root.
        let abs = self.repo_root.join(repo_relative);

        // Try to get a clean relative path
        if let Ok(canonical_abs) = abs.canonicalize() {
            if let Ok(canonical_ws) = self.workspace_root.canonicalize() {
                if let Some(rel) = pathdiff::diff_paths(&canonical_abs, &canonical_ws) {
                    // Check if the path starts with ".." which means it's outside workspace
                    let rel_str = slashify(&rel);
                    if !rel_str.starts_with("..") {
                        return Some(rel_str);
                    }
                }
            }
        }

        // Fallback to simple relative path
        pathdiff::diff_paths(&abs, &self.workspace_root).map(slashify)
    }
}

fn slashify(p: impl AsRef<Path>) -> String {
    p.as_ref()
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}
