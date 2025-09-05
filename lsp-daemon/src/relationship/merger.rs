//! Hybrid Relationship Merger
//!
//! This module provides comprehensive hybrid relationship merging that intelligently combines
//! Tree-sitter structural relationships with LSP semantic relationships, resolving conflicts
//! and providing unified relationship data.
//!
//! # Architecture
//!
//! The merger uses a multi-stage approach:
//! 1. **Preprocessing** - Normalize and validate input relationships
//! 2. **Conflict Detection** - Identify overlapping or contradictory relationships
//! 3. **Conflict Resolution** - Apply configured resolution strategies
//! 4. **Deduplication** - Remove duplicate relationships using various strategies
//! 5. **Confidence Calculation** - Assign final confidence scores
//! 6. **Metadata Merging** - Combine metadata from multiple sources
//!
//! # Merge Strategies
//!
//! - **LspPreferred**: Use LSP when available, fallback to tree-sitter
//! - **Complementary**: Use tree-sitter for structure, LSP for semantics
//! - **WeightedCombination**: Combine both sources using confidence weighting
//! - **LspOnly**: Only use LSP relationships
//! - **TreeSitterOnly**: Only use tree-sitter relationships
//!
//! # Conflict Resolution
//!
//! Multiple conflict resolution strategies:
//! - **HighestConfidence**: Keep relationship with highest confidence score
//! - **PreferLsp**: LSP relationships win conflicts
//! - **PreferTreeSitter**: Tree-sitter relationships win conflicts
//! - **KeepAll**: Maintain all relationships with conflict metadata
//! - **Custom**: Use custom resolution logic

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use thiserror::Error;
use tracing::{debug, error, info, warn};

use crate::analyzer::types::{ExtractedRelationship, RelationType};

/// Errors that can occur during relationship merging
#[derive(Debug, Error)]
pub enum MergeError {
    #[error("Conflict resolution failed: {message}")]
    ConflictResolutionFailed { message: String },

    #[error("Invalid merge configuration: {message}")]
    InvalidConfiguration { message: String },

    #[error("Deduplication strategy failed: {strategy} - {error}")]
    DeduplicationFailed { strategy: String, error: String },

    #[error("Confidence calculation failed: {message}")]
    ConfidenceCalculationFailed { message: String },

    #[error("Metadata merging failed: {message}")]
    MetadataMergingFailed { message: String },

    #[error("Validation error: {message}")]
    ValidationError { message: String },

    #[error("Internal merge error: {message}")]
    InternalError { message: String },
}

/// Context information for relationship merging
#[derive(Debug, Clone)]
pub struct MergeContext {
    /// Workspace identifier
    pub workspace_id: i64,

    /// File being analyzed
    pub file_path: PathBuf,

    /// Programming language
    pub language: String,

    /// Analysis timestamp
    pub analysis_timestamp: SystemTime,

    /// Additional context metadata
    pub metadata: HashMap<String, String>,
}

impl MergeContext {
    /// Create a new merge context
    pub fn new(workspace_id: i64, file_path: PathBuf, language: String) -> Self {
        Self {
            workspace_id,
            file_path,
            language,
            analysis_timestamp: SystemTime::now(),
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the context
    pub fn with_metadata(mut self, key: String, value: String) -> Self {
        self.metadata.insert(key, value);
        self
    }
}

/// Configuration for the hybrid relationship merger
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergerConfig {
    /// Primary merge strategy
    pub merge_strategy: MergeStrategy,

    /// Conflict resolution strategy
    pub conflict_resolution: ConflictResolution,

    /// Deduplication strategy
    pub deduplication_strategy: DeduplicationStrategy,

    /// Minimum confidence threshold for including relationships
    pub confidence_threshold: f32,

    /// Maximum relationships per symbol to prevent explosion
    pub max_relationships_per_symbol: usize,

    /// Whether to merge relationship metadata
    pub enable_metadata_merging: bool,

    /// Whether to enable confidence boosting based on multiple sources
    pub enable_confidence_boosting: bool,

    /// Source weights for confidence calculation
    pub source_weights: HashMap<RelationshipSource, f32>,

    /// Relation type modifiers for confidence
    pub relation_type_modifiers: HashMap<RelationType, f32>,

    /// Location accuracy bonus for confidence
    pub location_accuracy_bonus: f32,

    /// Validation settings
    pub strict_validation: bool,

    /// Performance optimization settings
    pub max_concurrent_merges: usize,
    pub batch_size_threshold: usize,
    pub enable_parallel_processing: bool,
    pub memory_limit_mb: Option<usize>,
}

impl Default for MergerConfig {
    fn default() -> Self {
        let mut source_weights = HashMap::new();
        source_weights.insert(RelationshipSource::Lsp, 1.2);
        source_weights.insert(RelationshipSource::TreeSitter, 1.0);
        source_weights.insert(RelationshipSource::Hybrid, 1.1);
        source_weights.insert(RelationshipSource::Cache, 0.9);

        let mut relation_type_modifiers = HashMap::new();
        relation_type_modifiers.insert(RelationType::Calls, 1.0);
        relation_type_modifiers.insert(RelationType::InheritsFrom, 0.95);
        relation_type_modifiers.insert(RelationType::References, 0.9);
        relation_type_modifiers.insert(RelationType::Contains, 1.1);

        Self {
            merge_strategy: MergeStrategy::LspPreferred,
            conflict_resolution: ConflictResolution::HighestConfidence,
            deduplication_strategy: DeduplicationStrategy::Combined,
            confidence_threshold: 0.5,
            max_relationships_per_symbol: 50,
            enable_metadata_merging: true,
            enable_confidence_boosting: true,
            source_weights,
            relation_type_modifiers,
            location_accuracy_bonus: 0.1,
            strict_validation: true,
            max_concurrent_merges: 4,
            batch_size_threshold: 1000,
            enable_parallel_processing: true,
            memory_limit_mb: Some(256),
        }
    }
}

/// Strategies for merging relationships from multiple sources
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MergeStrategy {
    /// Prefer LSP when available, fallback to tree-sitter
    LspPreferred,

    /// Use tree-sitter for structure, LSP for semantics
    Complementary,

    /// Use both sources with confidence weighting
    WeightedCombination,

    /// Only use LSP, ignore tree-sitter
    LspOnly,

    /// Only use tree-sitter, ignore LSP
    TreeSitterOnly,
}

/// Strategies for resolving conflicts between relationships
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ConflictResolution {
    /// Use the relationship with highest confidence
    HighestConfidence,

    /// Prefer LSP over tree-sitter
    PreferLsp,

    /// Prefer tree-sitter over LSP
    PreferTreeSitter,

    /// Keep all relationships with metadata about conflicts
    KeepAll,

    /// Use custom resolution logic
    Custom,
}

/// Strategies for deduplicating relationships
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DeduplicationStrategy {
    /// Exact match on source, target, and relation type
    Exact,

    /// Fuzzy match considering symbol name similarity
    Fuzzy { threshold: f32 },

    /// Position-based matching for similar locations
    Positional { tolerance: u32 },

    /// Combination of strategies
    Combined,
}

/// Source of a relationship for weighting purposes
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelationshipSource {
    TreeSitter,
    Lsp,
    Hybrid,
    Cache,
}

/// Set of conflicting relationships
#[derive(Debug, Clone)]
pub struct ConflictSet {
    /// Relationships in conflict
    pub relationships: Vec<ExtractedRelationship>,

    /// Type of conflict
    pub conflict_type: ConflictType,

    /// Resolution strategy to use
    pub resolution_strategy: ConflictResolution,

    /// Additional context for resolution
    pub context: HashMap<String, String>,
}

/// Types of conflicts that can occur between relationships
#[derive(Debug, Clone, PartialEq)]
pub enum ConflictType {
    /// Same source/target but different relation types
    RelationTypeMismatch,

    /// Same relation but different confidence scores
    ConfidenceDisparity,

    /// Similar locations but different symbols
    SymbolAmbiguity,

    /// Contradictory information from different sources
    SourceContradiction,
}

/// Trait for custom conflict resolution logic
#[async_trait]
pub trait ConflictResolver: Send + Sync {
    /// Resolve a conflict set into zero or more relationships
    async fn resolve_conflict(
        &self,
        conflict_set: &ConflictSet,
        context: &MergeContext,
    ) -> Result<Vec<ExtractedRelationship>, MergeError>;
}

/// Confidence calculation system
#[derive(Debug, Clone)]
pub struct ConfidenceCalculator {
    /// Weights for different relationship sources
    pub source_weights: HashMap<RelationshipSource, f32>,

    /// Modifiers for different relation types
    pub relation_type_modifiers: HashMap<RelationType, f32>,

    /// Bonus for relationships with location information
    pub location_accuracy_bonus: f32,

    /// Enable confidence boosting for multiple source confirmation
    pub enable_boosting: bool,
}

impl ConfidenceCalculator {
    /// Create a new confidence calculator
    pub fn new(config: &MergerConfig) -> Self {
        Self {
            source_weights: config.source_weights.clone(),
            relation_type_modifiers: config.relation_type_modifiers.clone(),
            location_accuracy_bonus: config.location_accuracy_bonus,
            enable_boosting: config.enable_confidence_boosting,
        }
    }

    /// Calculate confidence score for a relationship
    pub fn calculate_confidence(
        &self,
        relationship: &ExtractedRelationship,
        context: &MergeContext,
    ) -> f32 {
        let mut confidence = relationship.confidence;

        // Apply source weight
        if let Some(source) = self.get_relationship_source(relationship) {
            if let Some(weight) = self.source_weights.get(&source) {
                confidence *= weight;
                debug!(
                    "Applied source weight {}: {} -> {}",
                    weight, relationship.confidence, confidence
                );
            }
        }

        // Apply relation type modifier
        if let Some(modifier) = self
            .relation_type_modifiers
            .get(&relationship.relation_type)
        {
            confidence *= modifier;
            debug!(
                "Applied relation type modifier {}: confidence now {}",
                modifier, confidence
            );
        }

        // Apply location accuracy bonus
        if relationship.location.is_some() {
            confidence += self.location_accuracy_bonus;
            debug!(
                "Applied location accuracy bonus: {}",
                self.location_accuracy_bonus
            );
        }

        // Language-specific adjustments
        match context.language.as_str() {
            "rust" => confidence *= 1.1, // Rust has good type information
            "typescript" => confidence *= 1.05,
            "python" => confidence *= 0.95, // Dynamic typing
            _ => {}
        }

        confidence.clamp(0.0, 1.0)
    }

    /// Calculate confidence difference between relationships
    pub fn confidence_difference(
        &self,
        r1: &ExtractedRelationship,
        r2: &ExtractedRelationship,
        context: &MergeContext,
    ) -> f32 {
        let c1 = self.calculate_confidence(r1, context);
        let c2 = self.calculate_confidence(r2, context);
        (c1 - c2).abs()
    }

    /// Get relationship source from metadata
    fn get_relationship_source(
        &self,
        relationship: &ExtractedRelationship,
    ) -> Option<RelationshipSource> {
        relationship
            .metadata
            .get("source")
            .and_then(|v| v.as_str())
            .and_then(|s| match s {
                "tree_sitter" => Some(RelationshipSource::TreeSitter),
                "lsp" => Some(RelationshipSource::Lsp),
                "hybrid" => Some(RelationshipSource::Hybrid),
                "cache" => Some(RelationshipSource::Cache),
                _ => None,
            })
    }
}

/// Metrics for monitoring merge performance
#[derive(Debug, Default, Clone)]
pub struct MergeMetrics {
    /// Total relationships processed
    pub total_relationships_processed: u64,

    /// Conflicts detected and resolved
    pub conflicts_detected: u64,
    pub conflicts_resolved: u64,

    /// Deduplication statistics
    pub duplicates_removed: u64,

    /// Confidence adjustments made
    pub confidence_adjustments: u64,

    /// Time spent merging
    pub merge_time: Duration,

    /// Distribution of relationship sources
    pub source_distribution: HashMap<RelationshipSource, u64>,

    /// Error counts
    pub validation_errors: u64,
    pub resolution_failures: u64,
}

impl MergeMetrics {
    /// Reset all metrics to zero
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Add metrics from another instance
    pub fn add(&mut self, other: &MergeMetrics) {
        self.total_relationships_processed += other.total_relationships_processed;
        self.conflicts_detected += other.conflicts_detected;
        self.conflicts_resolved += other.conflicts_resolved;
        self.duplicates_removed += other.duplicates_removed;
        self.confidence_adjustments += other.confidence_adjustments;
        self.merge_time += other.merge_time;
        self.validation_errors += other.validation_errors;
        self.resolution_failures += other.resolution_failures;

        for (source, count) in &other.source_distribution {
            *self.source_distribution.entry(source.clone()).or_insert(0) += count;
        }
    }
}

/// Main hybrid relationship merger
pub struct HybridRelationshipMerger {
    /// Merger configuration
    config: MergerConfig,

    /// Confidence calculator
    confidence_calculator: ConfidenceCalculator,

    /// Optional custom conflict resolver
    custom_resolver: Option<Arc<dyn ConflictResolver>>,

    /// Metrics tracking
    metrics: Arc<std::sync::Mutex<MergeMetrics>>,
}

impl HybridRelationshipMerger {
    /// Create a new hybrid relationship merger
    pub fn new(config: MergerConfig) -> Self {
        let confidence_calculator = ConfidenceCalculator::new(&config);

        Self {
            config,
            confidence_calculator,
            custom_resolver: None,
            metrics: Arc::new(std::sync::Mutex::new(MergeMetrics::default())),
        }
    }

    /// Create with custom conflict resolver
    pub fn with_custom_resolver(mut self, resolver: Arc<dyn ConflictResolver>) -> Self {
        self.custom_resolver = Some(resolver);
        self
    }

    /// Get current metrics
    pub fn get_metrics(&self) -> MergeMetrics {
        self.metrics.lock().unwrap().clone()
    }

    /// Reset metrics counters
    pub fn reset_metrics(&self) {
        self.metrics.lock().unwrap().reset();
    }

    /// Main entry point for merging relationships from multiple sources
    pub async fn merge_relationships(
        &self,
        tree_sitter_relationships: Vec<ExtractedRelationship>,
        lsp_relationships: Vec<ExtractedRelationship>,
        context: &MergeContext,
    ) -> Result<Vec<ExtractedRelationship>, MergeError> {
        let total_relationships = tree_sitter_relationships.len() + lsp_relationships.len();

        // For large datasets, use batch processing
        if self.config.enable_parallel_processing
            && total_relationships > self.config.batch_size_threshold
        {
            return self
                .merge_relationships_parallel(tree_sitter_relationships, lsp_relationships, context)
                .await;
        }
        let start_time = Instant::now();

        info!(
            "Starting relationship merge: {} tree-sitter, {} LSP relationships",
            tree_sitter_relationships.len(),
            lsp_relationships.len()
        );

        // Step 1: Preprocess and validate relationships
        let ts_relationships = self
            .preprocess_relationships(tree_sitter_relationships, RelationshipSource::TreeSitter)?;
        let lsp_relationships =
            self.preprocess_relationships(lsp_relationships, RelationshipSource::Lsp)?;

        // Step 2: Apply merge strategy
        let combined_relationships = self
            .apply_merge_strategy(ts_relationships, lsp_relationships, context)
            .await?;

        // Step 3: Detect and resolve conflicts
        let resolved_relationships = self
            .detect_and_resolve_conflicts(combined_relationships, context)
            .await?;

        // Step 4: Deduplicate relationships
        let deduplicated_relationships =
            self.deduplicate_relationships(resolved_relationships, context)?;

        // Step 5: Calculate final confidence scores
        let final_relationships =
            self.calculate_final_confidence(deduplicated_relationships, context)?;

        // Step 6: Apply final validation and filtering
        let validated_relationships = self.validate_and_filter(final_relationships, context)?;

        // Update metrics
        let mut metrics = self.metrics.lock().unwrap();
        metrics.merge_time += start_time.elapsed();
        metrics.total_relationships_processed += validated_relationships.len() as u64;

        info!(
            "Relationship merge completed: {} relationships after merging and deduplication",
            validated_relationships.len()
        );

        Ok(validated_relationships)
    }

    /// Preprocess relationships and add source metadata
    fn preprocess_relationships(
        &self,
        relationships: Vec<ExtractedRelationship>,
        source: RelationshipSource,
    ) -> Result<Vec<ExtractedRelationship>, MergeError> {
        let source_str = match source {
            RelationshipSource::TreeSitter => "tree_sitter",
            RelationshipSource::Lsp => "lsp",
            RelationshipSource::Hybrid => "hybrid",
            RelationshipSource::Cache => "cache",
        };

        let processed: Vec<_> = relationships
            .into_iter()
            .map(|mut rel| {
                // Add source metadata
                rel.metadata.insert(
                    "source".to_string(),
                    serde_json::Value::String(source_str.to_string()),
                );

                // Add preprocessing timestamp
                rel.metadata.insert(
                    "processed_at".to_string(),
                    serde_json::Value::String(
                        SystemTime::now()
                            .duration_since(SystemTime::UNIX_EPOCH)
                            .unwrap()
                            .as_secs()
                            .to_string(),
                    ),
                );

                rel
            })
            .collect();

        // Update source distribution metrics
        if let Ok(mut metrics) = self.metrics.lock() {
            *metrics.source_distribution.entry(source).or_insert(0) += processed.len() as u64;
        }

        Ok(processed)
    }

    /// Apply the configured merge strategy
    async fn apply_merge_strategy(
        &self,
        tree_sitter_relationships: Vec<ExtractedRelationship>,
        lsp_relationships: Vec<ExtractedRelationship>,
        context: &MergeContext,
    ) -> Result<Vec<ExtractedRelationship>, MergeError> {
        match self.config.merge_strategy {
            MergeStrategy::LspPreferred => {
                self.merge_lsp_preferred(tree_sitter_relationships, lsp_relationships, context)
                    .await
            }
            MergeStrategy::Complementary => {
                self.merge_complementary(tree_sitter_relationships, lsp_relationships, context)
                    .await
            }
            MergeStrategy::WeightedCombination => {
                self.merge_weighted_combination(
                    tree_sitter_relationships,
                    lsp_relationships,
                    context,
                )
                .await
            }
            MergeStrategy::LspOnly => Ok(lsp_relationships),
            MergeStrategy::TreeSitterOnly => Ok(tree_sitter_relationships),
        }
    }

    /// LSP-preferred merge strategy implementation
    async fn merge_lsp_preferred(
        &self,
        tree_sitter_relationships: Vec<ExtractedRelationship>,
        lsp_relationships: Vec<ExtractedRelationship>,
        _context: &MergeContext,
    ) -> Result<Vec<ExtractedRelationship>, MergeError> {
        let mut result = lsp_relationships;

        // Create a set of LSP relationship keys for quick lookup
        let lsp_keys: HashSet<_> = result
            .iter()
            .map(|r| {
                (
                    r.source_symbol_uid.clone(),
                    r.target_symbol_uid.clone(),
                    r.relation_type,
                )
            })
            .collect();

        // Add tree-sitter relationships that don't conflict with LSP
        for ts_rel in tree_sitter_relationships {
            let key = (
                ts_rel.source_symbol_uid.clone(),
                ts_rel.target_symbol_uid.clone(),
                ts_rel.relation_type,
            );

            if !lsp_keys.contains(&key) {
                result.push(ts_rel);
            }
        }

        debug!("LSP-preferred merge: {} final relationships", result.len());
        Ok(result)
    }

    /// Complementary merge strategy implementation
    async fn merge_complementary(
        &self,
        tree_sitter_relationships: Vec<ExtractedRelationship>,
        lsp_relationships: Vec<ExtractedRelationship>,
        _context: &MergeContext,
    ) -> Result<Vec<ExtractedRelationship>, MergeError> {
        let mut result = Vec::new();

        // Add structural relationships from tree-sitter
        for ts_rel in tree_sitter_relationships {
            if ts_rel.relation_type.is_structural() {
                result.push(ts_rel);
            }
        }

        // Add semantic relationships from LSP
        for lsp_rel in lsp_relationships {
            if lsp_rel.relation_type.is_usage() {
                result.push(lsp_rel);
            }
        }

        debug!("Complementary merge: {} final relationships", result.len());
        Ok(result)
    }

    /// Weighted combination merge strategy implementation
    async fn merge_weighted_combination(
        &self,
        tree_sitter_relationships: Vec<ExtractedRelationship>,
        lsp_relationships: Vec<ExtractedRelationship>,
        _context: &MergeContext,
    ) -> Result<Vec<ExtractedRelationship>, MergeError> {
        let mut all_relationships = Vec::new();
        all_relationships.extend(tree_sitter_relationships);
        all_relationships.extend(lsp_relationships);

        // Will be deduplicated later with confidence weighting
        debug!(
            "Weighted combination merge: {} total relationships",
            all_relationships.len()
        );
        Ok(all_relationships)
    }

    /// Detect conflicts and resolve them
    async fn detect_and_resolve_conflicts(
        &self,
        relationships: Vec<ExtractedRelationship>,
        context: &MergeContext,
    ) -> Result<Vec<ExtractedRelationship>, MergeError> {
        let conflicts = self.detect_conflicts(&relationships)?;

        if conflicts.is_empty() {
            debug!("No conflicts detected");
            return Ok(relationships);
        }

        info!("Detected {} conflict sets", conflicts.len());

        let mut resolved_relationships = Vec::new();
        let mut processed_indices = HashSet::new();

        // Update conflict metrics
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.conflicts_detected += conflicts.len() as u64;
        }

        // Resolve each conflict set
        for conflict in conflicts {
            let resolved = self.resolve_conflict_set(conflict.clone(), context).await?;
            resolved_relationships.extend(resolved);

            // Mark indices as processed
            for rel in &conflict.relationships {
                if let Some(index) = relationships.iter().position(|r| {
                    r.source_symbol_uid == rel.source_symbol_uid
                        && r.target_symbol_uid == rel.target_symbol_uid
                        && r.relation_type == rel.relation_type
                }) {
                    processed_indices.insert(index);
                }
            }

            // Update resolved conflicts metric
            if let Ok(mut metrics) = self.metrics.lock() {
                metrics.conflicts_resolved += 1;
            }
        }

        // Add non-conflicting relationships
        for (i, rel) in relationships.into_iter().enumerate() {
            if !processed_indices.contains(&i) {
                resolved_relationships.push(rel);
            }
        }

        debug!(
            "Conflict resolution completed: {} relationships",
            resolved_relationships.len()
        );
        Ok(resolved_relationships)
    }

    /// Detect conflicts between relationships
    fn detect_conflicts(
        &self,
        relationships: &[ExtractedRelationship],
    ) -> Result<Vec<ConflictSet>, MergeError> {
        let mut conflicts = Vec::new();
        let _processed: HashSet<usize> = HashSet::new();

        // Group relationships by source-target pair
        let mut relationship_groups: HashMap<(String, String), Vec<&ExtractedRelationship>> =
            HashMap::new();

        for rel in relationships {
            let key = (rel.source_symbol_uid.clone(), rel.target_symbol_uid.clone());
            relationship_groups.entry(key).or_default().push(rel);
        }

        // Check each group for conflicts
        for ((_source, _target), group_relationships) in relationship_groups {
            if group_relationships.len() > 1 {
                let conflict_type = self.classify_conflict(&group_relationships)?;

                let conflict_set = ConflictSet {
                    relationships: group_relationships.into_iter().cloned().collect(),
                    conflict_type,
                    resolution_strategy: self.config.conflict_resolution.clone(),
                    context: HashMap::new(),
                };

                conflicts.push(conflict_set);
            }
        }

        Ok(conflicts)
    }

    /// Classify the type of conflict
    fn classify_conflict(
        &self,
        relationships: &[&ExtractedRelationship],
    ) -> Result<ConflictType, MergeError> {
        // Check for relation type mismatches
        let relation_types: HashSet<_> = relationships.iter().map(|r| r.relation_type).collect();
        if relation_types.len() > 1 {
            return Ok(ConflictType::RelationTypeMismatch);
        }

        // Check for confidence disparities
        let confidences: Vec<_> = relationships.iter().map(|r| r.confidence).collect();
        let max_confidence = confidences.iter().cloned().fold(0.0f32, f32::max);
        let min_confidence = confidences.iter().cloned().fold(1.0f32, f32::min);

        if max_confidence - min_confidence > 0.3 {
            return Ok(ConflictType::ConfidenceDisparity);
        }

        // Check for source contradictions
        let sources: HashSet<_> = relationships
            .iter()
            .filter_map(|r| r.metadata.get("source"))
            .collect();

        if sources.len() > 1 {
            return Ok(ConflictType::SourceContradiction);
        }

        // Default to symbol ambiguity
        Ok(ConflictType::SymbolAmbiguity)
    }

    /// Resolve a specific conflict set
    async fn resolve_conflict_set(
        &self,
        conflict_set: ConflictSet,
        context: &MergeContext,
    ) -> Result<Vec<ExtractedRelationship>, MergeError> {
        match &self.config.conflict_resolution {
            ConflictResolution::HighestConfidence => {
                self.resolve_highest_confidence(&conflict_set, context)
            }
            ConflictResolution::PreferLsp => self.resolve_prefer_lsp(&conflict_set, context),
            ConflictResolution::PreferTreeSitter => {
                self.resolve_prefer_tree_sitter(&conflict_set, context)
            }
            ConflictResolution::KeepAll => self.resolve_keep_all(&conflict_set, context),
            ConflictResolution::Custom => {
                if let Some(resolver) = &self.custom_resolver {
                    resolver.resolve_conflict(&conflict_set, context).await
                } else {
                    // Fallback to highest confidence
                    self.resolve_highest_confidence(&conflict_set, context)
                }
            }
        }
    }

    /// Resolve conflict by keeping highest confidence relationship
    fn resolve_highest_confidence(
        &self,
        conflict_set: &ConflictSet,
        context: &MergeContext,
    ) -> Result<Vec<ExtractedRelationship>, MergeError> {
        let best_relationship = conflict_set
            .relationships
            .iter()
            .max_by(|a, b| {
                let conf_a = self.confidence_calculator.calculate_confidence(a, context);
                let conf_b = self.confidence_calculator.calculate_confidence(b, context);
                conf_a
                    .partial_cmp(&conf_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .ok_or_else(|| MergeError::ConflictResolutionFailed {
                message: "No relationships in conflict set".to_string(),
            })?;

        Ok(vec![best_relationship.clone()])
    }

    /// Resolve conflict by preferring LSP relationships
    fn resolve_prefer_lsp(
        &self,
        conflict_set: &ConflictSet,
        context: &MergeContext,
    ) -> Result<Vec<ExtractedRelationship>, MergeError> {
        // Find LSP relationships first
        let lsp_relationships: Vec<_> = conflict_set
            .relationships
            .iter()
            .filter(|r| {
                r.metadata
                    .get("source")
                    .and_then(|v| v.as_str())
                    .map_or(false, |s| s == "lsp")
            })
            .collect();

        if !lsp_relationships.is_empty() {
            Ok(lsp_relationships.into_iter().cloned().collect())
        } else {
            // Fallback to highest confidence
            self.resolve_highest_confidence(conflict_set, context)
        }
    }

    /// Resolve conflict by preferring tree-sitter relationships
    fn resolve_prefer_tree_sitter(
        &self,
        conflict_set: &ConflictSet,
        context: &MergeContext,
    ) -> Result<Vec<ExtractedRelationship>, MergeError> {
        // Find tree-sitter relationships first
        let ts_relationships: Vec<_> = conflict_set
            .relationships
            .iter()
            .filter(|r| {
                r.metadata
                    .get("source")
                    .and_then(|v| v.as_str())
                    .map_or(false, |s| s == "tree_sitter")
            })
            .collect();

        if !ts_relationships.is_empty() {
            Ok(ts_relationships.into_iter().cloned().collect())
        } else {
            // Fallback to highest confidence
            self.resolve_highest_confidence(conflict_set, context)
        }
    }

    /// Resolve conflict by keeping all relationships with conflict metadata
    fn resolve_keep_all(
        &self,
        conflict_set: &ConflictSet,
        _context: &MergeContext,
    ) -> Result<Vec<ExtractedRelationship>, MergeError> {
        let mut relationships = conflict_set.relationships.clone();

        // Add conflict metadata to each relationship
        for rel in &mut relationships {
            rel.metadata.insert(
                "conflict_type".to_string(),
                serde_json::Value::String(format!("{:?}", conflict_set.conflict_type)),
            );
            rel.metadata
                .insert("in_conflict_set".to_string(), serde_json::Value::Bool(true));
        }

        Ok(relationships)
    }

    /// Deduplicate relationships using the configured strategy
    fn deduplicate_relationships(
        &self,
        relationships: Vec<ExtractedRelationship>,
        context: &MergeContext,
    ) -> Result<Vec<ExtractedRelationship>, MergeError> {
        let initial_count = relationships.len();

        let deduplicated = match &self.config.deduplication_strategy {
            DeduplicationStrategy::Exact => self.deduplicate_exact(relationships)?,
            DeduplicationStrategy::Fuzzy { threshold } => {
                self.deduplicate_fuzzy(relationships, *threshold)?
            }
            DeduplicationStrategy::Positional { tolerance } => {
                self.deduplicate_positional(relationships, *tolerance)?
            }
            DeduplicationStrategy::Combined => self.deduplicate_combined(relationships, context)?,
        };

        let final_count = deduplicated.len();
        let removed_count = initial_count - final_count;

        // Update deduplication metrics
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.duplicates_removed += removed_count as u64;
        }

        debug!(
            "Deduplication completed: {} -> {} relationships ({} duplicates removed)",
            initial_count, final_count, removed_count
        );

        Ok(deduplicated)
    }

    /// Exact deduplication based on UIDs and relation type
    fn deduplicate_exact(
        &self,
        relationships: Vec<ExtractedRelationship>,
    ) -> Result<Vec<ExtractedRelationship>, MergeError> {
        let mut seen = HashSet::new();
        let mut deduplicated = Vec::new();

        for relationship in relationships {
            let key = (
                relationship.source_symbol_uid.clone(),
                relationship.target_symbol_uid.clone(),
                relationship.relation_type,
            );

            if seen.insert(key.clone()) {
                deduplicated.push(relationship);
            } else {
                // Relationship already exists, potentially merge metadata
                if let Some(existing) = deduplicated.iter_mut().find(|r| {
                    (
                        r.source_symbol_uid.clone(),
                        r.target_symbol_uid.clone(),
                        r.relation_type,
                    ) == key
                }) {
                    self.merge_relationship_metadata(existing, &relationship)?;
                }
            }
        }

        Ok(deduplicated)
    }

    /// Fuzzy deduplication with symbol name similarity
    fn deduplicate_fuzzy(
        &self,
        relationships: Vec<ExtractedRelationship>,
        threshold: f32,
    ) -> Result<Vec<ExtractedRelationship>, MergeError> {
        let mut deduplicated = Vec::new();

        for relationship in relationships {
            let mut is_duplicate = false;

            // Check against existing relationships
            for existing in &mut deduplicated {
                if self.is_fuzzy_duplicate(&relationship, existing, threshold)? {
                    // Merge with existing relationship
                    self.merge_relationship_metadata(existing, &relationship)?;

                    // Update confidence to higher value
                    if relationship.confidence > existing.confidence {
                        existing.confidence = relationship.confidence;
                    }

                    is_duplicate = true;
                    break;
                }
            }

            if !is_duplicate {
                deduplicated.push(relationship);
            }
        }

        Ok(deduplicated)
    }

    /// Check if two relationships are fuzzy duplicates
    fn is_fuzzy_duplicate(
        &self,
        r1: &ExtractedRelationship,
        r2: &ExtractedRelationship,
        threshold: f32,
    ) -> Result<bool, MergeError> {
        // Must have same relation type
        if r1.relation_type != r2.relation_type {
            return Ok(false);
        }

        // Calculate symbol name similarity (simple approach)
        let source_similarity =
            self.calculate_string_similarity(&r1.source_symbol_uid, &r2.source_symbol_uid);
        let target_similarity =
            self.calculate_string_similarity(&r1.target_symbol_uid, &r2.target_symbol_uid);

        let average_similarity = (source_similarity + target_similarity) / 2.0;

        Ok(average_similarity >= threshold)
    }

    /// Calculate string similarity (Levenshtein distance normalized)
    fn calculate_string_similarity(&self, s1: &str, s2: &str) -> f32 {
        if s1 == s2 {
            return 1.0;
        }

        let len1 = s1.len();
        let len2 = s2.len();

        if len1 == 0 || len2 == 0 {
            return 0.0;
        }

        // Simple implementation - in production, use a proper string similarity algorithm
        let max_len = len1.max(len2) as f32;
        let distance = self.levenshtein_distance(s1, s2) as f32;

        (max_len - distance) / max_len
    }

    /// Calculate Levenshtein distance
    fn levenshtein_distance(&self, s1: &str, s2: &str) -> usize {
        let len1 = s1.len();
        let len2 = s2.len();

        let mut matrix = vec![vec![0; len2 + 1]; len1 + 1];

        // Initialize first row and column
        for i in 0..=len1 {
            matrix[i][0] = i;
        }
        for j in 0..=len2 {
            matrix[0][j] = j;
        }

        let chars1: Vec<char> = s1.chars().collect();
        let chars2: Vec<char> = s2.chars().collect();

        for i in 1..=len1 {
            for j in 1..=len2 {
                let cost = if chars1[i - 1] == chars2[j - 1] { 0 } else { 1 };

                matrix[i][j] = (matrix[i - 1][j] + 1)
                    .min(matrix[i][j - 1] + 1)
                    .min(matrix[i - 1][j - 1] + cost);
            }
        }

        matrix[len1][len2]
    }

    /// Position-based deduplication
    fn deduplicate_positional(
        &self,
        relationships: Vec<ExtractedRelationship>,
        tolerance: u32,
    ) -> Result<Vec<ExtractedRelationship>, MergeError> {
        let mut deduplicated = Vec::new();

        for relationship in relationships {
            let mut is_duplicate = false;

            for existing in &mut deduplicated {
                if self.is_positional_duplicate(&relationship, existing, tolerance)? {
                    self.merge_relationship_metadata(existing, &relationship)?;
                    is_duplicate = true;
                    break;
                }
            }

            if !is_duplicate {
                deduplicated.push(relationship);
            }
        }

        Ok(deduplicated)
    }

    /// Check if two relationships are positionally similar
    fn is_positional_duplicate(
        &self,
        r1: &ExtractedRelationship,
        r2: &ExtractedRelationship,
        tolerance: u32,
    ) -> Result<bool, MergeError> {
        // Must have same relation type
        if r1.relation_type != r2.relation_type {
            return Ok(false);
        }

        // Check location similarity if both have locations
        if let (Some(loc1), Some(loc2)) = (&r1.location, &r2.location) {
            let line_diff = (loc1.start_line as i32 - loc2.start_line as i32).abs() as u32;
            let char_diff = (loc1.start_char as i32 - loc2.start_char as i32).abs() as u32;

            return Ok(line_diff <= tolerance && char_diff <= tolerance * 10);
        }

        // If no locations, fall back to symbol UID comparison
        Ok(r1.source_symbol_uid == r2.source_symbol_uid
            && r1.target_symbol_uid == r2.target_symbol_uid)
    }

    /// Combined deduplication strategy
    fn deduplicate_combined(
        &self,
        relationships: Vec<ExtractedRelationship>,
        _context: &MergeContext,
    ) -> Result<Vec<ExtractedRelationship>, MergeError> {
        // First pass: exact deduplication
        let after_exact = self.deduplicate_exact(relationships)?;

        // Second pass: fuzzy deduplication with moderate threshold
        let after_fuzzy = self.deduplicate_fuzzy(after_exact, 0.8)?;

        // Third pass: positional deduplication with small tolerance
        let final_result = self.deduplicate_positional(after_fuzzy, 2)?;

        Ok(final_result)
    }

    /// Merge metadata from source relationship into target
    fn merge_relationship_metadata(
        &self,
        target: &mut ExtractedRelationship,
        source: &ExtractedRelationship,
    ) -> Result<(), MergeError> {
        if !self.config.enable_metadata_merging {
            return Ok(());
        }

        // Merge metadata, preferring existing values
        for (key, value) in &source.metadata {
            if !target.metadata.contains_key(key) {
                target.metadata.insert(key.clone(), value.clone());
            }
        }

        // Add merge information
        target.metadata.insert(
            "merged_sources".to_string(),
            serde_json::Value::String("multiple".to_string()),
        );

        Ok(())
    }

    /// Calculate final confidence scores for all relationships
    fn calculate_final_confidence(
        &self,
        relationships: Vec<ExtractedRelationship>,
        context: &MergeContext,
    ) -> Result<Vec<ExtractedRelationship>, MergeError> {
        let mut final_relationships = Vec::new();

        for mut relationship in relationships {
            let original_confidence = relationship.confidence;
            let final_confidence = self
                .confidence_calculator
                .calculate_confidence(&relationship, context);

            relationship.confidence = final_confidence;

            // Add confidence calculation metadata
            relationship.metadata.insert(
                "original_confidence".to_string(),
                serde_json::Value::Number(
                    serde_json::Number::from_f64(original_confidence as f64).unwrap(),
                ),
            );
            relationship.metadata.insert(
                "final_confidence".to_string(),
                serde_json::Value::Number(
                    serde_json::Number::from_f64(final_confidence as f64).unwrap(),
                ),
            );

            final_relationships.push(relationship);

            // Update confidence adjustment metrics
            if (final_confidence - original_confidence).abs() > 0.01 {
                if let Ok(mut metrics) = self.metrics.lock() {
                    metrics.confidence_adjustments += 1;
                }
            }
        }

        Ok(final_relationships)
    }

    /// Final validation and filtering
    fn validate_and_filter(
        &self,
        relationships: Vec<ExtractedRelationship>,
        _context: &MergeContext,
    ) -> Result<Vec<ExtractedRelationship>, MergeError> {
        let mut validated_relationships = Vec::new();
        let mut validation_errors = 0u64;

        for relationship in relationships {
            // Apply confidence threshold
            if relationship.confidence < self.config.confidence_threshold {
                debug!(
                    "Relationship filtered out due to low confidence: {}",
                    relationship.confidence
                );
                continue;
            }

            // Validate relationship structure
            if self.config.strict_validation {
                if let Err(e) = self.validate_relationship(&relationship) {
                    warn!("Relationship validation failed: {}", e);
                    validation_errors += 1;
                    continue;
                }
            }

            validated_relationships.push(relationship);
        }

        // Update validation error metrics
        if let Ok(mut metrics) = self.metrics.lock() {
            metrics.validation_errors += validation_errors;
        }

        // Apply max relationships per symbol limit
        validated_relationships = self.apply_relationship_limits(validated_relationships)?;

        Ok(validated_relationships)
    }

    /// Validate a single relationship
    fn validate_relationship(
        &self,
        relationship: &ExtractedRelationship,
    ) -> Result<(), MergeError> {
        // Check for empty UIDs
        if relationship.source_symbol_uid.is_empty() {
            return Err(MergeError::ValidationError {
                message: "Source symbol UID is empty".to_string(),
            });
        }

        if relationship.target_symbol_uid.is_empty() {
            return Err(MergeError::ValidationError {
                message: "Target symbol UID is empty".to_string(),
            });
        }

        // Check confidence range
        if relationship.confidence < 0.0 || relationship.confidence > 1.0 {
            return Err(MergeError::ValidationError {
                message: format!("Confidence out of range: {}", relationship.confidence),
            });
        }

        // Check for self-relationships (may or may not be valid depending on context)
        if relationship.source_symbol_uid == relationship.target_symbol_uid {
            debug!(
                "Self-relationship detected: {}",
                relationship.source_symbol_uid
            );
        }

        Ok(())
    }

    /// Apply limits on relationships per symbol
    fn apply_relationship_limits(
        &self,
        relationships: Vec<ExtractedRelationship>,
    ) -> Result<Vec<ExtractedRelationship>, MergeError> {
        let mut symbol_counts: HashMap<String, usize> = HashMap::new();
        let mut result = Vec::new();

        // Sort by confidence (descending) to keep highest confidence relationships
        let mut sorted_relationships = relationships;
        sorted_relationships.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for relationship in sorted_relationships {
            let source_count = symbol_counts
                .entry(relationship.source_symbol_uid.clone())
                .or_insert(0);

            if *source_count < self.config.max_relationships_per_symbol {
                result.push(relationship);
                *source_count += 1;
            } else {
                debug!(
                    "Relationship limit reached for symbol: {}",
                    relationship.source_symbol_uid
                );
            }
        }

        Ok(result)
    }

    /// Parallel processing for large relationship datasets
    async fn merge_relationships_parallel(
        &self,
        tree_sitter_relationships: Vec<ExtractedRelationship>,
        lsp_relationships: Vec<ExtractedRelationship>,
        context: &MergeContext,
    ) -> Result<Vec<ExtractedRelationship>, MergeError> {
        let start_time = Instant::now();
        let total_relationships = tree_sitter_relationships.len() + lsp_relationships.len();

        info!(
            "Starting parallel relationship merge: {} tree-sitter, {} LSP relationships (total: {})",
            tree_sitter_relationships.len(),
            lsp_relationships.len(),
            total_relationships
        );

        // Check memory limits
        if let Some(memory_limit_mb) = self.config.memory_limit_mb {
            let estimated_memory_mb = (total_relationships * 500) / (1024 * 1024); // Rough estimate: 500 bytes per relationship
            if estimated_memory_mb > memory_limit_mb {
                warn!(
                    "Estimated memory usage ({} MB) exceeds limit ({} MB), using sequential processing",
                    estimated_memory_mb, memory_limit_mb
                );
                return self
                    .merge_relationships_sequential(
                        tree_sitter_relationships,
                        lsp_relationships,
                        context,
                    )
                    .await;
            }
        }

        // Process in parallel batches
        let batch_size = self.config.batch_size_threshold / 2;
        let ts_chunks: Vec<_> = tree_sitter_relationships.chunks(batch_size).collect();
        let lsp_chunks: Vec<_> = lsp_relationships.chunks(batch_size).collect();

        let mut batch_results = Vec::new();
        let semaphore = Arc::new(tokio::sync::Semaphore::new(
            self.config.max_concurrent_merges,
        ));

        // Process tree-sitter chunks
        for (i, ts_chunk) in ts_chunks.into_iter().enumerate() {
            let permit =
                semaphore
                    .clone()
                    .acquire_owned()
                    .await
                    .map_err(|e| MergeError::InternalError {
                        message: format!("Failed to acquire semaphore permit: {}", e),
                    })?;

            let ts_relationships = ts_chunk.to_vec();
            let lsp_relationships = if i < lsp_chunks.len() {
                lsp_chunks[i].to_vec()
            } else {
                Vec::new()
            };

            let merger = self.clone_for_batch();
            let batch_context = context.clone();

            let handle = tokio::spawn(async move {
                let _permit = permit; // Hold permit for duration of task
                merger
                    .merge_relationships_sequential(
                        ts_relationships,
                        lsp_relationships,
                        &batch_context,
                    )
                    .await
            });

            batch_results.push(handle);
        }

        // Collect results from all batches
        let mut all_relationships = Vec::new();
        for handle in batch_results {
            let batch_result = handle.await.map_err(|e| MergeError::InternalError {
                message: format!("Batch processing task failed: {}", e),
            })??;
            all_relationships.extend(batch_result);
        }

        // Final deduplication and validation pass
        let final_relationships = self.deduplicate_relationships(all_relationships, context)?;
        let validated_relationships = self.validate_and_filter(final_relationships, context)?;

        // Update metrics
        let mut metrics = self.metrics.lock().unwrap();
        metrics.merge_time += start_time.elapsed();
        metrics.total_relationships_processed += validated_relationships.len() as u64;

        info!(
            "Parallel relationship merge completed: {} relationships after merging and deduplication (took {:?})",
            validated_relationships.len(),
            start_time.elapsed()
        );

        Ok(validated_relationships)
    }

    /// Sequential processing fallback
    async fn merge_relationships_sequential(
        &self,
        tree_sitter_relationships: Vec<ExtractedRelationship>,
        lsp_relationships: Vec<ExtractedRelationship>,
        context: &MergeContext,
    ) -> Result<Vec<ExtractedRelationship>, MergeError> {
        // This is the original implementation logic without the parallel processing check
        let start_time = Instant::now();

        info!(
            "Starting sequential relationship merge: {} tree-sitter, {} LSP relationships",
            tree_sitter_relationships.len(),
            lsp_relationships.len()
        );

        // Step 1: Preprocess and validate relationships
        let ts_relationships = self
            .preprocess_relationships(tree_sitter_relationships, RelationshipSource::TreeSitter)?;
        let lsp_relationships =
            self.preprocess_relationships(lsp_relationships, RelationshipSource::Lsp)?;

        // Step 2: Apply merge strategy
        let combined_relationships = self
            .apply_merge_strategy(ts_relationships, lsp_relationships, context)
            .await?;

        // Step 3: Detect and resolve conflicts
        let resolved_relationships = self
            .detect_and_resolve_conflicts(combined_relationships, context)
            .await?;

        // Step 4: Deduplicate relationships
        let deduplicated_relationships =
            self.deduplicate_relationships(resolved_relationships, context)?;

        // Step 5: Calculate final confidence scores
        let final_relationships =
            self.calculate_final_confidence(deduplicated_relationships, context)?;

        // Step 6: Apply final validation and filtering
        let validated_relationships = self.validate_and_filter(final_relationships, context)?;

        // Update metrics
        let mut metrics = self.metrics.lock().unwrap();
        metrics.merge_time += start_time.elapsed();
        metrics.total_relationships_processed += validated_relationships.len() as u64;

        info!(
            "Sequential relationship merge completed: {} relationships after merging and deduplication",
            validated_relationships.len()
        );

        Ok(validated_relationships)
    }

    /// Clone merger for batch processing (lightweight clone of config and calculator)
    fn clone_for_batch(&self) -> Self {
        Self {
            config: self.config.clone(),
            confidence_calculator: self.confidence_calculator.clone(),
            custom_resolver: self.custom_resolver.clone(),
            metrics: Arc::new(std::sync::Mutex::new(MergeMetrics::default())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyzer::types::RelationType;
    use crate::symbol::SymbolLocation;
    use std::path::PathBuf;

    fn create_test_relationship(
        source: &str,
        target: &str,
        relation_type: RelationType,
        confidence: f32,
        source_type: RelationshipSource,
    ) -> ExtractedRelationship {
        let mut rel =
            ExtractedRelationship::new(source.to_string(), target.to_string(), relation_type)
                .with_confidence(confidence);

        let source_str = match source_type {
            RelationshipSource::TreeSitter => "tree_sitter",
            RelationshipSource::Lsp => "lsp",
            RelationshipSource::Hybrid => "hybrid",
            RelationshipSource::Cache => "cache",
        };

        rel.metadata.insert(
            "source".to_string(),
            serde_json::Value::String(source_str.to_string()),
        );

        rel
    }

    fn create_test_context() -> MergeContext {
        MergeContext::new(1, PathBuf::from("test.rs"), "rust".to_string())
    }

    #[test]
    fn test_merger_creation() {
        let config = MergerConfig::default();
        let merger = HybridRelationshipMerger::new(config);

        assert_eq!(merger.config.merge_strategy, MergeStrategy::LspPreferred);
        assert_eq!(merger.config.confidence_threshold, 0.5);
    }

    #[test]
    fn test_confidence_calculator() {
        let config = MergerConfig::default();
        let calculator = ConfidenceCalculator::new(&config);
        let context = create_test_context();

        let rel = create_test_relationship(
            "source",
            "target",
            RelationType::Calls,
            0.8,
            RelationshipSource::Lsp,
        );

        let confidence = calculator.calculate_confidence(&rel, &context);

        // Should be boosted by LSP source weight (1.2)
        assert!(confidence > 0.8);
        assert!(confidence <= 1.0);
    }

    #[test]
    fn test_exact_deduplication() {
        let config = MergerConfig::default();
        let merger = HybridRelationshipMerger::new(config);

        let relationships = vec![
            create_test_relationship(
                "a",
                "b",
                RelationType::Calls,
                0.9,
                RelationshipSource::TreeSitter,
            ),
            create_test_relationship("a", "b", RelationType::Calls, 0.8, RelationshipSource::Lsp), // Duplicate
            create_test_relationship(
                "a",
                "c",
                RelationType::Calls,
                0.7,
                RelationshipSource::TreeSitter,
            ),
        ];

        let result = merger.deduplicate_exact(relationships).unwrap();

        assert_eq!(result.len(), 2); // One duplicate removed
    }

    #[test]
    fn test_fuzzy_deduplication() {
        let config = MergerConfig::default();
        let merger = HybridRelationshipMerger::new(config);

        let relationships = vec![
            create_test_relationship(
                "symbol_a",
                "symbol_b",
                RelationType::Calls,
                0.9,
                RelationshipSource::TreeSitter,
            ),
            create_test_relationship(
                "symbol_a_variant",
                "symbol_b",
                RelationType::Calls,
                0.8,
                RelationshipSource::Lsp,
            ),
            create_test_relationship(
                "completely_different",
                "symbol_c",
                RelationType::Calls,
                0.7,
                RelationshipSource::TreeSitter,
            ),
        ];

        let result = merger.deduplicate_fuzzy(relationships, 0.5).unwrap();

        // With moderate threshold, should keep all as they are quite different
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_conflict_detection() {
        let config = MergerConfig::default();
        let merger = HybridRelationshipMerger::new(config);

        let relationships = vec![
            create_test_relationship(
                "a",
                "b",
                RelationType::Calls,
                0.9,
                RelationshipSource::TreeSitter,
            ),
            create_test_relationship("a", "b", RelationType::Calls, 0.5, RelationshipSource::Lsp), // Same pair, different confidence
            create_test_relationship(
                "c",
                "d",
                RelationType::References,
                0.8,
                RelationshipSource::TreeSitter,
            ),
        ];

        let conflicts = merger.detect_conflicts(&relationships).unwrap();

        assert_eq!(conflicts.len(), 1); // One conflict between the two "a" -> "b" relationships
        assert_eq!(conflicts[0].relationships.len(), 2);
    }

    #[tokio::test]
    async fn test_lsp_preferred_merge() {
        let config = MergerConfig::default();
        let merger = HybridRelationshipMerger::new(config);
        let context = create_test_context();

        let ts_relationships = vec![
            create_test_relationship(
                "a",
                "b",
                RelationType::Calls,
                0.8,
                RelationshipSource::TreeSitter,
            ),
            create_test_relationship(
                "c",
                "d",
                RelationType::Contains,
                0.9,
                RelationshipSource::TreeSitter,
            ),
        ];

        let lsp_relationships = vec![
            create_test_relationship("a", "b", RelationType::Calls, 0.9, RelationshipSource::Lsp), // Conflicts with TS
            create_test_relationship(
                "e",
                "f",
                RelationType::References,
                0.8,
                RelationshipSource::Lsp,
            ),
        ];

        let result = merger
            .merge_lsp_preferred(ts_relationships, lsp_relationships, &context)
            .await
            .unwrap();

        // Should have 3 relationships: LSP "a"->"b", TS "c"->"d", LSP "e"->"f"
        assert_eq!(result.len(), 3);

        // LSP relationship should win the conflict
        let ab_relationship = result
            .iter()
            .find(|r| r.source_symbol_uid == "a" && r.target_symbol_uid == "b")
            .unwrap();
        assert_eq!(ab_relationship.confidence, 0.9); // LSP confidence
    }

    #[tokio::test]
    async fn test_complementary_merge() {
        let config = MergerConfig {
            merge_strategy: MergeStrategy::Complementary,
            ..Default::default()
        };
        let merger = HybridRelationshipMerger::new(config);
        let context = create_test_context();

        let ts_relationships = vec![
            create_test_relationship(
                "a",
                "b",
                RelationType::Contains,
                0.8,
                RelationshipSource::TreeSitter,
            ), // Structural
            create_test_relationship(
                "c",
                "d",
                RelationType::Calls,
                0.9,
                RelationshipSource::TreeSitter,
            ), // Usage
        ];

        let lsp_relationships = vec![
            create_test_relationship(
                "e",
                "f",
                RelationType::References,
                0.9,
                RelationshipSource::Lsp,
            ), // Usage
            create_test_relationship(
                "g",
                "h",
                RelationType::InheritsFrom,
                0.8,
                RelationshipSource::Lsp,
            ), // Structural
        ];

        let result = merger
            .merge_complementary(ts_relationships, lsp_relationships, &context)
            .await
            .unwrap();

        // Should have structural from TS and usage from LSP
        let contains_found = result
            .iter()
            .any(|r| r.relation_type == RelationType::Contains);
        let references_found = result
            .iter()
            .any(|r| r.relation_type == RelationType::References);

        assert!(contains_found);
        assert!(references_found);
    }

    #[tokio::test]
    async fn test_full_merge_pipeline() {
        let config = MergerConfig::default();
        let merger = HybridRelationshipMerger::new(config);
        let context = create_test_context();

        let ts_relationships = vec![
            create_test_relationship(
                "main",
                "helper",
                RelationType::Calls,
                0.8,
                RelationshipSource::TreeSitter,
            ),
            create_test_relationship(
                "class",
                "method",
                RelationType::Contains,
                0.9,
                RelationshipSource::TreeSitter,
            ),
        ];

        let lsp_relationships = vec![
            create_test_relationship(
                "main",
                "helper",
                RelationType::Calls,
                0.9,
                RelationshipSource::Lsp,
            ), // Higher confidence
            create_test_relationship(
                "service",
                "api",
                RelationType::References,
                0.8,
                RelationshipSource::Lsp,
            ),
        ];

        let result = merger
            .merge_relationships(ts_relationships, lsp_relationships, &context)
            .await
            .unwrap();

        // Should merge successfully with deduplication
        assert!(!result.is_empty());

        // Check metrics
        let metrics = merger.get_metrics();
        assert!(metrics.total_relationships_processed > 0);
        assert!(metrics.merge_time > Duration::from_nanos(0));
    }

    #[test]
    fn test_string_similarity() {
        let config = MergerConfig::default();
        let merger = HybridRelationshipMerger::new(config);

        assert_eq!(merger.calculate_string_similarity("hello", "hello"), 1.0);
        assert!(merger.calculate_string_similarity("hello", "hell") > 0.8);
        assert!(merger.calculate_string_similarity("hello", "world") < 0.5);
        assert_eq!(merger.calculate_string_similarity("", "hello"), 0.0);
    }

    #[test]
    fn test_validation() {
        let config = MergerConfig {
            strict_validation: true,
            ..Default::default()
        };
        let merger = HybridRelationshipMerger::new(config);

        // Valid relationship
        let valid_rel = create_test_relationship(
            "valid_source",
            "valid_target",
            RelationType::Calls,
            0.8,
            RelationshipSource::Lsp,
        );
        assert!(merger.validate_relationship(&valid_rel).is_ok());

        // Invalid relationship - empty source UID
        let mut invalid_rel = valid_rel.clone();
        invalid_rel.source_symbol_uid = String::new();
        assert!(merger.validate_relationship(&invalid_rel).is_err());

        // Invalid relationship - confidence out of range
        let mut invalid_rel2 = valid_rel.clone();
        invalid_rel2.confidence = 1.5;
        assert!(merger.validate_relationship(&invalid_rel2).is_err());
    }

    #[test]
    fn test_performance_config() {
        let config = MergerConfig {
            enable_parallel_processing: true,
            batch_size_threshold: 1000,
            max_concurrent_merges: 8,
            memory_limit_mb: Some(512),
            ..Default::default()
        };
        let merger = HybridRelationshipMerger::new(config);

        assert_eq!(merger.config.batch_size_threshold, 1000);
        assert_eq!(merger.config.max_concurrent_merges, 8);
        assert_eq!(merger.config.memory_limit_mb, Some(512));
        assert!(merger.config.enable_parallel_processing);
    }

    #[tokio::test]
    async fn test_large_dataset_parallel_fallback() {
        let config = MergerConfig {
            enable_parallel_processing: true,
            batch_size_threshold: 10, // Low threshold to trigger parallel processing
            memory_limit_mb: Some(1), // Very low limit to trigger fallback
            ..Default::default()
        };
        let merger = HybridRelationshipMerger::new(config);
        let context = create_test_context();

        // Create a large set of relationships
        let ts_relationships: Vec<_> = (0..20)
            .map(|i| {
                create_test_relationship(
                    &format!("source_{}", i),
                    &format!("target_{}", i),
                    RelationType::Calls,
                    0.8,
                    RelationshipSource::TreeSitter,
                )
            })
            .collect();

        let lsp_relationships: Vec<_> = (20..40)
            .map(|i| {
                create_test_relationship(
                    &format!("source_{}", i),
                    &format!("target_{}", i),
                    RelationType::References,
                    0.9,
                    RelationshipSource::Lsp,
                )
            })
            .collect();

        // Should trigger parallel processing but fall back to sequential due to memory limit
        let result = merger
            .merge_relationships(ts_relationships, lsp_relationships, &context)
            .await;

        assert!(result.is_ok());
        let merged = result.unwrap();
        assert!(!merged.is_empty());

        // Check that metrics were updated
        let metrics = merger.get_metrics();
        assert!(metrics.total_relationships_processed > 0);
    }
}
