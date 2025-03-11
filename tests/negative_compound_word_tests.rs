use std::collections::HashSet;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

use probe::search::elastic_query::parse_query;
use probe::search::query::extract_excluded_terms;
use probe::search::{perform_probe, SearchOptions};

/// Test negative compound word handling
#[test]
fn test_negative_compound_words() {
    // Create a temporary directory for testing
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create test files
    create_test_files(temp_path);

    // Test basic negative compound word
    test_basic_negative_compound_word(temp_path);

    // Test complex query with negative compound word
    test_complex_negative_compound_word(temp_path);

    // Test excluded terms extraction
    test_excluded_terms_extraction();
}

/// Create test files with content for negative compound word tests
fn create_test_files(temp_dir: &Path) {
    // File with "network" and "firewall" separately
    let file1_path = temp_dir.join("network_firewall.go");
    let file1_content = r#"
package security

// NetworkConfig configures the network settings
type NetworkConfig struct {
    FirewallEnabled bool
}

// FirewallRule represents a firewall rule
type FirewallRule struct {
    Name string
    Action string
}
"#;

    // File with only "network" and "settings"
    let file2_path = temp_dir.join("network_only.go");
    let file2_content = r#"
package network

// NetworkSettings configures the network settings
type NetworkSettings struct {
    Enabled bool
    Settings string
}
"#;

    // File with "networkfirewall" as a single term
    let file3_path = temp_dir.join("networkfirewall.go");
    let file3_content = r#"
package security

// NetworkFirewall configures the network firewall
type NetworkFirewall struct {
    Enabled bool
}
"#;

    // Write files to disk
    fs::write(file1_path, file1_content).unwrap();
    fs::write(file2_path, file2_content).unwrap();
    fs::write(file3_path, file3_content).unwrap();
}

/// Test basic negative compound word: "-networkfirewall"
fn test_basic_negative_compound_word(temp_path: &Path) {
    println!("\n=== Testing basic negative compound word: -networkfirewall ===");

    // Enable debug mode
    std::env::set_var("DEBUG", "1");

    // Create the query
    let query = "-networkfirewall";
    let queries = vec![query.to_string()];
    let custom_ignores: Vec<String> = vec![];

    // Create SearchOptions
    let options = SearchOptions {
        path: temp_path,
        queries: &queries,
        files_only: false,
        custom_ignores: &custom_ignores,
        exclude_filenames: false,
        reranker: "hybrid",
        frequency_search: true,
        max_results: None,
        max_bytes: None,
        max_tokens: None,
        allow_tests: true,

        exact: false, // Enable stemming and compound word splitting
        no_merge: true,
        merge_threshold: None,
        dry_run: false,
    };

    // Run the search
    let search_results = perform_probe(&options).unwrap();

    // Check that we didn't find the file with "networkfirewall" as a single term
    let found_networkfirewall = search_results
        .results
        .iter()
        .any(|r| r.file.contains("networkfirewall.go"));
    assert!(
        !found_networkfirewall,
        "Should NOT find networkfirewall.go with negative compound word"
    );

    // Check that we didn't find the file with "network" and "firewall" separately
    let found_network_firewall = search_results
        .results
        .iter()
        .any(|r| r.file.contains("network_firewall.go"));
    assert!(
        !found_network_firewall,
        "Should NOT find network_firewall.go with negative compound word"
    );

    // Check that we didn't find the file with only "network"
    let found_network_only = search_results
        .results
        .iter()
        .any(|r| r.file.contains("network_only.go"));
    assert!(
        !found_network_only,
        "Should NOT find network_only.go with negative compound word"
    );
}

/// Test complex query with negative compound word: "settings AND -networkfirewall"
fn test_complex_negative_compound_word(temp_path: &Path) {
    println!("\n=== Testing complex query with negative compound word: settings AND -networkfirewall ===");

    // Create the query
    let query = "settings AND -networkfirewall";
    let queries = vec![query.to_string()];
    let custom_ignores: Vec<String> = vec![];

    // Create SearchOptions
    let options = SearchOptions {
        path: temp_path,
        queries: &queries,
        files_only: false,
        custom_ignores: &custom_ignores,
        exclude_filenames: false,
        reranker: "hybrid",
        frequency_search: true,
        max_results: None,
        max_bytes: None,
        max_tokens: None,
        allow_tests: true,
        // Use all terms mode
        exact: false, // Enable stemming
        no_merge: true,
        merge_threshold: None,
        dry_run: false,
    };

    // Run the search
    let search_results = perform_probe(&options).unwrap();

    // Check that we found the file with "settings" but not "network" and "firewall"
    let found_network_only = search_results
        .results
        .iter()
        .any(|r| r.file.contains("network_only.go"));
    assert!(
        found_network_only,
        "Should find network_only.go with 'settings' but not 'networkfirewall'"
    );

    // Check that we didn't find the file with "networkfirewall" as a single term
    let found_networkfirewall = search_results
        .results
        .iter()
        .any(|r| r.file.contains("networkfirewall.go"));
    assert!(
        !found_networkfirewall,
        "Should NOT find networkfirewall.go with negative compound word"
    );

    // Check that we didn't find the file with "network" and "firewall" separately
    let found_network_firewall = search_results
        .results
        .iter()
        .any(|r| r.file.contains("network_firewall.go"));
    assert!(
        !found_network_firewall,
        "Should NOT find network_firewall.go with negative compound word"
    );
}

/// Test excluded terms extraction
fn test_excluded_terms_extraction() {
    println!("\n=== Testing excluded terms extraction ===");

    // Parse a query with a negative compound word
    let query = "-networkfirewall";
    // Check if ANY_TERM environment variable is set
    let any_term = std::env::var("ANY_TERM").unwrap_or_default() == "1";
    let ast = parse_query(query, any_term).unwrap();

    // Extract excluded terms
    let mut excluded_terms = HashSet::new();
    extract_excluded_terms(&ast, &mut excluded_terms);

    // Check that the original term is excluded
    assert!(
        excluded_terms.contains("networkfirewall"),
        "Original term 'networkfirewall' should be excluded"
    );

    // For negative terms, we don't apply compound word splitting anymore
    // So we don't expect the compound parts to be in the excluded terms set

    // Test with a more complex query
    let complex_query = "settings AND -networkfirewall";
    // Check if ANY_TERM environment variable is set
    let any_term = std::env::var("ANY_TERM").unwrap_or_default() == "1";
    let complex_ast = parse_query(complex_query, any_term).unwrap();

    // Extract excluded terms
    let mut complex_excluded_terms = HashSet::new();
    extract_excluded_terms(&complex_ast, &mut complex_excluded_terms);

    // Check that the original term is excluded
    assert!(
        complex_excluded_terms.contains("networkfirewall"),
        "Original term 'networkfirewall' should be excluded"
    );

    // For negative terms, we don't apply compound word splitting anymore
    // So we don't expect the compound parts to be in the excluded terms set

    // Reset debug mode
    std::env::remove_var("DEBUG");
}
