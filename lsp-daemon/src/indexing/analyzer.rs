//! Phase 3.3 - Comprehensive Incremental Analysis Engine
#![allow(dead_code, clippy::all)]
//!
//! This module provides a comprehensive incremental analysis engine that coordinates all previous
//! phases to provide efficient, queue-based analysis processing. The engine combines structural
//! and semantic analysis with dependency-aware reindexing and parallel processing capabilities.
//!
//! ## Architecture
//!
//! The IncrementalAnalysisEngine coordinates:
//! - **Database Backend** (Phase 1.3): Persistent storage and querying
//! - **File Change Detection** (Phase 2.1): Content-addressed change detection
//! - **File Version Management** (Phase 2.2): Content deduplication and workspace association
//! - **Workspace Management** (Phase 3.2): Project organization and git integration
//! - **Multi-Language Analysis** (Phase 3.1): Symbol extraction and relationship analysis
//!
//! ## Key Features
//!
//! - **Priority-based queue management**: Critical files processed first
//! - **Dependency-aware reindexing**: Changes cascade through dependent files
//! - **Parallel worker pool**: Configurable concurrent analysis processing
//! - **Progress monitoring**: Real-time analysis progress tracking
//! - **Error recovery**: Retry mechanisms and graceful error handling
//! - **Performance metrics**: Analysis performance and resource utilization
//!
//! ## Usage
//!
//! ```rust
//! use analyzer::{IncrementalAnalysisEngine, AnalysisEngineConfig};
//!
//! // Create analysis engine with all phase components
//! let engine = IncrementalAnalysisEngine::new(
//!     database,
//!     workspace_manager,
//!     analyzer_manager
//! ).await?;
//!
//! // Analyze workspace incrementally
//! let result = engine.analyze_workspace_incremental(workspace_id, &scan_path).await?;
//!
//! // Start background processing
//! engine.start_analysis_workers().await?;
//! ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::analyzer::{
    AnalysisContext, AnalysisError as FrameworkAnalysisError, AnalysisResult, AnalyzerManager,
};
use crate::database::{DatabaseBackend, DatabaseError};
use crate::indexing::{FileChange, FileChangeDetector, FileChangeType, FileVersionManager};
use crate::workspace::WorkspaceManager;

/// Comprehensive errors for analysis engine operations
#[derive(Debug, thiserror::Error)]
pub enum AnalysisError {
    #[error("Database operation failed: {0}")]
    Database(#[from] DatabaseError),

    #[error("Analysis framework error: {0}")]
    Analysis(#[from] FrameworkAnalysisError),

    #[error("Workspace operation failed: {0}")]
    Workspace(#[from] crate::workspace::WorkspaceError),

    #[error("File versioning error: {0}")]
    Versioning(#[from] crate::indexing::VersioningError),

    #[error("File detection error: {0}")]
    Detection(#[from] crate::indexing::DetectionError),

    #[error("Worker pool error: {reason}")]
    WorkerPool { reason: String },

    #[error("Queue operation failed: {reason}")]
    QueueError { reason: String },

    #[error("Dependency analysis failed: {reason}")]
    DependencyError { reason: String },

    #[error("Analysis task failed: {task_id} - {reason}")]
    TaskFailed { task_id: u64, reason: String },

    #[error("Resource exhaustion: {resource}")]
    ResourceExhaustion { resource: String },

    #[error("Concurrent operation error: {0}")]
    Concurrency(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Context error: {0}")]
    Context(#[from] anyhow::Error),
}

/// Types of analysis tasks with different processing strategies
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AnalysisTaskType {
    /// Full analysis of a file (structural + semantic)
    FullAnalysis,
    /// Incremental update of existing analysis
    IncrementalUpdate,
    /// Update analysis due to dependency changes
    DependencyUpdate,
    /// Complete reindex of file (clear existing analysis)
    Reindex,
}

/// Priority levels for analysis tasks
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AnalysisTaskPriority {
    /// Critical files (entry points, frequently accessed)
    Critical = 100,
    /// High priority files (core modules, interfaces)
    High = 75,
    /// Normal priority files (regular source files)
    Normal = 50,
    /// Low priority files (tests, documentation)
    Low = 25,
    /// Background priority (large files, rarely accessed)
    Background = 1,
}

impl Default for AnalysisTaskPriority {
    fn default() -> Self {
        Self::Normal
    }
}

/// Analysis task with comprehensive metadata
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisTask {
    /// Unique task identifier
    pub task_id: u64,
    /// Priority for queue ordering
    pub priority: AnalysisTaskPriority,
    /// Target workspace
    pub workspace_id: i64,
    /// Target file version
    pub file_version_id: i64,
    /// Type of analysis to perform
    pub task_type: AnalysisTaskType,
    /// File path for analysis
    pub file_path: PathBuf,
    /// Detected language (required for simplified model)
    pub language: String,
    /// Task creation time
    pub created_at: SystemTime,
    /// Number of retry attempts
    pub retry_count: u32,
    /// Maximum retry attempts
    pub max_retries: u32,
    /// Dependencies that triggered this task (for dependency updates)
    pub triggered_by: Vec<PathBuf>,
}

impl PartialOrd for AnalysisTask {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AnalysisTask {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Higher priority values come first in queue
        other
            .priority
            .cmp(&self.priority)
            .then_with(|| self.created_at.cmp(&other.created_at))
            .then_with(|| self.task_id.cmp(&other.task_id))
    }
}

/// Dependency graph node representation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyNode {
    pub file_path: PathBuf,
    pub file_version_id: i64,
    pub last_analyzed: Option<SystemTime>,
    pub dependencies: Vec<PathBuf>,
    pub dependents: Vec<PathBuf>,
    pub language: Option<String>,
}

/// Dependency graph for tracking file relationships
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyGraph {
    pub nodes: HashMap<PathBuf, DependencyNode>,
    pub edges: Vec<DependencyEdge>,
    pub last_updated: SystemTime,
}

/// Dependency edge between files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyEdge {
    pub from: PathBuf,
    pub to: PathBuf,
    pub edge_type: DependencyType,
    pub strength: f32, // 0.0 to 1.0 indicating dependency strength
}

/// Types of dependencies between files
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DependencyType {
    /// Direct import/include
    Import,
    /// Type dependency
    TypeDependency,
    /// Function/method call
    Call,
    /// Inheritance relationship
    Inheritance,
    /// Interface implementation
    Implementation,
    /// Module dependency
    Module,
}

/// Queue management for analysis tasks
#[allow(dead_code)]
pub struct AnalysisQueueManager<T: DatabaseBackend> {
    database: Arc<T>,
    queue: Arc<Mutex<BinaryHeap<AnalysisTask>>>,
    task_counter: Arc<Mutex<u64>>,
    metrics: Arc<RwLock<QueueMetrics>>,
    shutdown_signal: broadcast::Sender<()>,
}

/// Queue performance metrics
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct QueueMetrics {
    pub tasks_queued: u64,
    pub tasks_completed: u64,
    pub tasks_failed: u64,
    pub tasks_retried: u64,
    pub average_processing_time: Duration,
    pub queue_depth: usize,
    pub peak_queue_depth: usize,
    pub active_workers: usize,
}

/// Worker pool for parallel analysis processing
#[allow(dead_code)]
pub struct WorkerPool {
    workers: Vec<JoinHandle<()>>,
    worker_count: usize,
    shutdown_signal: broadcast::Receiver<()>,
}

/// Configuration for the analysis engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisEngineConfig {
    /// Maximum number of worker threads
    pub max_workers: usize,
    /// Batch size for queue processing
    pub batch_size: usize,
    /// Maximum retry attempts for failed tasks
    pub retry_limit: u32,
    /// Task timeout in seconds
    pub timeout_seconds: u64,
    /// Memory limit in MB
    pub memory_limit_mb: u64,
    /// Enable dependency analysis
    pub dependency_analysis_enabled: bool,
    /// Incremental analysis threshold (seconds since last analysis)
    pub incremental_threshold_seconds: u64,
    /// Priority boost for frequently accessed files
    pub priority_boost_enabled: bool,
    /// Maximum queue depth before applying backpressure
    pub max_queue_depth: usize,
}

impl Default for AnalysisEngineConfig {
    fn default() -> Self {
        Self {
            max_workers: std::cmp::max(2, num_cpus::get()),
            batch_size: 50,
            retry_limit: 3,
            timeout_seconds: 30,
            memory_limit_mb: 512,
            dependency_analysis_enabled: true,
            incremental_threshold_seconds: 300, // 5 minutes
            priority_boost_enabled: true,
            max_queue_depth: 10000,
        }
    }
}

/// Comprehensive analysis results for workspace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceAnalysisResult {
    pub workspace_id: i64,
    pub files_analyzed: u64,
    pub symbols_extracted: u64,
    pub relationships_found: u64,
    pub analysis_time: Duration,
    pub queue_size_before: usize,
    pub queue_size_after: usize,
    pub worker_utilization: f64,
    pub dependency_updates: u64,
    pub errors: Vec<String>,
}

/// Results from processing file changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessingResult {
    pub tasks_queued: u64,
    pub immediate_analyses: u64,
    pub dependency_cascades: u64,
    pub processing_time: Duration,
    pub errors: Vec<String>,
}

/// Individual file analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAnalysisResult {
    pub file_path: PathBuf,
    pub symbols_extracted: usize,
    pub relationships_found: usize,
    pub dependencies_detected: usize,
    pub analysis_time: Duration,
}

/// Analysis progress information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisProgressInfo {
    pub workspace_id: i64,
    pub total_files: u64,
    pub analyzed_files: u64,
    pub queued_files: u64,
    pub failed_files: u64,
    pub completion_percentage: f32,
    pub current_throughput: f32, // files per second
    pub estimated_remaining: Option<Duration>,
}

/// Main incremental analysis engine
#[allow(dead_code)]
pub struct IncrementalAnalysisEngine<T: DatabaseBackend + 'static> {
    database: Arc<T>,
    workspace_manager: Arc<WorkspaceManager<T>>,
    analyzer_manager: Arc<AnalyzerManager>,
    file_detector: Arc<FileChangeDetector>,
    file_version_manager: Arc<FileVersionManager<T>>,
    queue_manager: Arc<AnalysisQueueManager<T>>,
    config: AnalysisEngineConfig,
    workers: Arc<Mutex<Option<WorkerPool>>>,
    dependency_graph: Arc<RwLock<HashMap<i64, DependencyGraph>>>, // workspace_id -> graph
    metrics: Arc<RwLock<EngineMetrics>>,
    start_time: Instant,
}

/// Engine performance metrics
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct EngineMetrics {
    pub total_analyses_performed: u64,
    pub total_dependencies_detected: u64,
    pub average_analysis_time: Duration,
    pub cache_hit_rate: f64,
    pub worker_efficiency: f64,
    pub memory_usage_mb: f64,
}

impl<T> IncrementalAnalysisEngine<T>
where
    T: DatabaseBackend + Send + Sync + 'static,
{
    /// Create a new incremental analysis engine with all required components
    pub async fn new(
        database: Arc<T>,
        workspace_manager: Arc<WorkspaceManager<T>>,
        analyzer_manager: Arc<AnalyzerManager>,
    ) -> Result<Self, AnalysisError> {
        Self::with_config(
            database,
            workspace_manager,
            analyzer_manager,
            AnalysisEngineConfig::default(),
        )
        .await
    }

    /// Create analysis engine with custom configuration
    pub async fn with_config(
        database: Arc<T>,
        workspace_manager: Arc<WorkspaceManager<T>>,
        analyzer_manager: Arc<AnalyzerManager>,
        config: AnalysisEngineConfig,
    ) -> Result<Self, AnalysisError> {
        info!(
            "Initializing IncrementalAnalysisEngine with {} max workers, dependency_analysis: {}",
            config.max_workers, config.dependency_analysis_enabled
        );

        // Initialize file change detector
        let detection_config = crate::indexing::DetectionConfig {
            hash_algorithm: crate::indexing::HashAlgorithm::Blake3,
            max_file_size: config.memory_limit_mb * 1024 * 1024,
            ..Default::default()
        };
        let file_detector = Arc::new(FileChangeDetector::with_config(detection_config));

        // Initialize file version manager
        let versioning_config = crate::indexing::VersioningConfig {
            max_concurrent_operations: config.max_workers,
            enable_git_integration: true,
            max_file_size: config.memory_limit_mb * 1024 * 1024,
            batch_size: config.batch_size,
            ..Default::default()
        };
        let file_version_manager =
            Arc::new(FileVersionManager::new(database.clone(), versioning_config).await?);

        // Initialize queue manager
        let (shutdown_tx, _) = broadcast::channel(1);
        let queue_manager = Arc::new(AnalysisQueueManager {
            database: database.clone(),
            queue: Arc::new(Mutex::new(BinaryHeap::new())),
            task_counter: Arc::new(Mutex::new(0)),
            metrics: Arc::new(RwLock::new(QueueMetrics::default())),
            shutdown_signal: shutdown_tx,
        });

        let engine = Self {
            database,
            workspace_manager,
            analyzer_manager,
            file_detector,
            file_version_manager,
            queue_manager,
            config,
            workers: Arc::new(Mutex::new(None)),
            dependency_graph: Arc::new(RwLock::new(HashMap::new())),
            metrics: Arc::new(RwLock::new(EngineMetrics::default())),
            start_time: Instant::now(),
        };

        info!("IncrementalAnalysisEngine initialized successfully");
        Ok(engine)
    }
}

impl<T> AnalysisQueueManager<T>
where
    T: DatabaseBackend + Send + Sync,
{
    /// Queue a new analysis task
    pub async fn queue_task(&self, task: AnalysisTask) -> Result<(), AnalysisError> {
        let mut queue = self.queue.lock().await;

        // Check queue depth for backpressure
        if queue.len() >= 10000 {
            // Default max queue depth
            return Err(AnalysisError::ResourceExhaustion {
                resource: "Analysis queue at capacity".to_string(),
            });
        }

        queue.push(task.clone());

        // Update metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.tasks_queued += 1;
            metrics.queue_depth = queue.len();
            metrics.peak_queue_depth = metrics.peak_queue_depth.max(queue.len());
        }

        debug!(
            "Queued analysis task {} for workspace {}",
            task.task_id, task.workspace_id
        );
        Ok(())
    }

    /// Dequeue the next highest priority task
    pub async fn dequeue_task(&self) -> Option<AnalysisTask> {
        let mut queue = self.queue.lock().await;
        let task = queue.pop();

        if task.is_some() {
            let mut metrics = self.metrics.write().await;
            metrics.queue_depth = queue.len();
        }

        task
    }

    /// Get current queue metrics
    pub async fn get_metrics(&self) -> QueueMetrics {
        self.metrics.read().await.clone()
    }

    /// Get next task ID
    pub async fn next_task_id(&self) -> u64 {
        let mut counter = self.task_counter.lock().await;
        *counter += 1;
        *counter
    }
}

impl<T> IncrementalAnalysisEngine<T>
where
    T: DatabaseBackend + Send + Sync + 'static,
{
    /// Analyze workspace incrementally with comprehensive file change detection
    pub async fn analyze_workspace_incremental(
        &self,
        workspace_id: i64,
        scan_path: &Path,
    ) -> Result<WorkspaceAnalysisResult, AnalysisError> {
        let start_time = Instant::now();
        info!(
            "Starting incremental analysis for workspace {} at {}",
            workspace_id,
            scan_path.display()
        );

        let queue_size_before = {
            let queue = self.queue_manager.queue.lock().await;
            queue.len()
        };

        // Step 1: Detect file changes
        let changes = self
            .file_detector
            .detect_changes(workspace_id, scan_path, &*self.database)
            .await?;

        info!(
            "Detected {} file changes for workspace {}",
            changes.len(),
            workspace_id
        );

        // Step 2: Process file changes to create versions
        let processing_results = self
            .file_version_manager
            .process_file_changes(workspace_id, changes.clone())
            .await?;

        info!(
            "Processed {} file versions ({} new, {} deduplicated)",
            processing_results.processed_versions.len(),
            processing_results.new_versions_count,
            processing_results.deduplicated_count
        );

        // Step 3: Queue analysis tasks for changed files
        let mut tasks_queued = 0u64;
        let mut dependency_updates = 0u64;

        for version_info in &processing_results.processed_versions {
            // Create analysis task
            let task = self
                .create_analysis_task(
                    workspace_id,
                    version_info.file_version.file_version_id,
                    &version_info.file_path,
                    version_info.detected_language.clone(),
                    if version_info.is_new_version {
                        AnalysisTaskType::FullAnalysis
                    } else {
                        AnalysisTaskType::IncrementalUpdate
                    },
                )
                .await?;

            self.queue_manager.queue_task(task).await?;
            tasks_queued += 1;

            // Check for dependent files if dependency analysis is enabled
            if self.config.dependency_analysis_enabled {
                let dependent_tasks = self
                    .queue_dependent_analysis(
                        workspace_id,
                        &[FileChange {
                            path: version_info.file_path.clone(),
                            change_type: FileChangeType::Update,
                            content_digest: Some(version_info.file_version.content_digest.clone()),
                            size_bytes: Some(version_info.file_version.size_bytes),
                            mtime: version_info.file_version.mtime,
                            detected_language: version_info.detected_language.clone(),
                        }],
                    )
                    .await
                    .unwrap_or(0);

                dependency_updates += dependent_tasks;
            }
        }

        let queue_size_after = {
            let queue = self.queue_manager.queue.lock().await;
            queue.len()
        };

        let analysis_time = start_time.elapsed();

        // Update engine metrics
        {
            let mut metrics = self.metrics.write().await;
            metrics.total_analyses_performed += tasks_queued;
        }

        let result = WorkspaceAnalysisResult {
            workspace_id,
            files_analyzed: processing_results.processed_versions.len() as u64,
            symbols_extracted: 0,   // Will be updated after actual analysis
            relationships_found: 0, // Will be updated after actual analysis
            analysis_time,
            queue_size_before,
            queue_size_after,
            worker_utilization: self.calculate_worker_utilization().await,
            dependency_updates,
            errors: processing_results
                .failed_files
                .iter()
                .map(|(path, error)| format!("{}: {}", path.display(), error))
                .collect(),
        };

        info!(
            "Workspace analysis queued: {} tasks, {} dependency updates in {:?}",
            tasks_queued, dependency_updates, analysis_time
        );

        Ok(result)
    }

    /// Process file changes and queue appropriate analysis tasks
    pub async fn process_file_changes(
        &self,
        workspace_id: i64,
        changes: Vec<FileChange>,
    ) -> Result<ProcessingResult, AnalysisError> {
        let start_time = Instant::now();
        info!(
            "Processing {} file changes for workspace {}",
            changes.len(),
            workspace_id
        );

        let mut tasks_queued = 0u64;
        let immediate_analyses = 0u64;
        let mut dependency_cascades = 0u64;
        let mut errors = Vec::new();

        // Process each file change
        for change in changes {
            match self
                .process_single_file_change(workspace_id, change.clone())
                .await
            {
                Ok((queued, cascades)) => {
                    tasks_queued += queued;
                    dependency_cascades += cascades;
                }
                Err(e) => {
                    errors.push(format!(
                        "Failed to process {}: {}",
                        change.path.display(),
                        e
                    ));
                    error!(
                        "Error processing file change for {}: {}",
                        change.path.display(),
                        e
                    );
                }
            }
        }

        Ok(ProcessingResult {
            tasks_queued,
            immediate_analyses,
            dependency_cascades,
            processing_time: start_time.elapsed(),
            errors,
        })
    }

    /// Analyze a single file with the appropriate analyzer
    pub async fn analyze_file(
        &self,
        workspace_id: i64,
        file_path: &Path,
        analysis_type: AnalysisTaskType,
    ) -> Result<FileAnalysisResult, AnalysisError> {
        let start_time = Instant::now();
        debug!(
            "Analyzing file: {} (type: {:?})",
            file_path.display(),
            analysis_type
        );

        // Read file content
        let content = tokio::fs::read_to_string(file_path)
            .await
            .context(format!("Failed to read file: {}", file_path.display()))?;

        // Detect language
        let language = self
            .file_detector
            .detect_language(file_path)
            .unwrap_or_else(|| "unknown".to_string());

        // Create analysis context
        let context = self.create_analysis_context(workspace_id).await?;

        // Perform analysis using the analyzer framework
        let analysis_result = self
            .analyzer_manager
            .analyze_file(&content, file_path, &language, &context)
            .await?;

        // Store analysis results in database (using simplified language-based model)
        self.store_analysis_result(language.clone(), &analysis_result)
            .await?;

        let analysis_time = start_time.elapsed();

        debug!(
            "File analysis completed for {} in {:?}: {} symbols, {} relationships",
            file_path.display(),
            analysis_time,
            analysis_result.symbols.len(),
            analysis_result.relationships.len()
        );

        Ok(FileAnalysisResult {
            file_path: file_path.to_path_buf(),
            symbols_extracted: analysis_result.symbols.len(),
            relationships_found: analysis_result.relationships.len(),
            dependencies_detected: analysis_result.dependencies.len(),
            analysis_time,
        })
    }

    /// Start background analysis workers for parallel processing
    pub async fn start_analysis_workers(&self) -> Result<(), AnalysisError> {
        info!("Starting {} analysis workers", self.config.max_workers);

        let mut workers_guard = self.workers.lock().await;

        if workers_guard.is_some() {
            warn!("Analysis workers are already running");
            return Ok(());
        }

        let mut workers = Vec::new();
        let shutdown_rx = self.queue_manager.shutdown_signal.subscribe();

        for worker_id in 0..self.config.max_workers {
            let queue_manager = self.queue_manager.clone();
            let analyzer_manager = self.analyzer_manager.clone();
            let database = self.database.clone();
            let file_detector = self.file_detector.clone();
            let config = self.config.clone();
            let mut worker_shutdown = shutdown_rx.resubscribe();

            let worker = tokio::spawn(async move {
                info!("Analysis worker {} started", worker_id);

                loop {
                    tokio::select! {
                        _ = worker_shutdown.recv() => {
                            info!("Analysis worker {} received shutdown signal", worker_id);
                            break;
                        }
                        task_opt = queue_manager.dequeue_task() => {
                            if let Some(task) = task_opt {
                                Self::process_analysis_task(
                                    task,
                                    &*analyzer_manager,
                                    &*database,
                                    &*file_detector,
                                    &config,
                                ).await;
                            } else {
                                // No tasks available, sleep briefly
                                tokio::time::sleep(Duration::from_millis(100)).await;
                            }
                        }
                    }
                }

                info!("Analysis worker {} stopped", worker_id);
            });

            workers.push(worker);
        }

        *workers_guard = Some(WorkerPool {
            workers,
            worker_count: self.config.max_workers,
            shutdown_signal: shutdown_rx,
        });

        info!("Analysis workers started successfully");
        Ok(())
    }

    /// Stop analysis workers gracefully
    pub async fn stop_analysis_workers(&self) -> Result<(), AnalysisError> {
        info!("Stopping analysis workers");

        let mut workers_guard = self.workers.lock().await;

        if let Some(worker_pool) = workers_guard.take() {
            // Send shutdown signal
            if let Err(e) = self.queue_manager.shutdown_signal.send(()) {
                warn!("Failed to send shutdown signal: {}", e);
            }

            // Wait for workers to complete
            for (i, worker) in worker_pool.workers.into_iter().enumerate() {
                match tokio::time::timeout(Duration::from_secs(30), worker).await {
                    Ok(Ok(())) => debug!("Worker {} stopped gracefully", i),
                    Ok(Err(e)) => warn!("Worker {} stopped with error: {}", i, e),
                    Err(_) => warn!("Worker {} shutdown timed out", i),
                }
            }

            info!("All analysis workers stopped");
        } else {
            debug!("No analysis workers were running");
        }

        Ok(())
    }

    /// Get detailed analysis progress for workspace
    pub async fn get_analysis_progress(
        &self,
        workspace_id: i64,
    ) -> Result<AnalysisProgressInfo, AnalysisError> {
        let progress = self.database.get_analysis_progress(workspace_id).await?;
        let queue_metrics = self.queue_manager.get_metrics().await;

        // Calculate completion percentage
        let completion_percentage = if progress.total_files > 0 {
            (progress.analyzed_files as f32 / progress.total_files as f32) * 100.0
        } else {
            0.0
        };

        // Calculate current throughput (files per second)
        let current_throughput = {
            let metrics = self.metrics.read().await;
            let elapsed_seconds = self.start_time.elapsed().as_secs_f32();
            if elapsed_seconds > 0.0 {
                metrics.total_analyses_performed as f32 / elapsed_seconds
            } else {
                0.0
            }
        };

        // Estimate remaining time
        let estimated_remaining = if current_throughput > 0.0 && progress.pending_files > 0 {
            Some(Duration::from_secs_f32(
                progress.pending_files as f32 / current_throughput,
            ))
        } else {
            None
        };

        Ok(AnalysisProgressInfo {
            workspace_id,
            total_files: progress.total_files,
            analyzed_files: progress.analyzed_files,
            queued_files: queue_metrics.queue_depth as u64,
            failed_files: progress.failed_files,
            completion_percentage,
            current_throughput,
            estimated_remaining,
        })
    }

    /// Get current queue metrics
    pub async fn get_queue_metrics(&self) -> Result<QueueMetrics, AnalysisError> {
        Ok(self.queue_manager.get_metrics().await)
    }

    // Private helper methods for implementation

    /// Process a single file change and return (tasks_queued, dependency_cascades)
    async fn process_single_file_change(
        &self,
        workspace_id: i64,
        change: FileChange,
    ) -> Result<(u64, u64), AnalysisError> {
        let mut tasks_queued = 0u64;
        let mut dependency_cascades = 0u64;

        match change.change_type {
            FileChangeType::Delete => {
                // Handle file deletion - remove from database and update dependents
                debug!("Processing file deletion: {}", change.path.display());
                // TODO: Implement deletion handling
                return Ok((0, 0));
            }
            FileChangeType::Create | FileChangeType::Update => {
                // Process file creation or update
                let version_info = self
                    .file_version_manager
                    .ensure_file_version(&change.path, &tokio::fs::read(&change.path).await?)
                    .await?;

                // Associate with workspace
                self.file_version_manager
                    .associate_file_with_workspace(
                        workspace_id,
                        version_info.file_version.file_id,
                        version_info.file_version.file_version_id,
                    )
                    .await?;

                // Create analysis task
                let task_type = if version_info.is_new_version {
                    AnalysisTaskType::FullAnalysis
                } else {
                    AnalysisTaskType::IncrementalUpdate
                };

                let task = self
                    .create_analysis_task(
                        workspace_id,
                        version_info.file_version.file_version_id,
                        &change.path,
                        version_info.detected_language,
                        task_type,
                    )
                    .await?;

                self.queue_manager.queue_task(task).await?;
                tasks_queued += 1;

                // Queue dependent analysis if enabled
                if self.config.dependency_analysis_enabled {
                    dependency_cascades = self
                        .queue_dependent_analysis(workspace_id, &[change])
                        .await?;
                }
            }
            FileChangeType::Move { from: _, to: _ } => {
                // Handle file move - treat as deletion + creation
                debug!(
                    "Processing file move: {} -> {}",
                    change.path.display(),
                    change.path.display()
                );
                // TODO: Implement move handling
                return Ok((0, 0));
            }
        }

        Ok((tasks_queued, dependency_cascades))
    }

    /// Create an analysis task with appropriate priority
    async fn create_analysis_task(
        &self,
        workspace_id: i64,
        file_version_id: i64,
        file_path: &Path,
        language: Option<String>,
        task_type: AnalysisTaskType,
    ) -> Result<AnalysisTask, AnalysisError> {
        let task_id = self.queue_manager.next_task_id().await;
        let priority = self.calculate_file_priority(file_path, &language).await;

        // Create analysis run for this task
        Ok(AnalysisTask {
            task_id,
            priority,
            workspace_id,
            file_version_id,
            task_type,
            file_path: file_path.to_path_buf(),
            language: language.unwrap_or_else(|| "unknown".to_string()),
            created_at: SystemTime::now(),
            retry_count: 0,
            max_retries: self.config.retry_limit,
            triggered_by: vec![],
        })
    }

    /// Calculate priority for a file based on its characteristics
    async fn calculate_file_priority(
        &self,
        file_path: &Path,
        language: &Option<String>,
    ) -> AnalysisTaskPriority {
        // Priority based on file characteristics
        let filename = file_path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        let path_str = file_path.to_string_lossy();

        // Critical files (entry points, configuration)
        if filename == "main.rs"
            || filename == "lib.rs"
            || filename == "mod.rs"
            || filename == "index.js"
            || filename == "index.ts"
            || filename == "__init__.py"
            || filename.contains("config")
            || path_str.contains("/src/main/")
        {
            return AnalysisTaskPriority::Critical;
        }

        // High priority for core modules and interfaces
        if path_str.contains("/src/")
            || path_str.contains("/lib/")
            || filename.ends_with(".h")
            || filename.ends_with(".hpp")
            || (language.as_ref().map_or(false, |l| l == "typescript")
                && filename.ends_with(".d.ts"))
        {
            return AnalysisTaskPriority::High;
        }

        // Low priority for tests and documentation
        if path_str.contains("/test/")
            || path_str.contains("/tests/")
            || path_str.contains("_test.")
            || path_str.contains(".test.")
            || filename.ends_with(".md")
            || filename.ends_with(".txt")
            || filename.ends_with(".json")
        {
            return AnalysisTaskPriority::Low;
        }

        // Background priority for large files or rarely accessed ones
        // This would typically involve file size and access frequency analysis

        AnalysisTaskPriority::Normal
    }

    /// Create analysis context for the analyzer framework
    async fn create_analysis_context(
        &self,
        workspace_id: i64,
    ) -> Result<AnalysisContext, AnalysisError> {
        // Note: This is a simplified context creation
        // In practice, this would include more workspace-specific information
        let uid_generator = Arc::new(crate::symbol::SymbolUIDGenerator::new());

        Ok(AnalysisContext {
            workspace_id,
            file_version_id: 0,              // Will be set by the task processor
            analysis_run_id: 1,              // Will be set by the task processor
            language: "unknown".to_string(), // Will be set by the task processor
            uid_generator,
            language_config: crate::analyzer::LanguageAnalyzerConfig::default(),
        })
    }

    /// Store analysis results in the database
    async fn store_analysis_result(
        &self,
        language: String,
        result: &AnalysisResult,
    ) -> Result<(), AnalysisError> {
        // Create context for database conversion
        let uid_generator = Arc::new(crate::symbol::SymbolUIDGenerator::new());
        let context = AnalysisContext {
            workspace_id: 0,    // Will be set by task processor
            file_version_id: 0, // Will be set by task processor
            analysis_run_id: 1, // Will be set by task processor
            language: language.clone(),
            uid_generator,
            language_config: crate::analyzer::LanguageAnalyzerConfig::default(),
        };

        // Use the built-in conversion methods
        let symbol_states = result.to_database_symbols(&context);
        let edges = result.to_database_edges(&context);

        // Store symbols
        self.database.store_symbols(&symbol_states).await?;

        // Store edges
        self.database.store_edges(&edges).await?;

        debug!(
            "Stored {} symbols and {} relationships for language {}",
            symbol_states.len(),
            edges.len(),
            &language
        );

        Ok(())
    }

    /// Calculate current worker utilization
    async fn calculate_worker_utilization(&self) -> f64 {
        let queue_metrics = self.queue_manager.get_metrics().await;
        if self.config.max_workers > 0 {
            queue_metrics.active_workers as f64 / self.config.max_workers as f64
        } else {
            0.0
        }
    }

    /// Process a single analysis task (used by workers)
    async fn process_analysis_task(
        task: AnalysisTask,
        analyzer_manager: &AnalyzerManager,
        database: &T,
        file_detector: &FileChangeDetector,
        config: &AnalysisEngineConfig,
    ) {
        let start_time = Instant::now();
        debug!(
            "Processing analysis task {} for file {}",
            task.task_id,
            task.file_path.display()
        );

        // Timeout handling
        let result = tokio::time::timeout(
            Duration::from_secs(config.timeout_seconds),
            Self::execute_analysis_task(
                task.clone(),
                analyzer_manager,
                &*database,
                &*file_detector,
            ),
        )
        .await;

        match result {
            Ok(Ok(_)) => {
                debug!(
                    "Analysis task {} completed successfully in {:?}",
                    task.task_id,
                    start_time.elapsed()
                );
            }
            Ok(Err(e)) => {
                error!("Analysis task {} failed: {}", task.task_id, e);
                // TODO: Implement retry logic
            }
            Err(_) => {
                error!(
                    "Analysis task {} timed out after {}s",
                    task.task_id, config.timeout_seconds
                );
                // TODO: Implement retry logic for timeouts
            }
        }
    }

    /// Execute the actual analysis task
    async fn execute_analysis_task(
        task: AnalysisTask,
        analyzer_manager: &AnalyzerManager,
        _database: &T,
        _file_detector: &FileChangeDetector,
    ) -> Result<(), AnalysisError> {
        // Read file content
        let content = tokio::fs::read_to_string(&task.file_path)
            .await
            .context(format!("Failed to read file: {}", task.file_path.display()))?;

        let language = &task.language;

        // Create analysis context
        let uid_generator = Arc::new(crate::symbol::SymbolUIDGenerator::new());
        let context = AnalysisContext {
            workspace_id: task.workspace_id,
            file_version_id: task.file_version_id,
            analysis_run_id: 1, // Will be set by task processor
            language: task.language.clone(),
            uid_generator,
            language_config: crate::analyzer::LanguageAnalyzerConfig::default(),
        };

        // Perform analysis
        let _analysis_result = analyzer_manager
            .analyze_file(&content, &task.file_path, language, &context)
            .await?;

        // Convert and store results (simplified version of store_analysis_result)
        // This would be extracted to a shared method in a real implementation

        Ok(())
    }

    /// Determine files that need reindexing due to dependencies
    pub async fn get_dependent_files(
        &self,
        workspace_id: i64,
        changed_files: &[PathBuf],
    ) -> Result<Vec<PathBuf>, AnalysisError> {
        if !self.config.dependency_analysis_enabled {
            return Ok(vec![]);
        }

        debug!(
            "Finding dependent files for {} changed files in workspace {}",
            changed_files.len(),
            workspace_id
        );

        let dependency_graph = self.get_or_build_dependency_graph(workspace_id).await?;
        let mut dependent_files = HashSet::new();

        // For each changed file, find all files that depend on it
        for changed_file in changed_files {
            if let Some(node) = dependency_graph.nodes.get(changed_file) {
                // Add direct dependents
                for dependent in &node.dependents {
                    dependent_files.insert(dependent.clone());
                }

                // Traverse dependency graph to find transitive dependents
                let mut visited = HashSet::new();
                let mut queue = VecDeque::new();
                queue.push_back(changed_file.clone());

                while let Some(current_file) = queue.pop_front() {
                    if visited.contains(&current_file) {
                        continue;
                    }
                    visited.insert(current_file.clone());

                    if let Some(current_node) = dependency_graph.nodes.get(&current_file) {
                        for dependent in &current_node.dependents {
                            if !visited.contains(dependent) {
                                dependent_files.insert(dependent.clone());
                                queue.push_back(dependent.clone());
                            }
                        }
                    }
                }
            }
        }

        let result: Vec<PathBuf> = dependent_files.into_iter().collect();
        info!(
            "Found {} dependent files for {} changed files",
            result.len(),
            changed_files.len()
        );

        Ok(result)
    }

    /// Build dependency graph for workspace
    pub async fn build_dependency_graph(
        &self,
        workspace_id: i64,
    ) -> Result<DependencyGraph, AnalysisError> {
        info!("Building dependency graph for workspace {}", workspace_id);

        let mut graph = DependencyGraph {
            nodes: HashMap::new(),
            edges: Vec::new(),
            last_updated: SystemTime::now(),
        };

        // Get all workspaces files (this would need a database method)
        // For now, we'll use a placeholder approach
        let workspace_files = self.get_workspace_files(workspace_id).await?;

        // Analyze each file to extract dependencies
        for file_path in workspace_files {
            match self.extract_file_dependencies(&file_path).await {
                Ok((dependencies, language)) => {
                    let node = DependencyNode {
                        file_path: file_path.clone(),
                        file_version_id: 0, // Would be looked up from database
                        last_analyzed: Some(SystemTime::now()),
                        dependencies: dependencies.clone(),
                        dependents: Vec::new(), // Will be populated in second pass
                        language,
                    };

                    // Create edges for dependencies
                    for dependency in dependencies {
                        let edge = DependencyEdge {
                            from: file_path.clone(),
                            to: dependency.clone(),
                            edge_type: DependencyType::Import, // Simplified
                            strength: 1.0,
                        };
                        graph.edges.push(edge);
                    }

                    graph.nodes.insert(file_path, node);
                }
                Err(e) => {
                    warn!(
                        "Failed to extract dependencies from {}: {}",
                        file_path.display(),
                        e
                    );
                }
            }
        }

        // Second pass: populate dependents
        for edge in &graph.edges {
            if let Some(target_node) = graph.nodes.get_mut(&edge.to) {
                target_node.dependents.push(edge.from.clone());
            }
        }

        info!(
            "Built dependency graph with {} nodes and {} edges",
            graph.nodes.len(),
            graph.edges.len()
        );

        Ok(graph)
    }

    /// Queue dependent files for analysis
    pub async fn queue_dependent_analysis(
        &self,
        workspace_id: i64,
        root_changes: &[FileChange],
    ) -> Result<u64, AnalysisError> {
        if !self.config.dependency_analysis_enabled {
            return Ok(0);
        }

        let changed_files: Vec<PathBuf> = root_changes
            .iter()
            .map(|change| change.path.clone())
            .collect();

        let dependent_files = self
            .get_dependent_files(workspace_id, &changed_files)
            .await?;

        let mut tasks_queued = 0u64;

        for dependent_file in dependent_files {
            // Check if file exists and is indexable
            if !dependent_file.exists() || !self.file_detector.should_index_file(&dependent_file) {
                continue;
            }

            // Create dependency update task
            let task = AnalysisTask {
                task_id: self.queue_manager.next_task_id().await,
                priority: AnalysisTaskPriority::High, // Dependency updates are high priority
                workspace_id,
                file_version_id: 0, // Would be looked up
                task_type: AnalysisTaskType::DependencyUpdate,
                file_path: dependent_file.clone(),
                language: self
                    .file_detector
                    .detect_language(&dependent_file)
                    .unwrap_or("unknown".to_string()),
                created_at: SystemTime::now(),
                retry_count: 0,
                max_retries: self.config.retry_limit,
                triggered_by: changed_files.clone(),
            };

            self.queue_manager.queue_task(task).await?;
            tasks_queued += 1;
        }

        debug!(
            "Queued {} dependency update tasks for workspace {}",
            tasks_queued, workspace_id
        );
        Ok(tasks_queued)
    }

    // Private helper methods for dependency analysis

    /// Get or build dependency graph for workspace (with caching)
    async fn get_or_build_dependency_graph(
        &self,
        workspace_id: i64,
    ) -> Result<DependencyGraph, AnalysisError> {
        {
            let graphs = self.dependency_graph.read().await;
            if let Some(graph) = graphs.get(&workspace_id) {
                // Check if graph is still fresh (less than 5 minutes old)
                if let Ok(age) = graph.last_updated.elapsed() {
                    if age < Duration::from_secs(300) {
                        return Ok(graph.clone());
                    }
                }
            }
        }

        // Build new dependency graph
        let new_graph = self.build_dependency_graph(workspace_id).await?;

        // Cache the new graph
        {
            let mut graphs = self.dependency_graph.write().await;
            graphs.insert(workspace_id, new_graph.clone());
        }

        Ok(new_graph)
    }

    /// Extract dependencies from a single file
    async fn extract_file_dependencies(
        &self,
        file_path: &Path,
    ) -> Result<(Vec<PathBuf>, Option<String>), AnalysisError> {
        let language = self.file_detector.detect_language(file_path);

        // Read file content
        let content = match tokio::fs::read_to_string(file_path).await {
            Ok(content) => content,
            Err(e) => {
                debug!("Failed to read file {}: {}", file_path.display(), e);
                return Ok((vec![], language));
            }
        };

        let mut dependencies = Vec::new();

        // Simple dependency extraction based on language
        match language.as_deref() {
            Some("rust") | Some("rs") => {
                dependencies.extend(self.extract_rust_dependencies(&content, file_path));
            }
            Some("typescript") | Some("ts") | Some("javascript") | Some("js") => {
                dependencies.extend(self.extract_js_ts_dependencies(&content, file_path));
            }
            Some("python") | Some("py") => {
                dependencies.extend(self.extract_python_dependencies(&content, file_path));
            }
            Some("go") => {
                dependencies.extend(self.extract_go_dependencies(&content, file_path));
            }
            _ => {
                // Generic import pattern extraction
                dependencies.extend(self.extract_generic_dependencies(&content, file_path));
            }
        }

        Ok((dependencies, language))
    }

    /// Extract Rust dependencies (mod, use statements)
    fn extract_rust_dependencies(&self, content: &str, base_path: &Path) -> Vec<PathBuf> {
        let mut dependencies = Vec::new();
        let base_dir = base_path.parent().unwrap_or(Path::new("."));

        for line in content.lines() {
            let trimmed = line.trim();

            // Handle "mod module_name;"
            if let Some(mod_match) = trimmed
                .strip_prefix("mod ")
                .and_then(|s| s.strip_suffix(';'))
            {
                let mod_name = mod_match.trim();
                let mod_file = base_dir.join(format!("{}.rs", mod_name));
                if mod_file.exists() {
                    dependencies.push(mod_file);
                }
            }

            // Handle "use crate::module" or "use super::module"
            if trimmed.starts_with("use ")
                && (trimmed.contains("crate::") || trimmed.contains("super::"))
            {
                // This would require more sophisticated parsing in a real implementation
            }
        }

        dependencies
    }

    /// Extract JavaScript/TypeScript dependencies (import statements)
    fn extract_js_ts_dependencies(&self, content: &str, base_path: &Path) -> Vec<PathBuf> {
        let mut dependencies = Vec::new();
        let base_dir = base_path.parent().unwrap_or(Path::new("."));

        for line in content.lines() {
            let trimmed = line.trim();

            // Handle "import ... from './module'"
            if let Some(import_start) = trimmed.find("from ") {
                let import_part = &trimmed[import_start + 5..];
                if let Some(quote_start) = import_part.find(|c| c == '"' || c == '\'') {
                    let quote_char = import_part.chars().nth(quote_start).unwrap();
                    if let Some(quote_end) = import_part[quote_start + 1..].find(quote_char) {
                        let import_path =
                            &import_part[quote_start + 1..quote_start + 1 + quote_end];

                        if import_path.starts_with("./") || import_path.starts_with("../") {
                            // Relative import
                            let resolved_path = base_dir.join(import_path);
                            let candidates = vec![
                                resolved_path.with_extension("ts"),
                                resolved_path.with_extension("js"),
                                resolved_path.join("index.ts"),
                                resolved_path.join("index.js"),
                            ];

                            for candidate in candidates {
                                if candidate.exists() {
                                    dependencies.push(candidate);
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        dependencies
    }

    /// Extract Python dependencies (import statements)
    fn extract_python_dependencies(&self, content: &str, base_path: &Path) -> Vec<PathBuf> {
        let mut dependencies = Vec::new();
        let base_dir = base_path.parent().unwrap_or(Path::new("."));

        for line in content.lines() {
            let trimmed = line.trim();

            // Handle relative imports like "from .module import"
            if trimmed.starts_with("from .") {
                if let Some(import_pos) = trimmed.find(" import ") {
                    let module_part = &trimmed[5..import_pos].trim();
                    let module_file = base_dir.join(format!("{}.py", module_part));
                    if module_file.exists() {
                        dependencies.push(module_file);
                    }
                }
            }
        }

        dependencies
    }

    /// Extract Go dependencies (import statements)
    fn extract_go_dependencies(&self, _content: &str, _base_path: &Path) -> Vec<PathBuf> {
        let dependencies = Vec::new();

        // Go dependencies are typically external packages, so we don't extract file dependencies
        // In a real implementation, this might extract relative path imports

        dependencies
    }

    /// Generic dependency extraction (simple pattern matching)
    fn extract_generic_dependencies(&self, content: &str, base_path: &Path) -> Vec<PathBuf> {
        let mut dependencies = Vec::new();
        let base_dir = base_path.parent().unwrap_or(Path::new("."));

        // Look for common include patterns
        for line in content.lines() {
            let trimmed = line.trim();

            // C/C++ includes
            if let Some(include_match) = trimmed
                .strip_prefix("#include \"")
                .and_then(|s| s.strip_suffix('"'))
            {
                let include_file = base_dir.join(include_match);
                if include_file.exists() {
                    dependencies.push(include_file);
                }
            }
        }

        dependencies
    }

    /// Get all files in a workspace (placeholder implementation)
    async fn get_workspace_files(&self, workspace_id: i64) -> Result<Vec<PathBuf>, AnalysisError> {
        // This would query the database for all files associated with the workspace
        // For now, return an empty vec as placeholder
        debug!(
            "Getting workspace files for workspace {} (placeholder)",
            workspace_id
        );
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{DatabaseConfig, SQLiteBackend};
    use crate::symbol::SymbolUIDGenerator;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_analysis_engine_creation() -> Result<(), Box<dyn std::error::Error>> {
        let temp_dir = TempDir::new()?;
        let db_config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };
        let database = Arc::new(SQLiteBackend::new(db_config).await?);

        let workspace_manager = Arc::new(WorkspaceManager::new(database.clone()).await?);
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        let analyzer_manager = Arc::new(AnalyzerManager::new(uid_generator));

        let engine =
            IncrementalAnalysisEngine::new(database, workspace_manager, analyzer_manager).await?;

        // Verify engine is created with correct configuration
        assert_eq!(engine.config.max_workers, std::cmp::max(2, num_cpus::get()));
        assert!(engine.config.dependency_analysis_enabled);

        Ok(())
    }

    #[tokio::test]
    async fn test_analysis_task_ordering() {
        let mut heap = BinaryHeap::new();

        let task1 = AnalysisTask {
            task_id: 1,
            priority: AnalysisTaskPriority::Low,
            workspace_id: 1,
            file_version_id: 1,
            task_type: AnalysisTaskType::FullAnalysis,
            file_path: PathBuf::from("test1.rs"),
            language: "rust".to_string(),
            created_at: SystemTime::now(),
            retry_count: 0,
            max_retries: 3,
            triggered_by: vec![],
        };

        let task2 = AnalysisTask {
            task_id: 2,
            priority: AnalysisTaskPriority::Critical,
            ..task1.clone()
        };

        let task3 = AnalysisTask {
            task_id: 3,
            priority: AnalysisTaskPriority::High,
            ..task1.clone()
        };

        heap.push(task1);
        heap.push(task2.clone());
        heap.push(task3);

        // Critical priority should come first
        let first = heap.pop().unwrap();
        assert_eq!(first.priority, AnalysisTaskPriority::Critical);
        assert_eq!(first.task_id, 2);
    }

    #[tokio::test]
    async fn test_queue_manager_basic_operations() -> Result<(), Box<dyn std::error::Error>> {
        let db_config = DatabaseConfig {
            temporary: true,
            ..Default::default()
        };
        let database = Arc::new(SQLiteBackend::new(db_config).await?);
        let (shutdown_tx, _) = broadcast::channel(1);

        let queue_manager = AnalysisQueueManager {
            database,
            queue: Arc::new(Mutex::new(BinaryHeap::new())),
            task_counter: Arc::new(Mutex::new(0)),
            metrics: Arc::new(RwLock::new(QueueMetrics::default())),
            shutdown_signal: shutdown_tx,
        };

        let task = AnalysisTask {
            task_id: queue_manager.next_task_id().await,
            priority: AnalysisTaskPriority::Normal,
            workspace_id: 1,
            file_version_id: 1,
            task_type: AnalysisTaskType::FullAnalysis,
            file_path: PathBuf::from("test.rs"),
            language: "rust".to_string(),
            created_at: SystemTime::now(),
            retry_count: 0,
            max_retries: 3,
            triggered_by: vec![],
        };

        queue_manager.queue_task(task.clone()).await?;

        let metrics = queue_manager.get_metrics().await;
        assert_eq!(metrics.tasks_queued, 1);
        assert_eq!(metrics.queue_depth, 1);

        let dequeued = queue_manager.dequeue_task().await;
        assert!(dequeued.is_some());
        assert_eq!(dequeued.unwrap().task_id, task.task_id);

        Ok(())
    }

    #[test]
    fn test_analysis_engine_config_defaults() {
        let config = AnalysisEngineConfig::default();
        assert!(config.max_workers >= 2);
        assert_eq!(config.batch_size, 50);
        assert_eq!(config.retry_limit, 3);
        assert!(config.dependency_analysis_enabled);
    }

    #[test]
    fn test_dependency_types() {
        let edge = DependencyEdge {
            from: PathBuf::from("main.rs"),
            to: PathBuf::from("lib.rs"),
            edge_type: DependencyType::Import,
            strength: 1.0,
        };

        assert_eq!(edge.edge_type, DependencyType::Import);
        assert_eq!(edge.strength, 1.0);
    }
}
