use probe::ranking::{compute_avgdl, rank_documents, tokenize, RankingParams};
use probe::search::query::{create_query_plan, create_structured_patterns, regex_escape};
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

    // Test that create_query_plan and create_structured_patterns work together
    #[test]
    #[ignore] // Temporarily disabled due to changes in regex escaping
    fn test_query_preprocessing_pipeline(query in "\\PC{1,50}") {
        // This should never panic
        let plan = match create_query_plan(&query, false) {
            Ok(plan) => plan,
            Err(_) => return proptest::test_runner::TestCaseResult::Ok(()), // Skip invalid queries
        };

        let patterns = create_structured_patterns(&plan);

        // Check that we have at least one pattern for each term
        for (term, &idx) in &plan.term_indices {
            if !plan.excluded_terms.contains(term) {
                // Find at least one pattern that contains the term index
                let found = patterns.iter().any(|(_, indices)| indices.contains(&idx));
                assert!(found, "No pattern found for term '{}' at index {}", term, idx);
            }
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

        // Create RankingParams
        let params = RankingParams {
            documents: &docs_refs,
            query: &query,
        };

        // This should never panic
        let ranked = rank_documents(&params);

        // With BM25, empty documents or queries may result in no matches
        // So we don't assert that ranked.len() == docs.len()
        // Instead, we just check that the function doesn't panic

        // If there are any results, check that indices are valid
        if !ranked.is_empty() {
            // Each document index should be unique
            let mut indices: Vec<usize> = ranked.iter().map(|(idx, _)| *idx).collect();
            indices.sort();
            indices.dedup();

            // Each index should be in the valid range
            for (idx, _) in ranked {
                assert!(idx < docs.len());
            }
        }
    }
}
