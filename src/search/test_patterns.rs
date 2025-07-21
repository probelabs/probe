use probe_code::search::query::{preprocess_query, create_term_patterns};
use std::collections::HashSet;

#[test]
fn test_grouped_patterns() {
    // Test with "ip" and "whitelisting"
    let term_pairs = vec![
        ("ip".to_string(), "ip".to_string()),
        ("whitelisting".to_string(), "whitelist".to_string()),
    ];
    
    let patterns = create_term_patterns(&term_pairs);
    
    // Print the patterns for inspection
    println!("Generated patterns:");
    for (pattern, indices) in &patterns {
        println!("Pattern: {pattern:?}, Indices: {indices:?}");
    }
    
    // Verify we have the expected number of patterns
    // 1 pattern for each term (with combined boundaries) + 1 pattern for combinations
    assert_eq!(patterns.len(), 3);
    
    // Verify the first pattern is for "ip" with both boundaries
    let ip_pattern = patterns.iter().find(|(_, indices)| indices.len() == 1 && indices.contains(&0));
    assert!(ip_pattern.is_some());
    let (ip_pattern, _) = ip_pattern.unwrap();
    assert!(ip_pattern.contains("\\bip|ip\\b"));
    
    // Verify the second pattern is for "whitelisting|whitelist" with both boundaries
    let whitelist_pattern = patterns.iter().find(|(_, indices)| indices.len() == 1 && indices.contains(&1));
    assert!(whitelist_pattern.is_some());
    let (whitelist_pattern, _) = whitelist_pattern.unwrap();
    assert!(whitelist_pattern.contains("(whitelisting|whitelist)"));
    
    // Verify the third pattern contains all combinations
    let combo_pattern = patterns.iter().find(|(_, indices)| indices.len() == 2);
    assert!(combo_pattern.is_some());
    let (combo_pattern, _) = combo_pattern.unwrap();
    assert!(combo_pattern.contains("("));
    assert!(combo_pattern.contains("|"));
    assert!(combo_pattern.contains("ipwhitelisting"));
    assert!(combo_pattern.contains("ipwhitelist"));
}
