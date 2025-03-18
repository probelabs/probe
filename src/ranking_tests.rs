use crate::ranking::{
    tokenize, get_stemmer, compute_tf_df,
    compute_avgdl, rank_documents, RankingParams
};
use crate::search::tokenization::is_stop_word;

// Helper function to adapt the old interface to the new one
fn rank_documents(documents: &[&str], query: &str) -> Vec<(usize, f64, f64, f64, f64)> {
    let params = RankingParams {
        documents,
        query,
        file_unique_terms: None,
        file_total_matches: None,
        file_match_rank: None,
        block_unique_terms: None,
        block_total_matches: None,
        node_type: None,
    };
    
    // Convert the new return type (usize, f64) to the old one (usize, f64, f64, f64, f64)
    let results = crate::ranking::rank_documents(&params);
    results.into_iter()
        .map(|(idx, bm25_score)| (idx, bm25_score, 0.0, bm25_score, bm25_score))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_stop_words() {
        // Check common English stop words
        assert!(is_stop_word("the"));
        assert!(is_stop_word("and"));
        assert!(is_stop_word("of"));
        
        // Check programming-specific stop words
        assert!(is_stop_word("function"));
        assert!(is_stop_word("class"));
        assert!(is_stop_word("return"));
    }

    #[test]
    fn test_tokenize_basic() {
        let text = "The quick brown fox jumps over the lazy dog";
        let tokens = tokenize(text);
        
        // Stop words should be removed
        assert!(!tokens.contains(&"the".to_string()));
        assert!(!tokens.contains(&"over".to_string()));
        
        // Regular words should be stemmed
        assert!(tokens.contains(&"quick".to_string()));
        assert!(tokens.contains(&"brown".to_string()));
        assert!(tokens.contains(&"fox".to_string()));
        assert!(tokens.contains(&"jump".to_string())); // "jumps" stemmed to "jump"
        assert!(tokens.contains(&"lazi".to_string()));  // "lazy" stemmed to "lazi"
        assert!(tokens.contains(&"dog".to_string()));
    }

    #[test]
    fn test_tokenize_code() {
        let code = "function calculateTotal(items) { return items.reduce((sum, item) => sum + item.price, 0); }";
        let tokens = tokenize(code);
        
        // Programming keywords should be removed as stop words
        assert!(!tokens.contains(&"function".to_string()));
        assert!(!tokens.contains(&"return".to_string()));
        
        // Variable names should be kept and stemmed
        assert!(tokens.contains(&"calculatetot".to_string())); // "calculateTotal" stemmed
        assert!(tokens.contains(&"item".to_string()));
        assert!(tokens.contains(&"reduc".to_string())); // "reduce" stemmed
        assert!(tokens.contains(&"sum".to_string()));
        assert!(tokens.contains(&"price".to_string()));
    }

    #[test]
    fn test_tokenize_with_punctuation() {
        let text = "This, is a test. With multiple punctuation marks! And some numbers: 123, 456.";
        let tokens = tokenize(text);
        
        // Stop words should be removed
        assert!(!tokens.contains(&"a".to_string()));
        assert!(!tokens.contains(&"is".to_string()));
        assert!(!tokens.contains(&"and".to_string()));
        assert!(!tokens.contains(&"some".to_string()));
        assert!(!tokens.contains(&"with".to_string()));
        
        // Regular words should be tokenized without punctuation
        assert!(tokens.contains(&"test".to_string()));
        assert!(tokens.contains(&"multipl".to_string()));  // "multiple" stemmed
        assert!(tokens.contains(&"punctuat".to_string())); // "punctuation" stemmed
        assert!(tokens.contains(&"mark".to_string()));     // "marks" stemmed
        assert!(tokens.contains(&"number".to_string()));   // "numbers" stemmed
        
        // Numbers should be kept
        assert!(tokens.contains(&"123".to_string()));
        assert!(tokens.contains(&"456".to_string()));
    }

    #[test]
    fn test_stemming_consistency() {
        let stemmer = get_stemmer();
        
        // Test pairs of words that should stem to the same token
        let pairs = vec![
            ("run", "running"),
            ("code", "coding"),
            ("search", "searching"),
            ("function", "functions"),
            ("calculate", "calculation"),
        ];
        
        for (word1, word2) in pairs {
            let stem1 = stemmer.stem(word1).to_string();
            let stem2 = stemmer.stem(word2).to_string();
            assert_eq!(stem1, stem2, "{} and {} should stem to the same token", word1, word2);
        }
    }

    #[test]
    fn test_compute_tf_df() {
        let documents = vec![
            "the quick brown fox",
            "the quick brown",
            "the fox jumps",
        ];
        
        let (tfs, dfs, lengths) = compute_tf_df(&documents);
        
        // Check document lengths (after stop word removal)
        assert_eq!(lengths[0], 3); // quick, brown, fox
        assert_eq!(lengths[1], 2); // quick, brown
        assert_eq!(lengths[2], 2); // fox, jump
        
        // Check term frequencies
        assert_eq!(tfs[0].get("fox"), Some(&1));
        assert_eq!(tfs[0].get("quick"), Some(&1));
        assert_eq!(tfs[0].get("brown"), Some(&1));
        
        // Check document frequencies
        assert_eq!(dfs.get("fox"), Some(&2)); // in docs 0 and 2
        assert_eq!(dfs.get("quick"), Some(&2)); // in docs 0 and 1
        assert_eq!(dfs.get("brown"), Some(&2)); // in docs 0 and 1
        assert_eq!(dfs.get("jump"), Some(&1)); // only in doc 2
    }

    #[test]
    fn test_compute_avgdl() {
        let lengths = vec![5, 10, 15];
        let avgdl = compute_avgdl(&lengths);
        
        assert_eq!(avgdl, 10.0); // (5 + 10 + 15) / 3 = 10
        
        // Test with empty vector
        let empty_lengths: Vec<usize> = vec![];
        let avgdl_empty = compute_avgdl(&empty_lengths);
        
        assert_eq!(avgdl_empty, 0.0);
    }

    #[test]
    fn test_rank_documents() {
        let documents = vec![
            "the quick brown fox",  // doc 0
            "the quick brown",      // doc 1
            "the fox jumps",        // doc 2
            "fox fox fox",          // doc 3 - lots of "fox"
        ];
        
        // Query for "fox"
        let ranked_docs = rank_documents(&documents, "fox");
        
        // doc 3 should be highest ranked (most occurrences of "fox")
        assert_eq!(ranked_docs[0].0, 3);
        
        // docs 0 and 2 should be included (they contain "fox")
        let doc_indices: Vec<usize> = ranked_docs.iter().map(|(idx, _, _, _, _)| *idx).collect();
        assert!(doc_indices.contains(&0));
        assert!(doc_indices.contains(&2));
        
        // doc 1 should be lowest ranked or not included (no "fox")
        assert!(ranked_docs.last().unwrap().0 == 1 || !doc_indices.contains(&1));
    }

    #[test]
    fn test_rank_documents_multi_term() {
        let documents = vec![
            "search function implementation", // doc 0
            "search algorithm",              // doc 1
            "function declaration",          // doc 2
            "unrelated document",            // doc 3
        ];
        
        // Query for "search function"
        let ranked_docs = rank_documents(&documents, "search function");
        
        // doc 0 should be highest ranked (contains both terms)
        assert_eq!(ranked_docs[0].0, 0);
        
        // docs 1 and 2 should be included (they contain one of the terms)
        let doc_indices: Vec<usize> = ranked_docs.iter().map(|(idx, _, _, _, _)| *idx).collect();
        assert!(doc_indices.contains(&1));
        assert!(doc_indices.contains(&2));
        
        // doc 3 should be lowest ranked (no matching terms)
        assert_eq!(ranked_docs.last().unwrap().0, 3);
    }

    #[test]
    fn test_rank_documents_stemming() {
        let documents = vec![
            "searching functions",           // doc 0
            "search implementation",         // doc 1
            "functional programming",        // doc 2
        ];
        
        // Query with words that should match via stemming
        let ranked_docs = rank_documents(&documents, "search function");
        
        // Both doc 0 and doc 2 contain stemmed versions of the query terms
        let top_docs: Vec<usize> = ranked_docs.iter()
            .take(2)
            .map(|(idx, _, _, _, _)| *idx)
            .collect();
        
        assert!(top_docs.contains(&0)); // "searching" -> "search", "functions" -> "function"
        assert!(top_docs.contains(&2)); // "functional" -> "function"
    }
}