use lru::LruCache;
use probe_code::search::elastic_query::{self, parse_query_test as parse_query};
use probe_code::search::file_processing::filter_code_block_with_ast;
use probe_code::search::query::create_query_plan;
use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

/// Test direct usage of filter_code_block_with_ast with various complex queries
#[test]
fn test_filter_code_block_with_complex_ast() {
    // Enable debug mode
    std::env::set_var("DEBUG", "1");

    // Test case 1: Simple AND query
    test_simple_and_query();

    // Test case 2: Simple OR query
    test_simple_or_query();

    // Test case 3: Complex query with AND, OR, and negation
    test_complex_query_with_negation();

    // Test case 4: Query with all required terms
    test_required_terms_query();

    // Test case 5: Query with nested expressions
    test_nested_expressions_query();

    // Reset debug mode
    std::env::remove_var("DEBUG");
}

/// Test a simple AND query: "ip AND whitelist"
fn test_simple_and_query() {
    println!("\n=== Testing simple AND query: ip AND whitelist ===");

    // Create the query
    let query = "ip AND whitelist";

    // Parse the query into an AST
    // Using standard Elasticsearch behavior (AND for implicit combinations)
    let ast = parse_query(query).unwrap();
    println!("Parsed AST: {ast:?}");

    // Create a QueryPlan
    let plan = create_query_plan(query, false).unwrap();

    // Use the term indices from the QueryPlan
    let term_indices = &plan.term_indices;

    // Test case 1: Block with both terms (should match)
    {
        let mut term_matches = HashMap::new();

        // Add "ip" matches
        let mut ip_lines = HashSet::new();
        ip_lines.insert(3);
        term_matches.insert(*term_indices.get("ip").unwrap(), ip_lines);

        // Add "whitelist" matches directly (tokenization behavior has changed)
        let mut whitelist_lines = HashSet::new();
        whitelist_lines.insert(4);

        // Check if "whitelist" is in the term_indices, or if it's split into "white" and "list"
        if let Some(&idx) = term_indices.get("whitelist") {
            term_matches.insert(idx, whitelist_lines);
        } else {
            // If "whitelist" is not in the term_indices, try "white" and "list"
            if let Some(&idx) = term_indices.get("white") {
                let mut white_lines = HashSet::new();
                white_lines.insert(4);
                term_matches.insert(idx, white_lines);
            }

            if let Some(&idx) = term_indices.get("list") {
                let mut list_lines = HashSet::new();
                list_lines.insert(4);
                term_matches.insert(idx, list_lines);
            }
        }

        // Block lines
        let block_lines = (1, 10);

        // Test filtering
        let result = filter_code_block_with_ast(block_lines, &term_matches, &plan, true);
        assert!(
            result,
            "Block with both 'ip' and 'whitelist' should match the AND query"
        );
        println!("✓ Block with both terms matches");
    }

    // Test case 2: Block with only one term (should not match)
    {
        let mut term_matches = HashMap::new();

        // Add only "ip" matches
        let mut ip_lines = HashSet::new();
        ip_lines.insert(3);
        term_matches.insert(*term_indices.get("ip").unwrap(), ip_lines);

        // Block lines
        let block_lines = (1, 10);

        // Test filtering
        let result = filter_code_block_with_ast(block_lines, &term_matches, &plan, true);
        assert!(
            !result,
            "Block with only 'ip' should not match the AND query"
        );
        println!("✓ Block with only one term doesn't match");
    }
}

/// Test a simple OR query: "ip OR port"
fn test_simple_or_query() {
    println!("\n=== Testing simple OR query: ip OR port ===");

    // Create the query
    let query = "ip OR port";

    // Parse the query into an AST
    // Using standard Elasticsearch behavior (AND for implicit combinations)
    let ast = parse_query(query).unwrap();
    println!("Parsed AST: {ast:?}");

    // Create a QueryPlan
    let plan = create_query_plan(query, false).unwrap();

    // Use the term indices from the QueryPlan
    let term_indices = &plan.term_indices;

    // Test case 1: Block with both terms (should match)
    {
        let mut term_matches = HashMap::new();

        // Add "ip" matches
        let mut ip_lines = HashSet::new();
        ip_lines.insert(3);
        term_matches.insert(*term_indices.get("ip").unwrap(), ip_lines);

        // Add "port" matches
        let mut port_lines = HashSet::new();
        port_lines.insert(4);
        term_matches.insert(*term_indices.get("port").unwrap(), port_lines);

        // Block lines
        let block_lines = (1, 10);

        // Test filtering
        let result = filter_code_block_with_ast(block_lines, &term_matches, &plan, true);
        assert!(
            result,
            "Block with both 'ip' and 'port' should match the OR query"
        );
        println!("✓ Block with both terms matches");
    }

    // Test case 2: Block with only "ip" (should match)
    {
        let mut term_matches = HashMap::new();

        // Add only "ip" matches
        let mut ip_lines = HashSet::new();
        ip_lines.insert(3);
        term_matches.insert(*term_indices.get("ip").unwrap(), ip_lines);

        // Block lines
        let block_lines = (1, 10);

        // Test filtering
        let result = filter_code_block_with_ast(block_lines, &term_matches, &plan, true);
        assert!(result, "Block with only 'ip' should match the OR query");
        println!("✓ Block with only 'ip' matches");
    }

    // Test case 3: Block with only "port" (should match)
    {
        let mut term_matches = HashMap::new();

        // Add only "port" matches
        let mut port_lines = HashSet::new();
        port_lines.insert(4);
        term_matches.insert(*term_indices.get("port").unwrap(), port_lines);

        // Block lines
        let block_lines = (1, 10);

        // Test filtering
        let result = filter_code_block_with_ast(block_lines, &term_matches, &plan, true);
        assert!(result, "Block with only 'port' should match the OR query");
        println!("✓ Block with only 'port' matches");
    }

    // Test case 4: Block with neither term (should not match)
    {
        let term_matches = HashMap::new();

        // Block lines
        let block_lines = (1, 10);

        // Test filtering
        let result = filter_code_block_with_ast(block_lines, &term_matches, &plan, true);
        assert!(
            !result,
            "Block with neither 'ip' nor 'port' should not match the OR query"
        );
        println!("✓ Block with neither term doesn't match");
    }
}

/// Test a complex query with AND, OR, and negation: "(ip OR port) AND whitelist AND -denylist"
fn test_complex_query_with_negation() {
    println!("\n=== Testing complex query: (ip OR port) AND whitelist AND -denylist ===");

    // Create the query
    let query = "(ip OR port) AND whitelist AND -denylist";

    // Parse the query into an AST
    // Using standard Elasticsearch behavior (AND for implicit combinations)
    let ast = parse_query(query).unwrap();
    println!("Parsed AST: {ast:?}");

    // Create a QueryPlan
    let plan = create_query_plan(query, false).unwrap();
    // Use the term indices from the QueryPlan
    let term_indices = &plan.term_indices;

    // Test case 1: Block with "ip", "whitelist", no "denylist" (should match)
    {
        let mut term_matches = HashMap::new();

        // Add "ip" matches
        let mut ip_lines = HashSet::new();
        ip_lines.insert(3);
        term_matches.insert(*term_indices.get("ip").unwrap(), ip_lines);

        // Add "whitelist" matches directly (tokenization behavior has changed)
        let mut whitelist_lines = HashSet::new();
        whitelist_lines.insert(4);

        // Check if "whitelist" is in the term_indices, or if it's split into "white" and "list"
        if let Some(&idx) = term_indices.get("whitelist") {
            term_matches.insert(idx, whitelist_lines);
        } else {
            // If "whitelist" is not in the term_indices, try "white" and "list"
            if let Some(&idx) = term_indices.get("white") {
                let mut white_lines = HashSet::new();
                white_lines.insert(4);
                term_matches.insert(idx, white_lines);
            }

            if let Some(&idx) = term_indices.get("list") {
                let mut list_lines = HashSet::new();
                list_lines.insert(4);
                term_matches.insert(idx, list_lines);
            }
        }

        // Block lines
        let block_lines = (1, 10);

        // Test filtering
        let result = filter_code_block_with_ast(block_lines, &term_matches, &plan, true);
        assert!(
            result,
            "Block with 'ip', 'whitelist', no 'denylist' should match"
        );
        println!("✓ Block with 'ip', 'whitelist', no 'denylist' matches");
    }

    // Test case 2: Block with "port", "whitelist", no "denylist" (should match)
    {
        let mut term_matches = HashMap::new();

        // Add "port" matches
        let mut port_lines = HashSet::new();
        port_lines.insert(3);
        term_matches.insert(*term_indices.get("port").unwrap(), port_lines);

        // Add "whitelist" matches directly (tokenization behavior has changed)
        let mut whitelist_lines = HashSet::new();
        whitelist_lines.insert(4);

        // Check if "whitelist" is in the term_indices, or if it's split into "white" and "list"
        if let Some(&idx) = term_indices.get("whitelist") {
            term_matches.insert(idx, whitelist_lines);
        } else {
            // If "whitelist" is not in the term_indices, try "white" and "list"
            if let Some(&idx) = term_indices.get("white") {
                let mut white_lines = HashSet::new();
                white_lines.insert(4);
                term_matches.insert(idx, white_lines);
            }

            if let Some(&idx) = term_indices.get("list") {
                let mut list_lines = HashSet::new();
                list_lines.insert(4);
                term_matches.insert(idx, list_lines);
            }
        }

        // Block lines
        let block_lines = (1, 10);

        // Test filtering
        let result = filter_code_block_with_ast(block_lines, &term_matches, &plan, true);
        assert!(
            result,
            "Block with 'port', 'whitelist', no 'denylist' should match"
        );
        println!("✓ Block with 'port', 'whitelist', no 'denylist' matches");
    }

    // Test case 3: Block with "ip", "whitelist", and "denylist" (should NOT match)
    {
        let mut term_matches = HashMap::new();

        // Add "ip" matches
        let mut ip_lines = HashSet::new();
        ip_lines.insert(3);
        term_matches.insert(*term_indices.get("ip").unwrap(), ip_lines);

        // Add "whitelist" matches directly (tokenization behavior has changed)
        let mut whitelist_lines = HashSet::new();
        whitelist_lines.insert(4);

        // Check if "whitelist" is in the term_indices, or if it's split into "white" and "list"
        if let Some(&idx) = term_indices.get("whitelist") {
            term_matches.insert(idx, whitelist_lines);
        } else {
            // If "whitelist" is not in the term_indices, try "white" and "list"
            if let Some(&idx) = term_indices.get("white") {
                let mut white_lines = HashSet::new();
                white_lines.insert(4);
                term_matches.insert(idx, white_lines);
            }

            if let Some(&idx) = term_indices.get("list") {
                let mut list_lines = HashSet::new();
                list_lines.insert(4);
                term_matches.insert(idx, list_lines);
            }
        }

        // Add "denylist" matches
        let mut denylist_lines = HashSet::new();
        denylist_lines.insert(5);
        term_matches.insert(*term_indices.get("denylist").unwrap(), denylist_lines);

        // Block lines
        let block_lines = (1, 10);

        // Test filtering
        let result = filter_code_block_with_ast(block_lines, &term_matches, &plan, true);
        assert!(
            !result,
            "Block with 'ip', 'whitelist', and 'denylist' should NOT match"
        );
        println!("✓ Block with 'ip', 'whitelist', and 'denylist' doesn't match");
    }

    // Test case 4: Block with only "ip" and "port" (should NOT match due to missing "whitelist")
    {
        let mut term_matches = HashMap::new();

        // Add "ip" matches
        let mut ip_lines = HashSet::new();
        ip_lines.insert(3);
        term_matches.insert(*term_indices.get("ip").unwrap(), ip_lines);

        // Add "port" matches
        let mut port_lines = HashSet::new();
        port_lines.insert(4);
        term_matches.insert(*term_indices.get("port").unwrap(), port_lines);

        // Block lines
        let block_lines = (1, 10);

        // Test filtering
        let result = filter_code_block_with_ast(block_lines, &term_matches, &plan, true);
        assert!(
            !result,
            "Block with only 'ip' and 'port' should NOT match due to missing 'whitelist'"
        );
        println!("✓ Block with only 'ip' and 'port' doesn't match");
    }
}

/// Test a query with all required terms: "+ip +whitelist +security" (note: these are connected with OR, not AND)
fn test_required_terms_query() {
    println!("\n=== Testing required terms query: +ip +whitelist +security ===");

    // Create the query
    let _query = "+ip +whitelist +security";

    // Create a custom AST with OR semantics for required terms
    let keywords1 = vec!["ip".to_string()];
    let keywords2 = vec!["white".to_string(), "list".to_string()];
    let keywords3 = vec!["secur".to_string()];
    let ast = elastic_query::Expr::Or(
        Box::new(elastic_query::Expr::Or(
            Box::new(elastic_query::Expr::Term {
                lowercase_keywords: keywords1.iter().map(|k| k.to_lowercase()).collect(),
                keywords: keywords1,
                field: None,
                required: true,
                excluded: false,
                exact: false,
            }),
            Box::new(elastic_query::Expr::Term {
                lowercase_keywords: keywords2.iter().map(|k| k.to_lowercase()).collect(),
                keywords: keywords2,
                field: None,
                required: true,
                excluded: false,
                exact: false,
            }),
        )),
        Box::new(elastic_query::Expr::Term {
            lowercase_keywords: keywords3.iter().map(|k| k.to_lowercase()).collect(),
            keywords: keywords3,
            field: None,
            required: true,
            excluded: false,
            exact: false,
        }),
    );
    println!("Parsed AST: {ast:?}");

    // Create a custom QueryPlan with our AST
    let mut indices = HashMap::new();
    indices.insert("ip".to_string(), 0);
    indices.insert("list".to_string(), 1);
    indices.insert("secur".to_string(), 2);
    indices.insert("white".to_string(), 3);

    let has_required_anywhere = ast.has_required_term();
    let has_only_excluded_terms = ast.is_only_excluded_terms();
    let required_terms_indices = HashSet::new();
    let evaluation_cache = Arc::new(Mutex::new(LruCache::new(NonZeroUsize::new(1000).unwrap())));

    let plan = probe_code::search::query::QueryPlan {
        ast: ast.clone(),
        term_indices: indices.clone(),
        excluded_terms: HashSet::new(),
        exact: false,
        is_simple_query: false,
        required_terms: HashSet::new(),
        has_required_anywhere,
        required_terms_indices,
        has_only_excluded_terms,
        evaluation_cache,
        is_universal_query: false,
        special_case_indices: HashSet::new(),
        special_case_terms_lower: HashMap::new(),
    };

    // Use the term indices directly
    let term_indices = &indices;

    // Test case 1: Block with all required terms (should match)
    {
        let mut term_matches = HashMap::new();

        // Add all term matches
        let mut ip_lines = HashSet::new();
        ip_lines.insert(3);
        term_matches.insert(*term_indices.get("ip").unwrap(), ip_lines);

        // Add "white" and "list" matches (from tokenization of "whitelist")
        let mut white_lines = HashSet::new();
        white_lines.insert(4);
        term_matches.insert(*term_indices.get("white").unwrap(), white_lines);

        let mut list_lines = HashSet::new();
        list_lines.insert(4);
        term_matches.insert(*term_indices.get("list").unwrap(), list_lines);

        let mut secur_lines = HashSet::new();
        secur_lines.insert(5);
        term_matches.insert(*term_indices.get("secur").unwrap(), secur_lines);

        // Block lines
        let block_lines = (1, 10);

        // Test filtering
        let result = filter_code_block_with_ast(block_lines, &term_matches, &plan, true);
        assert!(result, "Block with all required terms should match");
        println!("✓ Block with all required terms matches");
    }

    // Test case 2: Block missing one required term (should NOT match)
    {
        let mut term_matches = HashMap::new();

        // Add only two term matches
        let mut ip_lines = HashSet::new();
        ip_lines.insert(3);
        term_matches.insert(*term_indices.get("ip").unwrap(), ip_lines);

        // Add "white" and "list" matches (from tokenization of "whitelist")
        let mut white_lines = HashSet::new();
        white_lines.insert(4);
        term_matches.insert(*term_indices.get("white").unwrap(), white_lines);

        let mut list_lines = HashSet::new();
        list_lines.insert(4);
        term_matches.insert(*term_indices.get("list").unwrap(), list_lines);

        // Block lines
        let block_lines = (1, 10);

        // Test filtering
        let result = filter_code_block_with_ast(block_lines, &term_matches, &plan, true);
        // Note: In correct Lucene semantics, ALL required terms must be present
        // This block is missing 'security', so it should NOT match
        assert!(
            !result,
            "Block missing required term 'security' should NOT match"
        );
        println!("✓ Block missing required terms correctly rejected");
    }
}

/// Test a query with nested expressions: "ip AND (whitelist OR (security AND firewall))"
fn test_nested_expressions_query() {
    println!(
        "\n=== Testing nested expressions query: ip AND (whitelist OR (security AND firewall)) ==="
    );

    // Create the query
    let query = "ip AND (whitelist OR (security AND firewall))";

    // Parse the query into an AST
    // Using standard Elasticsearch behavior (AND for implicit combinations)
    let ast = parse_query(query).unwrap();
    println!("Parsed AST: {ast:?}");

    // Create a QueryPlan
    let plan = create_query_plan(query, false).unwrap();

    // Use the term indices from the QueryPlan
    let term_indices = &plan.term_indices;

    // Test case 1: Block with "ip" and "whitelist" (should match)
    {
        let mut term_matches = HashMap::new();

        // Add "ip" matches
        let mut ip_lines = HashSet::new();
        ip_lines.insert(3);
        term_matches.insert(*term_indices.get("ip").unwrap(), ip_lines);

        // Add "whitelist" matches directly (tokenization behavior has changed)
        let mut whitelist_lines = HashSet::new();
        whitelist_lines.insert(4);

        // Check if "whitelist" is in the term_indices, or if it's split into "white" and "list"
        if let Some(&idx) = term_indices.get("whitelist") {
            term_matches.insert(idx, whitelist_lines);
        } else {
            // If "whitelist" is not in the term_indices, try "white" and "list"
            if let Some(&idx) = term_indices.get("white") {
                let mut white_lines = HashSet::new();
                white_lines.insert(4);
                term_matches.insert(idx, white_lines);
            }

            if let Some(&idx) = term_indices.get("list") {
                let mut list_lines = HashSet::new();
                list_lines.insert(4);
                term_matches.insert(idx, list_lines);
            }
        }

        // Block lines
        let block_lines = (1, 10);

        // Test filtering
        let result = filter_code_block_with_ast(block_lines, &term_matches, &plan, true);
        assert!(result, "Block with 'ip' and 'whitelist' should match");
        println!("✓ Block with 'ip' and 'whitelist' matches");
    }

    // Test case 2: Block with "ip", "security", and "firewall" (should match)
    {
        let mut term_matches = HashMap::new();

        // Add "ip" matches
        let mut ip_lines = HashSet::new();
        ip_lines.insert(3);
        term_matches.insert(*term_indices.get("ip").unwrap(), ip_lines);

        // Add "secur" matches (stemmed from "security")
        let mut secur_lines = HashSet::new();
        secur_lines.insert(4);
        term_matches.insert(*term_indices.get("secur").unwrap(), secur_lines);

        // Add "firewal" and "firewall" matches (stemmed from "firewall")
        let mut firewal_lines = HashSet::new();
        firewal_lines.insert(5);

        // Check if "firewal" is in the term_indices
        if let Some(&idx) = term_indices.get("firewal") {
            term_matches.insert(idx, firewal_lines.clone());
        }

        // Check if "firewall" is in the term_indices
        if let Some(&idx) = term_indices.get("firewall") {
            term_matches.insert(idx, firewal_lines);
        }

        // Block lines
        let block_lines = (1, 10);

        // Test filtering
        let result = filter_code_block_with_ast(block_lines, &term_matches, &plan, true);
        assert!(
            result,
            "Block with 'ip', 'security', and 'firewall' should match"
        );
        println!("✓ Block with 'ip', 'security', and 'firewall' matches");
    }

    // Test case 3: Block with "ip" and "security" but no "firewall" (should NOT match)
    {
        let mut term_matches = HashMap::new();

        // Add "ip" matches
        let mut ip_lines = HashSet::new();
        ip_lines.insert(3);
        term_matches.insert(*term_indices.get("ip").unwrap(), ip_lines);

        // Add "secur" matches (stemmed from "security")
        let mut secur_lines = HashSet::new();
        secur_lines.insert(4);
        term_matches.insert(*term_indices.get("secur").unwrap(), secur_lines);

        // Block lines
        let block_lines = (1, 10);

        // Test filtering
        let result = filter_code_block_with_ast(block_lines, &term_matches, &plan, true);
        assert!(
            !result,
            "Block with 'ip' and 'security' but no 'firewall' should NOT match"
        );
        println!("✓ Block with 'ip' and 'security' but no 'firewall' doesn't match");
    }

    // Test case 4: Block with only "ip" (should NOT match)
    {
        let mut term_matches = HashMap::new();

        // Add only "ip" matches
        let mut ip_lines = HashSet::new();
        ip_lines.insert(3);
        term_matches.insert(*term_indices.get("ip").unwrap(), ip_lines);

        // Block lines
        let block_lines = (1, 10);

        // Test filtering
        let result = filter_code_block_with_ast(block_lines, &term_matches, &plan, true);
        assert!(!result, "Block with only 'ip' should NOT match");
        println!("✓ Block with only 'ip' doesn't match");
    }
}
