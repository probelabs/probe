use probe::search::query::{create_term_patterns, preprocess_query};
use probe::search::tokenization::{load_vocabulary, split_camel_case, split_compound_word};

#[test]
fn test_camel_case_splitting() {
    // Test camelCase splitting with "RPCStorageHandler"
    let input = "RPCStorageHandler";
    let camel_parts = split_camel_case(input);

    println!("Direct camelCase split of '{}': {:?}", input, camel_parts);

    // We expect "RPCStorageHandler" to be split into ["rpc", "storage", "handler"]
    assert!(camel_parts.contains(&"rpc".to_string()));
    assert!(camel_parts.contains(&"storage".to_string()));
    assert!(camel_parts.contains(&"handler".to_string()));
}

#[test]
fn test_compound_word_splitting() {
    // Test compound word splitting with "whitelist"
    let vocab = load_vocabulary();
    let mut enhanced_vocab = vocab.clone();

    // Add specific programming terms to the vocabulary
    for term in [
        "rpc", "storage", "handler", "client", "server", "api", "service",
    ] {
        enhanced_vocab.insert(term.to_string());
    }

    let input = "whitelist";
    let compound_parts = split_compound_word(input, &enhanced_vocab);

    println!("Compound split of '{}': {:?}", input, compound_parts);

    // We expect "whitelist" to be split into ["white", "list"]
    assert!(compound_parts.contains(&"white".to_string()));
    assert!(compound_parts.contains(&"list".to_string()));
}

#[test]
fn test_query_preprocessing() {
    // Test query preprocessing with "RPCStorageHandler"
    let query = "RPCStorageHandler";
    let terms = preprocess_query(query, false);

    println!("Preprocessed terms for '{}': {:?}", query, terms);

    // Check if the terms include the expected parts
    let term_strings: Vec<String> = terms.iter().map(|(original, _)| original.clone()).collect();

    // We expect to see "rpc", "storage", "handler" in the terms
    assert!(
        term_strings.contains(&"rpc".to_string())
            || term_strings.contains(&"storage".to_string())
            || term_strings.contains(&"handler".to_string()),
        "Expected at least one of 'rpc', 'storage', or 'handler' in {:?}",
        term_strings
    );
}

#[test]
fn test_pattern_generation() {
    // Test pattern generation with "RPCStorageHandler"
    let query = "RPCStorageHandler";
    let terms = preprocess_query(query, false);
    let patterns = create_term_patterns(&terms);

    println!("Generated patterns:");
    for (i, (pattern, indices)) in patterns.iter().enumerate() {
        println!("Pattern {}: {} - Indices: {:?}", i, pattern, indices);
    }

    // Check that we don't have redundant patterns with both stemmed and original versions
    for (pattern, _) in &patterns {
        // Count the number of OR operators in the pattern
        let or_count = pattern.matches("|").count();

        // We should have at most one OR operator for word boundaries
        assert!(
            or_count <= 1,
            "Pattern contains redundant OR operators: {}",
            pattern
        );
    }
}

#[test]
fn test_multiple_word_query() {
    // Test multiple words with "ip whitelist"
    let query = "ip whitelist";
    let terms = preprocess_query(query, false);

    println!("Preprocessed terms for '{}': {:?}", query, terms);

    // Check if the terms include the expected parts
    let term_strings: Vec<String> = terms.iter().map(|(original, _)| original.clone()).collect();

    // We expect to see "ip", "white", "list" in the terms
    assert!(term_strings.contains(&"ip".to_string()));
    assert!(
        term_strings.contains(&"white".to_string())
            || term_strings.contains(&"whitelist".to_string())
    );
    assert!(
        term_strings.contains(&"list".to_string())
            || term_strings.contains(&"whitelist".to_string())
    );
}

#[test]
fn test_underscore_handling() {
    use probe::search::elastic_query;

    // Test tokenization with underscores
    let query = "keyword_underscore";
    let terms = preprocess_query(query, false);

    println!("Preprocessed terms for '{}': {:?}", query, terms);

    // Check if the term is preserved with the underscore
    let term_strings: Vec<String> = terms.iter().map(|(original, _)| original.clone()).collect();

    // We expect to see "keyword_underscore" as a single term
    assert!(
        term_strings.contains(&"keyword_underscore".to_string()),
        "Expected 'keyword_underscore' to be preserved as a single term in {:?}",
        term_strings
    );

    // Now we need to check if the term is properly tokenized in the elastic query parser
    // So we'll use a different assertion

    // Check if the term is properly tokenized in the elastic query parser
    let ast = elastic_query::parse_query("keyword_underscore", true).unwrap();
    if let elastic_query::Expr::Term { keywords, .. } = ast {
        // The term should be tokenized into ["key", "word", "under", "score"]
        // Note: "under" might be filtered out as a stop word
        assert!(
            keywords.contains(&"key".to_string()),
            "Expected 'key' in keywords: {:?}",
            keywords
        );
        assert!(
            keywords.contains(&"word".to_string()),
            "Expected 'word' in keywords: {:?}",
            keywords
        );
        assert!(
            keywords.contains(&"score".to_string()),
            "Expected 'score' in keywords: {:?}",
            keywords
        );
    } else {
        panic!("Expected Term expression");
    }
}

#[test]
fn test_underscore_in_elastic_query() {
    use probe::search::elastic_query;

    // Test that the elastic query parser preserves underscores
    let query = "keyword_underscore";
    // Use parse_query with any_term=true instead of parse_query_test
    let ast = elastic_query::parse_query(query, true).unwrap();

    // Check that the AST contains the tokenized terms from "keyword_underscore"
    if let elastic_query::Expr::Term { keywords, .. } = ast {
        // The term should be tokenized into ["key", "word", "under", "score"]
        // Note: "under" might be filtered out as a stop word
        assert!(
            keywords.contains(&"key".to_string()),
            "Expected 'key' in keywords: {:?}",
            keywords
        );
        assert!(
            keywords.contains(&"word".to_string()),
            "Expected 'word' in keywords: {:?}",
            keywords
        );
        assert!(
            keywords.contains(&"score".to_string()),
            "Expected 'score' in keywords: {:?}",
            keywords
        );
    } else {
        panic!("Expected Term expression");
    }
}
