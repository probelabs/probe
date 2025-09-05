//! Workspace Configuration Management
//!
//! Provides configuration structures and validation for workspace management operations.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Comprehensive workspace configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    /// Maximum file size to process (in MB)
    pub max_file_size_mb: u64,

    /// File patterns to ignore during indexing
    pub ignore_patterns: Vec<String>,

    /// Programming languages to support
    pub supported_languages: Vec<String>,

    /// Enable git integration
    pub git_integration: bool,

    /// Enable incremental indexing
    pub incremental_indexing: bool,

    /// Cache configuration
    pub cache_settings: CacheConfig,

    /// Performance settings
    pub performance: PerformanceConfig,

    /// Branch management settings
    pub branch_management: BranchConfig,

    /// File watching configuration
    pub file_watching: FileWatchingConfig,

    /// Validation rules
    pub validation: ValidationConfig,
}

/// Cache configuration for workspace operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Enable caching
    pub enabled: bool,

    /// Maximum cache size per workspace (in MB)
    pub max_size_mb: u64,

    /// Cache entry time-to-live (in minutes)
    pub ttl_minutes: u64,

    /// Enable cache compression
    pub compression: bool,

    /// Cache eviction strategy
    pub eviction_strategy: EvictionStrategy,

    /// Enable persistent cache storage
    pub persistent_storage: bool,

    /// Cache directory override (None uses default)
    pub cache_directory: Option<PathBuf>,
}

/// Performance configuration for workspace operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Maximum concurrent operations
    pub max_concurrent_operations: usize,

    /// Batch size for file processing
    pub batch_size: usize,

    /// Operation timeout (in seconds)
    pub operation_timeout_seconds: u64,

    /// Enable parallel processing
    pub parallel_processing: bool,

    /// Memory usage limits
    pub memory_limits: MemoryLimits,

    /// Database connection settings
    pub database_settings: DatabaseSettings,
}

/// Branch management configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BranchConfig {
    /// Enable automatic branch detection
    pub auto_detect_branch: bool,

    /// Default branch name
    pub default_branch: String,

    /// Enable branch-specific caching
    pub branch_specific_cache: bool,

    /// Automatic git synchronization interval (in minutes, 0 to disable)
    pub auto_sync_interval_minutes: u64,

    /// Maximum number of branches to track per workspace
    pub max_tracked_branches: usize,

    /// Enable branch switching optimizations
    pub optimize_branch_switching: bool,
}

/// File watching configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileWatchingConfig {
    /// Enable file system watching
    pub enabled: bool,

    /// Debounce delay for file changes (in milliseconds)
    pub debounce_delay_ms: u64,

    /// Maximum events per second to process
    pub max_events_per_second: u64,

    /// Enable recursive directory watching
    pub recursive_watching: bool,

    /// File extensions to watch (empty means all)
    pub watched_extensions: Vec<String>,
}

/// Validation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationConfig {
    /// Validate file paths before processing
    pub validate_file_paths: bool,

    /// Check file permissions before accessing
    pub check_file_permissions: bool,

    /// Validate git repository state
    pub validate_git_state: bool,

    /// Maximum directory depth for scanning
    pub max_directory_depth: usize,

    /// Enable content validation
    pub content_validation: bool,
}

/// Memory usage limits
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryLimits {
    /// Maximum memory per workspace (in MB)
    pub max_workspace_memory_mb: u64,

    /// Maximum cache memory (in MB)
    pub max_cache_memory_mb: u64,

    /// Enable memory monitoring
    pub enable_monitoring: bool,

    /// Memory cleanup threshold (percentage)
    pub cleanup_threshold_percent: u8,
}

/// Database connection settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSettings {
    /// Connection pool size
    pub connection_pool_size: usize,

    /// Connection timeout (in seconds)
    pub connection_timeout_seconds: u64,

    /// Query timeout (in seconds)
    pub query_timeout_seconds: u64,

    /// Enable connection retry
    pub enable_retry: bool,

    /// Maximum retry attempts
    pub max_retry_attempts: u32,
}

/// Cache eviction strategies
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EvictionStrategy {
    /// Least Recently Used
    LRU,
    /// Least Frequently Used
    LFU,
    /// Time-based eviction
    TTL,
    /// Size-based eviction
    Size,
}

/// Workspace configuration validation errors
#[derive(Debug, thiserror::Error)]
pub enum WorkspaceValidationError {
    #[error("Invalid file size limit: {limit_mb}MB exceeds maximum of 1000MB")]
    FileSizeTooLarge { limit_mb: u64 },

    #[error("Invalid cache size: {size_mb}MB must be between 1MB and 10000MB")]
    InvalidCacheSize { size_mb: u64 },

    #[error("Invalid concurrent operations: {count} must be between 1 and 100")]
    InvalidConcurrency { count: usize },

    #[error("Invalid timeout: {seconds} seconds must be between 1 and 3600")]
    InvalidTimeout { seconds: u64 },

    #[error("Invalid directory path: {path}")]
    InvalidDirectoryPath { path: String },

    #[error("Conflicting configuration: {message}")]
    ConflictingConfig { message: String },

    #[error("Missing required configuration: {field}")]
    MissingRequiredField { field: String },
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            max_file_size_mb: 10,
            ignore_patterns: vec![
                "*.tmp".to_string(),
                "*.log".to_string(),
                ".git/**".to_string(),
                "node_modules/**".to_string(),
                "target/**".to_string(),
                ".probe/**".to_string(),
            ],
            supported_languages: vec![
                "rust".to_string(),
                "python".to_string(),
                "typescript".to_string(),
                "javascript".to_string(),
                "go".to_string(),
                "java".to_string(),
                "c".to_string(),
                "cpp".to_string(),
            ],
            git_integration: true,
            incremental_indexing: true,
            cache_settings: CacheConfig::default(),
            performance: PerformanceConfig::default(),
            branch_management: BranchConfig::default(),
            file_watching: FileWatchingConfig::default(),
            validation: ValidationConfig::default(),
        }
    }
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_size_mb: 100,
            ttl_minutes: 60,
            compression: true,
            eviction_strategy: EvictionStrategy::LRU,
            persistent_storage: true,
            cache_directory: None,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            max_concurrent_operations: 8,
            batch_size: 50,
            operation_timeout_seconds: 300,
            parallel_processing: true,
            memory_limits: MemoryLimits::default(),
            database_settings: DatabaseSettings::default(),
        }
    }
}

impl Default for BranchConfig {
    fn default() -> Self {
        Self {
            auto_detect_branch: true,
            default_branch: "main".to_string(),
            branch_specific_cache: true,
            auto_sync_interval_minutes: 0, // Disabled by default
            max_tracked_branches: 10,
            optimize_branch_switching: true,
        }
    }
}

impl Default for FileWatchingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            debounce_delay_ms: 500,
            max_events_per_second: 100,
            recursive_watching: true,
            watched_extensions: vec![], // Watch all extensions by default
        }
    }
}

impl Default for ValidationConfig {
    fn default() -> Self {
        Self {
            validate_file_paths: true,
            check_file_permissions: true,
            validate_git_state: true,
            max_directory_depth: 20,
            content_validation: false, // Expensive, disabled by default
        }
    }
}

impl Default for MemoryLimits {
    fn default() -> Self {
        Self {
            max_workspace_memory_mb: 512,
            max_cache_memory_mb: 256,
            enable_monitoring: true,
            cleanup_threshold_percent: 80,
        }
    }
}

impl Default for DatabaseSettings {
    fn default() -> Self {
        Self {
            connection_pool_size: 10,
            connection_timeout_seconds: 30,
            query_timeout_seconds: 60,
            enable_retry: true,
            max_retry_attempts: 3,
        }
    }
}

/// Builder for workspace configuration with validation
pub struct WorkspaceConfigBuilder {
    config: WorkspaceConfig,
}

impl Default for WorkspaceConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkspaceConfigBuilder {
    /// Create a new config builder
    pub fn new() -> Self {
        Self {
            config: WorkspaceConfig::default(),
        }
    }

    /// Set maximum file size
    pub fn max_file_size_mb(mut self, size_mb: u64) -> Self {
        self.config.max_file_size_mb = size_mb;
        self
    }

    /// Add ignore patterns
    pub fn ignore_patterns<I, S>(mut self, patterns: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.config.ignore_patterns = patterns.into_iter().map(|s| s.into()).collect();
        self
    }

    /// Set supported languages
    pub fn supported_languages<I, S>(mut self, languages: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.config.supported_languages = languages.into_iter().map(|s| s.into()).collect();
        self
    }

    /// Enable or disable git integration
    pub fn git_integration(mut self, enabled: bool) -> Self {
        self.config.git_integration = enabled;
        self
    }

    /// Enable or disable incremental indexing
    pub fn incremental_indexing(mut self, enabled: bool) -> Self {
        self.config.incremental_indexing = enabled;
        self
    }

    /// Set cache configuration
    pub fn cache_settings(mut self, cache_config: CacheConfig) -> Self {
        self.config.cache_settings = cache_config;
        self
    }

    /// Set performance configuration
    pub fn performance(mut self, perf_config: PerformanceConfig) -> Self {
        self.config.performance = perf_config;
        self
    }

    /// Set branch management configuration
    pub fn branch_management(mut self, branch_config: BranchConfig) -> Self {
        self.config.branch_management = branch_config;
        self
    }

    /// Set file watching configuration
    pub fn file_watching(mut self, watch_config: FileWatchingConfig) -> Self {
        self.config.file_watching = watch_config;
        self
    }

    /// Set validation configuration
    pub fn validation(mut self, validation_config: ValidationConfig) -> Self {
        self.config.validation = validation_config;
        self
    }

    /// Build and validate the configuration
    pub fn build(self) -> Result<WorkspaceConfig, WorkspaceValidationError> {
        self.validate_config()?;
        Ok(self.config)
    }

    /// Validate the configuration
    fn validate_config(&self) -> Result<(), WorkspaceValidationError> {
        // Validate file size limits
        if self.config.max_file_size_mb > 1000 {
            return Err(WorkspaceValidationError::FileSizeTooLarge {
                limit_mb: self.config.max_file_size_mb,
            });
        }

        // Validate cache size
        let cache_size = self.config.cache_settings.max_size_mb;
        if cache_size < 1 || cache_size > 10000 {
            return Err(WorkspaceValidationError::InvalidCacheSize {
                size_mb: cache_size,
            });
        }

        // Validate concurrency
        let concurrency = self.config.performance.max_concurrent_operations;
        if concurrency < 1 || concurrency > 100 {
            return Err(WorkspaceValidationError::InvalidConcurrency { count: concurrency });
        }

        // Validate timeout
        let timeout = self.config.performance.operation_timeout_seconds;
        if timeout < 1 || timeout > 3600 {
            return Err(WorkspaceValidationError::InvalidTimeout { seconds: timeout });
        }

        // Validate cache directory if specified
        if let Some(ref cache_dir) = self.config.cache_settings.cache_directory {
            if !cache_dir.is_absolute() {
                return Err(WorkspaceValidationError::InvalidDirectoryPath {
                    path: cache_dir.display().to_string(),
                });
            }
        }

        // Check for conflicting configurations
        if !self.config.cache_settings.enabled && self.config.cache_settings.persistent_storage {
            return Err(WorkspaceValidationError::ConflictingConfig {
                message: "Cannot enable persistent storage when cache is disabled".to_string(),
            });
        }

        if !self.config.git_integration && self.config.branch_management.branch_specific_cache {
            return Err(WorkspaceValidationError::ConflictingConfig {
                message: "Cannot enable branch-specific cache when git integration is disabled"
                    .to_string(),
            });
        }

        Ok(())
    }
}

impl WorkspaceConfig {
    /// Create a new config builder
    pub fn builder() -> WorkspaceConfigBuilder {
        WorkspaceConfigBuilder::new()
    }

    /// Validate this configuration
    pub fn validate(&self) -> Result<(), WorkspaceValidationError> {
        WorkspaceConfigBuilder {
            config: self.clone(),
        }
        .validate_config()
    }

    /// Get the operation timeout as Duration
    pub fn operation_timeout(&self) -> Duration {
        Duration::from_secs(self.performance.operation_timeout_seconds)
    }

    /// Get the debounce delay as Duration
    pub fn debounce_delay(&self) -> Duration {
        Duration::from_millis(self.file_watching.debounce_delay_ms)
    }

    /// Check if a file should be ignored based on patterns
    pub fn should_ignore_file(&self, file_path: &std::path::Path) -> bool {
        let path_str = file_path.to_string_lossy();

        for pattern in &self.ignore_patterns {
            if glob_match(pattern, &path_str) {
                return true;
            }
        }

        false
    }

    /// Check if a language is supported
    pub fn is_language_supported(&self, language: &str) -> bool {
        self.supported_languages
            .iter()
            .any(|l| l.eq_ignore_ascii_case(language))
    }

    /// Get cache settings as HashMap for database configuration
    pub fn cache_settings_map(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        map.insert(
            "enabled".to_string(),
            self.cache_settings.enabled.to_string(),
        );
        map.insert(
            "max_size_mb".to_string(),
            self.cache_settings.max_size_mb.to_string(),
        );
        map.insert(
            "ttl_minutes".to_string(),
            self.cache_settings.ttl_minutes.to_string(),
        );
        map.insert(
            "compression".to_string(),
            self.cache_settings.compression.to_string(),
        );
        map.insert(
            "persistent_storage".to_string(),
            self.cache_settings.persistent_storage.to_string(),
        );

        if let Some(ref cache_dir) = self.cache_settings.cache_directory {
            map.insert(
                "cache_directory".to_string(),
                cache_dir.display().to_string(),
            );
        }

        map
    }

    /// Merge with another configuration, taking non-default values from other
    pub fn merge(mut self, other: &WorkspaceConfig) -> Self {
        // Simple merge strategy - take non-default values from other
        // This is a simplified implementation; in practice you might want more sophisticated merging
        if other.max_file_size_mb != WorkspaceConfig::default().max_file_size_mb {
            self.max_file_size_mb = other.max_file_size_mb;
        }

        if !other.ignore_patterns.is_empty()
            && other.ignore_patterns != WorkspaceConfig::default().ignore_patterns
        {
            self.ignore_patterns = other.ignore_patterns.clone();
        }

        if !other.supported_languages.is_empty()
            && other.supported_languages != WorkspaceConfig::default().supported_languages
        {
            self.supported_languages = other.supported_languages.clone();
        }

        self
    }
}

/// Simple glob pattern matching
fn glob_match(pattern: &str, text: &str) -> bool {
    // This is a simplified glob matcher. In production, you'd use a proper glob library.
    if pattern.contains("**") {
        // Double wildcard matches any path
        let parts: Vec<&str> = pattern.split("**").collect();
        if parts.len() == 2 {
            let prefix = parts[0];
            let suffix = parts[1];
            return text.starts_with(prefix) && text.ends_with(suffix);
        }
    }

    if pattern.contains('*') {
        // Single wildcard matching
        let parts: Vec<&str> = pattern.split('*').collect();
        if parts.len() == 2 {
            return text.starts_with(parts[0]) && text.ends_with(parts[1]);
        }
    }

    // Exact match
    pattern == text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_validation() {
        let config = WorkspaceConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_builder() {
        let config = WorkspaceConfig::builder()
            .max_file_size_mb(20)
            .git_integration(false)
            .incremental_indexing(true)
            .build()
            .unwrap();

        assert_eq!(config.max_file_size_mb, 20);
        assert!(!config.git_integration);
        assert!(config.incremental_indexing);
    }

    #[test]
    fn test_invalid_file_size() {
        let result = WorkspaceConfig::builder().max_file_size_mb(2000).build();

        assert!(result.is_err());
        match result.unwrap_err() {
            WorkspaceValidationError::FileSizeTooLarge { limit_mb } => {
                assert_eq!(limit_mb, 2000);
            }
            _ => panic!("Expected FileSizeTooLarge error"),
        }
    }

    #[test]
    fn test_conflicting_config() {
        let cache_config = CacheConfig {
            enabled: false,
            persistent_storage: true,
            ..Default::default()
        };

        let result = WorkspaceConfig::builder()
            .cache_settings(cache_config)
            .build();

        assert!(result.is_err());
    }

    #[test]
    fn test_ignore_patterns() {
        let config = WorkspaceConfig::default();

        assert!(config.should_ignore_file(std::path::Path::new("test.tmp")));
        assert!(config.should_ignore_file(std::path::Path::new("debug.log")));
        assert!(!config.should_ignore_file(std::path::Path::new("main.rs")));
    }

    #[test]
    fn test_language_support() {
        let config = WorkspaceConfig::default();

        assert!(config.is_language_supported("rust"));
        assert!(config.is_language_supported("RUST")); // Case insensitive
        assert!(config.is_language_supported("python"));
        assert!(!config.is_language_supported("cobol"));
    }

    #[test]
    fn test_glob_matching() {
        assert!(glob_match("*.tmp", "test.tmp"));
        assert!(glob_match("*.tmp", "file.tmp"));
        assert!(!glob_match("*.tmp", "file.txt"));

        assert!(glob_match(".git/**", ".git/config"));
        assert!(glob_match(
            "node_modules/**",
            "node_modules/package/index.js"
        ));
        assert!(!glob_match("target/**", "src/main.rs"));
    }
}
