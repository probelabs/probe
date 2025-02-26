use crate::ranking;

/// Preprocesses a query into original and stemmed term pairs
/// When exact is true, splits only on whitespace and skips stemming/stopword removal
/// When exact is false, uses stemming/stopword logic but splits primarily on whitespace
pub fn preprocess_query(query: &str, exact: bool) -> Vec<(String, String)> {
    // Convert to lowercase first
    let lowercase_query = query.to_lowercase();
    
    // Split by whitespace to preserve multi-word structure
    let words: Vec<&str> = lowercase_query
        .split_whitespace()
        .collect();
    
    if exact {
        // For exact matching, just return the words as-is without stemming
        words
            .into_iter()
            .filter(|word| !word.is_empty())
            .map(|word| {
                let original = word.to_string();
                (original.clone(), original)
            })
            .collect()
    } else {
        // For non-exact matching, apply stemming and stopword removal
        let stop_words = ranking::stop_words();
        let stemmer = ranking::get_stemmer();
        
        words
            .into_iter()
            .filter(|word| !word.is_empty() && !stop_words.contains(&word.to_string()))
            .map(|word| {
                let original = word.to_string();
                let stemmed = stemmer.stem(word).to_string();
                (original, stemmed)
            })
            .collect()
    }
}

/// Escapes special regex characters in a string
pub fn regex_escape(s: &str) -> String {
    let special_chars = [
        '.', '^', '$', '*', '+', '?', '(', ')', '[', ']', '{', '}', '|', '\\',
    ];
    let mut result = String::with_capacity(s.len() * 2);

    for c in s.chars() {
        if special_chars.contains(&c) {
            result.push('\\');
        }
        result.push(c);
    }

    result
}

/// Creates regex patterns that match either original or stemmed terms
/// Adds word boundaries to ensure each term is matched as a whole word
pub fn create_term_patterns(term_pairs: &[(String, String)]) -> Vec<String> {
    term_pairs
        .iter()
        .map(|(original, stemmed)| {
            let escaped_orig = regex_escape(original);
            if original == stemmed {
                // If stemmed and original are the same, just use one with word boundaries
                format!("\\b{}\\b", escaped_orig)
            } else {
                // Otherwise, create an OR pattern with word boundaries
                let escaped_stem = regex_escape(stemmed);
                format!("\\b({}|{})\\b", escaped_orig, escaped_stem)
            }
        })
        .collect()
}
