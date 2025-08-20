use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tracing::{debug, info, warn};

use crate::language_detector::Language;

/// Comprehensive configuration for the indexing subsystem
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingConfig {
    /// Master switch to enable/disable indexing entirely
    pub enabled: bool,

    /// Auto-index workspaces when they are initialized
    pub auto_index: bool,

    /// Enable file watching for incremental indexing
    pub watch_files: bool,

    /// Default indexing depth for nested projects
    pub default_depth: u32,

    /// Number of worker threads for indexing
    pub max_workers: usize,

    /// Memory budget in megabytes (0 = unlimited)
    pub memory_budget_mb: u64,

    /// Memory pressure threshold (0.0-1.0) to trigger backpressure
    pub memory_pressure_threshold: f64,

    /// Maximum queue size for pending files (0 = unlimited)
    pub max_queue_size: usize,

    /// Global file patterns to exclude from indexing
    pub global_exclude_patterns: Vec<String>,

    /// Global file patterns to include (empty = include all)
    pub global_include_patterns: Vec<String>,

    /// Maximum file size to index (bytes)
    pub max_file_size_bytes: u64,

    /// Whether to use incremental indexing based on file modification time
    pub incremental_mode: bool,

    /// Batch size for file discovery operations
    pub discovery_batch_size: usize,

    /// Interval between status updates (seconds)
    pub status_update_interval_secs: u64,

    /// Timeout for processing a single file (milliseconds)
    pub file_processing_timeout_ms: u64,

    /// Enable parallel processing within a single file
    pub parallel_file_processing: bool,

    /// Cache parsed results to disk
    pub persist_cache: bool,

    /// Directory for persistent cache storage
    pub cache_directory: Option<PathBuf>,

    /// Global indexing features configuration
    pub features: IndexingFeatures,

    /// Per-language configuration overrides
    pub language_configs: HashMap<Language, LanguageIndexConfig>,

    /// Priority languages to index first
    pub priority_languages: Vec<Language>,

    /// Languages to completely skip during indexing
    pub disabled_languages: Vec<Language>,
}

/// Enhanced indexing features with fine-grained control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingFeatures {
    /// Extract function and method signatures
    pub extract_functions: bool,

    /// Extract type definitions (classes, structs, interfaces)
    pub extract_types: bool,

    /// Extract variable and constant declarations
    pub extract_variables: bool,

    /// Extract import/export statements
    pub extract_imports: bool,

    /// Extract documentation comments and docstrings
    pub extract_docs: bool,

    /// Build call graph relationships (expensive)
    pub build_call_graph: bool,

    /// Extract string literals and constants
    pub extract_literals: bool,

    /// Analyze complexity metrics (cyclomatic complexity, etc.)
    pub analyze_complexity: bool,

    /// Extract test-related symbols and functions
    pub extract_tests: bool,

    /// Extract error handling patterns
    pub extract_error_handling: bool,

    /// Extract configuration and setup code
    pub extract_config: bool,

    /// Extract database/ORM related symbols
    pub extract_database: bool,

    /// Extract API/HTTP endpoint definitions
    pub extract_api_endpoints: bool,

    /// Extract security-related annotations and patterns
    pub extract_security: bool,

    /// Extract performance-critical sections
    pub extract_performance: bool,

    /// Language-specific feature flags
    pub language_features: HashMap<String, bool>,

    /// Custom feature flags for extensibility
    pub custom_features: HashMap<String, bool>,
}

/// Per-language indexing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageIndexConfig {
    /// Override global enabled flag for this language
    pub enabled: Option<bool>,

    /// Language-specific worker count override
    pub max_workers: Option<usize>,

    /// Language-specific memory budget override (MB)
    pub memory_budget_mb: Option<u64>,

    /// Language-specific file size limit override
    pub max_file_size_bytes: Option<u64>,

    /// Language-specific timeout override (ms)
    pub timeout_ms: Option<u64>,

    /// File extensions to process for this language
    pub file_extensions: Vec<String>,

    /// Language-specific exclude patterns
    pub exclude_patterns: Vec<String>,

    /// Language-specific include patterns
    pub include_patterns: Vec<String>,

    /// Features specific to this language
    pub features: Option<IndexingFeatures>,

    /// Custom parser configuration for this language
    pub parser_config: HashMap<String, serde_json::Value>,

    /// Priority level for this language (higher = processed first)
    pub priority: u32,

    /// Enable parallel processing for this language
    pub parallel_processing: Option<bool>,

    /// Cache strategy for this language
    pub cache_strategy: CacheStrategy,
}

/// Cache strategy for language-specific indexing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheStrategy {
    /// No caching
    None,
    /// Memory-only caching
    Memory,
    /// Disk-based caching
    Disk,
    /// Hybrid memory + disk caching
    Hybrid,
}

impl Default for IndexingConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Disabled by default for safety
            auto_index: false,
            watch_files: false,
            default_depth: 3,
            max_workers: num_cpus::get().min(8), // Reasonable default
            memory_budget_mb: 512,
            memory_pressure_threshold: 0.8,
            max_queue_size: 10000,
            global_exclude_patterns: vec![
                "*.git/*".to_string(),
                "*/node_modules/*".to_string(),
                "*/target/*".to_string(),
                "*/build/*".to_string(),
                "*/dist/*".to_string(),
                "*/.cargo/*".to_string(),
                "*/__pycache__/*".to_string(),
                "*.tmp".to_string(),
                "*.log".to_string(),
            ],
            global_include_patterns: vec![],
            max_file_size_bytes: 10 * 1024 * 1024, // 10MB
            incremental_mode: true,
            discovery_batch_size: 1000,
            status_update_interval_secs: 5,
            file_processing_timeout_ms: 30000, // 30 seconds
            parallel_file_processing: true,
            persist_cache: false,
            cache_directory: None,
            features: IndexingFeatures::default(),
            language_configs: HashMap::new(),
            priority_languages: vec![Language::Rust, Language::TypeScript, Language::Python],
            disabled_languages: vec![],
        }
    }
}

impl Default for IndexingFeatures {
    fn default() -> Self {
        Self {
            extract_functions: true,
            extract_types: true,
            extract_variables: true,
            extract_imports: true,
            extract_docs: true,
            build_call_graph: false,   // Expensive, off by default
            extract_literals: false,   // Can be noisy, off by default
            analyze_complexity: false, // CPU intensive, off by default
            extract_tests: true,
            extract_error_handling: false,
            extract_config: false,
            extract_database: false,
            extract_api_endpoints: false,
            extract_security: false,
            extract_performance: false,
            language_features: HashMap::new(),
            custom_features: HashMap::new(),
        }
    }
}

impl Default for LanguageIndexConfig {
    fn default() -> Self {
        Self {
            enabled: None,
            max_workers: None,
            memory_budget_mb: None,
            max_file_size_bytes: None,
            timeout_ms: None,
            file_extensions: vec![],
            exclude_patterns: vec![],
            include_patterns: vec![],
            features: None,
            parser_config: HashMap::new(),
            priority: 50, // Medium priority by default
            parallel_processing: None,
            cache_strategy: CacheStrategy::Memory,
        }
    }
}

impl IndexingFeatures {
    /// Create a minimal feature set for basic indexing
    pub fn minimal() -> Self {
        Self {
            extract_functions: true,
            extract_types: true,
            extract_variables: false,
            extract_imports: false,
            extract_docs: false,
            build_call_graph: false,
            extract_literals: false,
            analyze_complexity: false,
            extract_tests: false,
            extract_error_handling: false,
            extract_config: false,
            extract_database: false,
            extract_api_endpoints: false,
            extract_security: false,
            extract_performance: false,
            language_features: HashMap::new(),
            custom_features: HashMap::new(),
        }
    }

    /// Create a comprehensive feature set for full indexing
    pub fn comprehensive() -> Self {
        Self {
            extract_functions: true,
            extract_types: true,
            extract_variables: true,
            extract_imports: true,
            extract_docs: true,
            build_call_graph: true,
            extract_literals: true,
            analyze_complexity: true,
            extract_tests: true,
            extract_error_handling: true,
            extract_config: true,
            extract_database: true,
            extract_api_endpoints: true,
            extract_security: true,
            extract_performance: true,
            language_features: HashMap::new(),
            custom_features: HashMap::new(),
        }
    }

    /// Create a performance-focused feature set
    pub fn performance_focused() -> Self {
        Self {
            extract_functions: true,
            extract_types: true,
            extract_variables: false,
            extract_imports: true,
            extract_docs: false,
            build_call_graph: true, // Important for performance analysis
            extract_literals: false,
            analyze_complexity: true, // Important for performance
            extract_tests: false,
            extract_error_handling: true,
            extract_config: false,
            extract_database: true,
            extract_api_endpoints: true,
            extract_security: false,
            extract_performance: true,
            language_features: HashMap::new(),
            custom_features: HashMap::new(),
        }
    }

    /// Create a security-focused feature set
    pub fn security_focused() -> Self {
        Self {
            extract_functions: true,
            extract_types: true,
            extract_variables: true,
            extract_imports: true,
            extract_docs: true,
            build_call_graph: true, // Important for security analysis
            extract_literals: true, // Important for secrets detection
            analyze_complexity: false,
            extract_tests: false,
            extract_error_handling: true,
            extract_config: true, // Important for security misconfigurations
            extract_database: true,
            extract_api_endpoints: true,
            extract_security: true,
            extract_performance: false,
            language_features: HashMap::new(),
            custom_features: HashMap::new(),
        }
    }

    /// Enable/disable a language-specific feature
    pub fn set_language_feature(&mut self, feature_name: String, enabled: bool) {
        self.language_features.insert(feature_name, enabled);
    }

    /// Check if a language-specific feature is enabled
    pub fn is_language_feature_enabled(&self, feature_name: &str) -> bool {
        self.language_features
            .get(feature_name)
            .copied()
            .unwrap_or(false)
    }

    /// Enable/disable a custom feature
    pub fn set_custom_feature(&mut self, feature_name: String, enabled: bool) {
        self.custom_features.insert(feature_name, enabled);
    }

    /// Check if a custom feature is enabled
    pub fn is_custom_feature_enabled(&self, feature_name: &str) -> bool {
        self.custom_features
            .get(feature_name)
            .copied()
            .unwrap_or(false)
    }
}

impl IndexingConfig {
    /// Load configuration from environment variables
    pub fn from_env() -> Result<Self> {
        let mut config = Self::default();

        // Master switches
        if let Ok(value) = std::env::var("PROBE_INDEX_ENABLED") {
            config.enabled = parse_bool_env(&value, "PROBE_INDEX_ENABLED")?;
        }

        if let Ok(value) = std::env::var("PROBE_INDEX_AUTO") {
            config.auto_index = parse_bool_env(&value, "PROBE_INDEX_AUTO")?;
        }

        if let Ok(value) = std::env::var("PROBE_INDEX_WATCH") {
            config.watch_files = parse_bool_env(&value, "PROBE_INDEX_WATCH")?;
        }

        // Numeric configurations
        if let Ok(value) = std::env::var("PROBE_INDEX_DEPTH") {
            config.default_depth = value
                .parse()
                .context("Invalid value for PROBE_INDEX_DEPTH")?;
        }

        if let Ok(value) = std::env::var("PROBE_INDEX_WORKERS") {
            let workers: usize = value
                .parse()
                .context("Invalid value for PROBE_INDEX_WORKERS")?;
            if workers > 0 && workers <= 64 {
                config.max_workers = workers;
            } else {
                return Err(anyhow!(
                    "PROBE_INDEX_WORKERS must be between 1 and 64, got {}",
                    workers
                ));
            }
        }

        if let Ok(value) = std::env::var("PROBE_INDEX_MEMORY_MB") {
            config.memory_budget_mb = value
                .parse()
                .context("Invalid value for PROBE_INDEX_MEMORY_MB")?;
        }

        if let Ok(value) = std::env::var("PROBE_INDEX_MEMORY_THRESHOLD") {
            let threshold: f64 = value
                .parse()
                .context("Invalid value for PROBE_INDEX_MEMORY_THRESHOLD")?;
            if (0.0..=1.0).contains(&threshold) {
                config.memory_pressure_threshold = threshold;
            } else {
                return Err(anyhow!(
                    "PROBE_INDEX_MEMORY_THRESHOLD must be between 0.0 and 1.0, got {}",
                    threshold
                ));
            }
        }

        if let Ok(value) = std::env::var("PROBE_INDEX_QUEUE_SIZE") {
            config.max_queue_size = value
                .parse()
                .context("Invalid value for PROBE_INDEX_QUEUE_SIZE")?;
        }

        if let Ok(value) = std::env::var("PROBE_INDEX_FILE_SIZE_MB") {
            let size_mb: u64 = value
                .parse()
                .context("Invalid value for PROBE_INDEX_FILE_SIZE_MB")?;
            config.max_file_size_bytes = size_mb * 1024 * 1024;
        }

        if let Ok(value) = std::env::var("PROBE_INDEX_TIMEOUT_MS") {
            config.file_processing_timeout_ms = value
                .parse()
                .context("Invalid value for PROBE_INDEX_TIMEOUT_MS")?;
        }

        if let Ok(value) = std::env::var("PROBE_INDEX_BATCH_SIZE") {
            config.discovery_batch_size = value
                .parse()
                .context("Invalid value for PROBE_INDEX_BATCH_SIZE")?;
        }

        if let Ok(value) = std::env::var("PROBE_INDEX_STATUS_INTERVAL") {
            config.status_update_interval_secs = value
                .parse()
                .context("Invalid value for PROBE_INDEX_STATUS_INTERVAL")?;
        }

        // Boolean flags
        if let Ok(value) = std::env::var("PROBE_INDEX_INCREMENTAL") {
            config.incremental_mode = parse_bool_env(&value, "PROBE_INDEX_INCREMENTAL")?;
        }

        if let Ok(value) = std::env::var("PROBE_INDEX_PARALLEL") {
            config.parallel_file_processing = parse_bool_env(&value, "PROBE_INDEX_PARALLEL")?;
        }

        if let Ok(value) = std::env::var("PROBE_INDEX_PERSIST_CACHE") {
            config.persist_cache = parse_bool_env(&value, "PROBE_INDEX_PERSIST_CACHE")?;
        }

        // Patterns
        if let Ok(value) = std::env::var("PROBE_INDEX_EXCLUDE") {
            config.global_exclude_patterns = value
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }

        if let Ok(value) = std::env::var("PROBE_INDEX_INCLUDE") {
            config.global_include_patterns = value
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }

        // Cache directory
        if let Ok(value) = std::env::var("PROBE_INDEX_CACHE_DIR") {
            config.cache_directory = Some(PathBuf::from(value));
        }

        // Priority languages
        if let Ok(value) = std::env::var("PROBE_INDEX_PRIORITY_LANGS") {
            let languages: Result<Vec<Language>, _> =
                value.split(',').map(|s| s.trim().parse()).collect();
            config.priority_languages =
                languages.context("Invalid language in PROBE_INDEX_PRIORITY_LANGS")?;
        }

        // Disabled languages
        if let Ok(value) = std::env::var("PROBE_INDEX_DISABLED_LANGS") {
            let languages: Result<Vec<Language>, _> =
                value.split(',').map(|s| s.trim().parse()).collect();
            config.disabled_languages =
                languages.context("Invalid language in PROBE_INDEX_DISABLED_LANGS")?;
        }

        // Load feature configuration from environment
        config.features = IndexingFeatures::from_env()?;

        // Load per-language configurations
        config.language_configs = load_language_configs_from_env()?;

        Ok(config)
    }

    /// Load configuration from a TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .context(format!("Failed to read config file: {:?}", path.as_ref()))?;

        let config: Self =
            toml::from_str(&content).context("Failed to parse TOML configuration")?;

        config.validate()?;
        Ok(config)
    }

    /// Load configuration with priority: file -> env -> defaults
    pub fn load() -> Result<Self> {
        // Start with defaults
        let mut config = Self::default();

        // Try to load from standard config locations
        let config_paths = [
            std::env::var("PROBE_INDEX_CONFIG").ok().map(PathBuf::from),
            dirs::config_dir().map(|d| d.join("probe").join("indexing.toml")),
            dirs::home_dir().map(|d| d.join(".probe").join("indexing.toml")),
            Some(PathBuf::from("indexing.toml")),
        ];

        for config_path in config_paths.into_iter().flatten() {
            if config_path.exists() {
                info!("Loading indexing configuration from {:?}", config_path);
                config = Self::from_file(&config_path)
                    .with_context(|| format!("Failed to load config from {config_path:?}"))?;
                break;
            }
        }

        // Override with environment variables
        let env_config = Self::from_env()?;
        config.merge_with(env_config);

        // Final validation
        config.validate()?;

        Ok(config)
    }

    /// Merge configuration with another, giving priority to the other
    pub fn merge_with(&mut self, other: Self) {
        // Use macro to reduce boilerplate for optional fields
        macro_rules! merge_field {
            ($field:ident) => {
                if other.$field != Self::default().$field {
                    self.$field = other.$field;
                }
            };
        }

        merge_field!(enabled);
        merge_field!(auto_index);
        merge_field!(watch_files);
        merge_field!(default_depth);
        merge_field!(max_workers);
        merge_field!(memory_budget_mb);
        merge_field!(memory_pressure_threshold);
        merge_field!(max_queue_size);
        merge_field!(max_file_size_bytes);
        merge_field!(incremental_mode);
        merge_field!(discovery_batch_size);
        merge_field!(status_update_interval_secs);
        merge_field!(file_processing_timeout_ms);
        merge_field!(parallel_file_processing);
        merge_field!(persist_cache);

        if !other.global_exclude_patterns.is_empty() {
            self.global_exclude_patterns = other.global_exclude_patterns;
        }

        if !other.global_include_patterns.is_empty() {
            self.global_include_patterns = other.global_include_patterns;
        }

        if other.cache_directory.is_some() {
            self.cache_directory = other.cache_directory;
        }

        if !other.priority_languages.is_empty() {
            self.priority_languages = other.priority_languages;
        }

        if !other.disabled_languages.is_empty() {
            self.disabled_languages = other.disabled_languages;
        }

        // Merge features and language configs
        self.features.merge_with(other.features);
        for (lang, config) in other.language_configs {
            self.language_configs.insert(lang, config);
        }
    }

    /// Validate configuration for consistency and correctness
    pub fn validate(&self) -> Result<()> {
        // Check numeric constraints
        if self.max_workers == 0 {
            return Err(anyhow!("max_workers must be greater than 0"));
        }

        if self.max_workers > 64 {
            return Err(anyhow!(
                "max_workers should not exceed 64 for performance reasons"
            ));
        }

        if self.memory_pressure_threshold < 0.0 || self.memory_pressure_threshold > 1.0 {
            return Err(anyhow!(
                "memory_pressure_threshold must be between 0.0 and 1.0"
            ));
        }

        if self.default_depth == 0 {
            return Err(anyhow!("default_depth must be greater than 0"));
        }

        if self.file_processing_timeout_ms < 1000 {
            warn!(
                "file_processing_timeout_ms is very low ({}ms), may cause timeouts",
                self.file_processing_timeout_ms
            );
        }

        // Check cache directory if specified
        if let Some(ref cache_dir) = self.cache_directory {
            if self.persist_cache && !cache_dir.exists() {
                std::fs::create_dir_all(cache_dir)
                    .context(format!("Failed to create cache directory: {cache_dir:?}"))?;
            }
        }

        // Validate language configs
        for (language, config) in &self.language_configs {
            config.validate(language)?;
        }

        debug!("Configuration validation passed");
        Ok(())
    }

    /// Get effective configuration for a specific language
    pub fn for_language(&self, language: Language) -> EffectiveConfig {
        let language_config = self.language_configs.get(&language);

        EffectiveConfig {
            enabled: language_config
                .and_then(|c| c.enabled)
                .unwrap_or(self.enabled && !self.disabled_languages.contains(&language)),
            max_workers: language_config
                .and_then(|c| c.max_workers)
                .unwrap_or(self.max_workers),
            memory_budget_mb: language_config
                .and_then(|c| c.memory_budget_mb)
                .unwrap_or(self.memory_budget_mb),
            max_file_size_bytes: language_config
                .and_then(|c| c.max_file_size_bytes)
                .unwrap_or(self.max_file_size_bytes),
            timeout_ms: language_config
                .and_then(|c| c.timeout_ms)
                .unwrap_or(self.file_processing_timeout_ms),
            file_extensions: language_config
                .map(|c| c.file_extensions.clone())
                .unwrap_or_else(|| default_extensions_for_language(language)),
            exclude_patterns: {
                let mut patterns = self.global_exclude_patterns.clone();
                if let Some(lang_config) = language_config {
                    patterns.extend(lang_config.exclude_patterns.clone());
                }
                patterns
            },
            include_patterns: {
                let mut patterns = self.global_include_patterns.clone();
                if let Some(lang_config) = language_config {
                    patterns.extend(lang_config.include_patterns.clone());
                }
                patterns
            },
            features: language_config
                .and_then(|c| c.features.clone())
                .unwrap_or_else(|| self.features.clone()),
            parser_config: language_config
                .map(|c| c.parser_config.clone())
                .unwrap_or_default(),
            priority: language_config.map(|c| c.priority).unwrap_or_else(|| {
                if self.priority_languages.contains(&language) {
                    100
                } else {
                    50
                }
            }),
            parallel_processing: language_config
                .and_then(|c| c.parallel_processing)
                .unwrap_or(self.parallel_file_processing),
            cache_strategy: language_config.map(|c| c.cache_strategy.clone()).unwrap_or(
                if self.persist_cache {
                    CacheStrategy::Hybrid
                } else {
                    CacheStrategy::Memory
                },
            ),
        }
    }

    /// Convert to protocol IndexingConfig for API compatibility
    pub fn to_protocol_config(&self) -> crate::protocol::IndexingConfig {
        crate::protocol::IndexingConfig {
            max_workers: Some(self.max_workers),
            memory_budget_mb: Some(self.memory_budget_mb),
            exclude_patterns: self.global_exclude_patterns.clone(),
            include_patterns: self.global_include_patterns.clone(),
            max_file_size_mb: Some(self.max_file_size_bytes / 1024 / 1024),
            incremental: Some(self.incremental_mode),
            languages: self
                .priority_languages
                .iter()
                .map(|l| format!("{l:?}"))
                .collect(),
            recursive: true, // Always true in new config system
        }
    }

    /// Create from protocol IndexingConfig for API compatibility
    pub fn from_protocol_config(protocol: &crate::protocol::IndexingConfig) -> Self {
        let mut config = Self::default();

        if let Some(workers) = protocol.max_workers {
            config.max_workers = workers;
        }

        if let Some(memory) = protocol.memory_budget_mb {
            config.memory_budget_mb = memory;
        }

        if !protocol.exclude_patterns.is_empty() {
            config.global_exclude_patterns = protocol.exclude_patterns.clone();
        }

        if !protocol.include_patterns.is_empty() {
            config.global_include_patterns = protocol.include_patterns.clone();
        }

        if let Some(file_size) = protocol.max_file_size_mb {
            config.max_file_size_bytes = file_size * 1024 * 1024;
        }

        if let Some(incremental) = protocol.incremental {
            config.incremental_mode = incremental;
        }

        if !protocol.languages.is_empty() {
            config.priority_languages = protocol
                .languages
                .iter()
                .filter_map(|s| s.parse().ok())
                .collect();
        }

        config
    }
}

/// Effective configuration for a specific language after merging global and language-specific settings
#[derive(Debug, Clone)]
pub struct EffectiveConfig {
    pub enabled: bool,
    pub max_workers: usize,
    pub memory_budget_mb: u64,
    pub max_file_size_bytes: u64,
    pub timeout_ms: u64,
    pub file_extensions: Vec<String>,
    pub exclude_patterns: Vec<String>,
    pub include_patterns: Vec<String>,
    pub features: IndexingFeatures,
    pub parser_config: HashMap<String, serde_json::Value>,
    pub priority: u32,
    pub parallel_processing: bool,
    pub cache_strategy: CacheStrategy,
}

impl IndexingFeatures {
    /// Load feature configuration from environment variables
    pub fn from_env() -> Result<Self> {
        let mut features = Self::default();

        // Core features
        if let Ok(value) = std::env::var("PROBE_INDEX_EXTRACT_FUNCTIONS") {
            features.extract_functions = parse_bool_env(&value, "PROBE_INDEX_EXTRACT_FUNCTIONS")?;
        }

        if let Ok(value) = std::env::var("PROBE_INDEX_EXTRACT_TYPES") {
            features.extract_types = parse_bool_env(&value, "PROBE_INDEX_EXTRACT_TYPES")?;
        }

        if let Ok(value) = std::env::var("PROBE_INDEX_EXTRACT_VARIABLES") {
            features.extract_variables = parse_bool_env(&value, "PROBE_INDEX_EXTRACT_VARIABLES")?;
        }

        if let Ok(value) = std::env::var("PROBE_INDEX_EXTRACT_IMPORTS") {
            features.extract_imports = parse_bool_env(&value, "PROBE_INDEX_EXTRACT_IMPORTS")?;
        }

        if let Ok(value) = std::env::var("PROBE_INDEX_EXTRACT_DOCS") {
            features.extract_docs = parse_bool_env(&value, "PROBE_INDEX_EXTRACT_DOCS")?;
        }

        if let Ok(value) = std::env::var("PROBE_INDEX_BUILD_CALL_GRAPH") {
            features.build_call_graph = parse_bool_env(&value, "PROBE_INDEX_BUILD_CALL_GRAPH")?;
        }

        if let Ok(value) = std::env::var("PROBE_INDEX_EXTRACT_LITERALS") {
            features.extract_literals = parse_bool_env(&value, "PROBE_INDEX_EXTRACT_LITERALS")?;
        }

        if let Ok(value) = std::env::var("PROBE_INDEX_ANALYZE_COMPLEXITY") {
            features.analyze_complexity = parse_bool_env(&value, "PROBE_INDEX_ANALYZE_COMPLEXITY")?;
        }

        if let Ok(value) = std::env::var("PROBE_INDEX_EXTRACT_TESTS") {
            features.extract_tests = parse_bool_env(&value, "PROBE_INDEX_EXTRACT_TESTS")?;
        }

        // Extended features
        if let Ok(value) = std::env::var("PROBE_INDEX_EXTRACT_ERROR_HANDLING") {
            features.extract_error_handling =
                parse_bool_env(&value, "PROBE_INDEX_EXTRACT_ERROR_HANDLING")?;
        }

        if let Ok(value) = std::env::var("PROBE_INDEX_EXTRACT_CONFIG") {
            features.extract_config = parse_bool_env(&value, "PROBE_INDEX_EXTRACT_CONFIG")?;
        }

        if let Ok(value) = std::env::var("PROBE_INDEX_EXTRACT_DATABASE") {
            features.extract_database = parse_bool_env(&value, "PROBE_INDEX_EXTRACT_DATABASE")?;
        }

        if let Ok(value) = std::env::var("PROBE_INDEX_EXTRACT_API_ENDPOINTS") {
            features.extract_api_endpoints =
                parse_bool_env(&value, "PROBE_INDEX_EXTRACT_API_ENDPOINTS")?;
        }

        if let Ok(value) = std::env::var("PROBE_INDEX_EXTRACT_SECURITY") {
            features.extract_security = parse_bool_env(&value, "PROBE_INDEX_EXTRACT_SECURITY")?;
        }

        if let Ok(value) = std::env::var("PROBE_INDEX_EXTRACT_PERFORMANCE") {
            features.extract_performance =
                parse_bool_env(&value, "PROBE_INDEX_EXTRACT_PERFORMANCE")?;
        }

        // Load language-specific features using pattern matching
        for (key, value) in std::env::vars() {
            if let Some(feature_name) = key.strip_prefix("PROBE_INDEX_LANG_") {
                if let Some(suffix) = feature_name.strip_suffix("_PIPELINE") {
                    let enabled = parse_bool_env(&value, &key)?;
                    features.set_language_feature(suffix.to_lowercase(), enabled);
                }
            }

            if let Some(feature_name) = key.strip_prefix("PROBE_INDEX_CUSTOM_") {
                let enabled = parse_bool_env(&value, &key)?;
                features.set_custom_feature(feature_name.to_lowercase(), enabled);
            }
        }

        Ok(features)
    }

    /// Merge with another IndexingFeatures, giving priority to the other
    pub fn merge_with(&mut self, other: Self) {
        // Use macro to reduce boilerplate
        macro_rules! merge_bool_field {
            ($field:ident) => {
                if other.$field != Self::default().$field {
                    self.$field = other.$field;
                }
            };
        }

        merge_bool_field!(extract_functions);
        merge_bool_field!(extract_types);
        merge_bool_field!(extract_variables);
        merge_bool_field!(extract_imports);
        merge_bool_field!(extract_docs);
        merge_bool_field!(build_call_graph);
        merge_bool_field!(extract_literals);
        merge_bool_field!(analyze_complexity);
        merge_bool_field!(extract_tests);
        merge_bool_field!(extract_error_handling);
        merge_bool_field!(extract_config);
        merge_bool_field!(extract_database);
        merge_bool_field!(extract_api_endpoints);
        merge_bool_field!(extract_security);
        merge_bool_field!(extract_performance);

        // Merge maps
        for (key, value) in other.language_features {
            self.language_features.insert(key, value);
        }

        for (key, value) in other.custom_features {
            self.custom_features.insert(key, value);
        }
    }
}

impl LanguageIndexConfig {
    /// Validate language-specific configuration
    pub fn validate(&self, language: &Language) -> Result<()> {
        if let Some(workers) = self.max_workers {
            if workers == 0 || workers > 32 {
                return Err(anyhow!(
                    "max_workers for {:?} must be between 1 and 32",
                    language
                ));
            }
        }

        if let Some(timeout) = self.timeout_ms {
            if timeout < 1000 {
                warn!("timeout_ms for {:?} is very low ({}ms)", language, timeout);
            }
        }

        if self.priority > 255 {
            return Err(anyhow!("priority for {:?} must not exceed 255", language));
        }

        Ok(())
    }
}

/// Load per-language configurations from environment variables
fn load_language_configs_from_env() -> Result<HashMap<Language, LanguageIndexConfig>> {
    let mut configs = HashMap::new();

    // Load configurations for each supported language
    for language in [
        Language::Rust,
        Language::Python,
        Language::TypeScript,
        Language::JavaScript,
        Language::Go,
        Language::Java,
        Language::C,
        Language::Cpp,
    ] {
        let lang_str = format!("{language:?}").to_uppercase();
        let mut config = LanguageIndexConfig::default();
        let mut has_config = false;

        // Check for language-specific environment variables
        if let Ok(value) = std::env::var(format!("PROBE_INDEX_{lang_str}_ENABLED")) {
            config.enabled = Some(parse_bool_env(
                &value,
                &format!("PROBE_INDEX_{lang_str}_ENABLED"),
            )?);
            has_config = true;
        }

        if let Ok(value) = std::env::var(format!("PROBE_INDEX_{lang_str}_WORKERS")) {
            config.max_workers = Some(
                value
                    .parse()
                    .context(format!("Invalid value for PROBE_INDEX_{lang_str}_WORKERS"))?,
            );
            has_config = true;
        }

        if let Ok(value) = std::env::var(format!("PROBE_INDEX_{lang_str}_MEMORY_MB")) {
            config.memory_budget_mb = Some(value.parse().context(format!(
                "Invalid value for PROBE_INDEX_{lang_str}_MEMORY_MB"
            ))?);
            has_config = true;
        }

        if let Ok(value) = std::env::var(format!("PROBE_INDEX_{lang_str}_TIMEOUT_MS")) {
            config.timeout_ms = Some(value.parse().context(format!(
                "Invalid value for PROBE_INDEX_{lang_str}_TIMEOUT_MS"
            ))?);
            has_config = true;
        }

        if let Ok(value) = std::env::var(format!("PROBE_INDEX_{lang_str}_PRIORITY")) {
            config.priority = value
                .parse()
                .context(format!("Invalid value for PROBE_INDEX_{lang_str}_PRIORITY"))?;
            has_config = true;
        }

        if let Ok(value) = std::env::var(format!("PROBE_INDEX_{lang_str}_EXTENSIONS")) {
            config.file_extensions = value
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            has_config = true;
        }

        if let Ok(value) = std::env::var(format!("PROBE_INDEX_{lang_str}_EXCLUDE")) {
            config.exclude_patterns = value
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            has_config = true;
        }

        if let Ok(value) = std::env::var(format!("PROBE_INDEX_{lang_str}_PIPELINE")) {
            // Enable language-specific pipeline features
            let pipeline_enabled =
                parse_bool_env(&value, &format!("PROBE_INDEX_{lang_str}_PIPELINE"))?;
            if pipeline_enabled {
                let mut features = IndexingFeatures::default();

                // Enable language-specific features based on the language
                match language {
                    Language::Rust => {
                        features.set_language_feature("extract_macros".to_string(), true);
                        features.set_language_feature("extract_traits".to_string(), true);
                        features.set_language_feature("extract_lifetimes".to_string(), true);
                    }
                    Language::TypeScript | Language::JavaScript => {
                        features.set_language_feature("extract_interfaces".to_string(), true);
                        features.set_language_feature("extract_decorators".to_string(), true);
                        features.set_language_feature("extract_types".to_string(), true);
                    }
                    Language::Python => {
                        features.set_language_feature("extract_decorators".to_string(), true);
                        features.set_language_feature("extract_docstrings".to_string(), true);
                        features.set_language_feature("extract_async".to_string(), true);
                    }
                    Language::Go => {
                        features.set_language_feature("extract_interfaces".to_string(), true);
                        features.set_language_feature("extract_receivers".to_string(), true);
                        features.set_language_feature("extract_channels".to_string(), true);
                    }
                    Language::Java => {
                        features.set_language_feature("extract_annotations".to_string(), true);
                        features.set_language_feature("extract_generics".to_string(), true);
                    }
                    Language::C => {
                        features.set_language_feature("extract_preprocessor".to_string(), true);
                        features.set_language_feature("extract_headers".to_string(), true);
                    }
                    Language::Cpp => {
                        features.set_language_feature("extract_templates".to_string(), true);
                        features.set_language_feature("extract_namespaces".to_string(), true);
                        features.set_language_feature("extract_classes".to_string(), true);
                    }
                    _ => {}
                }

                config.features = Some(features);
                has_config = true;
            }
        }

        if has_config {
            configs.insert(language, config);
        }
    }

    Ok(configs)
}

/// Parse boolean environment variable with proper error handling
fn parse_bool_env(value: &str, var_name: &str) -> Result<bool> {
    match value.to_lowercase().as_str() {
        "true" | "1" | "yes" | "on" | "enabled" => Ok(true),
        "false" | "0" | "no" | "off" | "disabled" => Ok(false),
        _ => Err(anyhow!("Invalid boolean value for {}: {} (use true/false, 1/0, yes/no, on/off, enabled/disabled)", var_name, value)),
    }
}

/// Get default file extensions for a language
fn default_extensions_for_language(language: Language) -> Vec<String> {
    match language {
        Language::Rust => vec!["rs".to_string()],
        Language::Python => vec!["py".to_string(), "pyi".to_string()],
        Language::TypeScript => vec!["ts".to_string(), "tsx".to_string()],
        Language::JavaScript => vec!["js".to_string(), "jsx".to_string(), "mjs".to_string()],
        Language::Go => vec!["go".to_string()],
        Language::Java => vec!["java".to_string()],
        Language::C => vec!["c".to_string(), "h".to_string()],
        Language::Cpp => vec![
            "cpp".to_string(),
            "cc".to_string(),
            "cxx".to_string(),
            "hpp".to_string(),
            "hxx".to_string(),
        ],
        _ => vec![],
    }
}

impl FromStr for Language {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "rust" => Ok(Language::Rust),
            "python" => Ok(Language::Python),
            "typescript" => Ok(Language::TypeScript),
            "javascript" => Ok(Language::JavaScript),
            "go" => Ok(Language::Go),
            "java" => Ok(Language::Java),
            "c" => Ok(Language::C),
            "cpp" | "c++" => Ok(Language::Cpp),
            _ => Err(anyhow!("Unknown language: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = IndexingConfig::default();
        assert!(!config.enabled); // Should be disabled by default
        assert!(!config.auto_index);
        assert!(!config.watch_files);
        assert_eq!(config.default_depth, 3);
        assert!(config.max_workers > 0);
        assert!(config.memory_budget_mb > 0);
    }

    #[test]
    fn test_features_presets() {
        let minimal = IndexingFeatures::minimal();
        assert!(minimal.extract_functions);
        assert!(minimal.extract_types);
        assert!(!minimal.extract_variables);
        assert!(!minimal.build_call_graph);

        let comprehensive = IndexingFeatures::comprehensive();
        assert!(comprehensive.extract_functions);
        assert!(comprehensive.extract_types);
        assert!(comprehensive.extract_variables);
        assert!(comprehensive.build_call_graph);
        assert!(comprehensive.analyze_complexity);

        let security = IndexingFeatures::security_focused();
        assert!(security.extract_security);
        assert!(security.extract_literals); // Important for secrets
        assert!(security.extract_config);
        assert!(!security.extract_performance);

        let performance = IndexingFeatures::performance_focused();
        assert!(performance.extract_performance);
        assert!(performance.analyze_complexity);
        assert!(!performance.extract_security);
    }

    #[test]
    fn test_env_var_parsing() {
        // Test boolean parsing
        assert!(parse_bool_env("true", "TEST").unwrap());
        assert!(parse_bool_env("1", "TEST").unwrap());
        assert!(parse_bool_env("yes", "TEST").unwrap());
        assert!(parse_bool_env("on", "TEST").unwrap());
        assert!(parse_bool_env("enabled", "TEST").unwrap());

        assert!(!parse_bool_env("false", "TEST").unwrap());
        assert!(!parse_bool_env("0", "TEST").unwrap());
        assert!(!parse_bool_env("no", "TEST").unwrap());
        assert!(!parse_bool_env("off", "TEST").unwrap());
        assert!(!parse_bool_env("disabled", "TEST").unwrap());

        assert!(parse_bool_env("invalid", "TEST").is_err());
    }

    #[test]
    fn test_language_config_validation() {
        let mut config = LanguageIndexConfig::default();

        // Valid config should pass
        assert!(config.validate(&Language::Rust).is_ok());

        // Invalid worker count
        config.max_workers = Some(0);
        assert!(config.validate(&Language::Rust).is_err());

        config.max_workers = Some(16); // This should be ok (within 1-32 range)
        assert!(config.validate(&Language::Rust).is_ok());

        // Invalid priority
        config.priority = 300;
        assert!(config.validate(&Language::Rust).is_err());
    }

    #[test]
    fn test_effective_config() {
        let mut base_config = IndexingConfig::default();
        base_config.enabled = true;
        base_config.max_workers = 4;

        // Test language without specific config
        let effective = base_config.for_language(Language::Rust);
        assert!(effective.enabled);
        assert_eq!(effective.max_workers, 4);

        // Test language with specific config
        let mut rust_config = LanguageIndexConfig::default();
        rust_config.max_workers = Some(8);
        rust_config.enabled = Some(false);

        base_config
            .language_configs
            .insert(Language::Rust, rust_config);

        let effective = base_config.for_language(Language::Rust);
        assert!(!effective.enabled); // Language-specific override
        assert_eq!(effective.max_workers, 8); // Language-specific override
    }

    #[test]
    fn test_config_merge() {
        let mut base = IndexingConfig::default();
        base.enabled = false;
        base.max_workers = 2;

        let mut override_config = IndexingConfig::default();
        override_config.enabled = true;
        override_config.memory_budget_mb = 1024;

        base.merge_with(override_config);

        assert!(base.enabled); // Should be overridden
        assert_eq!(base.memory_budget_mb, 1024); // Should be overridden
        assert_eq!(base.max_workers, 2); // Should remain from base (if override was default)
    }

    #[test]
    fn test_default_extensions() {
        assert_eq!(default_extensions_for_language(Language::Rust), vec!["rs"]);
        assert_eq!(
            default_extensions_for_language(Language::Python),
            vec!["py", "pyi"]
        );
        assert_eq!(
            default_extensions_for_language(Language::TypeScript),
            vec!["ts", "tsx"]
        );
        assert!(default_extensions_for_language(Language::Unknown).is_empty());
    }

    #[test]
    fn test_language_from_str() {
        assert_eq!("rust".parse::<Language>().unwrap(), Language::Rust);
        assert_eq!("python".parse::<Language>().unwrap(), Language::Python);
        assert_eq!(
            "typescript".parse::<Language>().unwrap(),
            Language::TypeScript
        );
        assert_eq!("cpp".parse::<Language>().unwrap(), Language::Cpp);
        assert_eq!("c++".parse::<Language>().unwrap(), Language::Cpp);
        assert!("unknown".parse::<Language>().is_err());
    }

    #[test]
    fn test_comprehensive_config_creation() {
        let config = IndexingConfig::load().unwrap();

        // Test that it creates a valid configuration
        assert!(config.validate().is_ok());

        // Test effective config for different languages
        let rust_config = config.for_language(Language::Rust);
        assert_eq!(rust_config.file_extensions, vec!["rs"]);
        assert!(rust_config.features.extract_functions);

        let python_config = config.for_language(Language::Python);
        assert_eq!(python_config.file_extensions, vec!["py", "pyi"]);
        assert!(python_config.features.extract_functions);
    }

    #[test]
    fn test_feature_flag_inheritance() {
        let mut config = IndexingConfig::default();

        // Set global features
        config.features.extract_security = true;
        config.features.extract_performance = true;

        // Create language-specific config
        let mut rust_config = LanguageIndexConfig::default();
        let mut rust_features = IndexingFeatures::default();
        rust_features.extract_docs = false; // Override global
        rust_config.features = Some(rust_features);

        config.language_configs.insert(Language::Rust, rust_config);

        // Test effective configuration
        let effective = config.for_language(Language::Rust);
        assert!(!effective.features.extract_docs); // Should be overridden
        assert!(effective.features.extract_functions); // Should come from language default
    }

    #[test]
    fn test_environment_variable_patterns() {
        // Test that environment variable names follow expected patterns
        let config = IndexingConfig::default();

        // Test protocol conversion
        let protocol_config = config.to_protocol_config();
        assert_eq!(protocol_config.max_workers, Some(config.max_workers));
        assert_eq!(
            protocol_config.memory_budget_mb,
            Some(config.memory_budget_mb)
        );

        // Test round-trip conversion
        let restored_config = IndexingConfig::from_protocol_config(&protocol_config);
        assert_eq!(restored_config.max_workers, config.max_workers);
        assert_eq!(restored_config.memory_budget_mb, config.memory_budget_mb);
    }

    #[test]
    fn test_cache_strategy_defaults() {
        let config = LanguageIndexConfig::default();
        match config.cache_strategy {
            CacheStrategy::Memory => {} // Expected default
            _ => panic!("Expected Memory cache strategy as default"),
        }

        // Test that hybrid strategy works with persistence
        let mut indexing_config = IndexingConfig::default();
        indexing_config.persist_cache = true;

        let effective = indexing_config.for_language(Language::Rust);
        match effective.cache_strategy {
            CacheStrategy::Hybrid => {}
            _ => panic!("Expected Hybrid cache strategy when persistence is enabled"),
        }
    }

    #[test]
    fn test_disabled_languages() {
        let mut config = IndexingConfig::default();
        config.enabled = true;
        config.disabled_languages = vec![Language::C, Language::Cpp];

        let c_effective = config.for_language(Language::C);
        let rust_effective = config.for_language(Language::Rust);

        assert!(!c_effective.enabled); // Should be disabled
        assert!(rust_effective.enabled); // Should be enabled
    }

    #[test]
    fn test_priority_languages() {
        let mut config = IndexingConfig::default();
        config.priority_languages = vec![Language::Rust, Language::Python];

        let rust_effective = config.for_language(Language::Rust);
        let go_effective = config.for_language(Language::Go);

        assert_eq!(rust_effective.priority, 100); // Priority language
        assert_eq!(go_effective.priority, 50); // Default priority
    }
}
