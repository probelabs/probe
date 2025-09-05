//! Hybrid Code Analyzer
#![allow(dead_code, clippy::all)]
//!
//! This module provides a hybrid analyzer that combines tree-sitter structural analysis
//! with LSP semantic analysis to provide comprehensive code understanding. It leverages
//! the strengths of both approaches to deliver high-quality analysis results.

use async_trait::async_trait;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use super::framework::{AnalyzerCapabilities, AnalyzerConfig, CodeAnalyzer};
use super::lsp_analyzer::{LspAnalyzer, MockLspAnalyzer};
use super::tree_sitter_analyzer::TreeSitterAnalyzer;
use super::types::*;
use crate::language_detector::LanguageDetector;
use crate::relationship::{
    HybridRelationshipMerger, LspEnhancementConfig, LspRelationshipEnhancer, MergeContext,
    MergerConfig,
};
use crate::server_manager::SingleServerManager;
use crate::symbol::SymbolUIDGenerator;
use crate::universal_cache::CacheLayer;
use crate::workspace_resolver::WorkspaceResolver;

/// Hybrid analyzer that combines structural and semantic analysis
///
/// This analyzer uses both tree-sitter for structural analysis and LSP for semantic
/// analysis, then merges the results to provide comprehensive symbol and relationship
/// information. It falls back gracefully when LSP is not available.
pub struct HybridAnalyzer {
    /// Tree-sitter analyzer for structural analysis
    structural_analyzer: TreeSitterAnalyzer,

    /// LSP analyzer for semantic analysis
    semantic_analyzer: Box<dyn CodeAnalyzer + Send + Sync>,

    /// LSP relationship enhancer for semantic relationship enhancement
    lsp_enhancer: Option<Arc<LspRelationshipEnhancer>>,

    /// Hybrid relationship merger for intelligent relationship combination
    relationship_merger: Arc<HybridRelationshipMerger>,

    /// UID generator for consistent symbol identification
    uid_generator: Arc<SymbolUIDGenerator>,

    /// Configuration for hybrid analysis
    config: HybridAnalyzerConfig,
}

/// Configuration for hybrid analyzer
#[derive(Debug, Clone)]
pub struct HybridAnalyzerConfig {
    /// Base analyzer configuration
    pub base: AnalyzerConfig,

    /// Whether to prefer LSP results over tree-sitter when available
    pub prefer_lsp_symbols: bool,

    /// Whether to merge relationships from both analyzers
    pub merge_relationships: bool,

    /// Whether to fall back to structural analysis if LSP fails
    pub fallback_to_structural: bool,

    /// Minimum confidence threshold for including relationships
    pub min_relationship_confidence: f32,

    /// Whether to enable relationship deduplication
    pub deduplicate_relationships: bool,

    /// Maximum time to wait for LSP analysis before falling back
    pub lsp_timeout_seconds: u64,

    /// LSP enhancement configuration
    pub lsp_enhancement: LspEnhancementConfig,

    /// Relationship merger configuration
    pub merger_config: MergerConfig,
}

impl Default for HybridAnalyzerConfig {
    fn default() -> Self {
        Self {
            base: AnalyzerConfig::default(),
            prefer_lsp_symbols: true,
            merge_relationships: true,
            fallback_to_structural: true,
            min_relationship_confidence: 0.5,
            deduplicate_relationships: true,
            lsp_timeout_seconds: 15,
            lsp_enhancement: LspEnhancementConfig::default(),
            merger_config: MergerConfig::default(),
        }
    }
}

impl HybridAnalyzerConfig {
    /// Create configuration optimized for accuracy
    pub fn accuracy() -> Self {
        let mut merger_config = MergerConfig::default();
        merger_config.confidence_threshold = 0.8;
        merger_config.strict_validation = true;

        Self {
            base: AnalyzerConfig::completeness(),
            prefer_lsp_symbols: true,
            merge_relationships: true,
            fallback_to_structural: false, // Don't fall back for maximum accuracy
            min_relationship_confidence: 0.8,
            deduplicate_relationships: true,
            lsp_timeout_seconds: 30,
            lsp_enhancement: LspEnhancementConfig::default(),
            merger_config,
        }
    }

    /// Create configuration optimized for speed
    pub fn performance() -> Self {
        use crate::relationship::{DeduplicationStrategy, MergeStrategy};

        let mut merger_config = MergerConfig::default();
        merger_config.merge_strategy = MergeStrategy::TreeSitterOnly;
        merger_config.deduplication_strategy = DeduplicationStrategy::Exact;
        merger_config.confidence_threshold = 0.3;
        merger_config.strict_validation = false;

        Self {
            base: AnalyzerConfig::performance(),
            prefer_lsp_symbols: false, // Use faster tree-sitter
            merge_relationships: false,
            fallback_to_structural: true,
            min_relationship_confidence: 0.3,
            deduplicate_relationships: false, // Skip for speed
            lsp_timeout_seconds: 5,
            lsp_enhancement: LspEnhancementConfig::default(),
            merger_config,
        }
    }
}

impl HybridAnalyzer {
    /// Create a new hybrid analyzer with LSP support
    pub fn new(
        uid_generator: Arc<SymbolUIDGenerator>,
        server_manager: Arc<SingleServerManager>,
        language_detector: Arc<LanguageDetector>,
        workspace_resolver: Arc<tokio::sync::Mutex<WorkspaceResolver>>,
        cache_layer: Arc<CacheLayer>,
    ) -> Self {
        let structural_analyzer = TreeSitterAnalyzer::new(uid_generator.clone());
        let semantic_analyzer = Box::new(LspAnalyzer::new(
            uid_generator.clone(),
            server_manager.clone(),
        ));

        // Create LSP relationship enhancer
        let lsp_enhancer = Some(Arc::new(LspRelationshipEnhancer::new(
            Some(server_manager),
            language_detector,
            workspace_resolver,
            cache_layer,
            uid_generator.clone(),
        )));

        // Create hybrid relationship merger with default configuration
        let merger_config = MergerConfig::default();
        let relationship_merger = Arc::new(HybridRelationshipMerger::new(merger_config));

        Self {
            structural_analyzer,
            semantic_analyzer,
            lsp_enhancer,
            relationship_merger,
            uid_generator,
            config: HybridAnalyzerConfig::default(),
        }
    }

    /// Create hybrid analyzer with mock LSP (for testing)
    pub fn with_mock_lsp(uid_generator: Arc<SymbolUIDGenerator>) -> Self {
        let structural_analyzer = TreeSitterAnalyzer::new(uid_generator.clone());
        let semantic_analyzer = Box::new(MockLspAnalyzer::new(uid_generator.clone()));

        // Create hybrid relationship merger with default configuration
        let merger_config = MergerConfig::default();
        let relationship_merger = Arc::new(HybridRelationshipMerger::new(merger_config));

        Self {
            structural_analyzer,
            semantic_analyzer,
            lsp_enhancer: None, // No LSP enhancer for mock
            relationship_merger,
            uid_generator,
            config: HybridAnalyzerConfig::default(),
        }
    }

    /// Create hybrid analyzer with custom configuration
    pub fn with_config(
        uid_generator: Arc<SymbolUIDGenerator>,
        server_manager: Arc<SingleServerManager>,
        language_detector: Arc<LanguageDetector>,
        workspace_resolver: Arc<tokio::sync::Mutex<WorkspaceResolver>>,
        cache_layer: Arc<CacheLayer>,
        config: HybridAnalyzerConfig,
    ) -> Self {
        let structural_analyzer = TreeSitterAnalyzer::new(uid_generator.clone());
        let semantic_analyzer = Box::new(LspAnalyzer::new(
            uid_generator.clone(),
            server_manager.clone(),
        ));

        // Create LSP relationship enhancer with custom configuration
        let lsp_enhancer = Some(Arc::new(LspRelationshipEnhancer::with_config(
            Some(server_manager),
            language_detector,
            workspace_resolver,
            cache_layer,
            uid_generator.clone(),
            config.lsp_enhancement.clone(),
        )));

        // Create hybrid relationship merger with custom configuration
        let relationship_merger =
            Arc::new(HybridRelationshipMerger::new(config.merger_config.clone()));

        Self {
            structural_analyzer,
            semantic_analyzer,
            lsp_enhancer,
            relationship_merger,
            uid_generator,
            config,
        }
    }

    /// Merge symbols from structural and semantic analysis
    fn merge_symbols(
        &self,
        structural_symbols: Vec<ExtractedSymbol>,
        semantic_symbols: Vec<ExtractedSymbol>,
    ) -> Vec<ExtractedSymbol> {
        if self.config.prefer_lsp_symbols && !semantic_symbols.is_empty() {
            // Use LSP symbols as primary source, supplement with structural symbols
            self.merge_symbols_lsp_preferred(structural_symbols, semantic_symbols)
        } else {
            // Use structural symbols as primary source, supplement with semantic symbols
            self.merge_symbols_structural_preferred(structural_symbols, semantic_symbols)
        }
    }

    /// Merge symbols preferring LSP results
    fn merge_symbols_lsp_preferred(
        &self,
        structural_symbols: Vec<ExtractedSymbol>,
        semantic_symbols: Vec<ExtractedSymbol>,
    ) -> Vec<ExtractedSymbol> {
        let mut merged_symbols = semantic_symbols;
        let semantic_names: HashSet<String> =
            merged_symbols.iter().map(|s| s.name.clone()).collect();

        // Add structural symbols that are not covered by LSP
        for structural_symbol in structural_symbols {
            if !semantic_names.contains(&structural_symbol.name) {
                merged_symbols.push(structural_symbol);
            }
        }

        merged_symbols
    }

    /// Merge symbols preferring structural results
    fn merge_symbols_structural_preferred(
        &self,
        structural_symbols: Vec<ExtractedSymbol>,
        semantic_symbols: Vec<ExtractedSymbol>,
    ) -> Vec<ExtractedSymbol> {
        let mut merged_symbols = structural_symbols;
        let structural_names: HashSet<String> =
            merged_symbols.iter().map(|s| s.name.clone()).collect();

        // Enhance structural symbols with semantic information
        for semantic_symbol in semantic_symbols {
            if let Some(existing_symbol) = merged_symbols
                .iter_mut()
                .find(|s| s.name == semantic_symbol.name)
            {
                // Merge metadata and improve existing symbol
                existing_symbol.metadata.extend(semantic_symbol.metadata);

                // Prefer semantic signature if available
                if semantic_symbol.signature.is_some() {
                    existing_symbol.signature = semantic_symbol.signature;
                }

                // Prefer semantic qualified name if available
                if semantic_symbol.qualified_name.is_some() {
                    existing_symbol.qualified_name = semantic_symbol.qualified_name;
                }
            } else if !structural_names.contains(&semantic_symbol.name) {
                // Add semantic-only symbols
                merged_symbols.push(semantic_symbol);
            }
        }

        merged_symbols
    }

    /// Merge relationships from both analyzers using the sophisticated merger
    async fn merge_relationships(
        &self,
        structural_relationships: Vec<ExtractedRelationship>,
        semantic_relationships: Vec<ExtractedRelationship>,
        file_path: &Path,
        language: &str,
        context: &AnalysisContext,
    ) -> Vec<ExtractedRelationship> {
        if !self.config.merge_relationships {
            // Return only semantic relationships if available, otherwise structural
            if !semantic_relationships.is_empty() {
                return self.filter_relationships_by_confidence(semantic_relationships);
            } else {
                return self.filter_relationships_by_confidence(structural_relationships);
            }
        }

        // Create merge context
        let merge_context = MergeContext::new(
            context.workspace_id,
            file_path.to_path_buf(),
            language.to_string(),
        );

        // Use the sophisticated hybrid relationship merger
        match self
            .relationship_merger
            .merge_relationships(
                structural_relationships.clone(),
                semantic_relationships.clone(),
                &merge_context,
            )
            .await
        {
            Ok(merged) => {
                tracing::info!(
                    "Successfully merged {} tree-sitter + {} LSP relationships into {} final relationships",
                    structural_relationships.len(),
                    semantic_relationships.len(),
                    merged.len()
                );
                merged
            }
            Err(e) => {
                tracing::warn!(
                    "Relationship merging failed: {}, falling back to basic merge",
                    e
                );
                // Fallback to basic merge
                self.basic_merge_relationships(structural_relationships, semantic_relationships)
            }
        }
    }

    /// Basic fallback merge (used when sophisticated merger fails)
    fn basic_merge_relationships(
        &self,
        structural_relationships: Vec<ExtractedRelationship>,
        semantic_relationships: Vec<ExtractedRelationship>,
    ) -> Vec<ExtractedRelationship> {
        let mut all_relationships = structural_relationships;
        all_relationships.extend(semantic_relationships);

        // Filter by confidence
        let filtered = self.filter_relationships_by_confidence(all_relationships);

        // Deduplicate if enabled
        if self.config.deduplicate_relationships {
            self.deduplicate_relationships(filtered)
        } else {
            filtered
        }
    }

    /// Filter relationships by minimum confidence threshold
    fn filter_relationships_by_confidence(
        &self,
        relationships: Vec<ExtractedRelationship>,
    ) -> Vec<ExtractedRelationship> {
        relationships
            .into_iter()
            .filter(|rel| rel.confidence >= self.config.min_relationship_confidence)
            .collect()
    }

    /// Remove duplicate relationships
    fn deduplicate_relationships(
        &self,
        relationships: Vec<ExtractedRelationship>,
    ) -> Vec<ExtractedRelationship> {
        let mut seen = HashSet::new();
        let mut deduplicated = Vec::new();

        for relationship in relationships {
            // Create a deduplication key based on source, target, and relation type
            let key = (
                relationship.source_symbol_uid.clone(),
                relationship.target_symbol_uid.clone(),
                relationship.relation_type,
            );

            if !seen.contains(&key) {
                seen.insert(key);
                deduplicated.push(relationship);
            } else {
                // If we've seen this relationship, keep the one with higher confidence
                if let Some(existing) = deduplicated.iter_mut().find(|r| {
                    r.source_symbol_uid == relationship.source_symbol_uid
                        && r.target_symbol_uid == relationship.target_symbol_uid
                        && r.relation_type == relationship.relation_type
                }) {
                    if relationship.confidence > existing.confidence {
                        *existing = relationship;
                    }
                }
            }
        }

        deduplicated
    }

    /// Create analysis metadata for hybrid analysis
    fn create_hybrid_metadata(
        &self,
        structural_metadata: AnalysisMetadata,
        semantic_metadata: Option<AnalysisMetadata>,
        total_duration_ms: u64,
        analysis_strategy: &str,
    ) -> AnalysisMetadata {
        let mut metadata = AnalysisMetadata::new("HybridAnalyzer".to_string(), "1.0.0".to_string());

        metadata.duration_ms = total_duration_ms;
        metadata.add_metric(
            "analysis_strategy".to_string(),
            serde_json::Value::String(analysis_strategy.to_string())
                .as_f64()
                .unwrap_or(0.0),
        );

        // Merge structural metadata
        metadata.add_metric(
            "structural_duration_ms".to_string(),
            structural_metadata.duration_ms as f64,
        );
        metadata.metrics.extend(
            structural_metadata
                .metrics
                .into_iter()
                .map(|(k, v)| (format!("structural_{}", k), v)),
        );
        metadata.warnings.extend(structural_metadata.warnings);

        // Merge semantic metadata if available
        if let Some(semantic_metadata) = semantic_metadata {
            metadata.add_metric(
                "semantic_duration_ms".to_string(),
                semantic_metadata.duration_ms as f64,
            );
            metadata.metrics.extend(
                semantic_metadata
                    .metrics
                    .into_iter()
                    .map(|(k, v)| (format!("semantic_{}", k), v)),
            );
            metadata.warnings.extend(semantic_metadata.warnings);
        }

        metadata
    }
}

#[async_trait]
impl CodeAnalyzer for HybridAnalyzer {
    fn capabilities(&self) -> AnalyzerCapabilities {
        AnalyzerCapabilities::hybrid()
    }

    fn supported_languages(&self) -> Vec<String> {
        // Return union of supported languages from both analyzers
        let mut languages = self.structural_analyzer.supported_languages();
        languages.extend(self.semantic_analyzer.supported_languages());
        languages.sort();
        languages.dedup();
        languages
    }

    async fn analyze_file(
        &self,
        content: &str,
        file_path: &Path,
        language: &str,
        context: &AnalysisContext,
    ) -> Result<AnalysisResult, AnalysisError> {
        let start_time = std::time::Instant::now();

        // Always run structural analysis (it's fast and reliable)
        let structural_result = self
            .structural_analyzer
            .analyze_file(content, file_path, language, context)
            .await;

        // Try semantic analysis with timeout
        let semantic_result = if self.semantic_analyzer.can_analyze_language(language) {
            tokio::time::timeout(
                tokio::time::Duration::from_secs(self.config.lsp_timeout_seconds),
                self.semantic_analyzer
                    .analyze_file(content, file_path, language, context),
            )
            .await
            .map_err(|_| AnalysisError::Timeout {
                file: file_path.to_string_lossy().to_string(),
                timeout_seconds: self.config.lsp_timeout_seconds,
            })
        } else {
            Err(AnalysisError::ConfigError {
                message: format!("Semantic analyzer does not support language: {}", language),
            })
        };

        // Early return if structural analysis fails (required for hybrid)
        if structural_result.is_err() {
            return structural_result;
        }

        let analysis_strategy = match &semantic_result {
            Ok(Ok(_)) => "hybrid",
            _ if self.config.fallback_to_structural => "structural_fallback",
            _ => "structural_only",
        };

        // Merge results based on what succeeded
        let (merged_symbols, merged_relationships, hybrid_metadata) = match analysis_strategy {
            "hybrid" => {
                let struct_result = structural_result?;
                let semantic_result = semantic_result.unwrap()?;

                let merged_symbols =
                    self.merge_symbols(struct_result.symbols, semantic_result.symbols);

                let merged_relationships = self
                    .merge_relationships(
                        struct_result.relationships,
                        semantic_result.relationships,
                        file_path,
                        language,
                        context,
                    )
                    .await;

                // Apply LSP relationship enhancement if available
                let enhanced_relationships = if let Some(ref lsp_enhancer) = self.lsp_enhancer {
                    lsp_enhancer
                        .enhance_relationships(
                            file_path,
                            merged_relationships.clone(),
                            &merged_symbols,
                            context,
                        )
                        .await
                        .unwrap_or_else(|e| {
                            tracing::warn!("LSP relationship enhancement failed: {}", e);
                            merged_relationships
                        })
                } else {
                    merged_relationships
                };

                let metadata = self.create_hybrid_metadata(
                    struct_result.analysis_metadata,
                    Some(semantic_result.analysis_metadata),
                    start_time.elapsed().as_millis() as u64,
                    "hybrid",
                );

                (merged_symbols, enhanced_relationships, metadata)
            }
            "structural_fallback" | "structural_only" => {
                let struct_result = structural_result?;

                // Apply LSP relationship enhancement to structural results as well
                let enhanced_relationships = if let Some(ref lsp_enhancer) = self.lsp_enhancer {
                    lsp_enhancer
                        .enhance_relationships(
                            file_path,
                            struct_result.relationships.clone(),
                            &struct_result.symbols,
                            context,
                        )
                        .await
                        .unwrap_or_else(|e| {
                            tracing::warn!(
                                "LSP relationship enhancement failed in fallback mode: {}",
                                e
                            );
                            struct_result.relationships
                        })
                } else {
                    struct_result.relationships
                };

                let metadata = self.create_hybrid_metadata(
                    struct_result.analysis_metadata,
                    None,
                    start_time.elapsed().as_millis() as u64,
                    analysis_strategy,
                );

                (struct_result.symbols, enhanced_relationships, metadata)
            }
            "semantic_only" => {
                let semantic_result = semantic_result.unwrap()?;

                let metadata = self.create_hybrid_metadata(
                    AnalysisMetadata::default(),
                    Some(semantic_result.analysis_metadata),
                    start_time.elapsed().as_millis() as u64,
                    "semantic_only",
                );

                (
                    semantic_result.symbols,
                    semantic_result.relationships,
                    metadata,
                )
            }
            _ => unreachable!(),
        };

        // Create final result
        let mut result = AnalysisResult::new(file_path.to_path_buf(), language.to_string());

        for symbol in merged_symbols {
            result.add_symbol(symbol);
        }

        for relationship in merged_relationships {
            result.add_relationship(relationship);
        }

        result.analysis_metadata = hybrid_metadata;

        // Add strategy information to metadata
        result.analysis_metadata.custom.insert(
            "analysis_strategy".to_string(),
            serde_json::Value::String(analysis_strategy.to_string()),
        );

        Ok(result)
    }

    async fn analyze_incremental(
        &self,
        content: &str,
        file_path: &Path,
        language: &str,
        previous_result: Option<&AnalysisResult>,
        context: &AnalysisContext,
    ) -> Result<AnalysisResult, AnalysisError> {
        // For hybrid analysis, we can potentially be smarter about incremental updates
        // For now, we'll delegate to the semantic analyzer if it supports incremental,
        // otherwise fall back to full analysis

        if self.semantic_analyzer.capabilities().supports_incremental {
            // Try incremental semantic analysis first
            match self
                .semantic_analyzer
                .analyze_incremental(content, file_path, language, previous_result, context)
                .await
            {
                Ok(result) => {
                    // Enhance with structural analysis if needed
                    if result.symbols.is_empty() {
                        // Semantic analysis didn't find much, supplement with structural
                        let structural_result = self
                            .structural_analyzer
                            .analyze_file(content, file_path, language, context)
                            .await?;

                        let mut enhanced_result = result;
                        enhanced_result.symbols.extend(structural_result.symbols);
                        enhanced_result
                            .relationships
                            .extend(structural_result.relationships);

                        return Ok(enhanced_result);
                    }
                    Ok(result)
                }
                Err(_) => {
                    // Fall back to full hybrid analysis
                    self.analyze_file(content, file_path, language, context)
                        .await
                }
            }
        } else {
            // Full re-analysis
            self.analyze_file(content, file_path, language, context)
                .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::{SymbolKind, SymbolLocation, SymbolUIDGenerator};
    use std::path::PathBuf;

    fn create_test_analyzer() -> HybridAnalyzer {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        HybridAnalyzer::with_mock_lsp(uid_generator)
    }

    fn create_test_context() -> AnalysisContext {
        let uid_generator = Arc::new(SymbolUIDGenerator::new());
        AnalysisContext::new(1, 2, 3, "rust".to_string(), uid_generator)
    }

    fn create_test_symbol(name: &str, kind: SymbolKind) -> ExtractedSymbol {
        let location = SymbolLocation::new(PathBuf::from("test.rs"), 1, 0, 1, 10);
        ExtractedSymbol::new(format!("test::{}", name), name.to_string(), kind, location)
    }

    #[test]
    fn test_hybrid_analyzer_capabilities() {
        let analyzer = create_test_analyzer();
        let caps = analyzer.capabilities();

        assert!(caps.extracts_symbols);
        assert!(caps.extracts_relationships);
        assert!(caps.supports_incremental);
        assert!(caps.requires_lsp);
        assert!(!caps.parallel_safe);
        assert_eq!(caps.confidence, 0.98);
    }

    #[test]
    fn test_hybrid_analyzer_supported_languages() {
        let analyzer = create_test_analyzer();
        let languages = analyzer.supported_languages();

        // Should include languages from both analyzers
        assert!(languages.contains(&"mock".to_string()));
    }

    #[test]
    fn test_merge_symbols_lsp_preferred() {
        let analyzer = create_test_analyzer();

        let structural_symbols = vec![
            create_test_symbol("func1", SymbolKind::Function),
            create_test_symbol("func2", SymbolKind::Function),
        ];

        let semantic_symbols = vec![
            create_test_symbol("func1", SymbolKind::Function), // Overlapping
            create_test_symbol("class1", SymbolKind::Class),   // LSP only
        ];

        let merged = analyzer.merge_symbols_lsp_preferred(structural_symbols, semantic_symbols);

        // Should have 3 symbols: func1 (from LSP), class1 (from LSP), func2 (from structural)
        assert_eq!(merged.len(), 3);
        assert!(merged.iter().any(|s| s.name == "func1"));
        assert!(merged.iter().any(|s| s.name == "func2"));
        assert!(merged.iter().any(|s| s.name == "class1"));
    }

    #[test]
    fn test_merge_symbols_structural_preferred() {
        let analyzer = create_test_analyzer();

        let structural_symbols = vec![
            create_test_symbol("func1", SymbolKind::Function),
            create_test_symbol("func2", SymbolKind::Function),
        ];

        let semantic_symbols = vec![
            create_test_symbol("func1", SymbolKind::Function), // Overlapping
            create_test_symbol("class1", SymbolKind::Class),   // LSP only
        ];

        let merged =
            analyzer.merge_symbols_structural_preferred(structural_symbols, semantic_symbols);

        // Should have 3 symbols: func1 (enhanced), func2 (structural), class1 (semantic)
        assert_eq!(merged.len(), 3);
        assert!(merged.iter().any(|s| s.name == "func1"));
        assert!(merged.iter().any(|s| s.name == "func2"));
        assert!(merged.iter().any(|s| s.name == "class1"));
    }

    #[test]
    fn test_deduplicate_relationships() {
        let analyzer = create_test_analyzer();

        let relationships = vec![
            ExtractedRelationship::new(
                "source1".to_string(),
                "target1".to_string(),
                RelationType::Calls,
            )
            .with_confidence(0.8),
            ExtractedRelationship::new(
                "source1".to_string(),
                "target1".to_string(),
                RelationType::Calls, // Duplicate
            )
            .with_confidence(0.9), // Higher confidence
            ExtractedRelationship::new(
                "source2".to_string(),
                "target2".to_string(),
                RelationType::References,
            )
            .with_confidence(0.7),
        ];

        let deduplicated = analyzer.deduplicate_relationships(relationships);

        // Should have 2 relationships, with the higher confidence one kept
        assert_eq!(deduplicated.len(), 2);
        let calls_rel = deduplicated
            .iter()
            .find(|r| r.relation_type == RelationType::Calls)
            .unwrap();
        assert_eq!(calls_rel.confidence, 0.9);
    }

    #[test]
    fn test_filter_relationships_by_confidence() {
        let analyzer = HybridAnalyzer {
            config: HybridAnalyzerConfig {
                min_relationship_confidence: 0.7,
                ..Default::default()
            },
            ..create_test_analyzer()
        };

        let relationships = vec![
            ExtractedRelationship::new(
                "source1".to_string(),
                "target1".to_string(),
                RelationType::Calls,
            )
            .with_confidence(0.9), // Above threshold
            ExtractedRelationship::new(
                "source2".to_string(),
                "target2".to_string(),
                RelationType::References,
            )
            .with_confidence(0.5), // Below threshold
            ExtractedRelationship::new(
                "source3".to_string(),
                "target3".to_string(),
                RelationType::Calls,
            )
            .with_confidence(0.8), // Above threshold
        ];

        let filtered = analyzer.filter_relationships_by_confidence(relationships);

        // Should keep only relationships with confidence >= 0.7
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|r| r.confidence >= 0.7));
    }

    #[test]
    fn test_hybrid_config_presets() {
        let accuracy_config = HybridAnalyzerConfig::accuracy();
        assert!(accuracy_config.prefer_lsp_symbols);
        assert!(!accuracy_config.fallback_to_structural);
        assert_eq!(accuracy_config.min_relationship_confidence, 0.8);

        let performance_config = HybridAnalyzerConfig::performance();
        assert!(!performance_config.prefer_lsp_symbols);
        assert!(!performance_config.merge_relationships);
        assert_eq!(performance_config.lsp_timeout_seconds, 5);
    }

    #[tokio::test]
    async fn test_analyze_file() {
        let analyzer = create_test_analyzer();
        let context = create_test_context();
        let file_path = PathBuf::from("test.mock");

        let result = analyzer
            .analyze_file("test content", &file_path, "mock", &context)
            .await;
        assert!(result.is_ok());

        let analysis_result = result.unwrap();
        assert_eq!(analysis_result.file_path, file_path);
        assert_eq!(analysis_result.language, "mock");
        assert_eq!(
            analysis_result.analysis_metadata.analyzer_name,
            "HybridAnalyzer"
        );

        // Check that strategy was recorded
        assert!(analysis_result
            .analysis_metadata
            .custom
            .contains_key("analysis_strategy"));
    }
}
