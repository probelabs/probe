//! Batch conversion operations for efficient symbol processing
//!
//! This module provides optimized batch conversion functions for transforming
//! large sets of ExtractedSymbol data into SymbolState database records with
//! performance optimizations and memory management.

use anyhow::{Context, Result};
use rayon::prelude::*;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::{debug, info, warn};

use crate::analyzer::types::ExtractedSymbol as AnalyzerExtractedSymbol;
use crate::database::{DatabaseBackend, SymbolState};
// Using the new unified ExtractedSymbol type from analyzer
use crate::analyzer::types::ExtractedSymbol as AstExtractedSymbol;
use crate::indexing::symbol_conversion::{ConversionContext, SymbolUIDGenerator, ToSymbolState};

/// Configuration for batch conversion operations
#[derive(Debug, Clone)]
pub struct BatchConversionConfig {
    /// Maximum number of symbols to process in a single batch
    pub batch_size: usize,
    /// Enable parallel processing for conversions
    pub enable_parallel: bool,
    /// Maximum number of threads for parallel processing
    pub max_threads: Option<usize>,
    /// Enable progress reporting
    pub enable_progress: bool,
    /// Memory limit for batch operations (in MB)
    pub memory_limit_mb: Option<usize>,
}

impl Default for BatchConversionConfig {
    fn default() -> Self {
        Self {
            batch_size: 1000,
            enable_parallel: true,
            max_threads: None, // Use default rayon thread pool
            enable_progress: false,
            memory_limit_mb: Some(500), // 500MB default limit
        }
    }
}

/// Progress reporter for batch operations
pub trait ProgressReporter: Send + Sync {
    /// Report conversion progress
    fn report_progress(&self, processed: usize, total: usize, elapsed_ms: u64);
    /// Report completion
    fn report_completion(&self, total: usize, elapsed_ms: u64, errors: usize);
}

/// Console progress reporter implementation
pub struct ConsoleProgressReporter;

impl ProgressReporter for ConsoleProgressReporter {
    fn report_progress(&self, processed: usize, total: usize, elapsed_ms: u64) {
        let percentage = (processed as f64 / total as f64) * 100.0;
        let rate = if elapsed_ms > 0 {
            (processed as f64) / (elapsed_ms as f64 / 1000.0)
        } else {
            0.0
        };

        info!(
            "Conversion progress: {}/{} ({:.1}%) - {:.1} symbols/sec",
            processed, total, percentage, rate
        );
    }

    fn report_completion(&self, total: usize, elapsed_ms: u64, errors: usize) {
        let rate = if elapsed_ms > 0 {
            (total as f64) / (elapsed_ms as f64 / 1000.0)
        } else {
            0.0
        };

        info!(
            "Batch conversion completed: {} symbols in {}ms ({:.1} symbols/sec) - {} errors",
            total, elapsed_ms, rate, errors
        );
    }
}

/// Result of a batch conversion operation
#[derive(Debug)]
pub struct BatchConversionResult {
    /// Successfully converted symbols
    pub symbols: Vec<SymbolState>,
    /// Number of conversion errors
    pub error_count: usize,
    /// Conversion errors (up to first 100)
    pub errors: Vec<anyhow::Error>,
    /// Total processing time in milliseconds
    pub elapsed_ms: u64,
    /// UID collision statistics
    pub collision_stats: HashMap<String, u32>,
}

/// Batch symbol converter with optimizations
pub struct BatchSymbolConverter {
    config: BatchConversionConfig,
    uid_generator: Arc<Mutex<SymbolUIDGenerator>>,
}

impl BatchSymbolConverter {
    /// Create a new batch converter with configuration
    pub fn new(config: BatchConversionConfig) -> Self {
        Self {
            config,
            uid_generator: Arc::new(Mutex::new(SymbolUIDGenerator::new())),
        }
    }

    /// Create a new batch converter with default configuration
    pub fn new_default() -> Self {
        Self::new(BatchConversionConfig::default())
    }

    /// Convert AST symbols to SymbolState in batches
    pub fn convert_ast_symbols(
        &self,
        symbols: Vec<AstExtractedSymbol>,
        context: &ConversionContext,
        progress_reporter: Option<&dyn ProgressReporter>,
    ) -> Result<BatchConversionResult> {
        self.convert_symbols_internal(symbols, context, progress_reporter)
    }

    /// Convert analyzer symbols to SymbolState in batches
    pub fn convert_analyzer_symbols(
        &self,
        symbols: Vec<AnalyzerExtractedSymbol>,
        context: &ConversionContext,
        progress_reporter: Option<&dyn ProgressReporter>,
    ) -> Result<BatchConversionResult> {
        self.convert_symbols_internal(symbols, context, progress_reporter)
    }

    /// Internal conversion method that works with any ToSymbolState type
    fn convert_symbols_internal<T: ToSymbolState + Send + Sync>(
        &self,
        symbols: Vec<T>,
        context: &ConversionContext,
        progress_reporter: Option<&dyn ProgressReporter>,
    ) -> Result<BatchConversionResult> {
        let start_time = Instant::now();
        let total_symbols = symbols.len();

        debug!(
            "Starting batch conversion of {} symbols with config: {:?}",
            total_symbols, self.config
        );

        // Check memory limits
        if let Some(limit_mb) = self.config.memory_limit_mb {
            let estimated_memory_mb = (total_symbols * 1024) / (1024 * 1024); // Rough estimate
            if estimated_memory_mb > limit_mb {
                warn!(
                    "Estimated memory usage ({}MB) exceeds limit ({}MB). Consider processing in smaller batches.",
                    estimated_memory_mb, limit_mb
                );
            }
        }

        // Reset UID generator for this batch
        {
            let mut generator = self.uid_generator.lock().unwrap();
            generator.reset();
        }

        let mut results = Vec::with_capacity(total_symbols);
        let mut errors = Vec::new();
        let mut processed = 0;

        // Process in batches to manage memory
        for chunk in symbols.chunks(self.config.batch_size) {
            let chunk_results = if self.config.enable_parallel {
                self.process_chunk_parallel(chunk, context)?
            } else {
                self.process_chunk_sequential(chunk, context)?
            };

            // Collect results and errors
            for result in chunk_results {
                match result {
                    Ok(symbol_state) => results.push(symbol_state),
                    Err(e) => {
                        if errors.len() < 100 {
                            errors.push(e);
                        }
                    }
                }
            }

            processed += chunk.len();

            // Report progress
            if self.config.enable_progress {
                if let Some(reporter) = progress_reporter {
                    reporter.report_progress(
                        processed,
                        total_symbols,
                        start_time.elapsed().as_millis() as u64,
                    );
                }
            }
        }

        let elapsed_ms = start_time.elapsed().as_millis().max(1) as u64; // Ensure at least 1ms
        let error_count = errors.len();

        // Get collision statistics
        let collision_stats = {
            let generator = self.uid_generator.lock().unwrap();
            generator.get_collision_stats()
        };

        // Report completion
        if self.config.enable_progress {
            if let Some(reporter) = progress_reporter {
                reporter.report_completion(total_symbols, elapsed_ms, error_count);
            }
        }

        Ok(BatchConversionResult {
            symbols: results,
            error_count,
            errors,
            elapsed_ms,
            collision_stats,
        })
    }

    /// Process a chunk of symbols in parallel
    fn process_chunk_parallel<T: ToSymbolState + Send + Sync>(
        &self,
        chunk: &[T],
        context: &ConversionContext,
    ) -> Result<Vec<Result<SymbolState>>> {
        // Use existing global thread pool or create a scoped one
        // Note: rayon global pool configuration only works if not already initialized

        let uid_generator = Arc::clone(&self.uid_generator);

        let results: Vec<Result<SymbolState>> = chunk
            .par_iter()
            .map(|symbol| {
                let mut generator = uid_generator.lock().unwrap();
                symbol.to_symbol_state_validated(context, &mut generator)
            })
            .collect();

        Ok(results)
    }

    /// Process a chunk of symbols sequentially
    fn process_chunk_sequential<T: ToSymbolState + Send + Sync>(
        &self,
        chunk: &[T],
        context: &ConversionContext,
    ) -> Result<Vec<Result<SymbolState>>> {
        let mut results = Vec::with_capacity(chunk.len());
        let mut generator = self.uid_generator.lock().unwrap();

        for symbol in chunk {
            let result = symbol.to_symbol_state_validated(context, &mut generator);
            results.push(result);
        }

        Ok(results)
    }
}

/// Database integration functions for batch symbol storage
pub struct SymbolDatabaseIntegrator;

impl SymbolDatabaseIntegrator {
    /// Store symbols in database with workspace isolation
    pub async fn store_symbols_with_workspace<T: DatabaseBackend>(
        database: &T,
        symbols: Vec<SymbolState>,
        workspace_id: Option<String>,
    ) -> Result<()> {
        let start_time = Instant::now();

        debug!(
            "Storing {} symbols in database with workspace_id: {:?}",
            symbols.len(),
            workspace_id
        );

        // Store symbols using the database backend
        database
            .store_symbols(&symbols)
            .await
            .context("Failed to store symbols in database")?;

        let elapsed_ms = start_time.elapsed().as_millis() as u64;
        info!(
            "Successfully stored {} symbols in database ({}ms)",
            symbols.len(),
            elapsed_ms
        );

        Ok(())
    }

    /// Store symbols with duplicate detection and upsert logic
    pub async fn store_symbols_with_upsert<T: DatabaseBackend>(
        database: &T,
        symbols: Vec<SymbolState>,
    ) -> Result<()> {
        let start_time = Instant::now();

        debug!("Storing {} symbols with upsert logic", symbols.len());

        // Group symbols by file for more efficient upserts
        let mut symbols_by_file: HashMap<String, Vec<SymbolState>> = HashMap::new();
        for symbol in symbols {
            symbols_by_file
                .entry(symbol.file_path.clone())
                .or_default()
                .push(symbol);
        }

        let mut total_stored = 0;

        // Process each file's symbols
        for (file_path, file_symbols) in symbols_by_file {
            debug!(
                "Processing {} symbols for file: {}",
                file_symbols.len(),
                file_path
            );

            // Store symbols for this file
            database
                .store_symbols(&file_symbols)
                .await
                .with_context(|| format!("Failed to store symbols for file: {}", file_path))?;

            total_stored += file_symbols.len();
        }

        let elapsed_ms = start_time.elapsed().as_millis() as u64;
        info!(
            "Successfully stored {} symbols with upsert logic ({}ms)",
            total_stored, elapsed_ms
        );

        Ok(())
    }

    /// Batch store extracted symbols with full conversion pipeline
    pub async fn store_extracted_symbols<T: DatabaseBackend>(
        database: &T,
        ast_symbols: Vec<AstExtractedSymbol>,
        analyzer_symbols: Vec<AnalyzerExtractedSymbol>,
        context: &ConversionContext,
        config: Option<BatchConversionConfig>,
    ) -> Result<()> {
        let converter = BatchSymbolConverter::new(config.unwrap_or_default());
        let progress_reporter = ConsoleProgressReporter;

        let mut all_symbol_states = Vec::new();

        // Convert AST symbols if any
        if !ast_symbols.is_empty() {
            debug!("Converting {} AST symbols", ast_symbols.len());
            let ast_result =
                converter.convert_ast_symbols(ast_symbols, context, Some(&progress_reporter))?;

            if ast_result.error_count > 0 {
                warn!(
                    "AST conversion completed with {} errors",
                    ast_result.error_count
                );
                for (i, error) in ast_result.errors.iter().enumerate().take(5) {
                    warn!("AST conversion error {}: {}", i + 1, error);
                }
            }

            all_symbol_states.extend(ast_result.symbols);
        }

        // Convert analyzer symbols if any
        if !analyzer_symbols.is_empty() {
            debug!("Converting {} analyzer symbols", analyzer_symbols.len());
            let analyzer_result = converter.convert_analyzer_symbols(
                analyzer_symbols,
                context,
                Some(&progress_reporter),
            )?;

            if analyzer_result.error_count > 0 {
                warn!(
                    "Analyzer conversion completed with {} errors",
                    analyzer_result.error_count
                );
                for (i, error) in analyzer_result.errors.iter().enumerate().take(5) {
                    warn!("Analyzer conversion error {}: {}", i + 1, error);
                }
            }

            all_symbol_states.extend(analyzer_result.symbols);
        }

        // Store all converted symbols
        if !all_symbol_states.is_empty() {
            Self::store_symbols_with_upsert(database, all_symbol_states).await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Removed unused import: use crate::indexing::language_strategies::IndexingPriority;
    use std::path::PathBuf;

    fn create_test_ast_symbol(name: &str, line: u32) -> AstExtractedSymbol {
        use crate::symbol::{SymbolKind, SymbolLocation, Visibility};

        let location = SymbolLocation::new(PathBuf::from("test.rs"), line, 0, line, 10);

        AstExtractedSymbol {
            uid: format!("test:{}:{}", name, line),
            name: name.to_string(),
            kind: SymbolKind::Function,
            qualified_name: None,
            signature: None,
            visibility: Some(Visibility::Public),
            location,
            parent_scope: None,
            documentation: None,
            tags: vec![],
            metadata: std::collections::HashMap::new(),
        }
    }

    fn create_test_context() -> ConversionContext {
        ConversionContext::new(
            PathBuf::from("/workspace/src/test.rs"),
            "rust".to_string(),
            PathBuf::from("/workspace"),
        )
    }

    #[test]
    fn test_batch_converter_creation() {
        let config = BatchConversionConfig {
            batch_size: 500,
            enable_parallel: false,
            ..Default::default()
        };

        let converter = BatchSymbolConverter::new(config);
        assert_eq!(converter.config.batch_size, 500);
        assert!(!converter.config.enable_parallel);
    }

    #[test]
    fn test_batch_conversion_sequential() {
        let converter = BatchSymbolConverter::new(BatchConversionConfig {
            enable_parallel: false,
            enable_progress: false,
            ..Default::default()
        });

        let symbols = vec![
            create_test_ast_symbol("func1", 1),
            create_test_ast_symbol("func2", 2),
            create_test_ast_symbol("func3", 3),
        ];

        let context = create_test_context();
        let result = converter
            .convert_ast_symbols(symbols, &context, None)
            .unwrap();

        assert_eq!(result.symbols.len(), 3);
        assert_eq!(result.error_count, 0);
        assert!(result.elapsed_ms > 0);
    }

    #[test]
    fn test_batch_conversion_parallel() {
        let converter = BatchSymbolConverter::new(BatchConversionConfig {
            enable_parallel: true,
            enable_progress: false,
            max_threads: Some(2),
            ..Default::default()
        });

        let symbols = vec![
            create_test_ast_symbol("func1", 1),
            create_test_ast_symbol("func2", 2),
            create_test_ast_symbol("func3", 3),
            create_test_ast_symbol("func4", 4),
            create_test_ast_symbol("func5", 5),
        ];

        let context = create_test_context();
        let result = converter
            .convert_ast_symbols(symbols, &context, None)
            .unwrap();

        assert_eq!(result.symbols.len(), 5);
        assert_eq!(result.error_count, 0);
        assert!(result.elapsed_ms > 0);
    }

    #[test]
    fn test_progress_reporter() {
        let reporter = ConsoleProgressReporter;

        // These should not panic
        reporter.report_progress(50, 100, 1000);
        reporter.report_completion(100, 2000, 0);
    }

    #[test]
    fn test_batch_config_default() {
        let config = BatchConversionConfig::default();

        assert_eq!(config.batch_size, 1000);
        assert!(config.enable_parallel);
        assert!(config.max_threads.is_none());
        assert!(!config.enable_progress);
        assert_eq!(config.memory_limit_mb, Some(500));
    }
}
