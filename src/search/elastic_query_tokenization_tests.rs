// No need to import HashMap and HashSet as they're already imported in the parent module
use crate::search::tokenization::tokenize_and_stem;

#[test]
fn test_tokenize_and_stem() {
    // Test basic stemming
    let result = tokenize_and_stem("running");
    assert_eq!(result, vec!["run"]);
    
    // Test camel case splitting and stemming
    let result = tokenize_and_stem("enableIpWhiteListing");
    assert!(result.contains(&"enabl".to_string()));
    assert!(result.contains(&"ip".to_string()));
    assert!(result.contains(&"white".to_string()));
    assert!(result.contains(&"list".to_string()));
    
    // Test compound word splitting and stemming
    let result = tokenize_and_stem("whitelist");
    assert!(result.contains(&"white".to_string()));
    assert!(result.contains(&"list".to_string()));
    
    // Test stop word filtering
    let result = tokenize_and_stem("function");
    assert!(result.len() == 1); // "function" is not a stop word in this context
    
    // Test with a term that might be stemmed
    let result = tokenize_and_stem("firewall");
    // The stemmer might stem "firewall" to "firewal" depending on the stemming algorithm
    assert!(result.len() == 1);
    assert!(result[0] == "firewall" || result[0] == "firewal");
}

#[test]
fn test_process_ast_terms() {
    // Test processing a simple term
    let expr = Expr::Term {
        keywords: vec!["running".to_string()],
        field: None,
        required: false,
        excluded: false,
    };
    
    let processed = process_ast_terms(expr);
    
    if let Expr::Term { keywords, field, required, excluded } = processed {
        assert_eq!(keywords, vec!["run"]);
        assert_eq!(field, None);
        assert!(!required);
        assert!(!excluded);
    } else {
        panic!("Expected Term expression");
    }
    
    // Test processing a term with camel case
    let expr = Expr::Term {
        keywords: vec!["enableIpWhiteListing".to_string()],
        field: None,
        required: true,
        excluded: false,
    };
    
    let processed = process_ast_terms(expr);
    
    if let Expr::Term { keywords, field, required, excluded } = processed {
        assert!(keywords.contains(&"enabl".to_string()));
        assert!(keywords.contains(&"ip".to_string()));
        assert!(keywords.contains(&"white".to_string()));
        assert!(keywords.contains(&"list".to_string()));
        assert_eq!(field, None);
        assert!(required);
        assert!(!excluded);
    } else {
        panic!("Expected Term expression");
    }
    
    // Test processing a compound expression
    let expr = Expr::And(
        Box::new(Expr::Term {
            keywords: vec!["running".to_string()],
            field: None,
            required: false,
            excluded: false,
        }),
        Box::new(Expr::Term {
            keywords: vec!["whitelist".to_string()],
            field: None,
            required: false,
            excluded: false,
        })
    );
    
    let processed = process_ast_terms(expr);
    
    if let Expr::And(left, right) = processed {
        if let Expr::Term { keywords, .. } = *left {
            assert_eq!(keywords, vec!["run"]);
        } else {
            panic!("Expected Term expression for left side");
        }
        
        if let Expr::Term { keywords, .. } = *right {
            assert!(keywords.contains(&"white".to_string()));
            assert!(keywords.contains(&"list".to_string()));
        } else {
            panic!("Expected Term expression for right side");
        }
    } else {
        panic!("Expected And expression");
    }
}

#[test]
fn test_parse_query_with_tokenization() {
    // Test parsing a query with tokenization and stemming
    let result = parse_query_test("running").unwrap();
    
    if let Expr::Term { keywords, .. } = result {
        assert_eq!(keywords, vec!["run"]);
    } else {
        panic!("Expected Term expression");
    }
    
    // Test parsing a query with camel case
    let result = parse_query_test("enableIpWhiteListing").unwrap();
    
    if let Expr::Term { keywords, .. } = result {
        assert!(keywords.contains(&"enabl".to_string()));
        assert!(keywords.contains(&"ip".to_string()));
        assert!(keywords.contains(&"white".to_string()));
        assert!(keywords.contains(&"list".to_string()));
    } else {
        panic!("Expected Term expression");
    }
    
    // Test parsing a complex query
    let result = parse_query_test("running AND whitelist").unwrap();
    
    if let Expr::And(left, right) = result {
        if let Expr::Term { keywords, .. } = *left {
            assert_eq!(keywords, vec!["run"]);
        } else {
            panic!("Expected Term expression for left side");
        }
        
        if let Expr::Term { keywords, .. } = *right {
            assert!(keywords.contains(&"white".to_string()));
            assert!(keywords.contains(&"list".to_string()));
        } else {
            panic!("Expected Term expression for right side");
        }
    } else {
        panic!("Expected And expression");
    }
}

#[test]
fn test_query_evaluation_with_tokenization() {
    // Create term indices with stemmed terms
    let mut term_indices = HashMap::new();
    term_indices.insert("run".to_string(), 0);
    term_indices.insert("white".to_string(), 1);
    term_indices.insert("list".to_string(), 2);
    
    // Parse a query that will be tokenized and stemmed
    let expr = parse_query_test("running").unwrap();
    
    // Match when the stemmed term is present
    let matched_terms = HashSet::from([0]); // "run"
    assert!(expr.evaluate(&matched_terms, &term_indices));
    
    // No match when the stemmed term is absent
    let matched_terms = HashSet::from([1, 2]); // "white", "list"
    assert!(!expr.evaluate(&matched_terms, &term_indices));
    
    // Parse a compound query
    let expr = parse_query_test("running AND whitelist").unwrap();
    
    // Match when all stemmed terms are present
    let matched_terms = HashSet::from([0, 1, 2]); // "run", "white", "list"
    assert!(expr.evaluate(&matched_terms, &term_indices));
    
    // No match when only some stemmed terms are present
    let matched_terms = HashSet::from([0]); // "run"
    assert!(!expr.evaluate(&matched_terms, &term_indices));
}