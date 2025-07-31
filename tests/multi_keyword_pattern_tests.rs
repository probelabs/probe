use lru::LruCache;
use probe_code::search::elastic_query::Expr;
use probe_code::search::query::{create_structured_patterns, QueryPlan};
use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

/// Test the pattern generation for multi-keyword terms
#[test]
fn test_multi_keyword_pattern_generation() {
    // Create a simple AST with a multi-keyword term
    let ast = Expr::Term {
        keywords: vec!["white".to_string(), "list".to_string()],
        field: None,
        required: false,
        excluded: false,
        exact: false,
    };

    // Create a term indices map
    let mut term_indices = HashMap::new();
    term_indices.insert("white".to_string(), 0);
    term_indices.insert("list".to_string(), 1);

    // Create a QueryPlan
    let has_required_anywhere = ast.has_required_term();
    let has_only_excluded_terms = ast.is_only_excluded_terms();
    let required_terms_indices = HashSet::new();
    let evaluation_cache = Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(1000).unwrap())));

    let plan = QueryPlan {
        ast,
        term_indices,
        excluded_terms: HashSet::new(),
        exact: false,
        is_simple_query: false,
        required_terms: HashSet::new(),
        has_required_anywhere,
        required_terms_indices,
        has_only_excluded_terms,
        evaluation_cache,
    };

    // Generate patterns
    let patterns = create_structured_patterns(&plan);

    // We should have at least two patterns, one for each keyword
    assert!(
        patterns.len() >= 2,
        "Should generate at least two patterns for multi-keyword term"
    );

    // Check that we have patterns for both keywords
    let pattern_strings: Vec<&str> = patterns.iter().map(|(p, _)| p.as_str()).collect();

    // There should be a pattern containing "white"
    assert!(
        pattern_strings.iter().any(|p| p.contains("white")),
        "Should generate a pattern for 'white'"
    );

    // There should be a pattern containing "list"
    assert!(
        pattern_strings.iter().any(|p| p.contains("list")),
        "Should generate a pattern for 'list'"
    );

    // Check that the term indices are correct
    for (_, indices) in &patterns {
        // Each pattern should be associated with the correct term index
        assert!(
            indices.contains(&0) || indices.contains(&1),
            "Pattern should be associated with term index 0 or 1"
        );
    }
}

/// Test pattern generation for excluded terms
#[test]
fn test_excluded_term_pattern_generation() {
    // Create an AST with an excluded term
    let ast = Expr::Term {
        keywords: vec!["excluded".to_string()],
        field: None,
        required: false,
        excluded: true,
        exact: false,
    };

    // Create a term indices map
    let mut term_indices = HashMap::new();
    term_indices.insert("excluded".to_string(), 0);

    // Create a QueryPlan with excluded terms
    let mut excluded_terms = HashSet::new();
    excluded_terms.insert("excluded".to_string());

    let has_required_anywhere = ast.has_required_term();
    let has_only_excluded_terms = ast.is_only_excluded_terms();
    let required_terms_indices = HashSet::new();
    let evaluation_cache = Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(1000).unwrap())));

    let plan = QueryPlan {
        ast,
        term_indices,
        excluded_terms,
        exact: false,
        is_simple_query: false,
        required_terms: HashSet::new(),
        has_required_anywhere,
        required_terms_indices,
        has_only_excluded_terms,
        evaluation_cache,
    };

    // Generate patterns
    let patterns = create_structured_patterns(&plan);

    // No patterns should be generated for excluded terms
    assert!(
        patterns.is_empty(),
        "Should not generate patterns for excluded terms"
    );
}

/// Test pattern generation for AND expressions
#[test]
fn test_and_expression_pattern_generation() {
    // Create an AST with an AND expression
    let ast = Expr::And(
        Box::new(Expr::Term {
            keywords: vec!["term1".to_string()],
            field: None,
            required: false,
            excluded: false,
            exact: false,
        }),
        Box::new(Expr::Term {
            keywords: vec!["term2".to_string()],
            field: None,
            required: false,
            excluded: false,
            exact: false,
        }),
    );

    // Create a term indices map
    let mut term_indices = HashMap::new();
    term_indices.insert("term1".to_string(), 0);
    term_indices.insert("term2".to_string(), 1);

    // Create a QueryPlan
    let has_required_anywhere = ast.has_required_term();
    let has_only_excluded_terms = ast.is_only_excluded_terms();
    let required_terms_indices = HashSet::new();
    let evaluation_cache = Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(1000).unwrap())));

    let plan = QueryPlan {
        ast,
        term_indices,
        excluded_terms: HashSet::new(),
        exact: false,
        is_simple_query: false,
        required_terms: HashSet::new(),
        has_required_anywhere,
        required_terms_indices,
        has_only_excluded_terms,
        evaluation_cache,
    };

    // Generate patterns
    let patterns = create_structured_patterns(&plan);

    // We should have at least two patterns, one for each term
    assert!(
        patterns.len() >= 2,
        "Should generate at least two patterns for AND expression"
    );

    // Check that we have patterns for both terms
    let pattern_strings: Vec<&str> = patterns.iter().map(|(p, _)| p.as_str()).collect();

    // There should be a pattern containing "term1"
    assert!(
        pattern_strings.iter().any(|p| p.contains("term1")),
        "Should generate a pattern for 'term1'"
    );

    // There should be a pattern containing "term2"
    assert!(
        pattern_strings.iter().any(|p| p.contains("term2")),
        "Should generate a pattern for 'term2'"
    );
}

/// Test pattern generation for OR expressions
#[test]
fn test_or_expression_pattern_generation() {
    // Create an AST with an OR expression
    let ast = Expr::Or(
        Box::new(Expr::Term {
            keywords: vec!["term1".to_string()],
            field: None,
            required: false,
            excluded: false,
            exact: false,
        }),
        Box::new(Expr::Term {
            keywords: vec!["term2".to_string()],
            field: None,
            required: false,
            excluded: false,
            exact: false,
        }),
    );

    // Create a term indices map
    let mut term_indices = HashMap::new();
    term_indices.insert("term1".to_string(), 0);
    term_indices.insert("term2".to_string(), 1);

    // Create a QueryPlan
    let has_required_anywhere = ast.has_required_term();
    let has_only_excluded_terms = ast.is_only_excluded_terms();
    let required_terms_indices = HashSet::new();
    let evaluation_cache = Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(1000).unwrap())));

    let plan = QueryPlan {
        ast,
        term_indices,
        excluded_terms: HashSet::new(),
        exact: false,
        is_simple_query: false,
        required_terms: HashSet::new(),
        has_required_anywhere,
        required_terms_indices,
        has_only_excluded_terms,
        evaluation_cache,
    };

    // Generate patterns
    let patterns = create_structured_patterns(&plan);

    // We should have at least three patterns:
    // 1. A combined pattern for the OR expression
    // 2. Individual patterns for each term
    assert!(
        patterns.len() >= 3,
        "Should generate at least three patterns for OR expression"
    );

    // Check that we have patterns for both terms and a combined pattern
    let pattern_strings: Vec<&str> = patterns.iter().map(|(p, _)| p.as_str()).collect();

    // There should be a pattern containing "term1"
    assert!(
        pattern_strings.iter().any(|p| p.contains("term1")),
        "Should generate a pattern for 'term1'"
    );

    // There should be a pattern containing "term2"
    assert!(
        pattern_strings.iter().any(|p| p.contains("term2")),
        "Should generate a pattern for 'term2'"
    );

    // There should be a combined pattern containing both terms
    assert!(
        pattern_strings
            .iter()
            .any(|p| p.contains("term1") && p.contains("term2")),
        "Should generate a combined pattern for 'term1' and 'term2'"
    );
}

/// Test pattern generation for complex expressions with multi-keyword terms
#[test]
fn test_complex_expression_pattern_generation() {
    // Create a complex AST with multi-keyword terms and logical operators
    let ast = Expr::And(
        Box::new(Expr::Term {
            keywords: vec!["white".to_string(), "list".to_string()],
            field: None,
            required: false,
            excluded: false,
            exact: false,
        }),
        Box::new(Expr::Or(
            Box::new(Expr::Term {
                keywords: vec!["fire".to_string(), "wall".to_string()],
                field: None,
                required: false,
                excluded: false,
                exact: false,
            }),
            Box::new(Expr::Term {
                keywords: vec!["network".to_string()],
                field: None,
                required: false,
                excluded: false,
                exact: false,
            }),
        )),
    );

    // Create a term indices map
    let mut term_indices = HashMap::new();
    term_indices.insert("white".to_string(), 0);
    term_indices.insert("list".to_string(), 1);
    term_indices.insert("fire".to_string(), 2);
    term_indices.insert("wall".to_string(), 3);
    term_indices.insert("network".to_string(), 4);

    // Create a QueryPlan
    let has_required_anywhere = ast.has_required_term();
    let has_only_excluded_terms = ast.is_only_excluded_terms();
    let required_terms_indices = HashSet::new();
    let evaluation_cache = Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(1000).unwrap())));

    let plan = QueryPlan {
        ast,
        term_indices,
        excluded_terms: HashSet::new(),
        exact: false,
        is_simple_query: false,
        required_terms: HashSet::new(),
        has_required_anywhere,
        required_terms_indices,
        has_only_excluded_terms,
        evaluation_cache,
    };

    // Generate patterns
    let patterns = create_structured_patterns(&plan);

    // We should have patterns for all terms
    assert!(
        patterns.len() >= 5,
        "Should generate at least five patterns for complex expression"
    );

    // Check that we have patterns for all terms
    let pattern_strings: Vec<&str> = patterns.iter().map(|(p, _)| p.as_str()).collect();

    // Check for individual term patterns
    assert!(
        pattern_strings.iter().any(|p| p.contains("white")),
        "Should generate a pattern for 'white'"
    );
    assert!(
        pattern_strings.iter().any(|p| p.contains("list")),
        "Should generate a pattern for 'list'"
    );
    assert!(
        pattern_strings.iter().any(|p| p.contains("fire")),
        "Should generate a pattern for 'fire'"
    );
    assert!(
        pattern_strings.iter().any(|p| p.contains("wall")),
        "Should generate a pattern for 'wall'"
    );
    assert!(
        pattern_strings.iter().any(|p| p.contains("network")),
        "Should generate a pattern for 'network'"
    );

    // Check for combined pattern for the OR expression
    assert!(
        pattern_strings
            .iter()
            .any(|p| (p.contains("fire") && p.contains("wall"))
                || (p.contains("fire") && p.contains("network"))
                || (p.contains("wall") && p.contains("network"))),
        "Should generate a combined pattern for the OR expression"
    );
}

/// Test pattern deduplication
#[test]
fn test_pattern_deduplication() {
    // Create an AST with duplicate terms
    let ast = Expr::Or(
        Box::new(Expr::Term {
            keywords: vec!["term".to_string()],
            field: None,
            required: false,
            excluded: false,
            exact: false,
        }),
        Box::new(Expr::Term {
            keywords: vec!["term".to_string()],
            field: None,
            required: false,
            excluded: false,
            exact: false,
        }),
    );

    // Create a term indices map
    let mut term_indices = HashMap::new();
    term_indices.insert("term".to_string(), 0);

    // Create a QueryPlan
    let has_required_anywhere = ast.has_required_term();
    let has_only_excluded_terms = ast.is_only_excluded_terms();
    let required_terms_indices = HashSet::new();
    let evaluation_cache = Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(1000).unwrap())));

    let plan = QueryPlan {
        ast,
        term_indices,
        excluded_terms: HashSet::new(),
        exact: false,
        is_simple_query: false,
        required_terms: HashSet::new(),
        has_required_anywhere,
        required_terms_indices,
        has_only_excluded_terms,
        evaluation_cache,
    };

    // Generate patterns
    let patterns = create_structured_patterns(&plan);

    // We should have deduplicated patterns
    // Count how many patterns contain "term"
    let term_pattern_count = patterns.iter().filter(|(p, _)| p.contains("term")).count();

    // We should have at most 2 patterns containing "term":
    // 1. The individual pattern
    // 2. The combined pattern (which might be the same as the individual pattern)
    assert!(
        term_pattern_count <= 2,
        "Should deduplicate patterns for the same term"
    );
}
