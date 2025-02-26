use crate::ranking;

/// Preprocesses a query into original and stemmed term pairs
pub fn preprocess_query(query: &str) -> Vec<(String, String)> {
    let stop_words = ranking::stop_words();
    let stemmer = ranking::get_stemmer();

    // Split by whitespace and filter out stop words
    query
        .to_lowercase()
        .split_whitespace()
        .filter(|word| !stop_words.contains(&word.to_string()))
        .map(|word| {
            let original = word.to_string();
            let stemmed = stemmer.stem(word).to_string();
            (original, stemmed)
        })
        .filter(|(orig, _)| !orig.is_empty())
        .collect()
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
pub fn create_term_patterns(term_pairs: &[(String, String)]) -> Vec<String> {
    term_pairs
        .iter()
        .map(|(original, stemmed)| {
            if original == stemmed {
                // If stemmed and original are the same, just use one
                original.clone()
            } else {
                // Otherwise, create an OR pattern
                format!("({}|{})", regex_escape(original), regex_escape(stemmed))
            }
        })
        .collect()
}
