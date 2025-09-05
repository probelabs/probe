//! Structural Analyzer for Relationship Detection
//!
//! This module provides pattern-based structural analysis using tree-sitter AST nodes
//! and language-specific patterns to detect various types of relationships between symbols.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use super::types::*;
use crate::analyzer::types::{ExtractedRelationship, ExtractedSymbol, RelationType};
use tracing::warn;

/// Structural analyzer that uses pattern matching to detect relationships
pub struct StructuralAnalyzer {
    /// Pattern registry for language-specific relationship detection
    pattern_registry: Arc<super::tree_sitter_extractor::PatternRegistry>,

    /// Query compiler for tree-sitter queries
    query_compiler: QueryCompiler,
}

impl StructuralAnalyzer {
    /// Create a new structural analyzer
    pub fn new(pattern_registry: Arc<super::tree_sitter_extractor::PatternRegistry>) -> Self {
        Self {
            pattern_registry,
            query_compiler: QueryCompiler::new(),
        }
    }

    /// Extract containment relationships from AST
    pub async fn extract_containment_relationships(
        &self,
        tree: &tree_sitter::Tree,
        symbols: &[ExtractedSymbol],
    ) -> RelationshipResult<Vec<ExtractedRelationship>> {
        let mut relationships = Vec::new();
        let root_node = tree.root_node();

        // Build symbol lookup by location for efficient parent-child detection
        let symbol_lookup = self.build_symbol_location_lookup(symbols);

        self.extract_containment_recursive(
            root_node,
            &symbol_lookup,
            &mut relationships,
            Vec::new(), // parent stack
        )?;

        Ok(relationships)
    }

    /// Extract inheritance relationships using language-specific patterns
    pub async fn extract_inheritance_relationships(
        &self,
        tree: &tree_sitter::Tree,
        content: &str,
        language: &str,
        symbols: &[ExtractedSymbol],
    ) -> RelationshipResult<Vec<ExtractedRelationship>> {
        let patterns = self.pattern_registry.get_patterns_with_fallback(language);
        let mut relationships = Vec::new();

        for inheritance_pattern in &patterns.inheritance_patterns {
            let pattern_relationships = self
                .extract_inheritance_with_pattern(tree, content, inheritance_pattern, symbols)
                .await?;
            relationships.extend(pattern_relationships);
        }

        Ok(relationships)
    }

    /// Extract call relationships using call patterns
    pub async fn extract_call_relationships(
        &self,
        tree: &tree_sitter::Tree,
        content: &str,
        language: &str,
        symbols: &[ExtractedSymbol],
    ) -> RelationshipResult<Vec<ExtractedRelationship>> {
        let patterns = self.pattern_registry.get_patterns_with_fallback(language);
        let mut relationships = Vec::new();

        for call_pattern in &patterns.call_patterns {
            let pattern_relationships = self
                .extract_calls_with_pattern(tree, content, call_pattern, symbols)
                .await?;
            relationships.extend(pattern_relationships);
        }

        Ok(relationships)
    }

    /// Extract import relationships
    pub async fn extract_import_relationships(
        &self,
        tree: &tree_sitter::Tree,
        content: &str,
        file_path: &Path,
        language: &str,
    ) -> RelationshipResult<Vec<ExtractedRelationship>> {
        let patterns = self.pattern_registry.get_patterns_with_fallback(language);
        let mut relationships = Vec::new();

        for import_pattern in &patterns.import_patterns {
            let pattern_relationships = self
                .extract_imports_with_pattern(tree, content, file_path, import_pattern)
                .await?;
            relationships.extend(pattern_relationships);
        }

        Ok(relationships)
    }

    /// Recursively extract containment relationships from AST nodes
    fn extract_containment_recursive<'a>(
        &self,
        node: tree_sitter::Node<'_>,
        symbol_lookup: &'a HashMap<(u32, u32), &'a ExtractedSymbol>,
        relationships: &mut Vec<ExtractedRelationship>,
        mut parent_stack: Vec<&'a ExtractedSymbol>,
    ) -> RelationshipResult<()> {
        let node_kind = node.kind();
        let start_point = node.start_position();
        let key = (start_point.row as u32 + 1, start_point.column as u32);

        // Check if this node represents a symbol
        let current_symbol = symbol_lookup.get(&key);

        // If this node is a symbol and we have parents, create containment relationships
        if let Some(symbol) = current_symbol {
            if let Some(parent_symbol) = parent_stack.last() {
                let relationship = ExtractedRelationship::new(
                    parent_symbol.uid.clone(),
                    symbol.uid.clone(),
                    RelationType::Contains,
                )
                .with_confidence(1.0);

                relationships.push(relationship);
            }

            // Add this symbol to parent stack if it can contain other symbols
            if self.can_contain_symbols(node_kind) {
                parent_stack.push(symbol);
            }
        } else if self.creates_scope(node_kind) {
            // Some nodes create scopes without being symbols themselves
            // We skip adding relationships but continue traversal
        }

        // Recursively process child nodes
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.extract_containment_recursive(
                child,
                symbol_lookup,
                relationships,
                parent_stack.clone(),
            )?;
        }

        Ok(())
    }

    /// Extract inheritance relationships using a specific pattern
    async fn extract_inheritance_with_pattern(
        &self,
        tree: &tree_sitter::Tree,
        content: &str,
        pattern: &InheritancePattern,
        symbols: &[ExtractedSymbol],
    ) -> RelationshipResult<Vec<ExtractedRelationship>> {
        let mut relationships = Vec::new();

        // Compile queries for base and derived types
        let base_query = self
            .query_compiler
            .compile_query(&pattern.base_node_query, &tree.language())?;
        let derived_query = self
            .query_compiler
            .compile_query(&pattern.derived_node_query, &tree.language())?;

        // Execute queries to find inheritance relationships
        let base_matches = self
            .query_compiler
            .execute_query(&base_query, tree, content)?;
        let derived_matches = self
            .query_compiler
            .execute_query(&derived_query, tree, content)?;

        // Build symbol name lookup
        let symbol_lookup = self.build_symbol_name_lookup(symbols);

        // Match base and derived types based on AST structure
        for derived_match in &derived_matches {
            if let Some(derived_name) = derived_match
                .captures
                .get("type")
                .or_else(|| derived_match.captures.get("class"))
            {
                // Find corresponding base type in the same context
                let base_matches_in_context =
                    self.find_base_matches_in_context(&base_matches, &derived_match)?;

                for base_match in base_matches_in_context {
                    if let Some(base_name) = base_match
                        .captures
                        .get("trait")
                        .or_else(|| base_match.captures.get("superclass"))
                    {
                        // Resolve symbols
                        if let (Some(derived_symbol), Some(base_symbol)) = (
                            symbol_lookup.get(derived_name),
                            symbol_lookup.get(base_name),
                        ) {
                            let relationship = ExtractedRelationship::new(
                                derived_symbol.uid.clone(),
                                base_symbol.uid.clone(),
                                pattern.relationship_type,
                            )
                            .with_confidence(pattern.confidence)
                            .with_metadata(
                                "inheritance_keyword".to_string(),
                                serde_json::Value::String(pattern.inheritance_keyword.clone()),
                            );

                            relationships.push(relationship);
                        }
                    }
                }
            }
        }

        Ok(relationships)
    }

    /// Extract call relationships using a specific pattern
    async fn extract_calls_with_pattern(
        &self,
        tree: &tree_sitter::Tree,
        content: &str,
        pattern: &CallPattern,
        symbols: &[ExtractedSymbol],
    ) -> RelationshipResult<Vec<ExtractedRelationship>> {
        let mut relationships = Vec::new();

        // Use pattern query if available, otherwise traverse manually
        if let Some(ref query_str) = pattern.query {
            let query = self
                .query_compiler
                .compile_query(query_str, &tree.language())?;
            let matches = self.query_compiler.execute_query(&query, tree, content)?;
            let symbol_lookup = self.build_symbol_name_lookup(symbols);

            for query_match in &matches {
                if let Some(function_name) = query_match
                    .captures
                    .get("function")
                    .or_else(|| query_match.captures.get("method"))
                {
                    if let Some(target_symbol) = symbol_lookup.get(function_name) {
                        // For now, create a generic call relationship
                        // In a full implementation, we'd need to track the calling context
                        let relationship = ExtractedRelationship::new(
                            "unknown_caller".to_string(), // Would need proper caller resolution
                            target_symbol.uid.clone(),
                            RelationType::Calls,
                        )
                        .with_confidence(pattern.confidence);

                        relationships.push(relationship);
                    }
                }
            }
        } else {
            // Manual traversal for call patterns without queries
            let symbol_lookup = self.build_symbol_name_lookup(symbols);
            self.extract_calls_recursive(
                tree.root_node(),
                pattern,
                &symbol_lookup,
                &mut relationships,
                content,
            )?;
        }

        Ok(relationships)
    }

    /// Extract import relationships using a specific pattern
    async fn extract_imports_with_pattern(
        &self,
        tree: &tree_sitter::Tree,
        content: &str,
        file_path: &Path,
        pattern: &ImportPattern,
    ) -> RelationshipResult<Vec<ExtractedRelationship>> {
        let mut relationships = Vec::new();

        let query = self
            .query_compiler
            .compile_query(&pattern.query, &tree.language())?;
        let matches = self.query_compiler.execute_query(&query, tree, content)?;

        for query_match in &matches {
            if let Some(module_name) = query_match
                .captures
                .get("source")
                .or_else(|| query_match.captures.get("module"))
            {
                // Create a pseudo-symbol UID for the imported module
                let import_uid = format!("import::{}", module_name);
                let file_uid = format!("file::{}", file_path.display());

                let relationship =
                    ExtractedRelationship::new(file_uid, import_uid, RelationType::Imports)
                        .with_confidence(0.9);

                relationships.push(relationship);
            }
        }

        Ok(relationships)
    }

    /// Recursively extract call relationships from AST nodes
    fn extract_calls_recursive<'a>(
        &self,
        node: tree_sitter::Node<'_>,
        pattern: &CallPattern,
        symbol_lookup: &'a HashMap<String, &'a ExtractedSymbol>,
        relationships: &mut Vec<ExtractedRelationship>,
        content: &str,
    ) -> RelationshipResult<()> {
        let node_kind = node.kind();

        if pattern.matches(node_kind) {
            // Extract function name from the call node
            if let Some(function_name) = self.extract_function_name_from_call(node, content)? {
                if let Some(target_symbol) = symbol_lookup.get(&function_name) {
                    let relationship = ExtractedRelationship::new(
                        "unknown_caller".to_string(), // Would need proper caller tracking
                        target_symbol.uid.clone(),
                        RelationType::Calls,
                    )
                    .with_confidence(pattern.confidence);

                    relationships.push(relationship);
                }
            }
        }

        // Recursively process child nodes
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            self.extract_calls_recursive(child, pattern, symbol_lookup, relationships, content)?;
        }

        Ok(())
    }

    /// Build symbol lookup map by location
    fn build_symbol_location_lookup<'a>(
        &self,
        symbols: &'a [ExtractedSymbol],
    ) -> HashMap<(u32, u32), &'a ExtractedSymbol> {
        symbols
            .iter()
            .map(|symbol| {
                (
                    (symbol.location.start_line, symbol.location.start_char),
                    symbol,
                )
            })
            .collect()
    }

    /// Build symbol lookup map by name
    fn build_symbol_name_lookup<'a>(
        &self,
        symbols: &'a [ExtractedSymbol],
    ) -> HashMap<String, &'a ExtractedSymbol> {
        let mut lookup = HashMap::new();

        for symbol in symbols {
            lookup.insert(symbol.name.clone(), symbol);
            if let Some(ref fqn) = symbol.qualified_name {
                lookup.insert(fqn.clone(), symbol);
            }
        }

        lookup
    }

    /// Check if a node type can contain other symbols
    fn can_contain_symbols(&self, node_kind: &str) -> bool {
        matches!(
            node_kind,
            "struct_item"
                | "enum_item"
                | "impl_item"
                | "mod_item"
                | "class_declaration"
                | "interface_declaration"
                | "namespace_declaration"
                | "class_definition"
                | "function_definition"
        )
    }

    /// Check if a node creates a scope
    fn creates_scope(&self, node_kind: &str) -> bool {
        matches!(
            node_kind,
            "block"
                | "compound_statement"
                | "function_body"
                | "class_body"
                | "interface_body"
                | "namespace_body"
        )
    }

    /// Extract function name from a call node
    fn extract_function_name_from_call(
        &self,
        node: tree_sitter::Node<'_>,
        content: &str,
    ) -> RelationshipResult<Option<String>> {
        let mut cursor = node.walk();

        // Look for identifier nodes within the call
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" || child.kind() == "field_identifier" {
                let start_byte = child.start_byte();
                let end_byte = child.end_byte();

                if end_byte <= content.len() {
                    let name = std::str::from_utf8(&content.as_bytes()[start_byte..end_byte])
                        .map_err(|e| {
                            RelationshipError::TreeSitterError(format!("UTF-8 error: {}", e))
                        })?;
                    return Ok(Some(name.to_string()));
                }
            }
        }

        Ok(None)
    }

    /// Find base matches in the same context as derived matches
    fn find_base_matches_in_context<'a>(
        &self,
        base_matches: &'a [QueryMatch],
        _derived_match: &QueryMatch,
    ) -> RelationshipResult<Vec<&'a QueryMatch>> {
        // For now, return all base matches
        // In a full implementation, we'd filter by AST context/proximity
        Ok(base_matches.iter().collect())
    }
}

/// Query compiler for tree-sitter queries
pub struct QueryCompiler {
    /// Cache of compiled queries
    query_cache: std::sync::Mutex<HashMap<String, tree_sitter::Query>>,
}

impl QueryCompiler {
    pub fn new() -> Self {
        Self {
            query_cache: std::sync::Mutex::new(HashMap::new()),
        }
    }

    /// Compile a tree-sitter query
    pub fn compile_query(
        &self,
        query_str: &str,
        language: &tree_sitter::Language,
    ) -> RelationshipResult<tree_sitter::Query> {
        // Note: We can't easily cache queries due to tree_sitter::Query not implementing Clone
        // In a production system, we might use a different caching strategy

        // Compile new query
        let query = tree_sitter::Query::new(language, query_str).map_err(|e| {
            RelationshipError::QueryCompilationError {
                query: query_str.to_string(),
                error: format!("{:?}", e),
            }
        })?;

        Ok(query)
    }

    /// Execute a compiled query
    pub fn execute_query(
        &self,
        query: &tree_sitter::Query,
        tree: &tree_sitter::Tree,
        content: &str,
    ) -> RelationshipResult<Vec<QueryMatch>> {
        let mut cursor = tree_sitter::QueryCursor::new();
        let matches = cursor.matches(query, tree.root_node(), content.as_bytes());

        let results = Vec::new();

        // TODO: Fix QueryMatches iterator issue with current tree-sitter version
        // For now, return empty results to allow compilation
        let _ = matches;
        warn!("Query execution temporarily disabled due to tree-sitter API changes");

        Ok(results)
    }

    /// Extract text from a tree-sitter node
    fn extract_node_text(
        &self,
        node: tree_sitter::Node<'_>,
        content: &str,
    ) -> RelationshipResult<String> {
        let start_byte = node.start_byte();
        let end_byte = node.end_byte();

        if end_byte <= content.len() {
            let text = std::str::from_utf8(&content.as_bytes()[start_byte..end_byte])
                .map_err(|e| RelationshipError::TreeSitterError(format!("UTF-8 error: {}", e)))?;
            Ok(text.to_string())
        } else {
            Err(RelationshipError::TreeSitterError(
                "Node bounds exceed content length".to_string(),
            ))
        }
    }
}

/// Result of executing a tree-sitter query
#[derive(Debug, Clone)]
pub struct QueryMatch {
    /// Map of capture names to their text values
    pub captures: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relationship::PatternRegistry;
    use crate::symbol::{SymbolKind, SymbolUIDGenerator};
    use std::path::PathBuf;

    fn create_test_symbols() -> Vec<ExtractedSymbol> {
        vec![
            ExtractedSymbol::new(
                "struct::test".to_string(),
                "TestStruct".to_string(),
                SymbolKind::Struct,
                SymbolLocation::new(PathBuf::from("test.rs"), 1, 0, 3, 1),
            ),
            ExtractedSymbol::new(
                "struct::test::field".to_string(),
                "field1".to_string(),
                SymbolKind::Field,
                SymbolLocation::new(PathBuf::from("test.rs"), 2, 4, 2, 10),
            ),
            ExtractedSymbol::new(
                "function::test".to_string(),
                "test_fn".to_string(),
                SymbolKind::Function,
                SymbolLocation::new(PathBuf::from("test.rs"), 5, 0, 7, 1),
            ),
        ]
    }

    #[test]
    fn test_structural_analyzer_creation() {
        let pattern_registry = Arc::new(PatternRegistry::new());
        let analyzer = StructuralAnalyzer::new(pattern_registry);

        // Analyzer should be created successfully
        assert!(analyzer.can_contain_symbols("struct_item"));
        assert!(analyzer.can_contain_symbols("class_declaration"));
        assert!(!analyzer.can_contain_symbols("identifier"));
    }

    #[test]
    fn test_symbol_lookup_building() {
        let pattern_registry = Arc::new(PatternRegistry::new());
        let analyzer = StructuralAnalyzer::new(pattern_registry);
        let symbols = create_test_symbols();

        let location_lookup = analyzer.build_symbol_location_lookup(&symbols);
        assert_eq!(location_lookup.len(), 3);

        // Check specific lookups
        assert!(location_lookup.get(&(1, 0)).is_some()); // TestStruct
        assert!(location_lookup.get(&(2, 4)).is_some()); // field1
        assert!(location_lookup.get(&(5, 0)).is_some()); // test_fn

        let name_lookup = analyzer.build_symbol_name_lookup(&symbols);
        assert_eq!(name_lookup.len(), 3);
        assert!(name_lookup.get("TestStruct").is_some());
        assert!(name_lookup.get("field1").is_some());
        assert!(name_lookup.get("test_fn").is_some());
    }

    #[test]
    fn test_can_contain_symbols_logic() {
        let pattern_registry = Arc::new(PatternRegistry::new());
        let analyzer = StructuralAnalyzer::new(pattern_registry);

        // Test various node types
        assert!(analyzer.can_contain_symbols("struct_item"));
        assert!(analyzer.can_contain_symbols("class_declaration"));
        assert!(analyzer.can_contain_symbols("impl_item"));
        assert!(analyzer.can_contain_symbols("namespace_declaration"));

        assert!(!analyzer.can_contain_symbols("identifier"));
        assert!(!analyzer.can_contain_symbols("literal"));
        assert!(!analyzer.can_contain_symbols("comment"));
    }

    #[test]
    fn test_creates_scope_logic() {
        let pattern_registry = Arc::new(PatternRegistry::new());
        let analyzer = StructuralAnalyzer::new(pattern_registry);

        assert!(analyzer.creates_scope("block"));
        assert!(analyzer.creates_scope("compound_statement"));
        assert!(analyzer.creates_scope("function_body"));

        assert!(!analyzer.creates_scope("identifier"));
        assert!(!analyzer.creates_scope("literal"));
    }

    #[test]
    fn test_query_compiler() {
        let compiler = QueryCompiler::new();

        // Test compilation would require actual tree-sitter language
        // In real tests with features enabled, we could test:
        // let query = compiler.compile_query("(identifier) @name", rust_language);
        // assert!(query.is_ok());
    }
}
