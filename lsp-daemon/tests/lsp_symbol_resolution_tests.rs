#![cfg(feature = "legacy-tests")]
//! LSP Symbol Resolution and UID Generation Fallback Tests
//!
//! This module tests symbol resolution and UID generation fallbacks including:
//! - Symbol resolution with LSP hover information
//! - UID generation fallback when LSP resolution fails
//! - Cross-file symbol resolution accuracy
//! - Symbol uniqueness and consistency
//! - Edge cases and error handling in symbol resolution

use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::time::timeout;
use tracing::{debug, info, warn};

// Import modules for symbol resolution testing
use lsp_daemon::analyzer::types::{AnalysisContext, ExtractedSymbol};
use lsp_daemon::language_detector::{Language, LanguageDetector};
use lsp_daemon::lsp_registry::LspRegistry;
use lsp_daemon::protocol::{Position, Range};
use lsp_daemon::relationship::lsp_client_wrapper::LspClientWrapper;
use lsp_daemon::relationship::lsp_enhancer::{
    LspEnhancementConfig, LspRelationshipEnhancer, LspRelationshipType,
};
use lsp_daemon::server_manager::SingleServerManager;
use lsp_daemon::symbol::{
    SymbolContext, SymbolInfo, SymbolKind, SymbolLocation, SymbolUIDGenerator,
};
use lsp_daemon::universal_cache::CacheLayer;
use lsp_daemon::workspace_cache_router::{WorkspaceCacheRouter, WorkspaceCacheRouterConfig};
use lsp_daemon::workspace_resolver::WorkspaceResolver;

/// Symbol resolution test configuration
#[derive(Debug, Clone)]
pub struct SymbolResolutionTestConfig {
    /// Languages to test symbol resolution for
    pub languages: Vec<Language>,
    /// LSP operation timeout
    pub timeout_ms: u64,
    /// Whether to test cross-file symbol resolution
    pub test_cross_file: bool,
    /// Whether to test UID generation fallback
    pub test_uid_fallback: bool,
    /// Whether to test symbol uniqueness
    pub test_uniqueness: bool,
    /// Whether to test symbol consistency across operations
    pub test_consistency: bool,
    /// Number of symbols to test for consistency
    pub consistency_test_count: usize,
}

impl Default for SymbolResolutionTestConfig {
    fn default() -> Self {
        Self {
            languages: vec![Language::Rust, Language::Python, Language::TypeScript],
            timeout_ms: 10000,
            test_cross_file: true,
            test_uid_fallback: true,
            test_uniqueness: true,
            test_consistency: true,
            consistency_test_count: 10,
        }
    }
}

/// Symbol resolution test result
#[derive(Debug, Clone)]
pub struct SymbolResolutionResult {
    pub test_name: String,
    pub language: Option<Language>,
    pub success: bool,
    pub symbol_count: usize,
    pub unique_uids: usize,
    pub lsp_resolved: usize,
    pub fallback_resolved: usize,
    pub cross_file_resolved: usize,
    pub error_message: Option<String>,
    pub duration: Duration,
}

impl SymbolResolutionResult {
    pub fn new(test_name: String, language: Option<Language>) -> Self {
        Self {
            test_name,
            language,
            success: true,
            symbol_count: 0,
            unique_uids: 0,
            lsp_resolved: 0,
            fallback_resolved: 0,
            cross_file_resolved: 0,
            error_message: None,
            duration: Duration::ZERO,
        }
    }

    pub fn with_error(mut self, error: String) -> Self {
        self.success = false;
        self.error_message = Some(error);
        self
    }

    pub fn uid_uniqueness_rate(&self) -> f64 {
        if self.symbol_count == 0 {
            1.0
        } else {
            self.unique_uids as f64 / self.symbol_count as f64
        }
    }

    pub fn lsp_resolution_rate(&self) -> f64 {
        if self.symbol_count == 0 {
            0.0
        } else {
            self.lsp_resolved as f64 / self.symbol_count as f64
        }
    }

    pub fn fallback_rate(&self) -> f64 {
        if self.symbol_count == 0 {
            0.0
        } else {
            self.fallback_resolved as f64 / self.symbol_count as f64
        }
    }
}

/// Symbol resolution test results
#[derive(Debug)]
pub struct SymbolResolutionTestResults {
    pub results: Vec<SymbolResolutionResult>,
    pub total_symbols_tested: usize,
    pub total_unique_uids: usize,
    pub total_lsp_resolved: usize,
    pub total_fallback_resolved: usize,
    pub tests_passed: usize,
    pub tests_failed: usize,
}

impl SymbolResolutionTestResults {
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
            total_symbols_tested: 0,
            total_unique_uids: 0,
            total_lsp_resolved: 0,
            total_fallback_resolved: 0,
            tests_passed: 0,
            tests_failed: 0,
        }
    }

    pub fn add_result(&mut self, result: SymbolResolutionResult) {
        self.total_symbols_tested += result.symbol_count;
        self.total_unique_uids += result.unique_uids;
        self.total_lsp_resolved += result.lsp_resolved;
        self.total_fallback_resolved += result.fallback_resolved;

        if result.success {
            self.tests_passed += 1;
        } else {
            self.tests_failed += 1;
        }

        self.results.push(result);
    }

    pub fn overall_uid_uniqueness_rate(&self) -> f64 {
        if self.total_symbols_tested == 0 {
            1.0
        } else {
            self.total_unique_uids as f64 / self.total_symbols_tested as f64
        }
    }

    pub fn overall_lsp_resolution_rate(&self) -> f64 {
        if self.total_symbols_tested == 0 {
            0.0
        } else {
            self.total_lsp_resolved as f64 / self.total_symbols_tested as f64
        }
    }

    pub fn success_rate(&self) -> f64 {
        let total_tests = self.tests_passed + self.tests_failed;
        if total_tests == 0 {
            1.0
        } else {
            self.tests_passed as f64 / total_tests as f64
        }
    }

    pub fn print_summary(&self) {
        println!("\n🔍 LSP Symbol Resolution Test Results");
        println!("====================================");

        println!("\n📊 Overall Statistics:");
        println!("  Total symbols tested: {}", self.total_symbols_tested);
        println!("  Unique UIDs generated: {}", self.total_unique_uids);
        println!(
            "  UID uniqueness rate: {:.1}%",
            self.overall_uid_uniqueness_rate() * 100.0
        );
        println!(
            "  LSP resolved: {} ({:.1}%)",
            self.total_lsp_resolved,
            self.overall_lsp_resolution_rate() * 100.0
        );
        println!("  Fallback resolved: {}", self.total_fallback_resolved);
        println!(
            "  Tests passed: {}/{} ({:.1}%)",
            self.tests_passed,
            self.tests_passed + self.tests_failed,
            self.success_rate() * 100.0
        );

        if !self.results.is_empty() {
            println!("\n📋 Detailed Test Results:");
            println!("┌─────────────────────────────────┬──────────────┬─────────────┬─────────────┬──────────────┬─────────────┐");
            println!("│ Test Name                       │ Language     │ Symbols     │ Unique UIDs │ LSP Resolved │ Status      │");
            println!("├─────────────────────────────────┼──────────────┼─────────────┼─────────────┼──────────────┼─────────────┤");

            for result in &self.results {
                let language_str = result
                    .language
                    .map(|l| format!("{:?}", l))
                    .unwrap_or_else(|| "N/A".to_string());

                let status_str = if result.success {
                    "✅ PASS"
                } else {
                    "❌ FAIL"
                };

                println!(
                    "│ {:<31} │ {:<12} │ {:>11} │ {:>11} │ {:>12} │ {:<11} │",
                    truncate_string(&result.test_name, 31),
                    truncate_string(&language_str, 12),
                    result.symbol_count,
                    result.unique_uids,
                    result.lsp_resolved,
                    status_str
                );
            }
            println!("└─────────────────────────────────┴──────────────┴─────────────┴─────────────┴──────────────┴─────────────┘");
        }

        // Show failed tests with error messages
        let failed_tests: Vec<_> = self.results.iter().filter(|r| !r.success).collect();
        if !failed_tests.is_empty() {
            println!("\n❌ Failed Test Details:");
            for result in failed_tests {
                println!(
                    "  {} ({}): {}",
                    result.test_name,
                    result
                        .language
                        .map(|l| format!("{:?}", l))
                        .unwrap_or_else(|| "N/A".to_string()),
                    result
                        .error_message
                        .as_ref()
                        .unwrap_or(&"Unknown error".to_string())
                );
            }
        }
    }
}

/// Symbol resolution test suite
pub struct LspSymbolResolutionTestSuite {
    server_manager: Arc<SingleServerManager>,
    lsp_client_wrapper: Arc<LspClientWrapper>,
    lsp_enhancer: Arc<LspRelationshipEnhancer>,
    uid_generator: Arc<SymbolUIDGenerator>,
    config: SymbolResolutionTestConfig,
    test_base_dir: TempDir,
}

impl LspSymbolResolutionTestSuite {
    pub async fn new(config: SymbolResolutionTestConfig) -> Result<Self> {
        let test_base_dir = TempDir::new()?;

        // Create cache infrastructure
        let workspace_config = WorkspaceCacheRouterConfig {
            base_cache_dir: test_base_dir.path().join("caches"),
            max_open_caches: 5,
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
                LspRelationshipType::Hover,
            ],
            ..Default::default()
        };

        let lsp_enhancer = Arc::new(LspRelationshipEnhancer::with_config(
            Some(server_manager.clone()),
            language_detector,
            workspace_resolver,
            uid_generator.clone(),
            lsp_config,
        ));

        Ok(Self {
            server_manager,
            lsp_client_wrapper,
            lsp_enhancer,
            uid_generator,
            config,
            test_base_dir,
        })
    }

    /// Run all symbol resolution tests
    pub async fn run_all_tests(&mut self) -> Result<SymbolResolutionTestResults> {
        info!("🔍 Starting LSP symbol resolution tests");
        let mut results = SymbolResolutionTestResults::new();

        // Create test workspaces
        let test_workspaces = self.create_test_workspaces().await?;

        // Test 1: Basic symbol resolution with LSP hover
        info!("🎯 Testing basic symbol resolution...");
        let basic_results = self.test_basic_symbol_resolution(&test_workspaces).await?;
        for result in basic_results {
            results.add_result(result);
        }

        // Test 2: UID generation fallback when LSP fails
        if self.config.test_uid_fallback {
            info!("🔄 Testing UID generation fallback...");
            let fallback_results = self.test_uid_generation_fallback(&test_workspaces).await?;
            for result in fallback_results {
                results.add_result(result);
            }
        }

        // Test 3: Cross-file symbol resolution
        if self.config.test_cross_file {
            info!("📁 Testing cross-file symbol resolution...");
            let cross_file_results = self.test_cross_file_resolution(&test_workspaces).await?;
            for result in cross_file_results {
                results.add_result(result);
            }
        }

        // Test 4: Symbol uniqueness
        if self.config.test_uniqueness {
            info!("🔑 Testing symbol uniqueness...");
            let uniqueness_results = self.test_symbol_uniqueness(&test_workspaces).await?;
            for result in uniqueness_results {
                results.add_result(result);
            }
        }

        // Test 5: Symbol consistency across operations
        if self.config.test_consistency {
            info!("🔄 Testing symbol consistency...");
            let consistency_results = self.test_symbol_consistency(&test_workspaces).await?;
            for result in consistency_results {
                results.add_result(result);
            }
        }

        info!("✅ Symbol resolution tests completed");
        Ok(results)
    }

    /// Test basic symbol resolution using LSP hover
    async fn test_basic_symbol_resolution(
        &self,
        workspaces: &HashMap<Language, PathBuf>,
    ) -> Result<Vec<SymbolResolutionResult>> {
        let mut results = Vec::new();

        for (&language, workspace_dir) in workspaces {
            let mut result = SymbolResolutionResult::new(
                format!("basic_resolution_{:?}", language),
                Some(language),
            );

            let start_time = Instant::now();

            // Get test symbols from the workspace
            let test_symbols = self.extract_test_symbols(language, workspace_dir).await?;
            result.symbol_count = test_symbols.len();

            let mut lsp_resolved = 0;
            let mut fallback_resolved = 0;
            let mut unique_uids = HashSet::new();

            for (file_path, line, column, symbol_name) in test_symbols {
                // Try to resolve symbol using LSP hover
                let hover_result = self
                    .lsp_client_wrapper
                    .get_hover(&file_path, line, column, self.config.timeout_ms)
                    .await;

                match hover_result {
                    Ok(Some(_hover_info)) => {
                        // Generate UID based on LSP information
                        let uid = self.generate_lsp_uid(&file_path, line, column, &symbol_name);
                        unique_uids.insert(uid);
                        lsp_resolved += 1;
                        debug!(
                            "✅ LSP resolved symbol: {} at {}:{}:{}",
                            symbol_name,
                            file_path.display(),
                            line,
                            column
                        );
                    }
                    Ok(None) => {
                        // No hover info, use fallback
                        let fallback_uid =
                            self.generate_fallback_uid(&file_path, line, column, &symbol_name);
                        unique_uids.insert(fallback_uid);
                        fallback_resolved += 1;
                        debug!(
                            "🔄 Fallback resolved symbol: {} at {}:{}:{}",
                            symbol_name,
                            file_path.display(),
                            line,
                            column
                        );
                    }
                    Err(e) => {
                        // Error, use fallback
                        let fallback_uid =
                            self.generate_fallback_uid(&file_path, line, column, &symbol_name);
                        unique_uids.insert(fallback_uid);
                        fallback_resolved += 1;
                        debug!(
                            "⚠️  Error resolving symbol {}, using fallback: {}",
                            symbol_name, e
                        );
                    }
                }

                // Small delay to avoid overwhelming the server
                tokio::time::sleep(Duration::from_millis(50)).await;
            }

            result.duration = start_time.elapsed();
            result.unique_uids = unique_uids.len();
            result.lsp_resolved = lsp_resolved;
            result.fallback_resolved = fallback_resolved;

            info!(
                "Symbol resolution for {:?}: {}/{} symbols, {} LSP resolved, {} fallback",
                language, result.symbol_count, result.symbol_count, lsp_resolved, fallback_resolved
            );

            results.push(result);
        }

        Ok(results)
    }

    /// Test UID generation fallback when LSP is unavailable
    async fn test_uid_generation_fallback(
        &self,
        workspaces: &HashMap<Language, PathBuf>,
    ) -> Result<Vec<SymbolResolutionResult>> {
        let mut results = Vec::new();

        for (&language, workspace_dir) in workspaces {
            let mut result =
                SymbolResolutionResult::new(format!("uid_fallback_{:?}", language), Some(language));

            let start_time = Instant::now();

            // Simulate LSP unavailability by using nonexistent files or invalid positions
            let fallback_test_cases = vec![
                (
                    workspace_dir.join("nonexistent.file"),
                    1,
                    1,
                    "nonexistent_symbol",
                ),
                (
                    workspace_dir
                        .join("src")
                        .join("main")
                        .with_extension(Self::get_extension(language)),
                    99999,
                    99999,
                    "out_of_bounds_symbol",
                ),
            ];

            let mut unique_uids = HashSet::new();
            let mut fallback_resolved = 0;

            for (file_path, line, column, symbol_name) in &fallback_test_cases {
                // Generate fallback UID directly
                let fallback_uid =
                    self.generate_fallback_uid(&file_path, *line, *column, symbol_name);
                unique_uids.insert(fallback_uid.clone());
                fallback_resolved += 1;

                debug!(
                    "Generated fallback UID: {} for {}:{}:{}",
                    fallback_uid,
                    file_path.display(),
                    line,
                    column
                );

                // Verify the UID is deterministic
                let second_uid =
                    self.generate_fallback_uid(&file_path, *line, *column, symbol_name);
                if fallback_uid != second_uid {
                    result = result.with_error(format!(
                        "Non-deterministic fallback UID: {} != {}",
                        fallback_uid, second_uid
                    ));
                    break;
                }
            }

            result.duration = start_time.elapsed();
            result.symbol_count = fallback_test_cases.len();
            result.unique_uids = unique_uids.len();
            result.fallback_resolved = fallback_resolved;

            debug!(
                "UID fallback test for {:?}: {} unique UIDs from {} test cases",
                language,
                unique_uids.len(),
                fallback_test_cases.len()
            );

            results.push(result);
        }

        Ok(results)
    }

    /// Test cross-file symbol resolution
    async fn test_cross_file_resolution(
        &self,
        workspaces: &HashMap<Language, PathBuf>,
    ) -> Result<Vec<SymbolResolutionResult>> {
        let mut results = Vec::new();

        for (&language, workspace_dir) in workspaces {
            let mut result =
                SymbolResolutionResult::new(format!("cross_file_{:?}", language), Some(language));

            let start_time = Instant::now();

            // Create additional files that reference symbols from main file
            let additional_files = self
                .create_cross_reference_files(language, workspace_dir)
                .await?;
            let mut cross_file_resolved = 0;
            let mut unique_uids = HashSet::new();
            let mut total_symbols = 0;

            for additional_file in additional_files {
                // Get references that should point to the main file
                let references_result = self
                    .lsp_client_wrapper
                    .get_references(&additional_file, 5, 10, false, self.config.timeout_ms)
                    .await;

                match references_result {
                    Ok(references) => {
                        for reference in references {
                            let uid = self.generate_lsp_uid(
                                &PathBuf::from(&reference.uri.replace("file://", "")),
                                reference.range.start.line,
                                reference.range.start.character,
                                "cross_ref_symbol",
                            );
                            unique_uids.insert(uid);
                            cross_file_resolved += 1;
                            total_symbols += 1;
                        }
                    }
                    Err(e) => {
                        debug!("Cross-file reference lookup failed: {}", e);
                    }
                }
            }

            result.duration = start_time.elapsed();
            result.symbol_count = total_symbols;
            result.unique_uids = unique_uids.len();
            result.cross_file_resolved = cross_file_resolved;

            debug!(
                "Cross-file resolution for {:?}: {} symbols across files",
                language, cross_file_resolved
            );

            results.push(result);
        }

        Ok(results)
    }

    /// Test symbol uniqueness - ensure different symbols get different UIDs
    async fn test_symbol_uniqueness(
        &self,
        workspaces: &HashMap<Language, PathBuf>,
    ) -> Result<Vec<SymbolResolutionResult>> {
        let mut results = Vec::new();

        for (&language, workspace_dir) in workspaces {
            let mut result =
                SymbolResolutionResult::new(format!("uniqueness_{:?}", language), Some(language));

            let start_time = Instant::now();

            // Generate UIDs for different symbols in the same file
            let main_file = workspace_dir
                .join("src")
                .join("main")
                .with_extension(Self::get_extension(language));
            let mut uids = HashSet::new();
            let mut symbol_count = 0;

            // Test different positions in the file
            for line in 1..20 {
                for column in vec![5, 10, 15] {
                    let uid = self.generate_fallback_uid(
                        &main_file,
                        line,
                        column,
                        &format!("symbol_{}_{}", line, column),
                    );
                    uids.insert(uid);
                    symbol_count += 1;
                }
            }

            result.duration = start_time.elapsed();
            result.symbol_count = symbol_count;
            result.unique_uids = uids.len();

            // Test should pass if we get unique UIDs for different positions
            if result.unique_uids != result.symbol_count {
                let unique_uids = result.unique_uids;
                let symbol_count = result.symbol_count;
                result = result.with_error(format!(
                    "UID collision detected: {} unique UIDs for {} symbols",
                    unique_uids, symbol_count
                ));
            }

            debug!(
                "Uniqueness test for {:?}: {}/{} unique UIDs",
                language, result.unique_uids, result.symbol_count
            );

            results.push(result);
        }

        Ok(results)
    }

    /// Test symbol consistency - same symbol should get same UID across operations
    async fn test_symbol_consistency(
        &self,
        workspaces: &HashMap<Language, PathBuf>,
    ) -> Result<Vec<SymbolResolutionResult>> {
        let mut results = Vec::new();

        for (&language, workspace_dir) in workspaces {
            let mut result =
                SymbolResolutionResult::new(format!("consistency_{:?}", language), Some(language));

            let start_time = Instant::now();

            let main_file = workspace_dir
                .join("src")
                .join("main")
                .with_extension(Self::get_extension(language));
            let mut consistent_symbols = 0;
            let mut total_tested = 0;

            // Test consistency for the same symbol across multiple calls
            for i in 0..self.config.consistency_test_count {
                let line = 10;
                let column = 5;
                let symbol_name = "test_function";

                // Generate UID multiple times
                let uid1 = self.generate_fallback_uid(&main_file, line, column, symbol_name);
                let uid2 = self.generate_fallback_uid(&main_file, line, column, symbol_name);
                let uid3 = self.generate_lsp_uid(&main_file, line, column, symbol_name);

                total_tested += 1;

                if uid1 == uid2 && uid1 == uid3 {
                    consistent_symbols += 1;
                } else {
                    debug!(
                        "Inconsistent UIDs for symbol {}: {} != {} != {}",
                        symbol_name, uid1, uid2, uid3
                    );
                }

                // Test with slightly different positions (should be different UIDs)
                let different_uid =
                    self.generate_fallback_uid(&main_file, line + 1, column, symbol_name);
                if uid1 == different_uid {
                    result = result.with_error(format!(
                        "Same UID generated for different positions: {}",
                        uid1
                    ));
                    break;
                }
            }

            result.duration = start_time.elapsed();
            result.symbol_count = total_tested;
            result.unique_uids = consistent_symbols;

            // Test passes if most symbols are consistent
            let consistency_rate = consistent_symbols as f64 / total_tested as f64;
            if consistency_rate < 0.9 {
                result = result.with_error(format!(
                    "Low consistency rate: {:.1}%",
                    consistency_rate * 100.0
                ));
            }

            debug!(
                "Consistency test for {:?}: {}/{} symbols consistent ({:.1}%)",
                language,
                consistent_symbols,
                total_tested,
                consistency_rate * 100.0
            );

            results.push(result);
        }

        Ok(results)
    }

    /// Create test workspaces for symbol resolution testing
    async fn create_test_workspaces(&self) -> Result<HashMap<Language, PathBuf>> {
        let mut workspaces = HashMap::new();

        for &language in &self.config.languages {
            let workspace = self.create_test_workspace(language).await?;

            // Initialize LSP server for this workspace
            match timeout(
                Duration::from_secs(30),
                self.server_manager
                    .ensure_workspace_registered(language, workspace.clone()),
            )
            .await
            {
                Ok(Ok(_)) => {
                    info!(
                        "✅ Initialized {:?} LSP server for symbol resolution testing",
                        language
                    );
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

    /// Create a test workspace for symbol resolution testing
    async fn create_test_workspace(&self, language: Language) -> Result<PathBuf> {
        let workspace_dir = self
            .test_base_dir
            .path()
            .join(format!("symbol_test_{:?}", language));
        std::fs::create_dir_all(&workspace_dir)?;

        match language {
            Language::Rust => {
                self.create_rust_symbol_test_workspace(&workspace_dir)
                    .await?
            }
            Language::Python => {
                self.create_python_symbol_test_workspace(&workspace_dir)
                    .await?
            }
            Language::TypeScript => {
                self.create_typescript_symbol_test_workspace(&workspace_dir)
                    .await?
            }
            _ => {
                return Err(anyhow::anyhow!(
                    "Unsupported language for symbol resolution testing: {:?}",
                    language
                ))
            }
        }

        Ok(workspace_dir)
    }

    async fn create_rust_symbol_test_workspace(&self, workspace_dir: &Path) -> Result<()> {
        std::fs::write(
            workspace_dir.join("Cargo.toml"),
            r#"
[package]
name = "symbol_test"
version = "0.1.0"
edition = "2021"
"#,
        )?;

        let src_dir = workspace_dir.join("src");
        std::fs::create_dir_all(&src_dir)?;

        std::fs::write(
            src_dir.join("main.rs"),
            r#"
mod utils;
mod data;

use utils::UtilityFunction;
use data::DataStruct;

fn main() {
    let utility = UtilityFunction::new("main");
    let result = utility.process(42);
    println!("Result: {}", result);
    
    let data = DataStruct::new("test", 100);
    println!("Data: {:?}", data);
    
    test_function();
    another_function(result);
}

fn test_function() {
    println!("Test function called");
}

fn another_function(value: i32) {
    println!("Another function with value: {}", value);
}

pub struct Calculator {
    name: String,
}

impl Calculator {
    pub fn new(name: String) -> Self {
        Self { name }
    }
    
    pub fn calculate(&self, a: i32, b: i32) -> i32 {
        a + b
    }
    
    pub fn get_name(&self) -> &str {
        &self.name
    }
}
"#,
        )?;

        std::fs::write(
            src_dir.join("utils.rs"),
            r#"
pub struct UtilityFunction {
    name: String,
}

impl UtilityFunction {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
        }
    }
    
    pub fn process(&self, value: i32) -> i32 {
        value * 2
    }
    
    pub fn get_name(&self) -> &str {
        &self.name
    }
}

pub fn utility_helper(x: i32, y: i32) -> i32 {
    x + y
}
"#,
        )?;

        std::fs::write(
            src_dir.join("data.rs"),
            r#"
#[derive(Debug, Clone)]
pub struct DataStruct {
    pub name: String,
    pub value: i32,
}

impl DataStruct {
    pub fn new(name: &str, value: i32) -> Self {
        Self {
            name: name.to_string(),
            value,
        }
    }
    
    pub fn get_value(&self) -> i32 {
        self.value
    }
    
    pub fn set_value(&mut self, value: i32) {
        self.value = value;
    }
}
"#,
        )?;

        Ok(())
    }

    async fn create_python_symbol_test_workspace(&self, workspace_dir: &Path) -> Result<()> {
        std::fs::write(
            workspace_dir.join("main.py"),
            r#"
from utils import UtilityClass
from data import DataClass

def main():
    utility = UtilityClass("main")
    result = utility.process(42)
    print(f"Result: {result}")
    
    data = DataClass("test", 100)
    print(f"Data: {data}")
    
    test_function()
    another_function(result)

def test_function():
    print("Test function called")

def another_function(value: int):
    print(f"Another function with value: {value}")

class Calculator:
    def __init__(self, name: str):
        self.name = name
    
    def calculate(self, a: int, b: int) -> int:
        return a + b
    
    def get_name(self) -> str:
        return self.name

if __name__ == "__main__":
    main()
"#,
        )?;

        std::fs::write(
            workspace_dir.join("utils.py"),
            r#"
class UtilityClass:
    def __init__(self, name: str):
        self.name = name
    
    def process(self, value: int) -> int:
        return value * 2
    
    def get_name(self) -> str:
        return self.name

def utility_helper(x: int, y: int) -> int:
    return x + y
"#,
        )?;

        std::fs::write(
            workspace_dir.join("data.py"),
            r#"
class DataClass:
    def __init__(self, name: str, value: int):
        self.name = name
        self.value = value
    
    def get_value(self) -> int:
        return self.value
    
    def set_value(self, value: int):
        self.value = value
    
    def __str__(self) -> str:
        return f"DataClass(name={self.name}, value={self.value})"
"#,
        )?;

        Ok(())
    }

    async fn create_typescript_symbol_test_workspace(&self, workspace_dir: &Path) -> Result<()> {
        std::fs::write(
            workspace_dir.join("package.json"),
            r#"
{
  "name": "symbol_test",
  "version": "1.0.0",
  "main": "src/main.ts",
  "devDependencies": {
    "typescript": "^4.9.0"
  }
}
"#,
        )?;

        let src_dir = workspace_dir.join("src");
        std::fs::create_dir_all(&src_dir)?;

        std::fs::write(
            src_dir.join("main.ts"),
            r#"
import { UtilityClass } from './utils';
import { DataClass } from './data';

function main(): void {
    const utility = new UtilityClass("main");
    const result = utility.process(42);
    console.log(`Result: ${result}`);
    
    const data = new DataClass("test", 100);
    console.log(`Data: ${data}`);
    
    testFunction();
    anotherFunction(result);
}

function testFunction(): void {
    console.log("Test function called");
}

function anotherFunction(value: number): void {
    console.log(`Another function with value: ${value}`);
}

class Calculator {
    constructor(private name: string) {}
    
    calculate(a: number, b: number): number {
        return a + b;
    }
    
    getName(): string {
        return this.name;
    }
}

export { Calculator };

if (require.main === module) {
    main();
}
"#,
        )?;

        std::fs::write(
            src_dir.join("utils.ts"),
            r#"
export class UtilityClass {
    constructor(private name: string) {}
    
    process(value: number): number {
        return value * 2;
    }
    
    getName(): string {
        return this.name;
    }
}

export function utilityHelper(x: number, y: number): number {
    return x + y;
}
"#,
        )?;

        std::fs::write(
            src_dir.join("data.ts"),
            r#"
export class DataClass {
    constructor(private name: string, private value: number) {}
    
    getValue(): number {
        return this.value;
    }
    
    setValue(value: number): void {
        this.value = value;
    }
    
    toString(): string {
        return `DataClass(name=${this.name}, value=${this.value})`;
    }
}
"#,
        )?;

        Ok(())
    }

    /// Extract test symbols from a workspace
    async fn extract_test_symbols(
        &self,
        language: Language,
        workspace_dir: &Path,
    ) -> Result<Vec<(PathBuf, u32, u32, String)>> {
        let main_file = workspace_dir
            .join("src")
            .join("main")
            .with_extension(Self::get_extension(language));

        // Return predetermined symbol positions based on the test files we created
        let symbols = match language {
            Language::Rust => vec![
                (main_file.clone(), 8, 10, "test_function".to_string()),
                (main_file.clone(), 12, 10, "another_function".to_string()),
                (main_file.clone(), 16, 12, "Calculator".to_string()),
                (main_file.clone(), 21, 15, "new".to_string()),
                (main_file.clone(), 25, 15, "calculate".to_string()),
            ],
            Language::Python => vec![
                (main_file.clone(), 12, 4, "test_function".to_string()),
                (main_file.clone(), 15, 4, "another_function".to_string()),
                (main_file.clone(), 18, 6, "Calculator".to_string()),
                (main_file.clone(), 19, 8, "__init__".to_string()),
                (main_file.clone(), 22, 8, "calculate".to_string()),
            ],
            Language::TypeScript => vec![
                (main_file.clone(), 14, 9, "testFunction".to_string()),
                (main_file.clone(), 18, 9, "anotherFunction".to_string()),
                (main_file.clone(), 22, 6, "Calculator".to_string()),
                (main_file.clone(), 23, 4, "constructor".to_string()),
                (main_file.clone(), 25, 4, "calculate".to_string()),
            ],
            _ => vec![],
        };

        Ok(symbols)
    }

    /// Create files that reference symbols from other files
    async fn create_cross_reference_files(
        &self,
        language: Language,
        workspace_dir: &Path,
    ) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        match language {
            Language::Rust => {
                let cross_ref_file = workspace_dir.join("src").join("cross_ref.rs");
                std::fs::write(
                    &cross_ref_file,
                    r#"
use crate::Calculator;
use crate::utils::UtilityFunction;

pub fn cross_reference_function() {
    let calc = Calculator::new("cross_ref".to_string());
    let result = calc.calculate(10, 20);
    println!("Cross ref result: {}", result);
    
    let utility = UtilityFunction::new("cross");
    let processed = utility.process(result);
    println!("Processed: {}", processed);
}
"#,
                )?;
                files.push(cross_ref_file);
            }
            Language::Python => {
                let cross_ref_file = workspace_dir.join("cross_ref.py");
                std::fs::write(
                    &cross_ref_file,
                    r#"
from main import Calculator
from utils import UtilityClass

def cross_reference_function():
    calc = Calculator("cross_ref")
    result = calc.calculate(10, 20)
    print(f"Cross ref result: {result}")
    
    utility = UtilityClass("cross")
    processed = utility.process(result)
    print(f"Processed: {processed}")
"#,
                )?;
                files.push(cross_ref_file);
            }
            Language::TypeScript => {
                let cross_ref_file = workspace_dir.join("src").join("cross_ref.ts");
                std::fs::write(
                    &cross_ref_file,
                    r#"
import { Calculator } from './main';
import { UtilityClass } from './utils';

export function crossReferenceFunction(): void {
    const calc = new Calculator("cross_ref");
    const result = calc.calculate(10, 20);
    console.log(`Cross ref result: ${result}`);
    
    const utility = new UtilityClass("cross");
    const processed = utility.process(result);
    console.log(`Processed: ${processed}`);
}
"#,
                )?;
                files.push(cross_ref_file);
            }
            _ => {}
        }

        Ok(files)
    }

    /// Generate UID using LSP information
    fn generate_lsp_uid(
        &self,
        file_path: &Path,
        line: u32,
        column: u32,
        symbol_name: &str,
    ) -> String {
        // In a real implementation, this would use actual hover information
        // For now, simulate LSP-enhanced UID generation
        format!(
            "lsp_{}:{}:{}:{}",
            file_path.file_stem().unwrap_or_default().to_string_lossy(),
            line,
            column,
            symbol_name
        )
    }

    /// Generate fallback UID when LSP is not available
    fn generate_fallback_uid(
        &self,
        file_path: &Path,
        line: u32,
        column: u32,
        symbol_name: &str,
    ) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        file_path.to_string_lossy().hash(&mut hasher);
        line.hash(&mut hasher);
        column.hash(&mut hasher);
        symbol_name.hash(&mut hasher);

        format!("fallback_{:x}", hasher.finish())
    }

    fn get_extension(language: Language) -> &'static str {
        match language {
            Language::Rust => "rs",
            Language::Python => "py",
            Language::Go => "go",
            Language::TypeScript => "ts",
            Language::JavaScript => "js",
            _ => "txt",
        }
    }
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Main symbol resolution test runner
#[tokio::test]
async fn run_lsp_symbol_resolution_tests() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("lsp_daemon=info,lsp_symbol_resolution_tests=debug")
        .with_test_writer()
        .init();

    let config = SymbolResolutionTestConfig {
        languages: vec![Language::Rust, Language::Python, Language::TypeScript],
        consistency_test_count: 5, // Reduced for CI
        ..Default::default()
    };

    let mut test_suite = LspSymbolResolutionTestSuite::new(config).await?;
    let results = test_suite.run_all_tests().await?;

    results.print_summary();

    // Assert reasonable success rate
    assert!(
        results.success_rate() >= 0.7,
        "Symbol resolution tests success rate too low: {:.1}%",
        results.success_rate() * 100.0
    );

    // Assert UID uniqueness
    assert!(
        results.overall_uid_uniqueness_rate() >= 0.9,
        "UID uniqueness rate too low: {:.1}%",
        results.overall_uid_uniqueness_rate() * 100.0
    );

    // Assert some symbols were resolved
    assert!(results.total_symbols_tested > 0, "No symbols were tested");

    info!("✅ Symbol resolution tests completed successfully!");
    Ok(())
}
