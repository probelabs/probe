use crate::search::elastic_query::Expr;
use crate::search::tokenization;
use ahash::{AHashMap, AHashSet};
use rust_stemmers::{Algorithm, Stemmer};
use std::sync::OnceLock;

// Replace standard collections with ahash versions for better performance
type HashMap<K, V> = AHashMap<K, V>;
type HashSet<T> = AHashSet<T>;

/// Represents the result of term frequency and document frequency computation
pub struct TfDfResult {
    /// Term frequencies for each document
    pub term_frequencies: Vec<HashMap<String, usize>>,
    /// Document frequencies for each term
    pub document_frequencies: HashMap<String, usize>,
    /// Document lengths (number of tokens in each document)
    pub document_lengths: Vec<usize>,
}

/// Parameters for document ranking
pub struct RankingParams<'a> {
    /// Documents to rank
    pub documents: &'a [&'a str],
    /// Query string
    pub query: &'a str,
    /// Pre-tokenized content (optional)
    pub pre_tokenized: Option<&'a [Vec<String>]>,
}

/// Returns a reference to the global stemmer instance
pub fn get_stemmer() -> &'static Stemmer {
    static STEMMER: OnceLock<Stemmer> = OnceLock::new();
    STEMMER.get_or_init(|| Stemmer::create(Algorithm::English))
}

/// Tokenizes text into lowercase words by splitting on whitespace and non-alphanumeric characters,
/// removes stop words, and applies stemming. Also splits camelCase/PascalCase identifiers.
pub fn tokenize(text: &str) -> Vec<String> {
    tokenization::tokenize(text)
}

/// Preprocesses text with filename for search by tokenizing and removing duplicates
/// This is used for filename matching - it adds the filename to the tokens
pub fn preprocess_text_with_filename(text: &str, filename: &str) -> Vec<String> {
    let mut tokens = tokenize(text);
    let filename_tokens = tokenize(filename);
    tokens.extend(filename_tokens);
    tokens
}

/// Computes the average document length.
pub fn compute_avgdl(lengths: &[usize]) -> f64 {
    if lengths.is_empty() {
        return 0.0;
    }
    // Convert to f64 before summing to prevent potential integer overflow
    // when dealing with very large documents or a large number of documents
    let sum: f64 = lengths.iter().map(|&x| x as f64).sum();
    sum / lengths.len() as f64
}

// -------------------------------------------------------------------------
// BM25 EXACT (like Elasticsearch) with "bool" logic for must/should/must_not
// -------------------------------------------------------------------------

/// Parameters for BM25 calculation with precomputed IDF values
pub struct PrecomputedBm25Params<'a> {
    /// Document term frequencies
    pub doc_tf: &'a HashMap<String, usize>,
    /// Document length
    pub doc_len: usize,
    /// Average document length
    pub avgdl: f64,
    /// Precomputed IDF values for query terms
    pub idfs: &'a HashMap<String, f64>,
    /// BM25 k1 parameter
    pub k1: f64,
    /// BM25 b parameter
    pub b: f64,
}

/// Extracts unique terms from a query expression
pub fn extract_query_terms(expr: &Expr) -> HashSet<String> {
    use Expr::*;
    let mut terms = HashSet::new();

    match expr {
        Term { keywords, .. } => {
            terms.extend(keywords.iter().cloned());
        }
        And(left, right) | Or(left, right) => {
            terms.extend(extract_query_terms(left));
            terms.extend(extract_query_terms(right));
        }
    }

    terms
}

/// Precomputes IDF values for a set of terms
pub fn precompute_idfs(
    terms: &HashSet<String>,
    dfs: &HashMap<String, usize>,
    n_docs: usize,
) -> HashMap<String, f64> {
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    if debug_mode {
        println!("DEBUG: Precomputing IDF values for {} terms", terms.len());
    }

    terms
        .iter()
        .filter_map(|term| {
            let df = *dfs.get(term).unwrap_or(&0);
            if df > 0 {
                let numerator = (n_docs as f64 - df as f64) + 0.5;
                let denominator = df as f64 + 0.5;
                let idf = (1.0 + (numerator / denominator)).ln();
                Some((term.clone(), idf))
            } else {
                None
            }
        })
        .collect()
}

/// Optimized BM25 single-token function using precomputed IDF values:
/// tf_part = freq * (k1+1) / (freq + k1*(1 - b + b*(docLen/avgdl)))
fn bm25_single_token_optimized(token: &str, params: &PrecomputedBm25Params) -> f64 {
    let freq_in_doc = *params.doc_tf.get(token).unwrap_or(&0) as f64;
    if freq_in_doc <= 0.0 {
        return 0.0;
    }

    // Use precomputed IDF value
    let idf = *params.idfs.get(token).unwrap_or(&0.0);

    let tf_part = (freq_in_doc * (params.k1 + 1.0))
        / (freq_in_doc
            + params.k1 * (1.0 - params.b + params.b * (params.doc_len as f64 / params.avgdl)));

    idf * tf_part
}

/// Sum BM25 for all keywords in a single "Term" node using precomputed IDF values
fn score_term_bm25_optimized(keywords: &[String], params: &PrecomputedBm25Params) -> f64 {
    let mut total = 0.0;
    for kw in keywords {
        total += bm25_single_token_optimized(kw, params);
    }
    total
}

/// Recursively compute a doc's "ES-like BM25 bool query" score from the AST using precomputed IDF values:
/// - If it fails a must or matches a must_not => return None (exclude doc)
/// - Otherwise sum up matched subclause scores
/// - For "OR," doc must match at least one side
/// - For "AND," doc must match both sides
/// - For a "should" term, we add the BM25 if it matches; if the entire query has no must, then
///   at least one "should" must match in order to include the doc.
pub fn score_expr_bm25_optimized(expr: &Expr, params: &PrecomputedBm25Params) -> Option<f64> {
    use Expr::*;
    match expr {
        Term {
            keywords,
            required,
            excluded,
            ..
        } => {
            let score = score_term_bm25_optimized(keywords, params);

            if *excluded {
                // must_not => doc out if doc_score > 0
                if score > 0.0 {
                    None
                } else {
                    Some(0.0)
                }
            } else if *required {
                // must => doc out if doc_score=0
                if score > 0.0 {
                    Some(score)
                } else {
                    None
                }
            } else {
                // "should" => we don't exclude doc if score=0 here, because maybe it matches
                // something else in an OR. Return Some(0.0 or some positive).
                // The top-level logic ensures if no must in the entire query, we need at least one should>0.
                Some(score)
            }
        }
        And(left, right) => {
            let lscore = score_expr_bm25_optimized(left, params)?;
            let rscore = score_expr_bm25_optimized(right, params)?;
            Some(lscore + rscore)
        }
        Or(left, right) => {
            let l = score_expr_bm25_optimized(left, params);
            let r = score_expr_bm25_optimized(right, params);
            match (l, r) {
                (None, None) => None,
                (None, Some(rs)) => Some(rs),
                (Some(ls), None) => Some(ls),
                (Some(ls), Some(rs)) => Some(ls + rs),
            }
        }
    }
}

// -------------------------------------------------------------------------
// This is your main entry point for ranking. It now does "pure BM25 like ES."
// -------------------------------------------------------------------------
pub fn rank_documents(params: &RankingParams) -> Vec<(usize, f64)> {
    use rayon::prelude::*;
    use std::cmp::Ordering;

    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    // 1) Parse the user query into an AST (Expr)
    //    If your code uses parse_query(...) from `elastic_query.rs`, do:
    let parsed_expr = match crate::search::elastic_query::parse_query(params.query) {
        Ok(expr) => expr,
        Err(e) => {
            if debug_mode {
                eprintln!("DEBUG: parse_query failed: {:?}", e);
            }
            // Instead of silently returning empty results, log a warning even in non-debug mode
            // to ensure errors are visible and can be addressed
            eprintln!(
                "WARNING: Query parsing failed: {:?}. Returning empty results.",
                e
            );
            // In a future version, consider changing the return type to Result<Vec<(usize, f64)>, QueryError>
            // to properly propagate errors to the caller
            return vec![];
        }
    };

    // 2) Precompute TF/DF for docs
    let tf_df_result = if let Some(pre_tokenized) = &params.pre_tokenized {
        // Use pre-tokenized content if available
        if debug_mode {
            println!("DEBUG: Using pre-tokenized content for ranking");
        }
        compute_tf_df_from_tokenized(pre_tokenized)
    } else {
        // Fallback to tokenizing the documents
        if debug_mode {
            println!("DEBUG: Tokenizing documents for ranking");
        }
        // Tokenize documents on the fly
        let tokenized_docs: Vec<Vec<String>> =
            params.documents.iter().map(|doc| tokenize(doc)).collect();
        compute_tf_df_from_tokenized(&tokenized_docs)
    };

    let n_docs = params.documents.len();
    let avgdl = compute_avgdl(&tf_df_result.document_lengths);

    // 3) Extract query terms and precompute IDF values
    let query_terms = extract_query_terms(&parsed_expr);
    let precomputed_idfs =
        precompute_idfs(&query_terms, &tf_df_result.document_frequencies, n_docs);

    if debug_mode {
        println!(
            "DEBUG: Precomputed IDF values for {} unique query terms",
            precomputed_idfs.len()
        );
    }

    // 4) BM25 parameters
    // These values are standard defaults for BM25 as established in academic literature:
    // k1=1.2 controls term frequency saturation (higher values give more weight to term frequency)
    // b=0.75 controls document length normalization (higher values give more penalty to longer documents)
    // See: Robertson, S. E., & Zaragoza, H. (2009). The Probabilistic Relevance Framework: BM25 and Beyond
    let k1 = 1.2;
    let b = 0.75;

    if debug_mode {
        println!(
            "DEBUG: Starting parallel document scoring for {} documents",
            n_docs
        );
    }

    // 5) Compute BM25 bool logic score for each doc in parallel
    // Use a stable collection method to ensure deterministic ordering
    let scored_docs: Vec<(usize, Option<f64>)> = (0..tf_df_result.term_frequencies.len())
        .collect::<Vec<_>>() // Collect indices first to ensure stable ordering
        .par_iter() // Then parallelize
        .map(|&i| {
            let doc_tf = &tf_df_result.term_frequencies[i];
            let doc_len = tf_df_result.document_lengths[i];

            // Create optimized BM25 parameters with precomputed IDF values
            let precomputed_bm25_params = PrecomputedBm25Params {
                doc_tf,
                doc_len,
                avgdl,
                idfs: &precomputed_idfs,
                k1,
                b,
            };

            // Evaluate doc's BM25 sum or None if excluded using optimized function
            let bm25_score_opt = score_expr_bm25_optimized(&parsed_expr, &precomputed_bm25_params);

            (i, bm25_score_opt)
        })
        .collect();

    if debug_mode {
        println!("DEBUG: Parallel document scoring completed");
    }

    // Filter out documents that didn't match and collect scores
    let mut filtered_docs: Vec<(usize, f64)> = scored_docs
        .into_iter()
        .filter_map(|(i, score_opt)| score_opt.map(|score| (i, score)))
        .collect();

    // 6) Sort in descending order by BM25 score, with a stable secondary sort by document index
    filtered_docs.sort_by(|a, b| {
        // First compare by score (descending)
        // Note: unwrap_or(Ordering::Equal) handles NaN cases by treating them as equal
        // This ensures stable sorting even if a score calculation resulted in NaN
        // (which shouldn't happen with our implementation, but provides robustness)
        match b.1.partial_cmp(&a.1).unwrap_or(Ordering::Equal) {
            Ordering::Equal => {
                // If scores are equal, sort by document index (ascending) for stability
                a.0.cmp(&b.0)
            }
            other => other,
        }
    });

    if debug_mode {
        println!(
            "DEBUG: Sorted {} matching documents by score",
            filtered_docs.len()
        );
    }

    filtered_docs
}

/// Computes term frequencies (TF) for each document, document frequencies (DF) for each term,
/// and document lengths from pre-tokenized content.
pub fn compute_tf_df_from_tokenized(tokenized_docs: &[Vec<String>]) -> TfDfResult {
    use rayon::prelude::*;

    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    if debug_mode {
        println!("DEBUG: Starting parallel TF-DF computation from pre-tokenized content for {} documents", tokenized_docs.len());
    }

    // Process documents in parallel to compute term frequencies and document lengths
    let doc_results: Vec<(HashMap<String, usize>, usize, HashSet<String>)> = tokenized_docs
        .par_iter()
        .map(|tokens| {
            let mut tf = HashMap::new();

            // Compute term frequency for the current document
            for token in tokens.iter() {
                *tf.entry(token.clone()).or_insert(0) += 1;
            }

            // Collect unique terms for document frequency calculation
            let unique_terms: HashSet<String> = tf.keys().cloned().collect();

            (tf, tokens.len(), unique_terms)
        })
        .collect();

    // Extract term frequencies and document lengths
    let mut term_frequencies = Vec::with_capacity(tokenized_docs.len());
    let mut document_lengths = Vec::with_capacity(tokenized_docs.len());

    // Compute document frequencies in parallel using adaptive chunking
    // This balances parallelism with reduced contention
    // The chunk size calculation:
    // - Divides total documents by available threads to distribute work evenly
    // - Ensures at least 1 document per chunk to prevent empty chunks
    // - Larger chunks reduce thread coordination overhead but may lead to load imbalance
    // - Smaller chunks improve load balancing but increase synchronization costs
    // Use checked_div to safely handle the case where there are no threads (which shouldn't happen)
    // and ensure we always have at least one item per chunk
    let min_chunk_size = tokenized_docs
        .len()
        .checked_div(rayon::current_num_threads())
        .unwrap_or(1)
        .max(1);
    let document_frequencies = doc_results
        .par_iter()
        .with_min_len(min_chunk_size) // Adaptive chunking based on document count
        .map(|(_, _, unique_terms)| {
            // Create a local document frequency map for this chunk
            let mut local_df = HashMap::new();
            for term in unique_terms {
                *local_df.entry(term.clone()).or_insert(0) += 1;
            }
            local_df
        })
        .reduce(HashMap::new, |mut acc, local_df| {
            // Merge local document frequency maps
            for (term, count) in local_df {
                *acc.entry(term).or_insert(0) += count;
            }
            acc
        });

    if debug_mode {
        println!(
            "DEBUG: Parallel DF computation completed with {} unique terms",
            document_frequencies.len()
        );
    }

    // Collect results in a deterministic order
    for (tf, doc_len, _) in doc_results {
        term_frequencies.push(tf);
        document_lengths.push(doc_len);
    }

    if debug_mode {
        println!("DEBUG: Parallel TF-DF computation from pre-tokenized content completed");
    }

    TfDfResult {
        term_frequencies,
        document_frequencies,
        document_lengths,
    }
}

// -------------------------------------------------------------------------
// Unit tests (optional). Adapt or remove as you wish.
// -------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_bm25_scoring() {
        // A trivial test: 2 docs, 1 query
        let docs = vec!["api process load", "another random text with process"];
        let query = "+api +process +load"; // must have "api", must have "process", must have "load"

        let params = RankingParams {
            documents: &docs,
            query,
            pre_tokenized: None,
        };

        let results = rank_documents(&params);
        // Only the first doc should match, because it has all 3 required words
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 0); // doc index 0

        // Verify score is positive and within expected range for BM25
        // BM25 scores typically fall within certain ranges based on the algorithm's properties
        assert!(results[0].1 > 0.0);
        assert!(results[0].1 < 10.0); // Upper bound based on typical BM25 behavior with small documents
    }

    #[test]
    fn test_bm25_scoring_with_pre_tokenized() {
        // A trivial test: 2 docs, 1 query, with pre-tokenized content
        let docs = vec!["api process load", "another random text with process"];
        let query = "+api +process +load"; // must have "api", must have "process", must have "load"

        // Pre-tokenized content
        let pre_tokenized = vec![
            vec!["api".to_string(), "process".to_string(), "load".to_string()],
            vec![
                "another".to_string(),
                "random".to_string(),
                "text".to_string(),
                "with".to_string(),
                "process".to_string(),
            ],
        ];

        let params = RankingParams {
            documents: &docs,
            query,
            pre_tokenized: Some(&pre_tokenized),
        };

        let results = rank_documents(&params);
        // Only the first doc should match, because it has all 3 required words
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 0); // doc index 0

        // Verify score is positive and within expected range for BM25
        assert!(results[0].1 > 0.0);
        assert!(results[0].1 < 10.0); // Upper bound based on typical BM25 behavior with small documents
    }

    #[test]
    fn test_relative_bm25_scoring() {
        // Test that documents with more matching terms get higher scores
        let docs = vec![
            "api process load data", // 4 matching terms
            "api process load",      // 3 matching terms
            "api process",           // 2 matching terms
            "api",                   // 1 matching term
        ];
        let query = "api process load data"; // All terms are optional

        let params = RankingParams {
            documents: &docs,
            query,
            pre_tokenized: None,
        };

        let results = rank_documents(&params);
        // All docs should match since all terms are optional
        assert_eq!(results.len(), 4);

        // Verify that scores decrease as fewer terms match
        // Doc with 4 matches should be first, then 3, then 2, then 1
        assert_eq!(results[0].0, 0); // First doc (4 matches)
        assert_eq!(results[1].0, 1); // Second doc (3 matches)
        assert_eq!(results[2].0, 2); // Third doc (2 matches)
        assert_eq!(results[3].0, 3); // Fourth doc (1 match)

        // Verify that scores decrease as expected
        assert!(results[0].1 > results[1].1); // 4 matches > 3 matches
        assert!(results[1].1 > results[2].1); // 3 matches > 2 matches
        assert!(results[2].1 > results[3].1); // 2 matches > 1 match
    }
}
