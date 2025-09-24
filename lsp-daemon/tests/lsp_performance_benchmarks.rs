#![cfg(feature = "legacy-tests")]
//! LSP Performance Benchmarking Suite
//!
//! This module provides comprehensive performance benchmarks for LSP operations
//! including relationship extraction, call hierarchy analysis, and cache performance.

use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::time::timeout;
use tracing::{debug, info, warn};

// Import modules for benchmarking
use lsp_daemon::analyzer::types::{
    AnalysisContext, ExtractedRelationship, ExtractedSymbol, RelationType,
};
use lsp_daemon::language_detector::{Language, LanguageDetector};
use lsp_daemon::lsp_registry::LspRegistry;
use lsp_daemon::relationship::lsp_client_wrapper::LspClientWrapper;
use lsp_daemon::relationship::lsp_enhancer::{
    LspEnhancementConfig, LspRelationshipEnhancer, LspRelationshipType,
};
use lsp_daemon::server_manager::SingleServerManager;
use lsp_daemon::symbol::SymbolUIDGenerator;
use lsp_daemon::symbol::{SymbolKind, SymbolLocation};
use lsp_daemon::universal_cache::CacheLayer;
use lsp_daemon::workspace_cache_router::{WorkspaceCacheRouter, WorkspaceCacheRouterConfig};
use lsp_daemon::workspace_resolver::WorkspaceResolver;

/// Performance benchmark configuration
#[derive(Debug, Clone)]
pub struct BenchmarkConfig {
    /// Number of iterations for each benchmark
    pub iterations: usize,
    /// Timeout for LSP operations in milliseconds
    pub timeout_ms: u64,
    /// Languages to benchmark
    pub languages: Vec<Language>,
    /// Whether to include cache warmup
    pub include_cache_warmup: bool,
    /// Whether to test concurrent operations
    pub test_concurrency: bool,
    /// Maximum concurrent operations to test
    pub max_concurrent_ops: usize,
    /// Whether to generate detailed timing reports
    pub detailed_timing: bool,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            iterations: 10,
            timeout_ms: 15000, // Generous timeout for benchmarks
            languages: vec![
                Language::Rust,
                Language::Python,
                Language::Go,
                Language::TypeScript,
            ],
            include_cache_warmup: true,
            test_concurrency: true,
            max_concurrent_ops: 8,
            detailed_timing: true,
        }
    }
}

/// Performance benchmark results
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub operation: String,
    pub language: Language,
    pub min_duration: Duration,
    pub max_duration: Duration,
    pub avg_duration: Duration,
    pub median_duration: Duration,
    pub percentile_95: Duration,
    pub success_rate: f64,
    pub total_operations: usize,
    pub ops_per_second: f64,
}

impl BenchmarkResult {
    pub fn from_measurements(
        operation: String,
        language: Language,
        measurements: &[Duration],
        successes: usize,
    ) -> Self {
        let mut sorted = measurements.to_vec();
        sorted.sort();

        let min_duration = *sorted.first().unwrap_or(&Duration::ZERO);
        let max_duration = *sorted.last().unwrap_or(&Duration::ZERO);
        let avg_duration = measurements.iter().sum::<Duration>() / measurements.len().max(1) as u32;
        let median_duration = sorted
            .get(sorted.len() / 2)
            .copied()
            .unwrap_or(Duration::ZERO);
        let percentile_95 = sorted
            .get(sorted.len() * 95 / 100)
            .copied()
            .unwrap_or(Duration::ZERO);

        let success_rate = successes as f64 / measurements.len() as f64;
        let ops_per_second = if avg_duration.as_secs_f64() > 0.0 {
            1.0 / avg_duration.as_secs_f64()
        } else {
            0.0
        };

        Self {
            operation,
            language,
            min_duration,
            max_duration,
            avg_duration,
            median_duration,
            percentile_95,
            success_rate,
            total_operations: measurements.len(),
            ops_per_second,
        }
    }
}

/// Benchmark suite for LSP operations
pub struct LspBenchmarkSuite {
    server_manager: Arc<SingleServerManager>,
    lsp_client_wrapper: Arc<LspClientWrapper>,
    lsp_enhancer: Arc<LspRelationshipEnhancer>,
    cache_layer: Arc<CacheLayer>,
    uid_generator: Arc<SymbolUIDGenerator>,
    config: BenchmarkConfig,
    _temp_dir: TempDir, // Keep temp directory alive
}

impl LspBenchmarkSuite {
    pub async fn new(config: BenchmarkConfig) -> Result<Self> {
        // Create temporary directory for cache
        let temp_dir = TempDir::new()?;
        let workspace_config = WorkspaceCacheRouterConfig {
            base_cache_dir: temp_dir.path().join("caches"),
            max_open_caches: 8,
            max_parent_lookup_depth: 3,
            ..Default::default()
        };

        // Create LSP infrastructure
        let registry = Arc::new(LspRegistry::new()?);
        let child_processes = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let server_manager = Arc::new(SingleServerManager::new_with_tracker(
            registry,
            child_processes,
        ));

        let workspace_router = Arc::new(WorkspaceCacheRouter::new(
            workspace_config,
            server_manager.clone(),
        ));

        let universal_cache =
            Arc::new(lsp_daemon::universal_cache::UniversalCache::new(workspace_router).await?);

        let cache_layer = Arc::new(CacheLayer::new(universal_cache, None, None));

        let language_detector = Arc::new(LanguageDetector::new());
        let workspace_resolver = Arc::new(tokio::sync::Mutex::new(WorkspaceResolver::new(None)));

        let lsp_client_wrapper = Arc::new(LspClientWrapper::new(
            server_manager.clone(),
            language_detector.clone(),
            workspace_resolver.clone(),
        ));

        let uid_generator = Arc::new(SymbolUIDGenerator::new());

        let lsp_config = LspEnhancementConfig {
            timeout_ms: config.timeout_ms,
            cache_lsp_responses: true,
            enabled_relationship_types: vec![
                LspRelationshipType::References,
                LspRelationshipType::Definition,
                LspRelationshipType::IncomingCalls,
                LspRelationshipType::OutgoingCalls,
                LspRelationshipType::Implementation,
            ],
            ..Default::default()
        };

        let lsp_enhancer = Arc::new(LspRelationshipEnhancer::with_config(
            Some(server_manager.clone()),
            language_detector,
            workspace_resolver,
            cache_layer.clone(),
            uid_generator.clone(),
            lsp_config,
        ));

        Ok(Self {
            server_manager,
            lsp_client_wrapper,
            lsp_enhancer,
            cache_layer,
            uid_generator,
            config,
            _temp_dir: temp_dir,
        })
    }

    /// Run all performance benchmarks
    pub async fn run_all_benchmarks(&self) -> Result<Vec<BenchmarkResult>> {
        info!("🚀 Starting LSP performance benchmarks");
        let mut results = Vec::new();

        // Create test workspaces for each language
        let test_workspaces = self.create_test_workspaces().await?;

        // Benchmark 1: Basic LSP operations
        info!("📊 Benchmarking basic LSP operations...");
        let basic_results = self
            .benchmark_basic_lsp_operations(&test_workspaces)
            .await?;
        results.extend(basic_results);

        // Benchmark 2: Call hierarchy operations
        info!("📞 Benchmarking call hierarchy operations...");
        let call_hierarchy_results = self
            .benchmark_call_hierarchy_operations(&test_workspaces)
            .await?;
        results.extend(call_hierarchy_results);

        // Benchmark 3: Relationship enhancement
        info!("🔗 Benchmarking relationship enhancement...");
        let enhancement_results = self
            .benchmark_relationship_enhancement(&test_workspaces)
            .await?;
        results.extend(enhancement_results);

        // Benchmark 4: Cache performance
        info!("💾 Benchmarking cache performance...");
        let cache_results = self.benchmark_cache_performance(&test_workspaces).await?;
        results.extend(cache_results);

        // Benchmark 5: Concurrent operations (if enabled)
        if self.config.test_concurrency {
            info!("⚡ Benchmarking concurrent operations...");
            let concurrent_results = self
                .benchmark_concurrent_operations(&test_workspaces)
                .await?;
            results.extend(concurrent_results);
        }

        // Benchmark 6: Large file handling
        info!("📄 Benchmarking large file handling...");
        let large_file_results = self.benchmark_large_file_handling(&test_workspaces).await?;
        results.extend(large_file_results);

        info!("✅ All benchmarks completed");
        Ok(results)
    }

    async fn create_test_workspaces(&self) -> Result<HashMap<Language, TestWorkspace>> {
        let mut workspaces = HashMap::new();

        for &language in &self.config.languages {
            let workspace = TestWorkspace::create(language).await?;

            // Initialize LSP server for this workspace
            match timeout(
                Duration::from_secs(30),
                self.server_manager
                    .ensure_workspace_registered(language, workspace.root.clone()),
            )
            .await
            {
                Ok(Ok(_)) => {
                    info!("✅ Initialized {:?} LSP server for benchmarking", language);
                    workspaces.insert(language, workspace);
                }
                Ok(Err(e)) => {
                    warn!("❌ Failed to initialize {:?} LSP server: {}", language, e);
                }
                Err(_) => {
                    warn!("⏰ Timeout initializing {:?} LSP server", language);
                }
            }
        }

        Ok(workspaces)
    }

    async fn benchmark_basic_lsp_operations(
        &self,
        workspaces: &HashMap<Language, TestWorkspace>,
    ) -> Result<Vec<BenchmarkResult>> {
        let mut results = Vec::new();

        for (&language, workspace) in workspaces {
            // Benchmark references
            let ref_measurements = self
                .benchmark_operation(
                    format!("references_{:?}", language),
                    self.config.iterations,
                    || async {
                        self.lsp_client_wrapper
                            .get_references(
                                &workspace.main_file,
                                10,
                                5,
                                false,
                                self.config.timeout_ms,
                            )
                            .await
                            .is_ok()
                    },
                )
                .await;

            if !ref_measurements.0.is_empty() {
                results.push(BenchmarkResult::from_measurements(
                    format!("references_{:?}", language),
                    language,
                    &ref_measurements.0,
                    ref_measurements.1,
                ));
            }

            // Benchmark definitions
            let def_measurements = self
                .benchmark_operation(
                    format!("definition_{:?}", language),
                    self.config.iterations,
                    || async {
                        self.lsp_client_wrapper
                            .get_definition(&workspace.main_file, 10, 5, self.config.timeout_ms)
                            .await
                            .is_ok()
                    },
                )
                .await;

            if !def_measurements.0.is_empty() {
                results.push(BenchmarkResult::from_measurements(
                    format!("definition_{:?}", language),
                    language,
                    &def_measurements.0,
                    def_measurements.1,
                ));
            }

            // Benchmark hover
            let hover_measurements = self
                .benchmark_operation(
                    format!("hover_{:?}", language),
                    self.config.iterations,
                    || async {
                        self.lsp_client_wrapper
                            .get_hover(&workspace.main_file, 10, 5, self.config.timeout_ms)
                            .await
                            .is_ok()
                    },
                )
                .await;

            if !hover_measurements.0.is_empty() {
                results.push(BenchmarkResult::from_measurements(
                    format!("hover_{:?}", language),
                    language,
                    &hover_measurements.0,
                    hover_measurements.1,
                ));
            }
        }

        Ok(results)
    }

    async fn benchmark_call_hierarchy_operations(
        &self,
        workspaces: &HashMap<Language, TestWorkspace>,
    ) -> Result<Vec<BenchmarkResult>> {
        let mut results = Vec::new();

        for (&language, workspace) in workspaces {
            let measurements = self
                .benchmark_operation(
                    format!("call_hierarchy_{:?}", language),
                    self.config.iterations,
                    || async {
                        self.lsp_client_wrapper
                            .get_call_hierarchy(
                                &workspace.main_file,
                                15,
                                10, // Position of a function
                                self.config.timeout_ms,
                            )
                            .await
                            .is_ok()
                    },
                )
                .await;

            if !measurements.0.is_empty() {
                results.push(BenchmarkResult::from_measurements(
                    format!("call_hierarchy_{:?}", language),
                    language,
                    &measurements.0,
                    measurements.1,
                ));
            }
        }

        Ok(results)
    }

    async fn benchmark_relationship_enhancement(
        &self,
        workspaces: &HashMap<Language, TestWorkspace>,
    ) -> Result<Vec<BenchmarkResult>> {
        let mut results = Vec::new();

        for (&language, workspace) in workspaces {
            let mock_symbols = self.create_mock_symbols(&workspace.main_file, 10);
            let empty_relationships = Vec::new();
            let analysis_context = AnalysisContext::new(
                1,
                1,
                1,
                format!("{:?}", language).to_lowercase(),
                self.uid_generator.clone(),
            );

            let measurements = self
                .benchmark_operation(
                    format!("lsp_enhancement_{:?}", language),
                    self.config.iterations,
                    || {
                        let symbols = mock_symbols.clone();
                        let relationships = empty_relationships.clone();
                        let context = analysis_context.clone();
                        let file_path = workspace.main_file.clone();
                        async move {
                            self.lsp_enhancer
                                .enhance_relationships(
                                    &file_path,
                                    relationships,
                                    &symbols,
                                    &context,
                                )
                                .await
                                .is_ok()
                        }
                    },
                )
                .await;

            if !measurements.0.is_empty() {
                results.push(BenchmarkResult::from_measurements(
                    format!("lsp_enhancement_{:?}", language),
                    language,
                    &measurements.0,
                    measurements.1,
                ));
            }
        }

        Ok(results)
    }

    async fn benchmark_cache_performance(
        &self,
        workspaces: &HashMap<Language, TestWorkspace>,
    ) -> Result<Vec<BenchmarkResult>> {
        let mut results = Vec::new();

        for (&language, workspace) in workspaces {
            // First, warm up the cache
            if self.config.include_cache_warmup {
                debug!("Warming up cache for {:?}", language);
                for _ in 0..5 {
                    let _ = self
                        .lsp_client_wrapper
                        .get_references(&workspace.main_file, 10, 5, false, self.config.timeout_ms)
                        .await;
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }

            // Benchmark cached operations
            let cached_measurements = self
                .benchmark_operation(
                    format!("cached_references_{:?}", language),
                    self.config.iterations * 2, // More iterations for cache testing
                    || async {
                        self.lsp_client_wrapper
                            .get_references(
                                &workspace.main_file,
                                10,
                                5,
                                false,
                                self.config.timeout_ms,
                            )
                            .await
                            .is_ok()
                    },
                )
                .await;

            if !cached_measurements.0.is_empty() {
                results.push(BenchmarkResult::from_measurements(
                    format!("cached_references_{:?}", language),
                    language,
                    &cached_measurements.0,
                    cached_measurements.1,
                ));
            }
        }

        Ok(results)
    }

    async fn benchmark_concurrent_operations(
        &self,
        workspaces: &HashMap<Language, TestWorkspace>,
    ) -> Result<Vec<BenchmarkResult>> {
        let mut results = Vec::new();

        for (&language, workspace) in workspaces {
            let lsp_client = self.lsp_client_wrapper.clone();
            let timeout_ms = self.config.timeout_ms;
            let main_file = workspace.main_file.clone();
            let concurrent_measurements = self
                .benchmark_concurrent_operation(
                    format!("concurrent_references_{:?}", language),
                    self.config.max_concurrent_ops,
                    self.config.iterations,
                    move || {
                        let file = main_file.clone();
                        let lsp_client = lsp_client.clone();
                        async move {
                            lsp_client
                                .get_references(&file, 10, 5, false, timeout_ms)
                                .await
                                .is_ok()
                        }
                    },
                )
                .await;

            if !concurrent_measurements.0.is_empty() {
                results.push(BenchmarkResult::from_measurements(
                    format!("concurrent_references_{:?}", language),
                    language,
                    &concurrent_measurements.0,
                    concurrent_measurements.1,
                ));
            }
        }

        Ok(results)
    }

    async fn benchmark_large_file_handling(
        &self,
        workspaces: &HashMap<Language, TestWorkspace>,
    ) -> Result<Vec<BenchmarkResult>> {
        let mut results = Vec::new();

        for (&language, workspace) in workspaces {
            // Create a large file for testing
            let large_file = workspace.create_large_test_file(language, 1000).await?;

            let large_file_measurements = self
                .benchmark_operation(
                    format!("large_file_references_{:?}", language),
                    self.config.iterations / 2, // Fewer iterations for large files
                    || async {
                        self.lsp_client_wrapper
                            .get_references(
                                &large_file,
                                50,
                                10,
                                false,
                                self.config.timeout_ms * 2, // Double timeout for large files
                            )
                            .await
                            .is_ok()
                    },
                )
                .await;

            if !large_file_measurements.0.is_empty() {
                results.push(BenchmarkResult::from_measurements(
                    format!("large_file_references_{:?}", language),
                    language,
                    &large_file_measurements.0,
                    large_file_measurements.1,
                ));
            }
        }

        Ok(results)
    }

    /// Generic benchmark function for single operations
    async fn benchmark_operation<F, Fut>(
        &self,
        operation_name: String,
        iterations: usize,
        operation: F,
    ) -> (Vec<Duration>, usize)
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = bool>,
    {
        let mut measurements = Vec::new();
        let mut successes = 0;

        debug!(
            "📊 Benchmarking {} with {} iterations",
            operation_name, iterations
        );

        for i in 0..iterations {
            let start = Instant::now();
            let success = operation().await;
            let duration = start.elapsed();

            measurements.push(duration);
            if success {
                successes += 1;
            }

            if self.config.detailed_timing {
                debug!(
                    "  Iteration {}/{}: {:.2}ms ({})",
                    i + 1,
                    iterations,
                    duration.as_millis(),
                    if success { "✅" } else { "❌" }
                );
            }

            // Small delay to avoid overwhelming servers
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        debug!(
            "📈 {} completed: {}/{} successful, avg {:.2}ms",
            operation_name,
            successes,
            iterations,
            measurements.iter().sum::<Duration>().as_millis() / measurements.len().max(1) as u128
        );

        (measurements, successes)
    }

    /// Benchmark concurrent operations
    async fn benchmark_concurrent_operation<F, Fut>(
        &self,
        operation_name: String,
        concurrency: usize,
        total_operations: usize,
        operation: F,
    ) -> (Vec<Duration>, usize)
    where
        F: Fn() -> Fut + Clone + Send + 'static,
        Fut: std::future::Future<Output = bool> + Send,
    {
        debug!(
            "🔥 Benchmarking {} with {} concurrent operations",
            operation_name, concurrency
        );

        let start_time = Instant::now();
        let mut handles = Vec::new();
        let operations_per_task = total_operations / concurrency;

        for _task_id in 0..concurrency {
            let operation = operation.clone();
            let handle = tokio::spawn(async move {
                let mut task_measurements = Vec::new();
                let mut task_successes = 0;

                for _ in 0..operations_per_task {
                    let op_start = Instant::now();
                    let success = operation().await;
                    let op_duration = op_start.elapsed();

                    task_measurements.push(op_duration);
                    if success {
                        task_successes += 1;
                    }

                    tokio::time::sleep(Duration::from_millis(5)).await;
                }

                (task_measurements, task_successes)
            });
            handles.push(handle);
        }

        // Collect results from all tasks
        let mut all_measurements = Vec::new();
        let mut total_successes = 0;

        for handle in handles {
            if let Ok((measurements, successes)) = handle.await {
                all_measurements.extend(measurements);
                total_successes += successes;
            }
        }

        let total_duration = start_time.elapsed();
        let ops_per_second = all_measurements.len() as f64 / total_duration.as_secs_f64();

        debug!(
            "🚀 {} concurrent benchmark completed: {}/{} successful, {:.1} ops/sec",
            operation_name,
            total_successes,
            all_measurements.len(),
            ops_per_second
        );

        (all_measurements, total_successes)
    }

    fn create_mock_symbols(&self, file_path: &Path, count: usize) -> Vec<ExtractedSymbol> {
        let mut symbols = Vec::new();

        for i in 0..count {
            let symbol = ExtractedSymbol::new(
                format!("symbol_{}", i),
                format!("function_{}", i),
                SymbolKind::Function,
                SymbolLocation::new(
                    file_path.to_path_buf(),
                    i as u32 * 2 + 1,
                    0,
                    i as u32 * 2 + 2,
                    20,
                ),
            );
            symbols.push(symbol);
        }

        symbols
    }
}

/// Test workspace for benchmarking
struct TestWorkspace {
    root: PathBuf,
    main_file: PathBuf,
    _temp_dir: TempDir,
}

impl TestWorkspace {
    async fn create(language: Language) -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let root = temp_dir.path().to_path_buf();

        let (main_file, workspace_files) = match language {
            Language::Rust => Self::create_rust_workspace(&root)?,
            Language::Python => Self::create_python_workspace(&root)?,
            Language::Go => Self::create_go_workspace(&root)?,
            Language::TypeScript => Self::create_typescript_workspace(&root)?,
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported language for benchmarking: {:?}",
                    language
                ))
            }
        };

        // Write all workspace files
        for (path, content) in workspace_files {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, content)?;
        }

        Ok(Self {
            root,
            main_file,
            _temp_dir: temp_dir,
        })
    }

    fn create_rust_workspace(root: &Path) -> Result<(PathBuf, Vec<(PathBuf, String)>)> {
        let main_file = root.join("src/main.rs");
        let files = vec![
            (
                root.join("Cargo.toml"),
                r#"
[package]
name = "benchmark_project"
version = "0.1.0"
edition = "2021"
"#
                .to_string(),
            ),
            (main_file.clone(), Self::generate_rust_code(50)),
            (root.join("src/lib.rs"), Self::generate_rust_lib_code()),
        ];

        Ok((root.to_path_buf(), files))
    }

    fn create_python_workspace(root: &Path) -> Result<(PathBuf, Vec<(PathBuf, String)>)> {
        let main_file = root.join("main.py");
        let files = vec![
            (main_file.clone(), Self::generate_python_code(50)),
            (root.join("utils.py"), Self::generate_python_utils_code()),
        ];

        Ok((root.to_path_buf(), files))
    }

    fn create_go_workspace(root: &Path) -> Result<(PathBuf, Vec<(PathBuf, String)>)> {
        let main_file = root.join("main.go");
        let files = vec![
            (
                root.join("go.mod"),
                "module benchmark_project\n\ngo 1.19\n".to_string(),
            ),
            (main_file.clone(), Self::generate_go_code(50)),
        ];

        Ok((root.to_path_buf(), files))
    }

    fn create_typescript_workspace(root: &Path) -> Result<(PathBuf, Vec<(PathBuf, String)>)> {
        let main_file = root.join("src/main.ts");
        let files = vec![
            (
                root.join("package.json"),
                r#"
{
  "name": "benchmark_project",
  "version": "1.0.0",
  "main": "src/main.ts",
  "devDependencies": {
    "typescript": "^4.9.0"
  }
}
"#
                .to_string(),
            ),
            (
                root.join("tsconfig.json"),
                r#"
{
  "compilerOptions": {
    "target": "ES2020",
    "module": "commonjs",
    "outDir": "./dist",
    "rootDir": "./src",
    "strict": true
  }
}
"#
                .to_string(),
            ),
            (main_file.clone(), Self::generate_typescript_code(50)),
        ];

        Ok((root.to_path_buf(), files))
    }

    fn generate_rust_code(function_count: usize) -> String {
        let mut code = String::from("use std::collections::HashMap;\n\n");

        for i in 0..function_count {
            code.push_str(&format!(
                r#"
fn function_{}(x: i32) -> i32 {{
    let result = x * {} + 1;
    if result > 100 {{
        helper_function_{}(result)
    }} else {{
        result
    }}
}}

fn helper_function_{}(value: i32) -> i32 {{
    value / 2
}}

"#,
                i,
                i + 1,
                i,
                i
            ));
        }

        code.push_str(&format!(
            r#"
fn main() {{
    let mut results = HashMap::new();
    {}
    println!("Computed {{}} results", results.len());
}}
"#,
            (0..function_count)
                .map(|i| format!("    results.insert({}, function_{}({}));", i, i, i * 2))
                .collect::<Vec<_>>()
                .join("\n")
        ));

        code
    }

    fn generate_rust_lib_code() -> String {
        r#"
pub fn library_function(x: i32, y: i32) -> i32 {
    x + y
}

pub struct Calculator {
    pub name: String,
}

impl Calculator {
    pub fn new(name: String) -> Self {
        Self { name }
    }
    
    pub fn calculate(&self, a: i32, b: i32) -> i32 {
        library_function(a, b)
    }
}
"#
        .to_string()
    }

    fn generate_python_code(function_count: usize) -> String {
        let mut code = String::new();

        for i in 0..function_count {
            code.push_str(&format!(
                r#"
def function_{}(x: int) -> int:
    result = x * {} + 1
    if result > 100:
        return helper_function_{}(result)
    else:
        return result

def helper_function_{}(value: int) -> int:
    return value // 2

"#,
                i,
                i + 1,
                i,
                i
            ));
        }

        code.push_str(&format!(
            r#"
def main():
    results = {{}}
    {}
    print(f"Computed {{len(results)}} results")

if __name__ == "__main__":
    main()
"#,
            (0..function_count)
                .map(|i| format!("    results[{}] = function_{}({})", i, i, i * 2))
                .collect::<Vec<_>>()
                .join("\n")
        ));

        code
    }

    fn generate_python_utils_code() -> String {
        r#"
def utility_function(a: int, b: int) -> int:
    return a + b

class Calculator:
    def __init__(self, name: str):
        self.name = name
    
    def calculate(self, a: int, b: int) -> int:
        return utility_function(a, b)
"#
        .to_string()
    }

    fn generate_go_code(function_count: usize) -> String {
        let mut code = String::from("package main\n\nimport \"fmt\"\n\n");

        for i in 0..function_count {
            code.push_str(&format!(
                r#"
func function{}(x int) int {{
    result := x * {} + 1
    if result > 100 {{
        return helperFunction{}(result)
    }}
    return result
}}

func helperFunction{}(value int) int {{
    return value / 2
}}

"#,
                i,
                i + 1,
                i,
                i
            ));
        }

        code.push_str(&format!(
            r#"
func main() {{
    results := make(map[int]int)
    {}
    fmt.Printf("Computed %d results\n", len(results))
}}
"#,
            (0..function_count)
                .map(|i| format!("    results[{}] = function{}({})", i, i, i * 2))
                .collect::<Vec<_>>()
                .join("\n")
        ));

        code
    }

    fn generate_typescript_code(function_count: usize) -> String {
        let mut code = String::new();

        for i in 0..function_count {
            code.push_str(&format!(
                r#"
function function{}(x: number): number {{
    const result = x * {} + 1;
    if (result > 100) {{
        return helperFunction{}(result);
    }}
    return result;
}}

function helperFunction{}(value: number): number {{
    return Math.floor(value / 2);
}}

"#,
                i,
                i + 1,
                i,
                i
            ));
        }

        code.push_str(&format!(
            r#"
function main(): void {{
    const results = new Map<number, number>();
    {}
    console.log(`Computed ${{results.size}} results`);
}}

if (require.main === module) {{
    main();
}}
"#,
            (0..function_count)
                .map(|i| format!("    results.set({}, function{}({}));", i, i, i * 2))
                .collect::<Vec<_>>()
                .join("\n")
        ));

        code
    }

    async fn create_large_test_file(
        &self,
        language: Language,
        size_factor: usize,
    ) -> Result<PathBuf> {
        let large_file = match language {
            Language::Rust => self.root.join("src/large.rs"),
            Language::Python => self.root.join("large.py"),
            Language::Go => self.root.join("large.go"),
            Language::TypeScript => self.root.join("src/large.ts"),
            _ => self.root.join("large.txt"),
        };

        let content = match language {
            Language::Rust => Self::generate_rust_code(size_factor),
            Language::Python => Self::generate_python_code(size_factor),
            Language::Go => Self::generate_go_code(size_factor),
            Language::TypeScript => Self::generate_typescript_code(size_factor),
            _ => "// Large file content\n".repeat(size_factor),
        };

        if let Some(parent) = large_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&large_file, content)?;

        Ok(large_file)
    }
}

/// Print detailed benchmark results
pub fn print_benchmark_results(results: &[BenchmarkResult]) {
    println!("\n📊 LSP Performance Benchmark Results");
    println!("===================================");

    // Group results by language
    let mut by_language: HashMap<Language, Vec<&BenchmarkResult>> = HashMap::new();
    for result in results {
        by_language.entry(result.language).or_default().push(result);
    }

    for (language, lang_results) in by_language {
        println!("\n🔍 {:?} Language Server Performance:", language);
        println!("┌────────────────────────────┬─────────────┬─────────────┬─────────────┬─────────────┬───────────┬────────────┐");
        println!("│ Operation                  │ Avg (ms)    │ Min (ms)    │ Max (ms)    │ P95 (ms)    │ Success % │ Ops/sec    │");
        println!("├────────────────────────────┼─────────────┼─────────────┼─────────────┼─────────────┼───────────┼────────────┤");

        for result in lang_results {
            println!(
                "│ {:<26} │ {:>11.2} │ {:>11.2} │ {:>11.2} │ {:>11.2} │ {:>9.1} │ {:>10.1} │",
                result.operation,
                result.avg_duration.as_millis() as f64,
                result.min_duration.as_millis() as f64,
                result.max_duration.as_millis() as f64,
                result.percentile_95.as_millis() as f64,
                result.success_rate * 100.0,
                result.ops_per_second
            );
        }
        println!("└────────────────────────────┴─────────────┴─────────────┴─────────────┴─────────────┴───────────┴────────────┘");
    }

    // Summary statistics
    let total_operations: usize = results.iter().map(|r| r.total_operations).sum();
    let avg_success_rate =
        results.iter().map(|r| r.success_rate).sum::<f64>() / results.len() as f64;
    let fastest_operation = results.iter().min_by_key(|r| r.avg_duration);
    let slowest_operation = results.iter().max_by_key(|r| r.avg_duration);

    println!("\n📈 Summary Statistics:");
    println!("  Total operations benchmarked: {}", total_operations);
    println!("  Average success rate: {:.1}%", avg_success_rate * 100.0);

    if let Some(fastest) = fastest_operation {
        println!(
            "  Fastest operation: {} ({:.2}ms avg)",
            fastest.operation,
            fastest.avg_duration.as_millis()
        );
    }

    if let Some(slowest) = slowest_operation {
        println!(
            "  Slowest operation: {} ({:.2}ms avg)",
            slowest.operation,
            slowest.avg_duration.as_millis()
        );
    }
}

/// Main benchmark runner
#[tokio::test]
async fn run_lsp_performance_benchmarks() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("lsp_daemon=info,lsp_performance_benchmarks=debug")
        .with_test_writer()
        .init();

    let config = BenchmarkConfig {
        iterations: 5,           // Reduced for CI
        test_concurrency: false, // Disable concurrency tests in CI
        include_cache_warmup: true,
        ..Default::default()
    };

    let benchmark_suite = LspBenchmarkSuite::new(config).await?;
    let results = benchmark_suite.run_all_benchmarks().await?;

    print_benchmark_results(&results);

    // Assert performance requirements
    for result in &results {
        if result.operation.contains("basic") || result.operation.contains("references") {
            // Basic operations should complete within reasonable time
            assert!(
                result.avg_duration < Duration::from_secs(5),
                "Basic LSP operation {} took too long: {:.2}ms",
                result.operation,
                result.avg_duration.as_millis()
            );
        }

        // All operations should have some success
        assert!(
            result.success_rate > 0.0,
            "Operation {} had no successful executions",
            result.operation
        );
    }

    info!("✅ Performance benchmarks completed successfully!");
    Ok(())
}
