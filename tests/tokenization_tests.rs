use probe::search::query::{create_query_plan, create_structured_patterns};
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
        "rpc", "storage", "handler", "client", "server", "api", "service", "http", "handler",
    ] {
        enhanced_vocab.insert(term.to_string());
    }

    let input = "httpHandler";
    let compound_parts = split_compound_word(input, &enhanced_vocab);

    println!("Compound split of '{}': {:?}", input, compound_parts);

    assert!(compound_parts.contains(&"http".to_string()));
    assert!(compound_parts.contains(&"Handler".to_string()));
}

#[test]
fn test_query_preprocessing() {
    // Test query preprocessing with "RPCStorageHandler"
    let query = "RPCStorageHandler";
    let plan = create_query_plan(query, false).expect("Failed to create query plan");

    println!("Query plan for '{}': {:?}", query, plan);

    // Check if the term_indices include the expected parts
    let term_strings: Vec<String> = plan.term_indices.keys().cloned().collect();

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
    let plan = create_query_plan(query, false).expect("Failed to create query plan");
    let patterns = create_structured_patterns(&plan);

    println!("Generated patterns:");
    for (i, (pattern, indices)) in patterns.iter().enumerate() {
        println!("Pattern {}: {} - Indices: {:?}", i, pattern, indices);
    }

    // Check that we have patterns for the expected terms
    let term_indices = &plan.term_indices;

    // Verify that each term in the plan has at least one pattern
    for (term, &idx) in term_indices {
        if !plan.excluded_terms.contains(term) {
            let has_pattern = patterns.iter().any(|(_, indices)| indices.contains(&idx));
            assert!(
                has_pattern,
                "No pattern found for term '{}' at index {}",
                term, idx
            );
        }
    }

    // Check that all patterns are valid regexes
    for (pattern, _) in &patterns {
        assert!(
            regex::Regex::new(pattern).is_ok(),
            "Invalid regex pattern: {}",
            pattern
        );
    }
}

#[test]
fn test_multiple_word_query() {
    // Test multiple words with "ip whitelist"
    let query = "ip whitelist";
    let plan = create_query_plan(query, false).expect("Failed to create query plan");

    println!("Query plan for '{}': {:?}", query, plan);

    // Check if the terms include the expected parts
    let term_strings: Vec<String> = plan.term_indices.keys().cloned().collect();

    // We expect to see "ip", "white", "list" in the terms
    assert!(term_strings.contains(&"ip".to_string()));

    // Either "white" and "list" separately, or "whitelist" as a whole
    let has_white_and_list =
        term_strings.contains(&"white".to_string()) && term_strings.contains(&"list".to_string());
    let has_whitelist = term_strings.contains(&"whitelist".to_string());

    assert!(
        has_white_and_list || has_whitelist,
        "Expected either both 'white' and 'list', or 'whitelist' in {:?}",
        term_strings
    );
}

#[test]
fn test_underscore_handling() {
    use probe::search::elastic_query;

    // Test tokenization with underscores
    let query = "keyword_underscore";
    let plan = create_query_plan(query, false).expect("Failed to create query plan");

    println!("Query plan for '{}': {:?}", query, plan);

    // Check if the term is preserved with the underscore in the term_indices
    let term_strings: Vec<String> = plan.term_indices.keys().cloned().collect();

    // We expect to see "keyword_underscore" or its tokenized parts
    let has_tokenized_parts = term_strings.contains(&"key".to_string())
        || term_strings.contains(&"word".to_string())
        || term_strings.contains(&"score".to_string());

    assert!(
        has_tokenized_parts,
        "Expected tokenized parts of 'keyword_underscore' in {:?}",
        term_strings
    );

    // Check if the term is properly tokenized in the elastic query parser
    // This is already done in the create_query_plan function, but we'll verify the AST
    if let elastic_query::Expr::Term { keywords, .. } = &plan.ast {
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
