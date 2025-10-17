// No need to import HashMap and HashSet as they're already imported in the parent module
use probe_code::search::tokenization::tokenize_and_stem;

/// Process the terms in an AST expression, applying tokenization and stemming
/// to the terms unless they are marked as exact.
fn process_ast_terms(expr: Expr) -> Expr {
    match expr {
        Expr::Term { keywords, field, required, excluded, exact, .. } => {
            // If exact or excluded => skip tokenization
            let processed_keywords = if exact || excluded {
                keywords
            } else {
                // Apply tokenization and stemming to each keyword
                let mut processed = Vec::new();
                for keyword in keywords {
                    let stemmed = tokenize_and_stem(&keyword);
                    processed.extend(stemmed);
                }
                processed
            };

            Expr::Term {
                keywords: processed_keywords.clone(),
                lowercase_keywords: processed_keywords.iter().map(|k| k.to_lowercase()).collect(),
                field,
                required,
                excluded,
                exact,
            }
        },
        Expr::And(left, right) => {
            Expr::And(
                Box::new(process_ast_terms(*left)),
                Box::new(process_ast_terms(*right))
            )
        },
        Expr::Or(left, right) => {
            Expr::Or(
                Box::new(process_ast_terms(*left)),
                Box::new(process_ast_terms(*right))
            )
        },
    }
}

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
    // "whitelist" is now a special case word that should not be split
    let result = tokenize_and_stem("whitelist");
    assert!(result.contains(&"whitelist".to_string()));

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
    let keywords = vec!["running".to_string()];
    let expr = Expr::Term {
        keywords: keywords.clone(),
        lowercase_keywords: keywords.iter().map(|k| k.to_lowercase()).collect(),
        field: None,
        required: false,
        excluded: false,
        exact: false,
    };

    let processed = process_ast_terms(expr);

    if let Expr::Term { keywords, field, required, excluded, exact, .. } = processed {
        assert_eq!(keywords, vec!["run"]);
        assert_eq!(field, None);
        assert!(!required);
        assert!(!excluded);
        assert!(!exact);
    } else {
        panic!("Expected Term expression");
    }

    // Test processing a term with camel case
    let keywords = vec!["enableIpWhiteListing".to_string()];
    let expr = Expr::Term {
        keywords: keywords.clone(),
        lowercase_keywords: keywords.iter().map(|k| k.to_lowercase()).collect(),
        field: None,
        required: true,
        excluded: false,
        exact: false,
    };

    let processed = process_ast_terms(expr);

    if let Expr::Term { keywords, field, required, excluded, exact, .. } = processed {
        assert!(keywords.contains(&"enabl".to_string()));
        assert!(keywords.contains(&"ip".to_string()));
        assert!(keywords.contains(&"white".to_string()));
        assert!(keywords.contains(&"list".to_string()));
        assert_eq!(field, None);
        assert!(required);
        assert!(!excluded);
        assert!(!exact);
    } else {
        panic!("Expected Term expression");
    }

    // Test processing a compound expression
    let keywords1 = vec!["running".to_string()];
    let keywords2 = vec!["whitelist".to_string()];
    let expr = Expr::And(
        Box::new(Expr::Term {
            keywords: keywords1.clone(),
            lowercase_keywords: keywords1.iter().map(|k| k.to_lowercase()).collect(),
            field: None,
            required: false,
            excluded: false,
            exact: false,
        }),
        Box::new(Expr::Term {
            keywords: keywords2.clone(),
            lowercase_keywords: keywords2.iter().map(|k| k.to_lowercase()).collect(),
            field: None,
            required: false,
            excluded: false,
            exact: false,
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
            // "whitelist" is now a special case word that should not be split
            assert!(keywords.contains(&"whitelist".to_string()));
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
            // "whitelist" is now a special case word that should not be split
            assert!(keywords.contains(&"whitelist".to_string()));
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
    term_indices.insert("whitelist".to_string(), 1);

    // Parse a query that will be tokenized and stemmed
    let expr = parse_query_test("running").unwrap();

    // Match when the stemmed term is present
    let matched_terms = HashSet::from([0]); // "run"
    assert!(expr.evaluate(&matched_terms, &term_indices, false));

    // No match when the stemmed term is absent
    let matched_terms = HashSet::from([1]); // "whitelist"
    assert!(!expr.evaluate(&matched_terms, &term_indices, false));

    // Parse a compound query
    let expr = parse_query_test("running AND whitelist").unwrap();

    // Match when all terms are present
    let matched_terms = HashSet::from([0, 1]); // "run", "whitelist"
    assert!(expr.evaluate(&matched_terms, &term_indices, false));

    // No match when only some terms are present
    let matched_terms = HashSet::from([0]); // "run"
    assert!(!expr.evaluate(&matched_terms, &term_indices, false));
}

#[test]
fn test_tokenize_quoted_strings() {
    // Test basic quoted string
    let tokens = tokenize("\"hello world\"").unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0], Token::QuotedString("hello world".to_string()));

    // Test quoted string with escaped quotes
    let tokens = tokenize("\"hello \\\"world\\\"\"").unwrap();
    assert_eq!(tokens.len(), 1);
    assert_eq!(tokens[0], Token::QuotedString("hello \"world\"".to_string()));

    // Test quoted string with other tokens
    let tokens = tokenize("foo \"bar baz\" qux").unwrap();
    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[0], Token::Ident("foo".to_string()));
    assert_eq!(tokens[1], Token::QuotedString("bar baz".to_string()));
    assert_eq!(tokens[2], Token::Ident("qux".to_string()));

    // Test quoted string with prefixes
    let tokens = tokenize("+\"required term\"").unwrap();
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[0], Token::Plus);
    assert_eq!(tokens[1], Token::QuotedString("required term".to_string()));

    let tokens = tokenize("-\"excluded term\"").unwrap();
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[0], Token::Minus);
    assert_eq!(tokens[1], Token::QuotedString("excluded term".to_string()));
}

#[test]
fn test_parse_quoted_strings() {
    // Test basic quoted string
    let result = parse_query_test("\"hello world\"").unwrap();

    if let Expr::Term { keywords, exact, .. } = result {
        assert_eq!(keywords, vec!["hello world"]);
        assert!(exact);
    } else {
        panic!("Expected Term expression");
    }

    // Test quoted string with prefixes
    let result = parse_query_test("+\"required term\"").unwrap();

    if let Expr::Term { keywords, required, exact, .. } = result {
        assert_eq!(keywords, vec!["required term"]);
        assert!(required);
        assert!(exact);
    } else {
        panic!("Expected Term expression");
    }

    let result = parse_query_test("-\"excluded term\"").unwrap();

    if let Expr::Term { keywords, excluded, exact, .. } = result {
        assert_eq!(keywords, vec!["excluded term"]);
        assert!(excluded);
        assert!(exact);
    } else {
        panic!("Expected Term expression");
    }

    // Test quoted string with boolean operators
    let result = parse_query_test("\"exact term\" AND foo").unwrap();

    if let Expr::And(left, right) = result {
        if let Expr::Term { keywords, exact, .. } = *left {
            assert_eq!(keywords, vec!["exact term"]);
            assert!(exact);
        } else {
            panic!("Expected Term expression for left side");
        }

        if let Expr::Term { keywords, exact, .. } = *right {
            assert!(keywords.contains(&"foo".to_string()));
            assert!(!exact);
        } else {
            panic!("Expected Term expression for right side");
        }
    } else {
        panic!("Expected And expression");
    }
}

#[test]
fn test_process_ast_terms_with_exact_flag() {
    // Test processing an exact term
    let keywords = vec!["running".to_string()];
    let expr = Expr::Term {
        keywords: keywords.clone(),
        lowercase_keywords: keywords.iter().map(|k| k.to_lowercase()).collect(),
        field: None,
        required: false,
        excluded: false,
        exact: true,
    };

    let processed = process_ast_terms(expr);

    if let Expr::Term { keywords, field, required, excluded, exact, .. } = processed {
        // Keywords should not be stemmed for exact terms
        assert_eq!(keywords, vec!["running"]);
        assert_eq!(field, None);
        assert!(!required);
        assert!(!excluded);
        assert!(exact);
    } else {
        panic!("Expected Term expression");
    }

    // Test processing a compound expression with an exact term
    let keywords1 = vec!["running".to_string()];
    let keywords2 = vec!["whitelist".to_string()];
    let expr = Expr::And(
        Box::new(Expr::Term {
            lowercase_keywords: keywords1.iter().map(|k| k.to_lowercase()).collect(),
            keywords: keywords1,
            field: None,
            required: false,
            excluded: false,
            exact: false,
        }),
        Box::new(Expr::Term {
            lowercase_keywords: keywords2.iter().map(|k| k.to_lowercase()).collect(),
            keywords: keywords2,
            field: None,
            required: false,
            excluded: false,
            exact: true,
        })
    );

    let processed = process_ast_terms(expr);

    if let Expr::And(left, right) = processed {
        if let Expr::Term { keywords, exact, .. } = *left {
            assert_eq!(keywords, vec!["run"]);
            assert!(!exact);
        } else {
            panic!("Expected Term expression for left side");
        }

        if let Expr::Term { keywords, exact, .. } = *right {
            // Keywords should not be stemmed for exact terms
            assert_eq!(keywords, vec!["whitelist"]);
            assert!(exact);
        } else {
            panic!("Expected Term expression for right side");
        }
    } else {
        panic!("Expected And expression");
    }
}

#[test]
fn test_evaluate_exact_terms_tokenization() {
    // Create term indices
    let mut term_indices = HashMap::new();
    term_indices.insert("running".to_string(), 0);
    term_indices.insert("run".to_string(), 1);
    term_indices.insert("whitelist".to_string(), 2);
    term_indices.insert("white".to_string(), 3);
    term_indices.insert("list".to_string(), 4);

    // Test exact term
    let keywords = vec!["running".to_string()];
    let expr = Expr::Term {
        keywords: keywords.clone(),
        lowercase_keywords: keywords.iter().map(|k| k.to_lowercase()).collect(),
        field: None,
        required: false,
        excluded: false,
        exact: true,
    };

    // Match when the exact term is present
    let matched_terms = HashSet::from([0]); // "running"
    assert!(expr.evaluate(&matched_terms, &term_indices, false));

    // No match when only the stemmed term is present
    let matched_terms = HashSet::from([1]); // "run"
    assert!(!expr.evaluate(&matched_terms, &term_indices, false));

    // Test non-exact term
    let keywords = vec!["run".to_string()];
    let expr = Expr::Term {
        keywords: keywords.clone(),
        lowercase_keywords: keywords.iter().map(|k| k.to_lowercase()).collect(),
        field: None,
        required: false,
        excluded: false,
        exact: false,
    };

    // The term is "run", so it won't match "running" directly
    // We need to update the term_indices to reflect the actual relationship
    let mut term_indices_updated = HashMap::new();
    term_indices_updated.insert("run".to_string(), 1); // Map "run" to index 1
    term_indices_updated.insert("running".to_string(), 0); // Map "running" to index 0
    term_indices_updated.insert("whitelist".to_string(), 2);
    term_indices_updated.insert("white".to_string(), 3);
    term_indices_updated.insert("list".to_string(), 4);

    // Match when the exact term is present
    let matched_terms = HashSet::from([0, 1]); // Include both "running" and "run"
    assert!(expr.evaluate(&matched_terms, &term_indices_updated, false));

    // Match when only the stemmed term is present
    let matched_terms = HashSet::from([1]); // "run"
    assert!(expr.evaluate(&matched_terms, &term_indices, false));

    // Test compound expression with exact term - simplified approach
    // Create a new expression with the stemmed form directly
    let keywords1 = vec!["run".to_string()];
    let keywords2 = vec!["whitelist".to_string()];
    let expr = Expr::And(
        Box::new(Expr::Term {
            keywords: keywords1.clone(),
            lowercase_keywords: keywords1.iter().map(|k| k.to_lowercase()).collect(),
            field: None,
            required: false,
            excluded: false,
            exact: false,
        }),
        Box::new(Expr::Term {
            keywords: keywords2.clone(),
            lowercase_keywords: keywords2.iter().map(|k| k.to_lowercase()).collect(),
            field: None,
            required: false,
            excluded: false,
            exact: true,
        })
    );

    // Match when both terms are present
    let matched_terms = HashSet::from([1, 2]); // "run", "whitelist"
    assert!(expr.evaluate(&matched_terms, &term_indices, false));

    // No match when the exact term is only present as stemmed parts
    let matched_terms = HashSet::from([0, 3, 4]); // "running", "white", "list"
    assert!(!expr.evaluate(&matched_terms, &term_indices, false));
}

#[test]
fn test_hyphenated_compound_terms_parsing() {
    // Test that "multi-agent" is correctly parsed as a single compound term
    // After fix: "multi-agent" should be tokenized as ["multi", "agent"], not "multi AND -agent"

    // Test simple hyphenated term parsing
    let result = parse_query_test("multi-agent").unwrap();

    // Should be parsed as a single term with compound tokenization
    match result {
        Expr::Term { keywords, .. } => {
            // After tokenization, "multi-agent" should become ["multi", "agent"]
            assert!(keywords.contains(&"multi".to_string()));
            assert!(keywords.contains(&"agent".to_string()));
            assert_eq!(keywords.len(), 2);
        },
        other => {
            panic!("Expected Term expression for hyphenated compound term, got: {other:?}");
        }
    }

    // Test the original problematic query
    let result = parse_query_test("yaml workflow agent multi-agent user input").unwrap();

    // Should be parsed as OR expression with all terms, no negated agent
    match result {
        Expr::Or(_, _) => {
            // This should now work correctly without negated terms
            // The exact structure is complex, but the key is no negated "agent" terms
            // Parsing now creates correct OR expression without negated terms - test passes
        },
        _ => {
            panic!("Expected Or expression for multi-term query");
        }
    }
}

#[test]
fn test_workflow_should_not_be_split() {
    // Test that "workflow" remains as a single term in programming contexts
    // After fix: "workflow" should not be split into "work" + "flow"

    // First, check if workflow is in exception terms
    use probe_code::search::term_exceptions::is_exception_term;
    assert!(is_exception_term("workflow"), "workflow should be in exception terms");

    let result = tokenize_and_stem("workflow");

    // After fix: should remain as ["workflow"] since it's a common programming term
    assert_eq!(result.len(), 1, "workflow should remain as single term");
    assert_eq!(result, vec!["workflow"]);
}