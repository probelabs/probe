use rust_stemmers::{Algorithm, Stemmer};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

/// Represents the result of term frequency and document frequency computation
pub struct TfDfResult {
    /// Term frequencies for each document
    pub term_frequencies: Vec<HashMap<String, usize>>,
    /// Document frequencies for each term
    pub document_frequencies: HashMap<String, usize>,
    /// Document lengths (number of tokens in each document)
    pub document_lengths: Vec<usize>,
}

/// Parameters for BM25 scoring algorithm
pub struct Bm25Params {
    /// Document term frequencies
    pub tf_d: HashMap<String, usize>,
    /// Query term frequencies
    pub query_tf: HashMap<String, usize>,
    /// Document frequencies for each term
    pub dfs: HashMap<String, usize>,
    /// Total number of documents
    pub n: usize,
    /// Length of the current document
    pub doc_len: usize,
    /// Average document length
    pub avgdl: f64,
    /// Term frequency saturation parameter
    pub k1: f64,
    /// Length normalization parameter
    pub b: f64,
}

/// Parameters for document ranking
pub struct RankingParams<'a> {
    /// Documents to rank
    pub documents: &'a [&'a str],
    /// Query string
    pub query: &'a str,
    /// Number of unique terms in the file
    pub file_unique_terms: Option<usize>,
    /// Total number of matches in the file
    pub file_total_matches: Option<usize>,
    /// Rank of the file in the match list
    pub file_match_rank: Option<usize>,
    /// Number of unique terms in the block
    pub block_unique_terms: Option<usize>,
    /// Total number of matches in the block
    pub block_total_matches: Option<usize>,
    /// Type of the node (e.g., "method_declaration")
    pub node_type: Option<&'a str>,
}

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

/// Splits a camelCase or PascalCase string into individual words
fn split_camel_case(s: &str) -> Vec<String> {
    if s.is_empty() {
        return Vec::new();
    }

    // println!("Splitting: {}", s);

    let mut result = Vec::new();
    let mut current = String::new();
    let mut prev_is_lower = false;
    let mut prev_is_numeric = false;

    for c in s.chars() {
        let is_numeric = c.is_numeric();
        // println!("  Char {}: '{}', prev_is_lower: {}, prev_is_numeric: {}", i, c, prev_is_lower, prev_is_numeric);

        if c.is_uppercase() && prev_is_lower {
            // Transition from lowercase to uppercase indicates a new word
            if !current.is_empty() {
                // println!("  -> Word break (lowercase->uppercase): '{}'", current);
                result.push(current.to_lowercase());
                current = String::new();
            }
        } else if is_numeric && !current.is_empty() && !prev_is_numeric {
            // Transition from alpha to numeric indicates a new word
            // println!("  -> Word break (alpha->numeric): '{}'", current);
            result.push(current.to_lowercase());
            current = String::new();
        } else if !is_numeric && prev_is_numeric {
            // Transition from numeric to alpha indicates a new word
            if !current.is_empty() {
                // println!("  -> Word break (numeric->alpha): '{}'", current);
                result.push(current.to_lowercase());
                current = String::new();
            }
        }

        current.push(c);
        prev_is_lower = c.is_lowercase();
        prev_is_numeric = is_numeric;
    }

    // Add the last word
    if !current.is_empty() {
        // println!("  -> Final word: '{}'", current);
        result.push(current.to_lowercase());
    }

    // println!("Split result: {:?}", result);
    result
}

/// Tokenizes text into lowercase words by splitting on whitespace and non-alphanumeric characters,
/// removes stop words, and applies stemming. Also splits camelCase/PascalCase identifiers.
pub fn tokenize(text: &str) -> Vec<String> {
    let stop_words = stop_words();
    let stemmer = get_stemmer();

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

    // Create a set to track unique tokens after processing
    let mut processed_tokens = HashSet::new();
    let mut result = Vec::new();

    // Process each token: filter stop words, apply stemming, and add to result if unique
    for token in tokens {
        // First, split camelCase/PascalCase identifiers
        let mut parts = vec![token.to_lowercase()]; // Include the original token (lowercase)

        // Only try to split if the original token has mixed case
        if token.chars().any(|c| c.is_uppercase()) {
            parts.extend(split_camel_case(&token).into_iter());
        }

        // Process each part (original token and split parts)
        for part in parts {
            // Skip stop words
            if stop_words.contains(&part) {
                continue;
            }

            // Add the stemmed part if it's unique
            let stemmed_part = stemmer.stem(&part).to_string();
            if processed_tokens.insert(stemmed_part.clone()) {
                result.push(stemmed_part);
            }
        }
    }

    result
}

/// Preprocesses text for search operations
/// When exact is true, splits only on whitespace and skips stemming/stopword removal
/// When exact is false, uses tokenize which includes stemming, stopword removal, and camelCase splitting
pub fn preprocess_text(text: &str, exact: bool) -> Vec<String> {
    if exact {
        text.to_lowercase()
            .split_whitespace()
            .map(String::from)
            .collect()
    } else {
        tokenize(text)
    }
}

/// Computes term frequencies (TF) for each document, document frequencies (DF) for each term,
/// and document lengths.
pub fn compute_tf_df(documents: &[&str]) -> TfDfResult {
    let mut term_frequencies = Vec::with_capacity(documents.len());
    let mut document_frequencies = HashMap::new();
    let mut document_lengths = Vec::with_capacity(documents.len());

    for doc in documents {
        let tokens = tokenize(doc);
        let mut tf = HashMap::new();
        // Compute term frequency for the current document
        for token in tokens.iter() {
            *tf.entry(token.clone()).or_insert(0) += 1;
        }
        // Update document frequency based on unique terms in this document
        for term in tf.keys() {
            *document_frequencies.entry(term.clone()).or_insert(0) += 1;
        }
        term_frequencies.push(tf);
        document_lengths.push(tokens.len());
    }

    TfDfResult {
        term_frequencies,
        document_frequencies,
        document_lengths,
    }
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
fn bm25_score(params: &Bm25Params) -> f64 {
    let mut score = 0.0;
    for (term, &qf) in &params.query_tf {
        if let Some(&tf) = params.tf_d.get(term) {
            let idf = idf_bm25(*params.dfs.get(term).unwrap_or(&0), params.n);
            let tf_part = (tf as f64 * (params.k1 + 1.0))
                / (tf as f64
                    + params.k1
                        * (1.0 - params.b + params.b * (params.doc_len as f64 / params.avgdl)));
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
// Helper function to calculate mean
fn calculate_mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

// Helper function to calculate standard deviation
fn calculate_std_dev(values: &[f64], mean: f64) -> f64 {
    if values.len() <= 1 {
        return 0.0;
    }
    let variance =
        values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (values.len() - 1) as f64;
    variance.sqrt()
}

// Helper function to calculate z-score
fn zscore(value: f64, mean: f64, std_dev: f64) -> f64 {
    if std_dev == 0.0 {
        0.0
    } else {
        (value - mean) / std_dev
    }
}

// Helper function to invert rank (higher score for lower rank)
fn invert_rank(rank: usize, total: usize) -> f64 {
    if total <= 1 {
        1.0
    } else {
        1.0 - (rank - 1) as f64 / (total - 1) as f64
    }
}

/// Returns a vector of tuples containing:
/// - document index
/// - combined score
/// - TF-IDF score
/// - BM25 score
/// - new score (incorporating file and block metrics)
pub fn rank_documents(params: &RankingParams) -> Vec<(usize, f64, f64, f64, f64)> {
    // Preprocess documents
    let tf_df_result = compute_tf_df(params.documents);
    let n = params.documents.len();
    let avgdl = compute_avgdl(&tf_df_result.document_lengths);

    // Preprocess query
    let query_tokens = tokenize(params.query);
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
    for (i, tf_d) in tf_df_result.term_frequencies.iter().enumerate() {
        let tfidf = tfidf_score(tf_d, &query_tf, &tf_df_result.document_frequencies, n);
        let bm25_params = Bm25Params {
            tf_d: tf_d.clone(),
            query_tf: query_tf.clone(),
            dfs: tf_df_result.document_frequencies.clone(),
            n,
            doc_len: tf_df_result.document_lengths[i],
            avgdl,
            k1,
            b,
        };
        let bm25 = bm25_score(&bm25_params);
        let combined = alpha * tfidf + (1.0 - alpha) * bm25;
        scores.push((i, combined, tfidf, bm25));
    }

    // Extract all scores for normalization
    let combined_scores: Vec<f64> = scores.iter().map(|(_, c, _, _)| *c).collect();
    let tfidf_scores: Vec<f64> = scores.iter().map(|(_, _, t, _)| *t).collect();
    let bm25_scores: Vec<f64> = scores.iter().map(|(_, _, _, b)| *b).collect();
    let n = scores.len();

    // Calculate means and standard deviations
    let mean_cs = calculate_mean(&combined_scores);
    let std_cs = calculate_std_dev(&combined_scores, mean_cs);
    let mean_tf = calculate_mean(&tfidf_scores);
    let std_tf = calculate_std_dev(&tfidf_scores, mean_tf);
    let mean_bm = calculate_mean(&bm25_scores);
    let std_bm = calculate_std_dev(&bm25_scores, mean_bm);

    // Create rankings for TF-IDF and BM25
    let mut tfidf_ranks: Vec<(usize, f64)> = scores
        .iter()
        .enumerate()
        .map(|(i, (_, _, t, _))| (i, *t))
        .collect();
    let mut bm25_ranks: Vec<(usize, f64)> = scores
        .iter()
        .enumerate()
        .map(|(i, (_, _, _, b))| (i, *b))
        .collect();

    // Sort to determine ranks
    tfidf_ranks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    bm25_ranks.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Create rank lookup maps
    let mut tfidf_rank_map = HashMap::new();
    let mut bm25_rank_map = HashMap::new();
    for (rank, (idx, _)) in tfidf_ranks.iter().enumerate() {
        tfidf_rank_map.insert(*idx, rank + 1);
    }
    for (rank, (idx, _)) in bm25_ranks.iter().enumerate() {
        bm25_rank_map.insert(*idx, rank + 1);
    }

    // Calculate new scores
    let mut new_scores = Vec::new();
    for (i, (_, combined, tfidf, bm25)) in scores.iter().enumerate() {
        // Normalize scores
        let cs_norm = zscore(*combined, mean_cs, std_cs);
        let tf_norm = zscore(*tfidf, mean_tf, std_tf);
        let bm_norm = zscore(*bm25, mean_bm, std_bm);

        // Get ranks and invert them
        let tr_score = invert_rank(*tfidf_rank_map.get(&i).unwrap_or(&n), n);
        let br_score = invert_rank(*bm25_rank_map.get(&i).unwrap_or(&n), n);
        let fmr_score = invert_rank(params.file_match_rank.unwrap_or(n), n);

        // Convert metrics to f64 and collect for normalization
        let fut = params.file_unique_terms.map(|x| x as f64).unwrap_or(0.0);
        let ftm = params.file_total_matches.map(|x| x as f64).unwrap_or(0.0);
        let but = params.block_unique_terms.map(|x| x as f64).unwrap_or(0.0);
        let btm = params.block_total_matches.map(|x| x as f64).unwrap_or(0.0);

        // Calculate means and standard deviations for metrics
        let mean_fut = calculate_mean(&[fut]);
        let std_fut = calculate_std_dev(&[fut], mean_fut);
        let mean_ftm = calculate_mean(&[ftm]);
        let std_ftm = calculate_std_dev(&[ftm], mean_ftm);
        let mean_but = calculate_mean(&[but]);
        let std_but = calculate_std_dev(&[but], mean_but);
        let mean_btm = calculate_mean(&[btm]);
        let std_btm = calculate_std_dev(&[btm], mean_btm);

        // Normalize metrics using z-scores
        let fut_norm = zscore(fut, mean_fut, std_fut);
        let ftm_norm = zscore(ftm, mean_ftm, std_ftm);
        let but_norm = zscore(but, mean_but, std_but);
        let btm_norm = zscore(btm, mean_btm, std_btm);

        // Type bonus
        let type_bonus = match params.node_type {
            Some("method_declaration") => 0.05,
            Some("function_declaration") => 0.03,
            _ => 0.0,
        };

        // Calculate final score with weights
        let new_score = 0.20 * cs_norm
            + 0.10 * tf_norm
            + 0.10 * bm_norm
            + 0.05 * fut_norm
            + 0.05 * ftm_norm
            + 0.20 * but_norm
            + 0.15 * btm_norm
            + 0.05 * tr_score
            + 0.05 * br_score
            + 0.05 * fmr_score
            + type_bonus;

        println!(
            "Score components for doc {}: cs_norm={:.3}, tf_norm={:.3}, bm_norm={:.3}, fut_norm={:.3}, ftm_norm={:.3}, but_norm={:.3}, btm_norm={:.3}, tr_score={:.3}, br_score={:.3}, fmr_score={:.3}, type_bonus={:.3} => new_score={:.3}",
            i, cs_norm, tf_norm, bm_norm, fut_norm, ftm_norm, but_norm, btm_norm, tr_score, br_score, fmr_score, type_bonus, new_score
        );

        new_scores.push((i, *combined, *tfidf, *bm25, new_score));
    }

    // Sort by new score in descending order
    new_scores.sort_by(|a, b| b.4.partial_cmp(&a.4).unwrap_or(std::cmp::Ordering::Equal));

    new_scores
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

    #[test]
    fn test_camel_case_splitting() {
        // Test camelCase
        let tokens = tokenize("enableIpWhiteListing");
        println!("Tokens for 'enableIpWhiteListing': {:?}", tokens);

        // These should be present (in stemmed form)
        assert!(tokens.contains(&"enabl".to_string()));
        assert!(tokens.contains(&"ip".to_string()));
        assert!(tokens.contains(&"white".to_string()));
        assert!(tokens.contains(&"list".to_string()));

        // Test PascalCase
        let tokens = tokenize("EnableIpWhiteListing");
        println!("Tokens for 'EnableIpWhiteListing': {:?}", tokens);

        assert!(tokens.contains(&"enabl".to_string()));
        assert!(tokens.contains(&"ip".to_string()));
        assert!(tokens.contains(&"white".to_string()));
        assert!(tokens.contains(&"list".to_string()));

        // Test with numbers
        let tokens = tokenize("IPv4Address");
        println!("Tokens for 'IPv4Address': {:?}", tokens);

        assert!(tokens.contains(&"ipv".to_string()));
        assert!(tokens.contains(&"4".to_string()));
        assert!(tokens.contains(&"address".to_string()));
    }
}
