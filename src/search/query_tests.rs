use crate::search::query::{preprocess_query, regex_escape, create_term_patterns};

#[cfg(test)]
use std::collections::HashSet;

    #[test]
    fn test_preprocess_query() {
        // Test exact matching
        let exact_result = preprocess_query("findAPI inCode", true);
        assert_eq!(
            exact_result,
            vec![
                ("findapi".to_string(), "findapi".to_string()),
                ("incode".to_string(), "incode".to_string()),
            ]
        );

        // Test non-exact matching with camelCase and stop words
        let non_exact_result = preprocess_query("findAPIInCode typeIgnore", false);
        
        // Get the actual result for debugging
        println!("Actual result: {:?}", non_exact_result);
        
        // The current behavior is that camelCase words are split into individual terms
        // This is the correct behavior as per the implementation
        assert_eq!(
            non_exact_result,
            vec![
                ("find".to_string(), "find".to_string()),
                ("api".to_string(), "api".to_string()),
                ("code".to_string(), "code".to_string()),
                ("type".to_string(), "type".to_string()),
                ("ignore".to_string(), "ignor".to_string()),
            ]
        );
    }

    #[test]
    fn test_preprocess_query_with_stemming() {
        // Test preprocessing with stemming
        let query = "searching functions";
        let terms = preprocess_query(query, false); // Use non-exact mode
        
        // "searching" should be stemmed to "search"
        assert_eq!(terms.len(), 2);
        
        // Find the stemmed version of "searching"
        let search_term = terms.iter().find(|(orig, _)| orig == "searching");
        assert!(search_term.is_some());
        
        let (orig, stemmed) = search_term.unwrap();
        assert_eq!(orig, "searching");
        assert_eq!(stemmed, "search"); // Stemming should reduce "searching" to "search"
    }

    #[test]
    fn test_preprocess_query_empty() {
        // Test with empty query
        let query = "";
        let terms = preprocess_query(query, false); // Use non-exact mode
        
        assert_eq!(terms.len(), 0);
    }

    #[test]
    fn test_preprocess_query_only_stop_words() {
        // Test with only stop words
        let query = "the and of";
        let terms = preprocess_query(query, false); // Use non-exact mode
        
        assert_eq!(terms.len(), 0);
    }

    #[test]
    fn test_preprocess_query_exact_mode() {
        // Test exact mode preprocessing
        let query = "ip whitelist";
        
        // In exact mode
        let exact_terms = preprocess_query(query, true);
        
        // Should preserve both words without stemming
        assert_eq!(exact_terms.len(), 2);
        
        // Check that both words are preserved as-is
        let has_ip = exact_terms.iter().any(|(orig, stemmed)| orig == "ip" && stemmed == "ip");
        let has_whitelist = exact_terms.iter().any(|(orig, stemmed)| orig == "whitelist" && stemmed == "whitelist");
        
        assert!(has_ip);
        assert!(has_whitelist);
        
        // Test with stop words in exact mode
        let query_with_stop = "the ip whitelist for security";
        let exact_with_stop = preprocess_query(query_with_stop, true);
        
        // Should preserve all words including stop words
        assert_eq!(exact_with_stop.len(), 5);
        
        // Check that stop words are preserved
        let has_the = exact_with_stop.iter().any(|(orig, _)| orig == "the");
        let has_for = exact_with_stop.iter().any(|(orig, _)| orig == "for");
        
        assert!(has_the);
        assert!(has_for);
    }

    #[test]
    fn test_regex_escape() {
        // Test escaping special regex characters
        let special_chars = ".*+?()[]{}|^$\\";
        let escaped = regex_escape(special_chars);
        
        // Each special character should be escaped with a backslash
        assert_eq!(escaped, "\\.\\*\\+\\?\\(\\)\\[\\]\\{\\}\\|\\^\\$\\\\");
        
        // Test with normal text
        let normal_text = "normal text";
        let escaped_normal = regex_escape(normal_text);
        
        // Normal text should remain unchanged
        assert_eq!(escaped_normal, normal_text);
    }

    #[test]
    fn test_create_term_patterns() {
        // Test with multiple terms
        let term_pairs = vec![
            ("search".to_string(), "search".to_string()),
            ("function".to_string(), "function".to_string()),
            ("running".to_string(), "run".to_string()),
        ];
        
        let patterns = create_term_patterns(&term_pairs);
        
        // With the new grouped pattern format, we expect:
        // 1. One pattern for each term with combined boundaries
        // 2. Multiple patterns for term combinations
        
        // Check that we have patterns for each individual term
        let search_pattern = patterns.iter().find(|(p, indices)| 
            indices.len() == 1 && indices.contains(&0) && p.contains("search")
        );
        
        let function_pattern = patterns.iter().find(|(p, indices)| 
            indices.len() == 1 && indices.contains(&1) && p.contains("function")
        );
        
        let running_pattern = patterns.iter().find(|(p, indices)| 
            indices.len() == 1 && indices.contains(&2) && p.contains("running|run")
        );
        
        // Assert that patterns exist
        assert!(search_pattern.is_some(), "No pattern found for 'search'");
        assert!(function_pattern.is_some(), "No pattern found for 'function'");
        assert!(running_pattern.is_some(), "No pattern found for 'running|run'");
        
        // Check that term indices are correct
        assert_eq!(search_pattern.unwrap().1, HashSet::from([0]));
        assert_eq!(function_pattern.unwrap().1, HashSet::from([1]));
        assert_eq!(running_pattern.unwrap().1, HashSet::from([2]));
        
        // Check that patterns have word boundaries
        assert!(search_pattern.unwrap().0.contains("\\b"));
        assert!(function_pattern.unwrap().0.contains("\\b"));
        assert!(running_pattern.unwrap().0.contains("\\b"));
        
        // Check for concatenated patterns
        let search_function_pattern = patterns.iter().find(|(_, indices)| 
            indices.len() == 2 && indices.contains(&0) && indices.contains(&1)
        );
        
        let search_running_pattern = patterns.iter().find(|(_, indices)| 
            indices.len() == 2 && indices.contains(&0) && indices.contains(&2)
        );
        
        let function_running_pattern = patterns.iter().find(|(_, indices)| 
            indices.len() == 2 && indices.contains(&1) && indices.contains(&2)
        );
        
        assert!(search_function_pattern.is_some(), "No pattern found for 'search' + 'function'");
        assert!(search_running_pattern.is_some(), "No pattern found for 'search' + 'running'");
        assert!(function_running_pattern.is_some(), "No pattern found for 'function' + 'running'");
    }

    #[test]
    fn test_create_term_patterns_with_regex_chars() {
        // Test with terms containing regex special characters
        let terms = vec![
            ("search.term".to_string(), "search.term".to_string()),
            ("function(x)".to_string(), "function(x)".to_string()),
        ];
        
        let patterns = create_term_patterns(&terms);
        
        // Check that regex special characters are escaped
        let search_term_pattern = patterns.iter().find(|(p, indices)| 
            indices.len() == 1 && indices.contains(&0) && p.contains("search\\.term")
        );
        
        let function_x_pattern = patterns.iter().find(|(p, indices)| 
            indices.len() == 1 && indices.contains(&1) && p.contains("function\\(x\\)")
        );
        
        assert!(search_term_pattern.is_some(), "No pattern found for 'search.term'");
        assert!(function_x_pattern.is_some(), "No pattern found for 'function(x)'");
        
        // Check term indices
        assert_eq!(search_term_pattern.unwrap().1, HashSet::from([0]));
        assert_eq!(function_x_pattern.unwrap().1, HashSet::from([1]));
        
        // Check for concatenated patterns with escaped characters
        let concatenated = patterns.iter().find(|(p, indices)| 
            indices.len() == 2 && indices.contains(&0) && indices.contains(&1) &&
            (p.contains("search\\.term") && p.contains("function\\(x\\)"))
        );
        
        assert!(concatenated.is_some(), "No concatenated pattern found");
    }

    #[test]
    fn test_create_term_patterns_with_flexible_boundaries() {
        // Test with IP addresses and other terms that need flexible boundary handling
        let term_pairs = vec![
            ("ip".to_string(), "ip".to_string()),
            ("address".to_string(), "address".to_string()),
        ];
        
        let patterns = create_term_patterns(&term_pairs);
        
        // Check that we have patterns for each individual term
        let ip_pattern = patterns.iter().find(|(p, indices)| 
            indices.len() == 1 && indices.contains(&0) && p.contains("ip")
        );
        
        let address_pattern = patterns.iter().find(|(p, indices)| 
            indices.len() == 1 && indices.contains(&1) && p.contains("address")
        );
        
        assert!(ip_pattern.is_some(), "No pattern found for 'ip'");
        assert!(address_pattern.is_some(), "No pattern found for 'address'");
        
        // Check that term indices are correct
        assert_eq!(ip_pattern.unwrap().1, HashSet::from([0]));
        assert_eq!(address_pattern.unwrap().1, HashSet::from([1]));
        
        // Check that patterns have word boundaries
        assert!(ip_pattern.unwrap().0.contains("\\b"));
        assert!(address_pattern.unwrap().0.contains("\\b"));
        
        // Check for concatenated patterns
        let ip_address_pattern = patterns.iter().find(|(_, indices)| 
            indices.len() == 2 && indices.contains(&0) && indices.contains(&1)
        );
        
        assert!(ip_address_pattern.is_some(), "No pattern found for 'ip' + 'address'");
        
        // Verify the pattern contains both terms
        let pattern_str = ip_address_pattern.unwrap().0.clone();
        assert!(pattern_str.contains("ip") && pattern_str.contains("address"), 
                "Concatenated pattern doesn't contain both terms: {}", pattern_str);
    }

    #[test]
    fn test_preprocess_query_with_compound_words() {
        // Test preprocessing with compound words
        let query = "whitelist";
        
        // Get the processed terms
        let terms = preprocess_query(query, false); // Use non-exact mode
        
        // Print the actual result for debugging
        println!("Processed terms for 'whitelist': {:?}", terms);
        
        // Check that "whitelist" is split into "white" and "list"
        let has_white = terms.iter().any(|(_, stemmed)| stemmed == "white");
        let has_list = terms.iter().any(|(_, stemmed)| stemmed == "list");
        
        assert!(
            has_white && has_list,
            "Expected 'whitelist' to be split into 'white' and 'list', but got: {:?}",
            terms
        );
    }
    #[test]
    fn test_preprocess_query_english_stop_words() {
        // Test with "ENGLISH_STOP_WORDS"
        let query = "ENGLISH_STOP_WORDS";
        let terms = preprocess_query(query, false); // Use non-exact mode
        
        println!("Preprocessed terms for 'ENGLISH_STOP_WORDS': {:?}", terms);
        
        // Check that it's stemmed correctly
        let has_english = terms.iter().any(|(_, stemmed)| stemmed == "english");
        let has_stop = terms.iter().any(|(_, stemmed)| stemmed == "stop");
        let has_word = terms.iter().any(|(_, stemmed)| stemmed == "word");
        
        assert!(has_english, "Expected 'english' in the stemmed terms");
        assert!(has_stop, "Expected 'stop' in the stemmed terms");
        assert!(has_word, "Expected 'word' in the stemmed terms");
    }
    
    #[test]
    fn test_preprocess_query_with_negated_terms() {
        // Test with negated terms
        let query = "foo AND -bar";
        let terms = preprocess_query(query, false); // Use non-exact mode
        
        println!("Preprocessed terms for 'foo AND -bar': {:?}", terms);
        
        // Check that "foo" is included
        let has_foo = terms.iter().any(|(orig, _)| orig == "foo");
        assert!(has_foo, "Expected 'foo' in the terms");
        
        // Check that "bar" is NOT included (since it's negated)
        let has_bar = terms.iter().any(|(orig, _)| orig == "bar");
        assert!(!has_bar, "Did not expect 'bar' in the terms as it was negated");
        
        // Test with multiple negated terms
        let query = "search -test -ignore";
        let terms = preprocess_query(query, false);
        
        println!("Preprocessed terms for 'search -test -ignore': {:?}", terms);
        
        // Check that "search" is included
        let has_search = terms.iter().any(|(orig, _)| orig == "search");
        assert!(has_search, "Expected 'search' in the terms");
        
        // Check that negated terms are NOT included
        let has_test = terms.iter().any(|(orig, _)| orig == "test");
        let has_ignore = terms.iter().any(|(orig, _)| orig == "ignore");
        
        assert!(!has_test, "Did not expect 'test' in the terms as it was negated");
        assert!(!has_ignore, "Did not expect 'ignore' in the terms as it was negated");
    }
