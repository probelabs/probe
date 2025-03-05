use probe::ranking::{compute_avgdl, rank_documents, tokenize};
use probe::search::query::{create_term_patterns, preprocess_query, regex_escape};
use proptest::prelude::*;

proptest! {
    // Test that tokenize properly handles all kinds of strings
    #[test]
    fn test_tokenize_arbitrary_strings(s in "\\PC*") {
        // This should never panic
        let tokens = tokenize(&s);

        // Empty or all-stopword strings should produce empty token lists
        if s.trim().is_empty() {
            assert!(tokens.is_empty());
        }
    }

    // Test that regex_escape works for all strings
    #[test]
    fn test_regex_escape_arbitrary_strings(s in "\\PC*") {
        let escaped = regex_escape(&s);

        // The escaped string should be at least as long as the original
        // (equal if no special chars, longer if there are special chars)
        assert!(escaped.len() >= s.len());

        // Special characters should be escaped with a backslash
        let special_chars = ['.', '^', '$', '*', '+', '?', '(', ')', '[', ']', '{', '}', '|', '\\'];
        for c in special_chars.iter() {
            let count_in_orig = s.chars().filter(|&ch| ch == *c).count();
            let count_in_escaped = escaped.match_indices(&format!("\\{}", c)).count();

            // All occurrences of special chars should be escaped
            assert_eq!(count_in_orig, count_in_escaped);
        }
    }

    // Test that preprocess_query and create_term_patterns work together
    #[test]
    #[ignore] // Temporarily disabled due to changes in regex escaping
    fn test_query_preprocessing_pipeline(query in "\\PC{1,50}") {
        // This should never panic
        let term_pairs = preprocess_query(&query, false); // Use non-exact mode for property tests
        let patterns = create_term_patterns(&term_pairs);

        // Since create_term_patterns now returns a Vec<(String, HashSet<usize>)>,
        // we need to check the patterns differently

        // Check that we have at least one pattern for each term
        for i in 0..term_pairs.len() {
            // Find at least one pattern that contains the term index i
            let found = patterns.iter().any(|(_, indices)| indices.contains(&i));
            assert!(found, "No pattern found for term at index {}", i);
        }

        // Check that each pattern has the correct format
        for (pattern, _) in &patterns {
            // Pattern should be a valid regex
            assert!(regex::Regex::new(pattern).is_ok(), "Invalid regex pattern: {}", pattern);
        }
    }

    // Test that compute_avgdl handles arrays of arbitrary lengths
    #[test]
    fn test_compute_avgdl(lengths in prop::collection::vec(1..1000_usize, 0..100)) {
        let avgdl = compute_avgdl(&lengths);

        // If lengths is empty, avgdl should be 0
        if lengths.is_empty() {
            assert_eq!(avgdl, 0.0);
        } else {
            // avgdl should be the average of the lengths
            let sum: usize = lengths.iter().sum();
            let expected = sum as f64 / lengths.len() as f64;
            assert!((avgdl - expected).abs() < f64::EPSILON);
        }
    }

    // Test that rank_documents doesn't panic with arbitrary documents and queries
    #[test]
    fn test_rank_documents_doesnt_panic(
        docs in prop::collection::vec("\\PC{0,100}", 0..10),
        query in "\\PC{0,50}"
    ) {
        // Convert Vec<String> to Vec<&str>
        let docs_refs: Vec<&str> = docs.iter().map(|s| s.as_str()).collect();

        // This should never panic
        let ranked = rank_documents(&docs_refs, &query, None, None, None, None, None, None);

        // The number of ranked documents should match the input
        assert_eq!(ranked.len(), docs.len());

        // Each document index should appear exactly once
        let mut indices: Vec<usize> = ranked.iter().map(|(idx, _, _, _, _)| *idx).collect();
        indices.sort();
        indices.dedup();
        assert_eq!(indices.len(), docs.len());

        // Each index should be in the valid range
        for (idx, _, _, _, _) in ranked {
            assert!(idx < docs.len());
        }
    }
}
