//! Symbol conversion utilities for transforming ExtractedSymbol data into SymbolState database records
//!
//! This module provides robust conversion functions that handle:
//! - UID generation with collision detection
//! - Comprehensive metadata serialization
//! - Field validation and error handling
//! - Batch conversion operations
//! - Performance optimizations for large symbol sets

use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use tracing::warn;

use crate::analyzer::types::ExtractedSymbol as AnalyzerExtractedSymbol;
use crate::database::SymbolState;
// Removed unused import: use crate::indexing::ast_extractor::ExtractedSymbol as AstExtractedSymbol;
use crate::indexing::language_strategies::IndexingPriority;

/// Context for symbol conversion operations
#[derive(Debug, Clone)]
pub struct ConversionContext {
    /// File path (will be normalized to relative path)
    pub file_path: PathBuf,
    /// Programming language
    pub language: String,
    /// Workspace root path for relative path calculation
    pub workspace_root: PathBuf,
    /// Additional metadata to include in conversion
    pub metadata: HashMap<String, Value>,
}

impl ConversionContext {
    /// Create a new conversion context
    pub fn new(file_path: PathBuf, language: String, workspace_root: PathBuf) -> Self {
        Self {
            file_path,
            language,
            workspace_root,
            metadata: HashMap::new(),
        }
    }

    /// Add metadata to the context
    pub fn with_metadata(mut self, key: String, value: Value) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Get relative file path for database storage
    pub fn get_relative_path(&self) -> String {
        if let Ok(relative) = self.file_path.strip_prefix(&self.workspace_root) {
            relative.to_string_lossy().to_string()
        } else {
            // Fallback to absolute path if relative calculation fails
            self.file_path.to_string_lossy().to_string()
        }
    }
}

/// Enhanced UID generator with collision detection and normalization
pub struct SymbolUIDGenerator {
    /// Track generated UIDs to detect collisions
    generated_uids: HashSet<String>,
    /// Counter for collision resolution
    collision_counter: HashMap<String, u32>,
}

impl SymbolUIDGenerator {
    /// Create a new UID generator
    pub fn new() -> Self {
        Self {
            generated_uids: HashSet::new(),
            collision_counter: HashMap::new(),
        }
    }

    /// Generate a unique UID for a symbol with collision handling
    pub fn generate_uid(
        &mut self,
        file_path: &str,
        symbol_name: &str,
        start_line: u32,
        start_char: u32,
    ) -> Result<String> {
        // Validate inputs
        if symbol_name.trim().is_empty() {
            return Err(anyhow::anyhow!("Symbol name cannot be empty"));
        }

        // Normalize file path (use forward slashes consistently)
        let normalized_path = file_path.replace('\\', "/");

        // Generate base UID
        let base_uid = format!(
            "{}:{}:{}:{}",
            normalized_path, symbol_name, start_line, start_char
        );

        // Check for collision and add disambiguator if needed
        let mut final_uid = base_uid.clone();
        let mut attempt = 0;

        while self.generated_uids.contains(&final_uid) {
            attempt += 1;
            final_uid = format!("{}#{}", base_uid, attempt);

            if attempt > 1000 {
                return Err(anyhow::anyhow!(
                    "Too many UID collisions for symbol '{}' at {}:{}:{}",
                    symbol_name,
                    normalized_path,
                    start_line,
                    start_char
                ));
            }
        }

        // Track the generated UID
        self.generated_uids.insert(final_uid.clone());
        if attempt > 0 {
            self.collision_counter.insert(base_uid, attempt);
            warn!(
                "UID collision resolved for symbol '{}' (attempt {})",
                symbol_name, attempt
            );
        }

        Ok(final_uid)
    }

    /// Get collision statistics for monitoring
    pub fn get_collision_stats(&self) -> HashMap<String, u32> {
        self.collision_counter.clone()
    }

    /// Reset the generator (useful for batch operations)
    pub fn reset(&mut self) {
        self.generated_uids.clear();
        self.collision_counter.clear();
    }
}

impl Default for SymbolUIDGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Comprehensive metadata builder for SymbolState
pub struct MetadataBuilder {
    metadata: HashMap<String, Value>,
}

impl MetadataBuilder {
    /// Create a new metadata builder
    pub fn new() -> Self {
        Self {
            metadata: HashMap::new(),
        }
    }

    /// Add priority information
    pub fn with_priority(mut self, priority: IndexingPriority) -> Self {
        self.metadata
            .insert("priority".to_string(), serde_json::json!(priority));
        self
    }

    /// Add test status
    pub fn with_test_status(mut self, is_test: bool) -> Self {
        self.metadata
            .insert("is_test".to_string(), serde_json::json!(is_test));
        self
    }

    /// Add extractor information
    pub fn with_extractor_info(mut self, extractor_type: &str, version: &str) -> Self {
        self.metadata.insert(
            "extracted_by".to_string(),
            serde_json::json!(extractor_type),
        );
        self.metadata
            .insert("extractor_version".to_string(), serde_json::json!(version));
        self
    }

    /// Add language-specific metadata
    pub fn with_language_metadata(
        mut self,
        language: &str,
        metadata: HashMap<String, Value>,
    ) -> Self {
        let mut lang_specific = HashMap::new();
        lang_specific.insert(language.to_string(), serde_json::json!(metadata));
        self.metadata.insert(
            "language_specific".to_string(),
            serde_json::json!(lang_specific),
        );
        self
    }

    /// Add symbol relationships
    pub fn with_relationships(
        mut self,
        parent_uid: Option<String>,
        namespace: Option<String>,
    ) -> Self {
        let mut relationships = HashMap::new();
        if let Some(parent) = parent_uid {
            relationships.insert("parent_symbol".to_string(), serde_json::json!(parent));
        }
        if let Some(ns) = namespace {
            relationships.insert("namespace".to_string(), serde_json::json!(ns));
        }
        if !relationships.is_empty() {
            self.metadata.insert(
                "symbol_relationships".to_string(),
                serde_json::json!(relationships),
            );
        }
        self
    }

    /// Add custom metadata
    pub fn with_custom(mut self, key: String, value: Value) -> Self {
        self.metadata.insert(key, value);
        self
    }

    /// Build the metadata JSON string
    pub fn build(self) -> Result<Option<String>> {
        if self.metadata.is_empty() {
            return Ok(None);
        }

        serde_json::to_string(&self.metadata)
            .map(Some)
            .context("Failed to serialize metadata to JSON")
    }
}

impl Default for MetadataBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Field validator for SymbolState conversion
pub struct FieldValidator;

impl FieldValidator {
    /// Validate symbol name
    pub fn validate_name(name: &str) -> Result<()> {
        if name.trim().is_empty() {
            return Err(anyhow::anyhow!("Symbol name cannot be empty"));
        }
        if name.len() > 1000 {
            return Err(anyhow::anyhow!(
                "Symbol name too long (max 1000 characters)"
            ));
        }
        Ok(())
    }

    /// Validate symbol kind
    pub fn validate_kind(kind: &str) -> Result<()> {
        if kind.trim().is_empty() {
            return Err(anyhow::anyhow!("Symbol kind cannot be empty"));
        }
        Ok(())
    }

    /// Validate position information
    pub fn validate_position(
        start_line: u32,
        start_char: u32,
        end_line: u32,
        end_char: u32,
    ) -> Result<()> {
        if start_line > end_line {
            return Err(anyhow::anyhow!(
                "Start line ({}) cannot be greater than end line ({})",
                start_line,
                end_line
            ));
        }
        if start_line == end_line && start_char > end_char {
            return Err(anyhow::anyhow!(
                "Start char ({}) cannot be greater than end char ({}) on same line",
                start_char,
                end_char
            ));
        }
        Ok(())
    }

    /// Validate file path
    pub fn validate_file_path(path: &str) -> Result<()> {
        if path.trim().is_empty() {
            return Err(anyhow::anyhow!(
                "File path cannot be empty. This indicates a bug in AST extraction or symbol conversion."
            ));
        }
        if path.len() > 4096 {
            return Err(anyhow::anyhow!("File path too long (max 4096 characters)"));
        }
        // Additional check for common placeholder paths that indicate bugs
        if path == "unknown" || path == "" {
            return Err(anyhow::anyhow!(
                "File path is placeholder '{}'. This indicates a bug in AST extraction.",
                path
            ));
        }
        Ok(())
    }

    /// Validate optional string field
    pub fn validate_optional_string(
        value: &Option<String>,
        field_name: &str,
        max_length: usize,
    ) -> Result<()> {
        if let Some(s) = value {
            if s.len() > max_length {
                return Err(anyhow::anyhow!(
                    "{} too long (max {} characters)",
                    field_name,
                    max_length
                ));
            }
        }
        Ok(())
    }
}

/// Trait for converting different ExtractedSymbol types to SymbolState
pub trait ToSymbolState {
    /// Convert to SymbolState with validation
    fn to_symbol_state_validated(
        &self,
        context: &ConversionContext,
        uid_generator: &mut SymbolUIDGenerator,
    ) -> Result<SymbolState>;
}

// Note: AstExtractedSymbol now uses the same type as AnalyzerExtractedSymbol,
// so we use the same ToSymbolState implementation

/// Implementation for Analyzer ExtractedSymbol
impl ToSymbolState for AnalyzerExtractedSymbol {
    fn to_symbol_state_validated(
        &self,
        context: &ConversionContext,
        uid_generator: &mut SymbolUIDGenerator,
    ) -> Result<SymbolState> {
        // Validate inputs
        FieldValidator::validate_name(&self.name)?;
        FieldValidator::validate_position(
            self.location.start_line,
            self.location.start_char,
            self.location.end_line,
            self.location.end_char,
        )?;

        let relative_path = context.get_relative_path();
        FieldValidator::validate_file_path(&relative_path)?;

        // Validate optional fields
        FieldValidator::validate_optional_string(&self.qualified_name, "Qualified Name", 2000)?;
        FieldValidator::validate_optional_string(&self.signature, "Signature", 5000)?;
        FieldValidator::validate_optional_string(&self.documentation, "Documentation", 50000)?;

        // Generate UID (use existing one if available, otherwise generate)
        let symbol_uid = if !self.uid.is_empty() {
            self.uid.clone()
        } else {
            uid_generator.generate_uid(
                &relative_path,
                &self.name,
                self.location.start_line,
                self.location.start_char,
            )?
        };

        // Build metadata from analyzer-specific data
        let mut metadata_builder = MetadataBuilder::new().with_extractor_info("analyzer", "1.0");

        // Convert existing metadata
        if !self.metadata.is_empty() {
            for (key, value) in &self.metadata {
                metadata_builder = metadata_builder.with_custom(key.clone(), value.clone());
            }
        }

        // Add parent scope relationship if available
        if let Some(parent) = &self.parent_scope {
            metadata_builder = metadata_builder.with_relationships(Some(parent.clone()), None);
        }

        // Add tags as metadata
        if !self.tags.is_empty() {
            metadata_builder =
                metadata_builder.with_custom("tags".to_string(), serde_json::json!(self.tags));
        }

        // Add context metadata
        for (key, value) in &context.metadata {
            metadata_builder = metadata_builder.with_custom(key.clone(), value.clone());
        }

        let metadata = metadata_builder.build()?;

        Ok(SymbolState {
            symbol_uid,
            file_path: relative_path,
            language: context.language.clone(),
            name: self.name.clone(),
            fqn: self.qualified_name.clone(),
            kind: self.kind.to_string(),
            signature: self.signature.clone(),
            visibility: self.visibility.as_ref().map(|v| v.to_string()),
            def_start_line: self.location.start_line,
            def_start_char: self.location.start_char,
            def_end_line: self.location.end_line,
            def_end_char: self.location.end_char,
            is_definition: true, // Analyzer symbols are typically definitions
            documentation: self.documentation.clone(),
            metadata,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::indexing::language_strategies::IndexingPriority;

    #[test]
    fn test_uid_generator_basic() {
        let mut generator = SymbolUIDGenerator::new();

        let uid = generator.generate_uid("src/main.rs", "main", 1, 0).unwrap();
        assert_eq!(uid, "src/main.rs:main:1:0");
    }

    #[test]
    fn test_uid_generator_collision_handling() {
        let mut generator = SymbolUIDGenerator::new();

        // Generate the same UID twice
        let uid1 = generator.generate_uid("src/main.rs", "main", 1, 0).unwrap();
        let uid2 = generator.generate_uid("src/main.rs", "main", 1, 0).unwrap();

        assert_eq!(uid1, "src/main.rs:main:1:0");
        assert_eq!(uid2, "src/main.rs:main:1:0#1");

        // Check collision stats
        let stats = generator.get_collision_stats();
        assert_eq!(stats.get("src/main.rs:main:1:0"), Some(&1));
    }

    #[test]
    fn test_uid_generator_path_normalization() {
        let mut generator = SymbolUIDGenerator::new();

        let uid = generator
            .generate_uid("src\\main.rs", "main", 1, 0)
            .unwrap();
        assert_eq!(uid, "src/main.rs:main:1:0");
    }

    #[test]
    fn test_metadata_builder() {
        let metadata = MetadataBuilder::new()
            .with_priority(IndexingPriority::High)
            .with_test_status(true)
            .with_extractor_info("ast", "1.0")
            .build()
            .unwrap()
            .unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&metadata).unwrap();
        assert_eq!(parsed["priority"], "High");
        assert_eq!(parsed["is_test"], true);
        assert_eq!(parsed["extracted_by"], "ast");
        assert_eq!(parsed["extractor_version"], "1.0");
    }

    #[test]
    fn test_field_validator() {
        // Valid cases
        assert!(FieldValidator::validate_name("valid_name").is_ok());
        assert!(FieldValidator::validate_kind("function").is_ok());
        assert!(FieldValidator::validate_position(1, 0, 1, 10).is_ok());
        assert!(FieldValidator::validate_position(1, 0, 2, 0).is_ok());

        // Invalid cases
        assert!(FieldValidator::validate_name("").is_err());
        assert!(FieldValidator::validate_name(&"x".repeat(1001)).is_err());
        assert!(FieldValidator::validate_kind("").is_err());
        assert!(FieldValidator::validate_position(2, 0, 1, 0).is_err());
        assert!(FieldValidator::validate_position(1, 10, 1, 5).is_err());
    }

    #[test]
    fn test_conversion_context() {
        let context = ConversionContext::new(
            PathBuf::from("/workspace/src/main.rs"),
            "rust".to_string(),
            PathBuf::from("/workspace"),
        );

        assert_eq!(context.get_relative_path(), "src/main.rs");
    }
}
