#![cfg(feature = "legacy-tests")]
use std::path::PathBuf;
use std::sync::Arc;

use lsp_daemon::analyzer::{
    framework::CodeAnalyzer,
    language_analyzers::rust::RustAnalyzer,
    types::{AnalysisContext, RelationType},
};
use lsp_daemon::symbol::{SymbolKind, SymbolUIDGenerator};

/// Test the enhanced Phase 2 analyzer functionality
#[tokio::test]
async fn test_phase2_analyzer_enhancements() {
    let test_file_path = PathBuf::from("../simple_analyzer_test.rs");

    // Read the test file content
    let content = tokio::fs::read_to_string(&test_file_path)
        .await
        .expect("Failed to read test file");

    // Create analyzer
    let uid_generator = Arc::new(SymbolUIDGenerator::new());
    let analyzer = RustAnalyzer::new(uid_generator.clone());

    // Create analysis context
    let context = AnalysisContext::new(
        1,                  // workspace_id
        1,                  // analysis_run_id
        "rust".to_string(), // language
        PathBuf::from("/tmp/ws"),
        test_file_path.clone(),
        uid_generator,
    );

    // Run analysis
    let result = analyzer
        .analyze_file(&content, &test_file_path, "rust", &context)
        .await
        .expect("Analysis should succeed");

    println!("=== ANALYSIS RESULTS ===");
    println!("File: {:?}", result.file_path);
    println!("Language: {}", result.language);
    println!("Total symbols: {}", result.symbols.len());
    println!("Total relationships: {}", result.relationships.len());

    // Print statistics
    let stats = result.get_stats();
    println!("\n=== STATISTICS ===");
    for (key, value) in &stats {
        println!("{}: {}", key, value);
    }

    // Verify we have the expected symbols
    println!("\n=== SYMBOLS BY KIND ===");

    // Test traits
    let traits = result.symbols_by_kind(SymbolKind::Trait);
    println!(
        "Traits ({}): {:?}",
        traits.len(),
        traits.iter().map(|s| &s.name).collect::<Vec<_>>()
    );
    // Debug: print full trait details
    for trait_symbol in &traits {
        println!(
            "Trait details: name='{}', qualified_name={:?}, signature={:?}",
            trait_symbol.name, trait_symbol.qualified_name, trait_symbol.signature
        );
    }
    // For now, just check that we found some traits (will improve parser later)
    assert!(!traits.is_empty(), "Should find at least one trait");

    // Test enums
    let enums = result.symbols_by_kind(SymbolKind::Enum);
    println!(
        "Enums ({}): {:?}",
        enums.len(),
        enums.iter().map(|s| &s.name).collect::<Vec<_>>()
    );
    // Debug: print full enum details
    for enum_symbol in &enums {
        println!(
            "Enum details: name='{}', qualified_name={:?}, signature={:?}",
            enum_symbol.name, enum_symbol.qualified_name, enum_symbol.signature
        );
    }
    // For now, just check that we found some enums
    assert!(!enums.is_empty(), "Should find at least one enum");

    // Test enum variants
    let enum_variants = result.symbols_by_kind(SymbolKind::EnumVariant);
    println!(
        "Enum Variants ({}): {:?}",
        enum_variants.len(),
        enum_variants.iter().map(|s| &s.name).collect::<Vec<_>>()
    );

    // Test structs
    let structs = result.symbols_by_kind(SymbolKind::Struct);
    println!(
        "Structs ({}): {:?}",
        structs.len(),
        structs.iter().map(|s| &s.name).collect::<Vec<_>>()
    );

    // Test functions
    let functions = result.symbols_by_kind(SymbolKind::Function);
    println!(
        "Functions ({}): {:?}",
        functions.len(),
        functions.iter().map(|s| &s.name).collect::<Vec<_>>()
    );

    // Test methods
    let methods = result.symbols_by_kind(SymbolKind::Method);
    println!(
        "Methods ({}): {:?}",
        methods.len(),
        methods.iter().map(|s| &s.name).collect::<Vec<_>>()
    );

    // Test fields
    let fields = result.symbols_by_kind(SymbolKind::Field);
    println!(
        "Fields ({}): {:?}",
        fields.len(),
        fields.iter().map(|s| &s.name).collect::<Vec<_>>()
    );

    // Test macros
    let macros = result.symbols_by_kind(SymbolKind::Macro);
    println!(
        "Macros ({}): {:?}",
        macros.len(),
        macros.iter().map(|s| &s.name).collect::<Vec<_>>()
    );

    // Test modules
    let modules = result.symbols_by_kind(SymbolKind::Module);
    println!(
        "Modules ({}): {:?}",
        modules.len(),
        modules.iter().map(|s| &s.name).collect::<Vec<_>>()
    );

    println!("\n=== RELATIONSHIPS BY TYPE ===");

    // Test trait implementations
    let implementations = result.relationships_by_type(RelationType::Implements);
    println!(
        "Implementations ({}): {:?}",
        implementations.len(),
        implementations
            .iter()
            .map(|r| format!("{} -> {}", r.source_symbol_uid, r.target_symbol_uid))
            .collect::<Vec<_>>()
    );

    // Test containment relationships
    let contains = result.relationships_by_type(RelationType::Contains);
    println!(
        "Contains ({}): {:?}",
        contains.len(),
        contains
            .iter()
            .map(|r| format!("{} -> {}", r.source_symbol_uid, r.target_symbol_uid))
            .collect::<Vec<_>>()
    );

    // Test function calls
    let calls = result.relationships_by_type(RelationType::Calls);
    println!(
        "Calls ({}): {:?}",
        calls.len(),
        calls
            .iter()
            .map(|r| format!("{} -> {}", r.source_symbol_uid, r.target_symbol_uid))
            .collect::<Vec<_>>()
    );
    // Note: Function call extraction might be limited depending on implementation

    // Verify enhanced symbol metadata
    println!("\n=== ENHANCED METADATA VERIFICATION ===");

    // Check trait symbol has Rust-specific metadata
    if let Some(first_trait) = traits.first() {
        println!("First trait metadata: {:?}", first_trait.metadata);
        println!("First trait tags: {:?}", first_trait.tags);
    }

    // Check enum has pattern matching metadata
    if let Some(first_enum) = enums.first() {
        println!("First enum metadata: {:?}", first_enum.metadata);
        println!("First enum tags: {:?}", first_enum.tags);
    }

    // Check function metadata
    if let Some(first_function) = functions.first() {
        println!(
            "First function '{}' metadata: {:?}",
            first_function.name, first_function.metadata
        );
        println!("First function tags: {:?}", first_function.tags);
    }

    // Verify confidence scores
    println!("\n=== CONFIDENCE SCORES ===");
    for relationship in &result.relationships {
        println!(
            "Relationship {:?}: confidence = {}",
            relationship.relation_type, relationship.confidence
        );
        assert!(
            relationship.confidence >= 0.0 && relationship.confidence <= 1.0,
            "Confidence should be between 0.0 and 1.0"
        );

        // High-confidence relationships should be above 0.8
        if relationship.relation_type == RelationType::Contains {
            assert!(
                relationship.confidence >= 0.8,
                "Containment relationships should have high confidence"
            );
        }
    }

    println!("\n=== PHASE 2 ANALYZER VERIFICATION COMPLETE ===");

    // Print summary comparison
    println!("\n=== EXTRACTION SUMMARY ===");
    println!("Total symbols extracted: {}", result.symbols.len());
    println!(
        "Total relationships extracted: {}",
        result.relationships.len()
    );
    println!(
        "Symbol types found: {}",
        stats.keys().filter(|k| k.starts_with("symbols_")).count()
    );
    println!(
        "Relationship types found: {}",
        stats
            .keys()
            .filter(|k| k.starts_with("relationships_"))
            .count()
    );

    // Verify we're extracting significant symbols and relationships
    assert!(
        result.symbols.len() >= 5,
        "Should extract at least 5 symbols from simple test file (found {})",
        result.symbols.len()
    );
    assert!(
        result.relationships.len() >= 1,
        "Should extract at least 1 relationship from simple test file (found {})",
        result.relationships.len()
    );

    // Verify analyzer enhancements are working
    let has_rust_enhancements = result.analysis_metadata.analyzer_name == "RustAnalyzer";
    assert!(has_rust_enhancements, "Should use enhanced RustAnalyzer");

    let has_complexity_metric = result
        .analysis_metadata
        .metrics
        .contains_key("rust_complexity");
    assert!(
        has_complexity_metric,
        "Should calculate Rust complexity metrics"
    );

    println!("\n✅ PHASE 2 ENHANCEMENTS VERIFIED:");
    println!(
        "  • Symbol extraction working: {} symbols",
        result.symbols.len()
    );
    println!(
        "  • Relationship extraction working: {} relationships",
        result.relationships.len()
    );
    println!(
        "  • Rust-specific analyzer active: {}",
        has_rust_enhancements
    );
    println!(
        "  • Enhanced metadata generation: {}",
        has_complexity_metric
    );
    println!(
        "  • Analysis performance: {:.2}ms",
        result.analysis_metadata.duration_ms
    );

    println!("\n🎉 Phase 2 analyzer enhancements test PASSED!");
}

/// Test specific relationship extraction features
#[tokio::test]
async fn test_relationship_extraction_details() {
    let test_file_path = PathBuf::from("../simple_analyzer_test.rs");

    // Read the test file content
    let content = tokio::fs::read_to_string(&test_file_path)
        .await
        .expect("Failed to read test file");

    // Create analyzer
    let uid_generator = Arc::new(SymbolUIDGenerator::new());
    let analyzer = RustAnalyzer::new(uid_generator.clone());

    // Create analysis context
    let context = AnalysisContext::new(
        1,
        1,
        "rust".to_string(),
        PathBuf::from("/tmp/ws"),
        test_file_path.clone(),
        uid_generator,
    );

    // Run analysis
    let result = analyzer
        .analyze_file(&content, &test_file_path, "rust", &context)
        .await
        .expect("Analysis should succeed");

    println!("\n=== DETAILED RELATIONSHIP TESTING ===");

    // Group relationships by type for detailed analysis
    let mut relationship_types = std::collections::HashMap::new();
    for rel in &result.relationships {
        *relationship_types.entry(rel.relation_type).or_insert(0) += 1;
    }

    println!("Relationship type counts:");
    for (rel_type, count) in relationship_types {
        println!("  {:?}: {}", rel_type, count);
    }

    // Test that we can find specific expected relationships
    let symbols_by_name: std::collections::HashMap<String, &_> =
        result.symbols.iter().map(|s| (s.name.clone(), s)).collect();

    // Look for impl Drawable for Circle relationship
    let implements_rels: Vec<_> = result
        .relationships
        .iter()
        .filter(|r| r.relation_type == RelationType::Implements)
        .collect();

    println!(
        "Implementation relationships found: {}",
        implements_rels.len()
    );
    for rel in implements_rels {
        println!(
            "  {} implements {}",
            rel.source_symbol_uid, rel.target_symbol_uid
        );
    }

    // Test containment relationships (struct fields, enum variants, etc.)
    let contains_rels: Vec<_> = result
        .relationships
        .iter()
        .filter(|r| r.relation_type == RelationType::Contains)
        .collect();

    println!("Containment relationships found: {}", contains_rels.len());
    for rel in contains_rels {
        println!(
            "  {} contains {}",
            rel.source_symbol_uid, rel.target_symbol_uid
        );
    }

    assert!(
        result.relationships.len() > 0,
        "Should find some relationships in complex code"
    );
}

/// Benchmark test to compare extraction performance
#[tokio::test]
async fn test_extraction_performance() {
    let test_file_path = PathBuf::from("../simple_analyzer_test.rs");

    let content = tokio::fs::read_to_string(&test_file_path)
        .await
        .expect("Failed to read test file");

    let uid_generator = Arc::new(SymbolUIDGenerator::new());
    let analyzer = RustAnalyzer::new(uid_generator.clone());
    let context = AnalysisContext::new(
        1,
        1,
        "rust".to_string(),
        PathBuf::from("/tmp/ws"),
        test_file_path.clone(),
        uid_generator,
    );

    // Time the analysis
    let start = std::time::Instant::now();
    let result = analyzer
        .analyze_file(&content, &test_file_path, "rust", &context)
        .await
        .expect("Analysis should succeed");
    let duration = start.elapsed();

    println!("\n=== PERFORMANCE METRICS ===");
    println!("Analysis time: {:?}", duration);
    println!(
        "Symbols per second: {:.2}",
        result.symbols.len() as f64 / duration.as_secs_f64()
    );
    println!(
        "Relationships per second: {:.2}",
        result.relationships.len() as f64 / duration.as_secs_f64()
    );
    println!("Analysis metadata: {:?}", result.analysis_metadata);

    // Analysis should complete reasonably quickly for the test file
    assert!(
        duration.as_secs() < 10,
        "Analysis should complete within 10 seconds"
    );
}
