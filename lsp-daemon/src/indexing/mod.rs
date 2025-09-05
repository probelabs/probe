//! Indexing subsystem for semantic code search and analysis
//!
//! This module provides infrastructure for indexing code repositories with:
//! - Lock-free atomic progress tracking
//! - Multi-level priority queue for file processing
//! - Language-specific processing pipelines
//! - Worker pool management with configurable concurrency
//! - Memory budget awareness and backpressure handling
//!
//! The indexing subsystem is designed to operate in the background while the
//! LSP daemon serves requests, providing semantic enhancement capabilities.

pub mod analyzer;
pub mod config;
pub mod file_detector;
pub mod language_strategies;
pub mod manager;
pub mod pipelines;
pub mod progress;
pub mod queue;
pub mod versioning;

// Re-export commonly used types
pub use analyzer::{
    AnalysisEngineConfig, AnalysisTask, AnalysisTaskPriority, AnalysisTaskType, DependencyGraph,
    DependencyNode, FileAnalysisResult, IncrementalAnalysisEngine, ProcessingResult,
    WorkspaceAnalysisResult,
};
pub use config::{
    CacheStrategy, EffectiveConfig, IndexingConfig, IndexingFeatures, LanguageIndexConfig,
};
pub use file_detector::{
    DetectionConfig, DetectionError, FileChange, FileChangeDetector, FileChangeType, HashAlgorithm,
};
pub use language_strategies::{
    FileImportanceStrategy, IndexingPriority, LanguageIndexingStrategy, LanguageStrategyFactory,
    LspOperationStrategy, SymbolPriorityStrategy,
};
pub use manager::{IndexingManager, ManagerConfig, ManagerStatus, WorkerStats};
pub use pipelines::{IndexingPipeline, LanguagePipeline, PipelineConfig, PipelineResult};
pub use progress::{IndexingProgress, ProgressMetrics, ProgressSnapshot};
pub use queue::{IndexingQueue, Priority, QueueItem, QueueMetrics, QueueSnapshot};
pub use versioning::{
    FileVersionInfo, FileVersionManager, ProcessingResults, VersioningConfig, VersioningError,
    VersioningMetrics,
};
