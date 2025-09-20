//! LSP Semantic Relationship Enhancer
//!
//! This module provides LSP-powered semantic enhancement of tree-sitter relationships,
//! adding cross-file semantic relationships and improving relationship accuracy using
//! the existing LSP daemon infrastructure.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::time::{timeout, Duration};
use tracing::{debug, error, info, warn};

use super::lsp_client_wrapper::LspClientWrapper;
use crate::analyzer::types::{
    AnalysisContext, ExtractedRelationship, ExtractedSymbol, RelationType,
};
use crate::language_detector::LanguageDetector;
use crate::protocol::{CallHierarchyResult, Location, Position, Range};
use crate::server_manager::SingleServerManager;
use crate::symbol::{SymbolLocation, SymbolUIDGenerator};
use crate::workspace_resolver::WorkspaceResolver;

/// Configuration for LSP relationship enhancement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspEnhancementConfig {
    /// Enabled LSP relationship types
    pub enabled_relationship_types: Vec<LspRelationshipType>,

    /// Whether to cache LSP responses
    pub cache_lsp_responses: bool,

    /// Timeout for LSP operations in milliseconds
    pub timeout_ms: u64,

    /// Maximum references to process per symbol
    pub max_references_per_symbol: usize,

    /// Whether to enable cross-file analysis
    pub cross_file_analysis: bool,

    /// Minimum confidence for LSP relationships
    pub min_lsp_confidence: f32,

    /// Whether to merge with tree-sitter relationships
    pub merge_with_tree_sitter: bool,

    /// Whether to prefer LSP results over tree-sitter when conflicts occur
    pub prefer_lsp_over_tree_sitter: bool,
}

impl Default for LspEnhancementConfig {
    fn default() -> Self {
        Self {
            enabled_relationship_types: vec![
                LspRelationshipType::References,
                LspRelationshipType::IncomingCalls,
                LspRelationshipType::OutgoingCalls,
                LspRelationshipType::Definition,
                LspRelationshipType::Implementation,
            ],
            cache_lsp_responses: true,
            timeout_ms: 5000,
            max_references_per_symbol: 100,
            cross_file_analysis: true,
            min_lsp_confidence: 0.8,
            merge_with_tree_sitter: true,
            prefer_lsp_over_tree_sitter: false,
        }
    }
}

/// LSP relationship types supported by the enhancer
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LspRelationshipType {
    /// textDocument/references
    References,

    /// textDocument/definition
    Definition,

    /// callHierarchy/incomingCalls
    IncomingCalls,

    /// callHierarchy/outgoingCalls
    OutgoingCalls,

    /// textDocument/implementation
    Implementation,

    /// textDocument/typeDefinition
    TypeDefinition,

    /// textDocument/hover (for symbol resolution)
    Hover,
}

impl LspRelationshipType {
    /// Convert to RelationType for database storage
    pub fn to_relation_type(&self) -> RelationType {
        match self {
            LspRelationshipType::References => RelationType::References,
            LspRelationshipType::Definition => RelationType::References, // Map to references
            LspRelationshipType::IncomingCalls => RelationType::Calls,
            LspRelationshipType::OutgoingCalls => RelationType::Calls,
            LspRelationshipType::Implementation => RelationType::Implements,
            LspRelationshipType::TypeDefinition => RelationType::TypeOf,
            LspRelationshipType::Hover => RelationType::References, // Fallback
        }
    }
}

/// Error types specific to LSP enhancement
#[derive(Debug, thiserror::Error)]
pub enum LspEnhancementError {
    #[error("LSP server not available for file: {file_path}")]
    LspNotAvailable { file_path: String },

    #[error("LSP timeout after {timeout_ms}ms for operation: {operation}")]
    LspTimeout { operation: String, timeout_ms: u64 },

    #[error("Failed to resolve symbol UID: {symbol} - {error}")]
    SymbolResolutionError { symbol: String, error: String },

    #[error("Invalid LSP response format: {method} - {error}")]
    InvalidLspResponse { method: String, error: String },

    #[error("Cache error: {0}")]
    CacheError(#[from] anyhow::Error),

    #[error("Internal enhancer error: {message}")]
    InternalError { message: String },
}

/// LSP-powered relationship enhancer
pub struct LspRelationshipEnhancer {
    /// LSP client wrapper for operations
    lsp_client: Option<Arc<LspClientWrapper>>,

    /// Symbol UID generator
    uid_generator: Arc<SymbolUIDGenerator>,

    /// Enhancement configuration
    config: LspEnhancementConfig,
}

impl LspRelationshipEnhancer {
    /// Create a new LSP relationship enhancer
    pub fn new(
        server_manager: Option<Arc<SingleServerManager>>,
        language_detector: Arc<LanguageDetector>,
        workspace_resolver: Arc<tokio::sync::Mutex<WorkspaceResolver>>,
        uid_generator: Arc<SymbolUIDGenerator>,
    ) -> Self {
        Self::with_config(
            server_manager,
            language_detector,
            workspace_resolver,
            uid_generator,
            LspEnhancementConfig::default(),
        )
    }

    /// Create a new LSP relationship enhancer with custom configuration
    pub fn with_config(
        server_manager: Option<Arc<SingleServerManager>>,
        language_detector: Arc<LanguageDetector>,
        workspace_resolver: Arc<tokio::sync::Mutex<WorkspaceResolver>>,
        uid_generator: Arc<SymbolUIDGenerator>,
        config: LspEnhancementConfig,
    ) -> Self {
        let lsp_client = server_manager.map(|sm| {
            Arc::new(LspClientWrapper::new(
                sm,
                language_detector,
                workspace_resolver,
            ))
        });

        Self {
            lsp_client,
            uid_generator,
            config,
        }
    }

    /// Enhance tree-sitter relationships with LSP semantic data
    pub async fn enhance_relationships(
        &self,
        file_path: &Path,
        tree_sitter_relationships: Vec<ExtractedRelationship>,
        symbols: &[ExtractedSymbol],
        _context: &AnalysisContext,
    ) -> Result<Vec<ExtractedRelationship>, LspEnhancementError> {
        debug!(
            "Enhancing {} tree-sitter relationships with LSP for file: {:?}",
            tree_sitter_relationships.len(),
            file_path
        );

        // If no LSP client available, return original relationships
        if self.lsp_client.is_none() {
            debug!("No LSP client available, returning tree-sitter relationships unchanged");
            return Ok(tree_sitter_relationships);
        }

        let mut enhanced_relationships = if self.config.merge_with_tree_sitter {
            tree_sitter_relationships
        } else {
            Vec::new()
        };

        // Get additional LSP relationships
        let lsp_relationships = self
            .get_lsp_relationships(file_path, symbols, &self.config.enabled_relationship_types)
            .await
            .unwrap_or_else(|e| {
                warn!("Failed to get LSP relationships: {}", e);
                Vec::new()
            });

        // Merge relationships
        if self.config.merge_with_tree_sitter {
            enhanced_relationships.extend(lsp_relationships.clone());
            self.deduplicate_relationships(&mut enhanced_relationships);
        } else {
            enhanced_relationships = lsp_relationships.clone();
        }

        let tree_sitter_count = if self.config.merge_with_tree_sitter {
            enhanced_relationships
                .len()
                .saturating_sub(lsp_relationships.len())
        } else {
            0
        };

        info!(
            "Enhanced relationships count: {} (from {} tree-sitter + {} LSP)",
            enhanced_relationships.len(),
            tree_sitter_count,
            lsp_relationships.len()
        );

        Ok(enhanced_relationships)
    }

    /// Get semantic relationships using LSP
    pub async fn get_lsp_relationships(
        &self,
        file_path: &Path,
        symbols: &[ExtractedSymbol],
        relationship_types: &[LspRelationshipType],
    ) -> Result<Vec<ExtractedRelationship>, LspEnhancementError> {
        let lsp_client =
            self.lsp_client
                .as_ref()
                .ok_or_else(|| LspEnhancementError::LspNotAvailable {
                    file_path: file_path.to_string_lossy().to_string(),
                })?;

        let mut all_relationships = Vec::new();

        // Process each symbol up to the configured limit
        let symbols_to_process = symbols
            .iter()
            .take(self.config.max_references_per_symbol)
            .collect::<Vec<_>>();

        let symbol_count = symbols_to_process.len();

        for symbol in symbols_to_process {
            for relationship_type in relationship_types {
                let relationships = self
                    .extract_relationship_type(file_path, symbol, relationship_type, lsp_client)
                    .await
                    .unwrap_or_else(|e| {
                        debug!(
                            "Failed to extract {} for symbol {}: {}",
                            format!("{:?}", relationship_type),
                            symbol.name,
                            e
                        );
                        Vec::new()
                    });

                all_relationships.extend(relationships);
            }
        }

        // Filter by confidence threshold
        let filtered_relationships: Vec<_> = all_relationships
            .into_iter()
            .filter(|r| r.confidence >= self.config.min_lsp_confidence)
            .collect();

        debug!(
            "Extracted {} LSP relationships for {} symbols",
            filtered_relationships.len(),
            symbol_count
        );

        Ok(filtered_relationships)
    }

    /// Extract relationships for a specific LSP relationship type
    async fn extract_relationship_type(
        &self,
        file_path: &Path,
        symbol: &ExtractedSymbol,
        relationship_type: &LspRelationshipType,
        lsp_client: &Arc<LspClientWrapper>,
    ) -> Result<Vec<ExtractedRelationship>, LspEnhancementError> {
        let timeout_duration = Duration::from_millis(self.config.timeout_ms);

        let result = timeout(
            timeout_duration,
            self.extract_relationship_type_inner(file_path, symbol, relationship_type, lsp_client),
        )
        .await
        .map_err(|_| LspEnhancementError::LspTimeout {
            operation: format!("{:?}", relationship_type),
            timeout_ms: self.config.timeout_ms,
        })?;

        result
    }

    /// Internal implementation for relationship extraction
    async fn extract_relationship_type_inner(
        &self,
        file_path: &Path,
        symbol: &ExtractedSymbol,
        relationship_type: &LspRelationshipType,
        lsp_client: &Arc<LspClientWrapper>,
    ) -> Result<Vec<ExtractedRelationship>, LspEnhancementError> {
        match relationship_type {
            LspRelationshipType::References => {
                self.extract_references(file_path, symbol, lsp_client).await
            }
            LspRelationshipType::Definition => {
                self.extract_definitions(file_path, symbol, lsp_client)
                    .await
            }
            LspRelationshipType::IncomingCalls | LspRelationshipType::OutgoingCalls => {
                self.extract_call_hierarchy(file_path, symbol, relationship_type, lsp_client)
                    .await
            }
            LspRelationshipType::Implementation => {
                self.extract_implementations(file_path, symbol, lsp_client)
                    .await
            }
            LspRelationshipType::TypeDefinition => {
                self.extract_type_definitions(file_path, symbol, lsp_client)
                    .await
            }
            LspRelationshipType::Hover => {
                // Hover is used for symbol resolution, not relationship extraction
                Ok(Vec::new())
            }
        }
    }

    /// Extract references using LSP textDocument/references
    async fn extract_references(
        &self,
        file_path: &Path,
        symbol: &ExtractedSymbol,
        lsp_client: &Arc<LspClientWrapper>,
    ) -> Result<Vec<ExtractedRelationship>, LspEnhancementError> {
        debug!(
            "Extracting references for symbol: {} at {:?}",
            symbol.name, file_path
        );

        // Get cached or fresh references
        let _cache_key = format!(
            "references:{}:{}:{}",
            file_path.to_string_lossy(),
            symbol.location.start_line,
            symbol.location.start_char
        );

        // Cache support would be implemented here with proper cache methods
        // For now, skip caching to get the basic functionality working

        // Use the LSP client wrapper to get references
        let locations = lsp_client
            .get_references(
                file_path,
                symbol.location.start_line,
                symbol.location.start_char,
                false,
                self.config.timeout_ms,
            )
            .await
            .unwrap_or_else(|e| {
                debug!("Failed to get references: {}", e);
                Vec::new()
            });

        self.locations_to_relationships(&locations, symbol, RelationType::References)
            .await
    }

    /// Extract definitions using LSP textDocument/definition
    async fn extract_definitions(
        &self,
        file_path: &Path,
        symbol: &ExtractedSymbol,
        lsp_client: &Arc<LspClientWrapper>,
    ) -> Result<Vec<ExtractedRelationship>, LspEnhancementError> {
        debug!(
            "Extracting definitions for symbol: {} at {:?}",
            symbol.name, file_path
        );

        // Use the LSP client wrapper to get definitions
        let locations = lsp_client
            .get_definition(
                file_path,
                symbol.location.start_line,
                symbol.location.start_char,
                self.config.timeout_ms,
            )
            .await
            .unwrap_or_else(|e| {
                debug!("Failed to get definitions: {}", e);
                Vec::new()
            });

        self.locations_to_relationships(&locations, symbol, RelationType::References)
            .await
    }

    /// Extract call hierarchy using LSP callHierarchy methods
    async fn extract_call_hierarchy(
        &self,
        file_path: &Path,
        symbol: &ExtractedSymbol,
        relationship_type: &LspRelationshipType,
        lsp_client: &Arc<LspClientWrapper>,
    ) -> Result<Vec<ExtractedRelationship>, LspEnhancementError> {
        debug!(
            "Extracting call hierarchy ({:?}) for symbol: {} at {:?}",
            relationship_type, symbol.name, file_path
        );

        // Use the LSP client wrapper to get call hierarchy
        let call_hierarchy = lsp_client
            .get_call_hierarchy(
                file_path,
                symbol.location.start_line,
                symbol.location.start_char,
                self.config.timeout_ms,
            )
            .await
            .unwrap_or_else(|e| {
                debug!("Failed to get call hierarchy: {}", e);
                return CallHierarchyResult {
                    item: crate::protocol::CallHierarchyItem {
                        name: "unknown".to_string(),
                        kind: "unknown".to_string(),
                        uri: String::new(),
                        range: crate::protocol::Range {
                            start: Position {
                                line: 0,
                                character: 0,
                            },
                            end: Position {
                                line: 0,
                                character: 0,
                            },
                        },
                        selection_range: crate::protocol::Range {
                            start: Position {
                                line: 0,
                                character: 0,
                            },
                            end: Position {
                                line: 0,
                                character: 0,
                            },
                        },
                    },
                    incoming: Vec::new(),
                    outgoing: Vec::new(),
                };
            });

        let mut relationships = Vec::new();

        match relationship_type {
            LspRelationshipType::IncomingCalls => {
                for call in call_hierarchy.incoming {
                    // Convert call hierarchy calls to relationships
                    let source_uid = self.generate_fallback_uid(&call.from.uri, &call.from.range);

                    let from_file_path = self
                        .uri_to_path(&call.from.uri)
                        .unwrap_or_else(|_| PathBuf::from("unknown"));
                    relationships.push(ExtractedRelationship {
                        source_symbol_uid: source_uid,
                        target_symbol_uid: symbol.uid.clone(),
                        relation_type: RelationType::Calls,
                        location: Some(
                            self.lsp_range_to_symbol_location(&call.from.range, &from_file_path),
                        ),
                        confidence: 1.0,
                        metadata: HashMap::new(),
                    });
                }
            }
            LspRelationshipType::OutgoingCalls => {
                for call in call_hierarchy.outgoing {
                    // Convert call hierarchy calls to relationships
                    let target_uid = self.generate_fallback_uid(&call.from.uri, &call.from.range);

                    let to_file_path = self
                        .uri_to_path(&call.from.uri)
                        .unwrap_or_else(|_| PathBuf::from("unknown"));
                    relationships.push(ExtractedRelationship {
                        source_symbol_uid: symbol.uid.clone(),
                        target_symbol_uid: target_uid,
                        relation_type: RelationType::Calls,
                        location: Some(
                            self.lsp_range_to_symbol_location(&call.from.range, &to_file_path),
                        ),
                        confidence: 1.0,
                        metadata: HashMap::new(),
                    });
                }
            }
            _ => {} // Other types not relevant here
        }

        Ok(relationships)
    }

    /// Extract implementations using LSP textDocument/implementation
    async fn extract_implementations(
        &self,
        file_path: &Path,
        symbol: &ExtractedSymbol,
        lsp_client: &Arc<LspClientWrapper>,
    ) -> Result<Vec<ExtractedRelationship>, LspEnhancementError> {
        debug!(
            "Extracting implementations for symbol: {} at {:?}",
            symbol.name, file_path
        );

        // Use the LSP client wrapper to get implementations
        let locations = lsp_client
            .get_implementation(
                file_path,
                symbol.location.start_line,
                symbol.location.start_char,
                self.config.timeout_ms,
            )
            .await
            .unwrap_or_else(|e| {
                debug!("Failed to get implementations: {}", e);
                Vec::new()
            });

        self.locations_to_relationships(&locations, symbol, RelationType::Implements)
            .await
    }

    /// Extract type definitions using LSP textDocument/typeDefinition
    /// Note: This method is not yet implemented in the server manager
    async fn extract_type_definitions(
        &self,
        file_path: &Path,
        symbol: &ExtractedSymbol,
        _lsp_client: &Arc<LspClientWrapper>,
    ) -> Result<Vec<ExtractedRelationship>, LspEnhancementError> {
        debug!(
            "Extracting type definitions for symbol: {} at {:?}",
            symbol.name, file_path
        );

        // Type definition method is not yet available in server manager
        warn!("LSP type definition method not yet available in server manager");
        Ok(Vec::new())
    }

    /// Convert LSP locations to relationships
    async fn locations_to_relationships(
        &self,
        locations: &[Location],
        target_symbol: &ExtractedSymbol,
        relation_type: RelationType,
    ) -> Result<Vec<ExtractedRelationship>, LspEnhancementError> {
        let mut relationships = Vec::new();

        for location in locations {
            // Resolve symbol UID for the source location
            let source_symbol_uid = self
                .resolve_symbol_uid_at_location(&location.uri, &location.range)
                .await
                .unwrap_or_else(|e| {
                    debug!("Failed to resolve symbol UID at location: {}", e);
                    self.generate_fallback_uid(&location.uri, &location.range)
                });

            let file_path = self
                .uri_to_path(&location.uri)
                .unwrap_or_else(|_| PathBuf::from("unknown"));
            let relationship = ExtractedRelationship {
                source_symbol_uid,
                target_symbol_uid: target_symbol.uid.clone(),
                relation_type: relation_type.clone(),
                location: Some(self.lsp_range_to_symbol_location(&location.range, &file_path)),
                confidence: 1.0, // LSP is authoritative
                metadata: HashMap::new(),
            };

            relationships.push(relationship);
        }

        Ok(relationships)
    }

    /// Resolve symbol UID at a specific location using LSP hover
    async fn resolve_symbol_uid_at_location(
        &self,
        uri: &str,
        range: &Range,
    ) -> Result<String, LspEnhancementError> {
        let _file_path = self.uri_to_path(uri)?;

        // Try to get hover information for symbol resolution
        // This would use the server manager to get hover info
        // For now, return a fallback UID
        let fallback_uid = self.generate_fallback_uid(uri, range);

        debug!(
            "Would resolve symbol UID using LSP hover at {}:{}",
            uri, range.start.line
        );
        Ok(fallback_uid)
    }

    /// Generate a fallback UID when LSP resolution fails
    fn generate_fallback_uid(&self, uri: &str, range: &Range) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        uri.hash(&mut hasher);
        range.start.line.hash(&mut hasher);
        range.start.character.hash(&mut hasher);

        format!("fallback_{:x}", hasher.finish())
    }

    /// Convert LSP Range to SymbolLocation
    fn lsp_range_to_symbol_location(&self, range: &Range, file_path: &Path) -> SymbolLocation {
        SymbolLocation {
            file_path: file_path.to_path_buf(),
            start_line: range.start.line,
            start_char: range.start.character,
            end_line: range.end.line,
            end_char: range.end.character,
        }
    }

    /// Convert URI to file path
    fn uri_to_path(&self, uri: &str) -> Result<PathBuf, LspEnhancementError> {
        if uri.starts_with("file://") {
            Ok(PathBuf::from(&uri[7..]))
        } else {
            Ok(PathBuf::from(uri))
        }
    }

    /// Deduplicate relationships by removing exact duplicates
    fn deduplicate_relationships(&self, relationships: &mut Vec<ExtractedRelationship>) {
        let mut seen = HashSet::new();
        relationships.retain(|r| {
            let key = (
                r.source_symbol_uid.clone(),
                r.target_symbol_uid.clone(),
                r.relation_type.clone(),
            );
            seen.insert(key)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lsp_relationship_type_conversion() {
        assert_eq!(
            LspRelationshipType::References.to_relation_type(),
            RelationType::References
        );
        assert_eq!(
            LspRelationshipType::IncomingCalls.to_relation_type(),
            RelationType::Calls
        );
        assert_eq!(
            LspRelationshipType::Definition.to_relation_type(),
            RelationType::References
        );
    }

    #[test]
    fn test_lsp_enhancement_config_defaults() {
        let config = LspEnhancementConfig::default();
        assert!(config
            .enabled_relationship_types
            .contains(&LspRelationshipType::References));
        assert!(config
            .enabled_relationship_types
            .contains(&LspRelationshipType::IncomingCalls));
        assert!(config.cache_lsp_responses);
        assert_eq!(config.timeout_ms, 5000);
        assert_eq!(config.max_references_per_symbol, 100);
    }
}
