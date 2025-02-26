use rust_stemmers::{Algorithm, Stemmer};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

/// Returns a reference to a HashSet containing common English stop words.
pub fn stop_words() -> &'static HashSet<String> {
    static STOP_WORDS: OnceLock<HashSet<String>> = OnceLock::new();
    STOP_WORDS.get_or_init(|| {
        let words = vec![
            "a",
            "about",
            "above",
            "after",
            "again",
            "against",
            "all",
            "am",
            "an",
            "and",
            "any",
            "are",
            "aren't",
            "as",
            "at",
            "be",
            "because",
            "been",
            "before",
            "being",
            "below",
            "between",
            "both",
            "but",
            "by",
            "can't",
            "cannot",
            "could",
            "couldn't",
            "did",
            "didn't",
            "do",
            "does",
            "doesn't",
            "doing",
            "don't",
            "down",
            "during",
            "each",
            "few",
            "for",
            "from",
            "further",
            "had",
            "hadn't",
            "has",
            "hasn't",
            "have",
            "haven't",
            "having",
            "he",
            "he'd",
            "he'll",
            "he's",
            "her",
            "here",
            "here's",
            "hers",
            "herself",
            "him",
            "himself",
            "his",
            "how",
            "how's",
            "i",
            "i'd",
            "i'll",
            "i'm",
            "i've",
            "if",
            "in",
            "into",
            "is",
            "isn't",
            "it",
            "it's",
            "its",
            "itself",
            "let's",
            "me",
            "more",
            "most",
            "mustn't",
            "my",
            "myself",
            "no",
            "nor",
            "not",
            "of",
            "off",
            "on",
            "once",
            "only",
            "or",
            "other",
            "ought",
            "our",
            "ours",
            "ourselves",
            "out",
            "over",
            "own",
            "same",
            "shan't",
            "she",
            "she'd",
            "she'll",
            "she's",
            "should",
            "shouldn't",
            "so",
            "some",
            "such",
            "than",
            "that",
            "that's",
            "the",
            "their",
            "theirs",
            "them",
            "themselves",
            "then",
            "there",
            "there's",
            "these",
            "they",
            "they'd",
            "they'll",
            "they're",
            "they've",
            "this",
            "those",
            "through",
            "to",
            "too",
            "under",
            "until",
            "up",
            "very",
            "was",
            "wasn't",
            "we",
            "we'd",
            "we'll",
            "we're",
            "we've",
            "were",
            "weren't",
            "what",
            "what's",
            "when",
            "when's",
            "where",
            "where's",
            "which",
            "while",
            "who",
            "who's",
            "whom",
            "why",
            "why's",
            "with",
            "won't",
            "would",
            "wouldn't",
            "you",
            "you'd",
            "you'll",
            "you're",
            "you've",
            "your",
            "yours",
            "yourself",
            "yourselves",
            // Programming-specific stop words
            "function",
            "class",
            "method",
            "var",
            "let",
            "const",
            "import",
            "export",
            "return",
            "if",
            "else",
            "for",
            "while",
            "switch",
            "case",
            "break",
            "continue",
            "default",
            "try",
            "catch",
            "finally",
            "throw",
            "new",
            "this",
            "super",
            "extends",
            "implements",
            "interface",
            "public",
            "private",
            "protected",
            "static",
            "final",
            "abstract",
            "enum",
            "async",
            "await",
            "true",
            "false",
            "null",
            "undefined",
            "void",
            "typeof",
            "instanceof",
        ];
        words.into_iter().map(String::from).collect()
    })
}

/// Returns a reference to the English stemmer.
pub fn get_stemmer() -> &'static Stemmer {
    static STEMMER: OnceLock<Stemmer> = OnceLock::new();
    STEMMER.get_or_init(|| Stemmer::create(Algorithm::English))
}

/// Tokenizes text into lowercase words by splitting on whitespace and non-alphanumeric characters,
/// removes stop words, and applies stemming.
pub fn tokenize(text: &str) -> Vec<String> {
    let stop_words = stop_words();
    let stemmer = get_stemmer();

    // First, convert to lowercase
    let text = text.to_lowercase();

    // Split by whitespace and collect words
    let mut tokens = Vec::new();
    for word in text.split_whitespace() {
        // Further split by non-alphanumeric characters
        let mut current_token = String::new();
        for c in word.chars() {
            if c.is_alphanumeric() {
                current_token.push(c);
            } else if !current_token.is_empty() {
                // We found a non-alphanumeric character, add the current token if not empty
                tokens.push(current_token);
                current_token = String::new();
            }
        }

        // Add the last token if not empty
        if !current_token.is_empty() {
            tokens.push(current_token);
        }
    }

    // Filter out stop words and apply stemming
    tokens
        .into_iter()
        .filter(|s| !stop_words.contains(s)) // Remove stop words
        .map(|s| stemmer.stem(&s).to_string()) // Apply stemming
        .collect()
}

/// Computes term frequencies (TF) for each document, document frequencies (DF) for each term,
/// and document lengths.
pub fn compute_tf_df(
    documents: &[&str],
) -> (
    Vec<HashMap<String, usize>>,
    HashMap<String, usize>,
    Vec<usize>,
) {
    let mut tfs = Vec::with_capacity(documents.len());
    let mut dfs = HashMap::new();
    let mut lengths = Vec::with_capacity(documents.len());

    for doc in documents {
        let tokens = tokenize(doc);
        let mut tf = HashMap::new();
        // Compute term frequency for the current document
        for token in tokens.iter() {
            *tf.entry(token.clone()).or_insert(0) += 1;
        }
        // Update document frequency based on unique terms in this document
        for term in tf.keys() {
            *dfs.entry(term.clone()).or_insert(0) += 1;
        }
        tfs.push(tf);
        lengths.push(tokens.len());
    }
    (tfs, dfs, lengths)
}

/// Computes the TF-IDF IDF value for a term.
fn idf_tf(df: usize, n: usize) -> f64 {
    if df == 0 {
        0.0 // Avoid division by zero for terms not in any document
    } else {
        (n as f64 / df as f64).ln()
    }
}

/// Computes the BM25 IDF value for a term.
fn idf_bm25(df: usize, n: usize) -> f64 {
    let num = (n - df) as f64 + 0.5;
    let den = df as f64 + 0.5;
    (num / den).ln()
}

/// Computes the TF-IDF score for a document given a query.
fn tfidf_score(
    tf_d: &HashMap<String, usize>,
    query_tf: &HashMap<String, usize>,
    dfs: &HashMap<String, usize>,
    n: usize,
) -> f64 {
    let mut score = 0.0;
    for (term, &qf) in query_tf {
        if let Some(&tf) = tf_d.get(term) {
            let idf = idf_tf(*dfs.get(term).unwrap_or(&0), n);
            // TF-IDF contribution: TF(Q) * TF(D) * IDF^2
            score += (qf as f64) * (tf as f64) * idf * idf;
        }
    }
    score
}

/// Computes the BM25 score for a document given a query.
fn bm25_score(
    tf_d: &HashMap<String, usize>,
    query_tf: &HashMap<String, usize>,
    dfs: &HashMap<String, usize>,
    n: usize,
    doc_len: usize,
    avgdl: f64,
    k1: f64,
    b: f64,
) -> f64 {
    let mut score = 0.0;
    for (term, &qf) in query_tf {
        if let Some(&tf) = tf_d.get(term) {
            let idf = idf_bm25(*dfs.get(term).unwrap_or(&0), n);
            let tf_part = (tf as f64 * (k1 + 1.0))
                / (tf as f64 + k1 * (1.0 - b + b * (doc_len as f64 / avgdl)));
            // BM25 contribution: TF(Q) * IDF * TF_part
            score += (qf as f64) * idf * tf_part;
        }
    }
    score
}

/// Computes the average document length.
pub fn compute_avgdl(lengths: &[usize]) -> f64 {
    if lengths.is_empty() {
        return 0.0;
    }
    let sum: usize = lengths.iter().sum();
    sum as f64 / lengths.len() as f64
}

/// Ranks documents based on a query using a hybrid scoring approach.
/// Returns a vector of tuples containing:
/// - document index
/// - combined score
/// - TF-IDF score
/// - BM25 score
pub fn rank_documents(documents: &[&str], query: &str) -> Vec<(usize, f64, f64, f64)> {
    // Preprocess documents
    let (tfs, dfs, lengths) = compute_tf_df(documents);
    let n = documents.len();
    let avgdl = compute_avgdl(&lengths);

    // Preprocess query
    let query_tokens = tokenize(query);
    let mut query_tf = HashMap::new();
    for token in query_tokens {
        *query_tf.entry(token).or_insert(0) += 1;
    }

    // Parameters
    let k1 = 1.2; // BM25 parameter for term frequency saturation
    let b = 0.75; // BM25 parameter for length normalization
    let alpha = 0.5; // Weight for TF-IDF in hybrid score (1-alpha for BM25)

    // Compute scores for each document
    let mut scores = Vec::new();
    for (i, tf_d) in tfs.iter().enumerate() {
        let tfidf = tfidf_score(tf_d, &query_tf, &dfs, n);
        let bm25 = bm25_score(tf_d, &query_tf, &dfs, n, lengths[i], avgdl, k1, b);
        let combined = alpha * tfidf + (1.0 - alpha) * bm25;
        scores.push((i, combined, tfidf, bm25));
    }

    // Sort documents by combined score in descending order
    scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    scores
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stop_word_removal() {
        let text = "The quick brown fox jumps over the lazy dog";
        let tokens = tokenize(text);

        // "the" should be removed as it's a stop word
        assert!(!tokens.contains(&"the".to_string()));
        assert!(!tokens.contains(&"over".to_string()));

        // These words should remain (in stemmed form)
        assert!(tokens.contains(&"quick".to_string()));
        assert!(tokens.contains(&"brown".to_string()));
        assert!(tokens.contains(&"fox".to_string()));
        assert!(tokens.contains(&"jump".to_string())); // "jumps" should be stemmed to "jump"
        assert!(tokens.contains(&"lazi".to_string())); // "lazy" should be stemmed to "lazi"
        assert!(tokens.contains(&"dog".to_string()));
    }

    #[test]
    fn test_programming_stop_words() {
        let code = "function calculateTotal(items) { return items.reduce((sum, item) => sum + item.price, 0); }";
        let tokens = tokenize(code);

        println!("Tokens for programming code: {:?}", tokens);

        // Programming keywords should be removed
        assert!(!tokens.contains(&"function".to_string()));
        assert!(!tokens.contains(&"return".to_string()));

        // These words should remain (in stemmed form)
        assert!(tokens.contains(&"calculatetot".to_string())); // "calculateTotal" should be stemmed
        assert!(tokens.contains(&"item".to_string()));
        assert!(tokens.contains(&"reduc".to_string())); // "reduce" should be stemmed
        assert!(tokens.contains(&"sum".to_string()));
        assert!(tokens.contains(&"price".to_string()));
    }

    #[test]
    fn test_stemming() {
        // Test specific stemming examples
        assert_eq!(tokenize("running")[0], "run");
        assert_eq!(tokenize("jumps")[0], "jump");
        assert_eq!(tokenize("fruitlessly")[0], "fruitless");
        assert_eq!(tokenize("calculation")[0], "calcul");

        // Test that different forms of the same word stem to the same token
        let stem1 = tokenize("calculate")[0].clone();
        let stem2 = tokenize("calculating")[0].clone();
        let stem3 = tokenize("calculated")[0].clone();

        assert_eq!(stem1, stem2);
        assert_eq!(stem2, stem3);
    }
}
