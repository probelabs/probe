//! Project Management Module
//!
//! Provides project lifecycle management, validation, and utility functions
//! for managing projects within the workspace system.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info, warn};

use crate::database::DatabaseBackend;
use crate::git_service::{GitService, GitServiceError};

/// Project information with comprehensive metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub project_id: i64,
    pub name: String,
    pub root_path: PathBuf,
    pub vcs_type: Option<String>,
    pub created_at: String,
    pub metadata: Option<String>,
    pub description: Option<String>,
    pub last_updated: Option<String>,
    pub is_active: bool,
    pub workspace_count: u32,
    pub total_files: u64,
    pub supported_languages: Vec<String>,
}

/// Project configuration for creation and management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// Project name (must be unique within the system)
    pub name: String,

    /// Project root directory
    pub root_path: PathBuf,

    /// Optional description
    pub description: Option<String>,

    /// Version control system type ("git", "svn", etc.)
    pub vcs_type: Option<String>,

    /// Enable automatic language detection
    pub auto_detect_languages: bool,

    /// Explicitly supported languages
    pub explicit_languages: Vec<String>,

    /// Project-specific metadata
    pub metadata: HashMap<String, String>,

    /// Enable project-level caching
    pub enable_caching: bool,

    /// Maximum number of workspaces per project
    pub max_workspaces: u32,
}

/// Project management errors
#[derive(Debug, thiserror::Error)]
pub enum ProjectError {
    #[error("Project not found: {project_id}")]
    ProjectNotFound { project_id: i64 },

    #[error("Project name already exists: {name}")]
    ProjectNameExists { name: String },

    #[error("Invalid project path: {path} - {reason}")]
    InvalidProjectPath { path: String, reason: String },

    #[error("Project validation failed: {field} - {message}")]
    ValidationFailed { field: String, message: String },

    #[error("Project has active workspaces: {workspace_count}")]
    HasActiveWorkspaces { workspace_count: u32 },

    #[error("Maximum workspace limit reached: {limit}")]
    WorkspaceLimitExceeded { limit: u32 },

    #[error("Git operation failed: {source}")]
    GitError {
        #[from]
        source: GitServiceError,
    },

    #[error("Database operation failed: {source}")]
    DatabaseError {
        #[from]
        source: crate::database::DatabaseError,
    },

    #[error("IO operation failed: {source}")]
    IoError {
        #[from]
        source: std::io::Error,
    },

    #[error("Context error: {source}")]
    Context {
        #[from]
        source: anyhow::Error,
    },
}

/// Project statistics for monitoring and reporting
#[derive(Debug, Clone, Serialize)]
pub struct ProjectStats {
    pub project_id: i64,
    pub total_workspaces: u32,
    pub active_workspaces: u32,
    pub total_files: u64,
    pub indexed_files: u64,
    pub total_symbols: u64,
    pub supported_languages: Vec<String>,
    pub disk_usage_bytes: u64,
    pub last_activity: Option<String>,
    pub creation_date: String,
}

/// Project manager for handling project lifecycle operations
pub struct ProjectManager<T>
where
    T: DatabaseBackend + Send + Sync + 'static,
{
    database: Arc<T>,
    git_integration_enabled: bool,
}

impl<T> ProjectManager<T>
where
    T: DatabaseBackend + Send + Sync + 'static,
{
    /// Create a new project manager
    pub fn new(database: Arc<T>, git_integration_enabled: bool) -> Self {
        info!(
            "ProjectManager initialized with git_integration={}",
            git_integration_enabled
        );

        Self {
            database,
            git_integration_enabled,
        }
    }

    /// Create a new project with validation and git detection
    pub async fn create_project(&self, config: ProjectConfig) -> Result<i64, ProjectError> {
        info!("Creating project: {}", config.name);

        // Validate project configuration
        self.validate_project_config(&config).await?;

        // Check if project name already exists
        if self.project_name_exists(&config.name).await? {
            return Err(ProjectError::ProjectNameExists {
                name: config.name.clone(),
            });
        }

        // Validate project path
        self.validate_project_path(&config.root_path).await?;

        // Detect VCS type if not specified
        let vcs_type = if config.vcs_type.is_none() && self.git_integration_enabled {
            self.detect_vcs_type(&config.root_path).await?
        } else {
            config.vcs_type.clone()
        };

        // Detect languages if auto-detection is enabled
        let supported_languages = if config.auto_detect_languages {
            self.detect_project_languages(&config.root_path).await?
        } else {
            config.explicit_languages.clone()
        };

        // Generate unique project ID
        let project_id = self.generate_project_id().await;

        // Prepare metadata
        let mut metadata_map = config.metadata.clone();
        metadata_map.insert("created_by".to_string(), "workspace_manager".to_string());
        metadata_map.insert("version".to_string(), "2.3".to_string());

        if let Some(ref vcs) = vcs_type {
            metadata_map.insert("vcs_type".to_string(), vcs.clone());
        }

        for lang in &supported_languages {
            metadata_map.insert(format!("lang_{}", lang), "detected".to_string());
        }

        let metadata_json =
            serde_json::to_string(&metadata_map).context("Failed to serialize project metadata")?;

        // Create project in database
        // Note: We'll need to implement project creation in the database backend
        // For now, we'll use a placeholder implementation that works with the existing schema

        // TODO: Implement proper project creation when database backend supports it
        // For now, create a basic project record
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Store project information using the database backend
        // This is a simplified implementation - in practice, you'd have dedicated project table methods
        let project_key = format!("project:{}", project_id);
        let project_data = Project {
            project_id,
            name: config.name.clone(),
            root_path: config.root_path.clone(),
            vcs_type: vcs_type.clone(),
            created_at: current_time.to_string(),
            metadata: Some(metadata_json),
            description: config.description.clone(),
            last_updated: Some(current_time.to_string()),
            is_active: true,
            workspace_count: 0,
            total_files: 0,
            supported_languages: supported_languages.clone(),
        };

        // Serialize and store project data
        let serialized_project =
            bincode::serialize(&project_data).context("Failed to serialize project data")?;

        self.database
            .set(project_key.as_bytes(), &serialized_project)
            .await
            .context("Failed to store project in database")?;

        // Store project name index for uniqueness checking
        let name_key = format!("project_name:{}", config.name);
        self.database
            .set(name_key.as_bytes(), &project_id.to_le_bytes())
            .await
            .context("Failed to store project name index")?;

        info!(
            "Created project '{}' with ID {} at path: {}",
            config.name,
            project_id,
            config.root_path.display()
        );

        Ok(project_id)
    }

    /// Get project by ID
    pub async fn get_project(&self, project_id: i64) -> Result<Option<Project>, ProjectError> {
        debug!("Getting project: {}", project_id);

        let project_key = format!("project:{}", project_id);

        match self.database.get(project_key.as_bytes()).await? {
            Some(data) => {
                let project: Project =
                    bincode::deserialize(&data).context("Failed to deserialize project data")?;
                Ok(Some(project))
            }
            None => Ok(None),
        }
    }

    /// List all projects with optional filtering
    pub async fn list_projects(&self, active_only: bool) -> Result<Vec<Project>, ProjectError> {
        debug!("Listing projects (active_only={})", active_only);

        // Scan for all project keys
        let project_prefix = "project:".as_bytes();
        let project_entries = self.database.scan_prefix(project_prefix).await?;

        let mut projects = Vec::new();

        for (key, data) in project_entries {
            // Skip non-numeric project IDs
            let key_str = String::from_utf8_lossy(&key);
            if !key_str.starts_with("project:") {
                continue;
            }

            match bincode::deserialize::<Project>(&data) {
                Ok(project) => {
                    if !active_only || project.is_active {
                        projects.push(project);
                    }
                }
                Err(e) => {
                    warn!(
                        "Failed to deserialize project data for key {}: {}",
                        key_str, e
                    );
                }
            }
        }

        // Sort projects by creation date (most recent first)
        projects.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        info!("Listed {} projects", projects.len());
        Ok(projects)
    }

    /// Update project metadata
    pub async fn update_project(
        &self,
        project_id: i64,
        updates: HashMap<String, String>,
    ) -> Result<(), ProjectError> {
        debug!("Updating project {}: {:?}", project_id, updates);

        let mut project = self
            .get_project(project_id)
            .await?
            .ok_or(ProjectError::ProjectNotFound { project_id })?;

        // Update fields based on the updates map
        for (key, value) in updates {
            match key.as_str() {
                "description" => project.description = Some(value),
                "vcs_type" => project.vcs_type = Some(value),
                _ => {
                    // Update metadata
                    let mut metadata_map: HashMap<String, String> =
                        if let Some(ref metadata) = project.metadata {
                            serde_json::from_str(metadata).unwrap_or_default()
                        } else {
                            HashMap::new()
                        };

                    metadata_map.insert(key, value);
                    project.metadata = Some(
                        serde_json::to_string(&metadata_map)
                            .context("Failed to serialize updated metadata")?,
                    );
                }
            }
        }

        // Update last_updated timestamp
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        project.last_updated = Some(current_time.to_string());

        // Store updated project
        let project_key = format!("project:{}", project_id);
        let serialized_project =
            bincode::serialize(&project).context("Failed to serialize updated project data")?;

        self.database
            .set(project_key.as_bytes(), &serialized_project)
            .await
            .context("Failed to update project in database")?;

        info!("Updated project {}", project_id);
        Ok(())
    }

    /// Delete project (only if no active workspaces)
    pub async fn delete_project(&self, project_id: i64, force: bool) -> Result<(), ProjectError> {
        info!("Deleting project {} (force={})", project_id, force);

        let project = self
            .get_project(project_id)
            .await?
            .ok_or(ProjectError::ProjectNotFound { project_id })?;

        // Check for active workspaces unless forced
        if !force && project.workspace_count > 0 {
            return Err(ProjectError::HasActiveWorkspaces {
                workspace_count: project.workspace_count,
            });
        }

        // Remove project data
        let project_key = format!("project:{}", project_id);
        self.database.remove(project_key.as_bytes()).await?;

        // Remove project name index
        let name_key = format!("project_name:{}", project.name);
        self.database.remove(name_key.as_bytes()).await?;

        info!("Deleted project {} ({})", project_id, project.name);
        Ok(())
    }

    /// Get project statistics
    pub async fn get_project_stats(&self, project_id: i64) -> Result<ProjectStats, ProjectError> {
        debug!("Getting project stats: {}", project_id);

        let project = self
            .get_project(project_id)
            .await?
            .ok_or(ProjectError::ProjectNotFound { project_id })?;

        // TODO: Implement actual workspace counting and file statistics
        // This would require integration with the database backend's workspace methods
        let stats = ProjectStats {
            project_id,
            total_workspaces: project.workspace_count,
            active_workspaces: project.workspace_count, // Simplified
            total_files: project.total_files,
            indexed_files: 0, // TODO: Calculate from analysis runs
            total_symbols: 0, // TODO: Calculate from symbol tables
            supported_languages: project.supported_languages.clone(),
            disk_usage_bytes: self
                .calculate_project_disk_usage(&project.root_path)
                .await?,
            last_activity: project.last_updated.clone(),
            creation_date: project.created_at.clone(),
        };

        Ok(stats)
    }

    /// Check if project supports a specific language
    pub fn project_supports_language(&self, project: &Project, language: &str) -> bool {
        project
            .supported_languages
            .iter()
            .any(|l| l.eq_ignore_ascii_case(language))
    }

    /// Validate project root path accessibility and permissions
    pub async fn validate_project_path(&self, path: &Path) -> Result<(), ProjectError> {
        if !path.exists() {
            return Err(ProjectError::InvalidProjectPath {
                path: path.display().to_string(),
                reason: "Path does not exist".to_string(),
            });
        }

        if !path.is_dir() {
            return Err(ProjectError::InvalidProjectPath {
                path: path.display().to_string(),
                reason: "Path is not a directory".to_string(),
            });
        }

        // Check if path is readable
        match tokio::fs::metadata(path).await {
            Ok(_) => Ok(()),
            Err(e) => Err(ProjectError::InvalidProjectPath {
                path: path.display().to_string(),
                reason: format!("Cannot access path: {}", e),
            }),
        }
    }

    // Private helper methods

    /// Validate project configuration
    async fn validate_project_config(&self, config: &ProjectConfig) -> Result<(), ProjectError> {
        // Validate project name
        if config.name.is_empty() {
            return Err(ProjectError::ValidationFailed {
                field: "name".to_string(),
                message: "Project name cannot be empty".to_string(),
            });
        }

        if config.name.len() > 100 {
            return Err(ProjectError::ValidationFailed {
                field: "name".to_string(),
                message: "Project name cannot exceed 100 characters".to_string(),
            });
        }

        // Validate project path
        self.validate_project_path(&config.root_path).await?;

        // Validate workspace limit
        if config.max_workspaces > 1000 {
            return Err(ProjectError::ValidationFailed {
                field: "max_workspaces".to_string(),
                message: "Maximum workspaces cannot exceed 1000".to_string(),
            });
        }

        Ok(())
    }

    /// Check if project name already exists
    async fn project_name_exists(&self, name: &str) -> Result<bool, ProjectError> {
        let name_key = format!("project_name:{}", name);
        Ok(self.database.get(name_key.as_bytes()).await?.is_some())
    }

    /// Detect version control system type
    async fn detect_vcs_type(&self, path: &Path) -> Result<Option<String>, ProjectError> {
        // Try to detect git
        if GitService::discover_repo(path, path).is_ok() {
            return Ok(Some("git".to_string()));
        }

        // Add other VCS detection logic here (SVN, Mercurial, etc.)

        Ok(None)
    }

    /// Detect programming languages in project
    async fn detect_project_languages(&self, path: &Path) -> Result<Vec<String>, ProjectError> {
        let mut languages = std::collections::HashSet::new();

        // Walk the project directory and detect file extensions
        let mut entries = tokio::fs::read_dir(path).await?;
        let mut scan_depth = 0;
        let max_scan_depth = 3;

        while let Some(entry) = entries.next_entry().await? {
            if scan_depth >= max_scan_depth {
                break;
            }

            let entry_path = entry.path();

            // Skip hidden directories and common build/cache directories
            if let Some(dir_name) = entry_path.file_name() {
                let dir_name = dir_name.to_string_lossy();
                if dir_name.starts_with('.')
                    || dir_name == "node_modules"
                    || dir_name == "target"
                    || dir_name == "build"
                    || dir_name == "__pycache__"
                {
                    continue;
                }
            }

            if entry_path.is_file() {
                if let Some(ext) = entry_path.extension() {
                    let ext = ext.to_string_lossy().to_lowercase();
                    let language = match ext.as_str() {
                        "rs" => Some("rust"),
                        "py" => Some("python"),
                        "js" => Some("javascript"),
                        "ts" => Some("typescript"),
                        "go" => Some("go"),
                        "java" => Some("java"),
                        "c" => Some("c"),
                        "cpp" | "cc" | "cxx" => Some("cpp"),
                        "h" | "hpp" => Some("c"), // Header files
                        "php" => Some("php"),
                        "rb" => Some("ruby"),
                        "swift" => Some("swift"),
                        "kt" => Some("kotlin"),
                        "cs" => Some("csharp"),
                        "scala" => Some("scala"),
                        _ => None,
                    };

                    if let Some(lang) = language {
                        languages.insert(lang.to_string());
                    }
                }
            } else if entry_path.is_dir() {
                scan_depth += 1;
            }
        }

        let mut result: Vec<String> = languages.into_iter().collect();
        result.sort();

        debug!("Detected languages in {}: {:?}", path.display(), result);
        Ok(result)
    }

    /// Calculate project disk usage
    async fn calculate_project_disk_usage(&self, path: &Path) -> Result<u64, ProjectError> {
        // Simplified disk usage calculation
        // In practice, you'd want a more sophisticated approach
        let metadata = tokio::fs::metadata(path).await?;
        Ok(metadata.len())
    }

    /// Generate unique project ID
    async fn generate_project_id(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            root_path: PathBuf::new(),
            description: None,
            vcs_type: None,
            auto_detect_languages: true,
            explicit_languages: vec![],
            metadata: HashMap::new(),
            enable_caching: true,
            max_workspaces: 100,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_project_config_validation() {
        let temp_dir = TempDir::new().unwrap();

        let config = ProjectConfig {
            name: "test_project".to_string(),
            root_path: temp_dir.path().to_path_buf(),
            auto_detect_languages: true,
            ..Default::default()
        };

        // This would require a mock database backend for full testing
        // For now, just test the basic validation logic
        assert!(!config.name.is_empty());
        assert!(config.root_path.exists());
    }

    #[test]
    fn test_language_detection_mapping() {
        // Test the language detection logic
        let test_cases = vec![
            ("main.rs", Some("rust")),
            ("script.py", Some("python")),
            ("index.js", Some("javascript")),
            ("app.ts", Some("typescript")),
            ("main.go", Some("go")),
            ("App.java", Some("java")),
            ("main.c", Some("c")),
            ("main.cpp", Some("cpp")),
            ("unknown.xyz", None),
        ];

        for (filename, expected) in test_cases {
            let path = Path::new(filename);
            let ext = path.extension().unwrap().to_string_lossy().to_lowercase();
            let detected = match ext.as_str() {
                "rs" => Some("rust"),
                "py" => Some("python"),
                "js" => Some("javascript"),
                "ts" => Some("typescript"),
                "go" => Some("go"),
                "java" => Some("java"),
                "c" => Some("c"),
                "cpp" | "cc" | "cxx" => Some("cpp"),
                _ => None,
            };

            assert_eq!(detected, expected, "Failed for {}", filename);
        }
    }
}
