use super::Expr;
use std::collections::{HashMap, HashSet};

// Helper functions to create common expressions
fn create_term(keyword: &str) -> Expr {
    // For testing purposes, we'll bypass the tokenization and stemming
    // by directly creating a term with the exact keyword
    Expr::Term {
        keywords: vec![keyword.to_string()],
        field: None,
        required: false,
        excluded: false,
        exact: false,
    }
}

fn create_required_term(keyword: &str) -> Expr {
    // For testing purposes, we'll bypass the tokenization and stemming
    // by directly creating a term with the exact keyword
    Expr::Term {
        keywords: vec![keyword.to_string()],
        field: None,
        required: true,
        excluded: false,
        exact: false,
    }
}

fn create_excluded_term(keyword: &str) -> Expr {
    // For testing purposes, we'll bypass the tokenization and stemming
    // by directly creating a term with the exact keyword
    Expr::Term {
        keywords: vec![keyword.to_string()],
        field: None,
        required: false,
        excluded: true,
        exact: false,
    }
}

fn create_exact_term(keyword: &str) -> Expr {
    // For testing purposes, we'll bypass the tokenization and stemming
    // by directly creating a term with the exact keyword
    Expr::Term {
        keywords: vec![keyword.to_string()],
        field: None,
        required: false,
        excluded: false,
        exact: true,
    }
}

// Helper function to create a term index map
fn create_term_indices(terms: &[&str]) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    for (i, &term) in terms.iter().enumerate() {
        map.insert(term.to_string(), i);
    }
    map
}

// Helper function to create a matched terms set
fn create_matched_terms(indices: &[usize]) -> HashSet<usize> {
    indices.iter().copied().collect()
}

#[test]
fn test_evaluate_simple_terms() {
    // Create term indices
    let term_indices = create_term_indices(&["foo", "bar", "baz"]);

    // Test a simple term
    let expr = create_term("foo");

    // Match when term is present
    let matched_terms = create_matched_terms(&[0]);
    assert!(expr.evaluate(&matched_terms, &term_indices, false));

    // No match when term is absent
    let matched_terms = create_matched_terms(&[1, 2]);
    assert!(!expr.evaluate(&matched_terms, &term_indices, false));

    // Test required term
    let expr = create_required_term("foo");

    // Match when required term is present
    let matched_terms = create_matched_terms(&[0]);
    assert!(expr.evaluate(&matched_terms, &term_indices, false));

    // No match when required term is absent
    let matched_terms = create_matched_terms(&[1, 2]);
    assert!(!expr.evaluate(&matched_terms, &term_indices, false));

    // Test excluded term
    let expr = create_excluded_term("foo");

    // Match when excluded term is absent
    let matched_terms = create_matched_terms(&[1, 2]);
    assert!(expr.evaluate(&matched_terms, &term_indices, false));

    // No match when excluded term is present
    let matched_terms = create_matched_terms(&[0, 1]);
    assert!(!expr.evaluate(&matched_terms, &term_indices, false));
}

#[test]
fn test_evaluate_and_expressions() {
    // Create term indices
    let term_indices = create_term_indices(&["foo", "bar", "baz"]);

    // Test AND expression
    let expr = Expr::And(
        Box::new(create_term("foo")),
        Box::new(create_term("bar"))
    );

    // Match when both terms are present
    let matched_terms = create_matched_terms(&[0, 1]);
    assert!(expr.evaluate(&matched_terms, &term_indices, false));

    // No match when only one term is present
    let matched_terms = create_matched_terms(&[0]);
    assert!(!expr.evaluate(&matched_terms, &term_indices, false));

    let matched_terms = create_matched_terms(&[1]);
    assert!(!expr.evaluate(&matched_terms, &term_indices, false));

    // No match when neither term is present
    let matched_terms = create_matched_terms(&[2]);
    assert!(!expr.evaluate(&matched_terms, &term_indices, false));
}

#[test]
fn test_evaluate_or_expressions() {
    // Create term indices
    let term_indices = create_term_indices(&["foo", "bar", "baz"]);

    // Test OR expression
    let expr = Expr::Or(
        Box::new(create_term("foo")),
        Box::new(create_term("bar"))
    );

    // Match when both terms are present
    let matched_terms = create_matched_terms(&[0, 1]);
    assert!(expr.evaluate(&matched_terms, &term_indices, false));

    // Match when only one term is present
    let matched_terms = create_matched_terms(&[0]);
    assert!(expr.evaluate(&matched_terms, &term_indices, false));

    let matched_terms = create_matched_terms(&[1]);
    assert!(expr.evaluate(&matched_terms, &term_indices, false));

    // No match when neither term is present
    let matched_terms = create_matched_terms(&[2]);
    assert!(!expr.evaluate(&matched_terms, &term_indices, false));
}

#[test]
fn test_evaluate_complex_expressions() {
    // Create term indices
    let term_indices = create_term_indices(&["foo", "bar", "baz", "qux", "zod"]);

    // Test complex expression: (foo AND bar) OR baz
    let expr = Expr::Or(
        Box::new(Expr::And(
            Box::new(create_term("foo")),
            Box::new(create_term("bar"))
        )),
        Box::new(create_term("baz"))
    );

    // Match when both foo and bar are present
    let matched_terms = create_matched_terms(&[0, 1]);
    assert!(expr.evaluate(&matched_terms, &term_indices, false));

    // Match when baz is present
    let matched_terms = create_matched_terms(&[2]);
    assert!(expr.evaluate(&matched_terms, &term_indices, false));

    // Match when all terms are present
    let matched_terms = create_matched_terms(&[0, 1, 2]);
    assert!(expr.evaluate(&matched_terms, &term_indices, false));

    // No match when only foo is present
    let matched_terms = create_matched_terms(&[0]);
    assert!(!expr.evaluate(&matched_terms, &term_indices, false));

    // No match when only bar is present
    let matched_terms = create_matched_terms(&[1]);
    assert!(!expr.evaluate(&matched_terms, &term_indices, false));

    // No match when neither (foo AND bar) nor baz is present
    let matched_terms = create_matched_terms(&[3, 4]);
    assert!(!expr.evaluate(&matched_terms, &term_indices, false));
}

#[test]
fn test_evaluate_required_excluded_terms() {
    // Create term indices
    let term_indices = create_term_indices(&["foo", "bar", "baz", "qux"]);

    // Test expression: +foo -bar
    let expr = Expr::And(
        Box::new(create_required_term("foo")),
        Box::new(create_excluded_term("bar"))
    );

    // Match when foo is present and bar is absent
    let matched_terms = create_matched_terms(&[0, 2, 3]);
    assert!(expr.evaluate(&matched_terms, &term_indices, false));

    // No match when foo is absent
    let matched_terms = create_matched_terms(&[2, 3]);
    assert!(!expr.evaluate(&matched_terms, &term_indices, false));

    // No match when bar is present
    let matched_terms = create_matched_terms(&[0, 1, 2]);
    assert!(!expr.evaluate(&matched_terms, &term_indices, false));
}

#[test]
fn test_evaluate_elastic_style_queries() {
    // Create term indices
    let term_indices = create_term_indices(&["keyword1", "keyword2", "keyword3", "keyword4"]);

    // Test expression: +(keyword1 OR keyword2) -keyword3
    // This means: (keyword1 OR keyword2) is required as a group, keyword3 is excluded
    // So we create one OR group where at least one term must match, then AND with exclusion
    let expr = Expr::And(
        Box::new(Expr::Or(
            Box::new(create_term("keyword1")),
            Box::new(create_term("keyword2"))
        )),
        Box::new(create_excluded_term("keyword3"))
    );

    // Match when keyword1 is present and keyword3 is absent
    let matched_terms = create_matched_terms(&[0]);
    assert!(expr.evaluate(&matched_terms, &term_indices, false));

    // Match when keyword2 is present and keyword3 is absent
    let matched_terms = create_matched_terms(&[1]);
    assert!(expr.evaluate(&matched_terms, &term_indices, false));

    // Match when both keyword1 and keyword2 are present and keyword3 is absent
    let matched_terms = create_matched_terms(&[0, 1]);
    assert!(expr.evaluate(&matched_terms, &term_indices, false));

    // No match when keyword3 is present, even if keyword1 is present
    let matched_terms = create_matched_terms(&[0, 2]);
    assert!(!expr.evaluate(&matched_terms, &term_indices, false));

    // No match when keyword3 is present, even if keyword2 is present
    let matched_terms = create_matched_terms(&[1, 2]);
    assert!(!expr.evaluate(&matched_terms, &term_indices, false));

    // No match when neither keyword1 nor keyword2 is present
    let matched_terms = create_matched_terms(&[3]);
    assert!(!expr.evaluate(&matched_terms, &term_indices, false));
}

#[test]
fn test_evaluate_nested_expressions() {
    // Create term indices
    let term_indices = create_term_indices(&["a", "b", "c", "d", "e"]);

    // Test expression: a AND (b OR (c AND (d OR e)))
    let expr = Expr::And(
        Box::new(create_term("a")),
        Box::new(Expr::Or(
            Box::new(create_term("b")),
            Box::new(Expr::And(
                Box::new(create_term("c")),
                Box::new(Expr::Or(
                    Box::new(create_term("d")),
                    Box::new(create_term("e"))
                ))
            ))
        ))
    );

    // Match when a and b are present
    let matched_terms = create_matched_terms(&[0, 1]);
    assert!(expr.evaluate(&matched_terms, &term_indices, false));

    // Match when a, c, and d are present
    let matched_terms = create_matched_terms(&[0, 2, 3]);
    assert!(expr.evaluate(&matched_terms, &term_indices, false));

    // Match when a, c, and e are present
    let matched_terms = create_matched_terms(&[0, 2, 4]);
    assert!(expr.evaluate(&matched_terms, &term_indices, false));

    // Match when all terms are present
    let matched_terms = create_matched_terms(&[0, 1, 2, 3, 4]);
    assert!(expr.evaluate(&matched_terms, &term_indices, false));

    // No match when a is absent
    let matched_terms = create_matched_terms(&[1, 2, 3, 4]);
    assert!(!expr.evaluate(&matched_terms, &term_indices, false));

    // No match when neither b nor (c AND (d OR e)) is satisfied
    let matched_terms = create_matched_terms(&[0, 2]); // a and c, but no d or e
    assert!(!expr.evaluate(&matched_terms, &term_indices, false));
}

#[test]
fn test_evaluate_with_missing_terms() {
    // Create term indices with some terms missing
    let term_indices = create_term_indices(&["foo", "bar"]);

    // Test expression with a term not in the index
    let expr = Expr::And(
        Box::new(create_term("foo")),
        Box::new(create_term("baz")) // Not in the index
    );

    // Should not match because baz is not in the index
    let matched_terms = create_matched_terms(&[0]);
    assert!(!expr.evaluate(&matched_terms, &term_indices, false));

    // Test excluded term not in the index
    let expr = Expr::And(
        Box::new(create_term("foo")),
        Box::new(create_excluded_term("baz")) // Not in the index
    );

    // Should match because baz is not in the index (and thus not matched)
    let matched_terms = create_matched_terms(&[0]);
    assert!(expr.evaluate(&matched_terms, &term_indices, false));
}

#[test]
fn test_evaluate_optional_terms() {
    // Create term indices
    let term_indices = create_term_indices(&["required", "optional", "excluded"]);

    // Test expression with mixed term types: +required optional -excluded
    // Note: With the new behavior, space-separated terms are OR, not AND
    let expr = Expr::And(
        Box::new(Expr::Or(
            Box::new(create_required_term("required")),
            Box::new(create_term("optional"))
        )),
        Box::new(create_excluded_term("excluded"))
    );

    // Match when required is present but optional is absent and excluded is absent
    // This now matches because we only need either required OR optional
    let matched_terms = create_matched_terms(&[0]);
    assert!(expr.evaluate(&matched_terms, &term_indices, false));

    // Match when both required and optional are present and excluded is absent
    let matched_terms = create_matched_terms(&[0, 1]);
    assert!(expr.evaluate(&matched_terms, &term_indices, false));

    // No match when only optional is present but required is missing
    // In correct Lucene semantics, required terms must be present even in OR
    let matched_terms = create_matched_terms(&[1]);
    assert!(!expr.evaluate(&matched_terms, &term_indices, false));

    // No match when excluded is present, even with the OR behavior
    let matched_terms = create_matched_terms(&[0, 2]);
    assert!(!expr.evaluate(&matched_terms, &term_indices, false));
}

#[test]
fn test_evaluate_exact_terms() {
    // Create term indices with original and stemmed terms
    let term_indices = create_term_indices(&["running", "run", "whitelist", "white", "list"]);

    // Test exact term
    let expr = create_exact_term("running");

    // Match when the exact term is present
    let matched_terms = create_matched_terms(&[0]); // "running"
    assert!(expr.evaluate(&matched_terms, &term_indices, false));

    // No match when only the stemmed term is present
    let matched_terms = create_matched_terms(&[1]); // "run"
    assert!(!expr.evaluate(&matched_terms, &term_indices, false));

    // Test non-exact term - use the stemmed form directly
    let expr = Expr::Term {
        keywords: vec!["run".to_string()],
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
    let matched_terms = create_matched_terms(&[0, 1]); // Include both "running" and "run"
    assert!(expr.evaluate(&matched_terms, &term_indices_updated, false));

    // Match when only the stemmed term is present
    let matched_terms = create_matched_terms(&[1]); // "run"
    assert!(expr.evaluate(&matched_terms, &term_indices, false));

    // Test compound expression with exact term - simplified approach
    // Create a new expression with the stemmed form directly
    let expr = Expr::And(
        Box::new(Expr::Term {
            keywords: vec!["run".to_string()], // Use stemmed form directly
            field: None,
            required: false,
            excluded: false,
            exact: false,
        }),
        Box::new(create_exact_term("whitelist"))
    );

    // Match when both terms are present
    let matched_terms = create_matched_terms(&[1, 2]); // "run", "whitelist"
    assert!(expr.evaluate(&matched_terms, &term_indices, false));

    // No match when the exact term is only present as stemmed parts
    let matched_terms = create_matched_terms(&[0, 3, 4]); // "running", "white", "list"
    assert!(!expr.evaluate(&matched_terms, &term_indices, false));
}

#[test]
fn test_required_term_in_or_bug() {
    // Create term indices for "github" and "actions"
    let term_indices = create_term_indices(&["github", "actions"]);

    // Test query: "+github OR actions" - equivalent to "(+github OR actions)"
    // This should be parsed as OR(required_term("github"), term("actions"))
    let expr = Expr::Or(
        Box::new(create_required_term("github")),  // +github (required)
        Box::new(create_term("actions"))           // actions (optional)
    );

    println!("Testing query: (+github OR actions)");
    println!("AST: {expr:?}");

    // Test case 1: only "actions" is present (github is missing)
    // This should FAIL because github is required in the OR expression
    let matched_terms = create_matched_terms(&[1]); // Only "actions" (index 1)
    let result = expr.evaluate(&matched_terms, &term_indices, false);

    println!("Test 1 - Only 'actions' present: matched_terms={matched_terms:?}, result={result}");

    // BUG: This currently returns true but should return false
    // Because if a required term is in an OR, the entire OR should fail if that required term is missing
    // EXPECT: false - required term 'github' is missing
    // This is now FIXED to match correct Lucene semantics
    assert!(!result, "Should return false when required term 'github' is missing, even in OR expression");

    // Test case 2: only "github" is present
    // This should PASS because the required term is present
    let matched_terms = create_matched_terms(&[0]); // Only "github" (index 0)
    let result = expr.evaluate(&matched_terms, &term_indices, false);

    println!("Test 2 - Only 'github' present: matched_terms={matched_terms:?}, result={result}");
    assert!(result, "Should return true when required term 'github' is present");

    // Test case 3: both "github" and "actions" are present
    // This should PASS
    let matched_terms = create_matched_terms(&[0, 1]); // Both terms
    let result = expr.evaluate(&matched_terms, &term_indices, false);

    println!("Test 3 - Both terms present: matched_terms={matched_terms:?}, result={result}");
    assert!(result, "Should return true when both terms are present");

    // Test case 4: neither term is present
    // This should FAIL
    let matched_terms = create_matched_terms(&[]); // No terms
    let result = expr.evaluate(&matched_terms, &term_indices, false);

    println!("Test 4 - No terms present: matched_terms={matched_terms:?}, result={result}");
    assert!(!result, "Should return false when no terms are present");
}