use crate::ranking;

/// Preprocesses a query into original and stemmed term pairs
pub fn preprocess_query(query: &str) -> Vec<(String, String)> {
    let stop_words = ranking::stop_words();
    let stemmer = ranking::get_stemmer();

    // Convert to lowercase first
    let lowercase_query = query.to_lowercase();
    
    // Split by non-alphanumeric characters
    let words: Vec<&str> = lowercase_query
        .split(|c: char| !c.is_alphanumeric())
        .collect();

    // Filter out stop words and empty strings
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
