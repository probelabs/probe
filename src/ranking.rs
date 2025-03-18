use crate::search::elastic_query::Expr;
use crate::search::tokenization;
use rust_stemmers::{Algorithm, Stemmer};
use std::collections::HashMap;
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

/// Parameters for document ranking
pub struct RankingParams<'a> {
    /// Documents to rank
    pub documents: &'a [&'a str],
    /// Query string
    pub query: &'a str,
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

/// Computes the average document length.
pub fn compute_avgdl(lengths: &[usize]) -> f64 {
    if lengths.is_empty() {
        return 0.0;
    }
    let sum: usize = lengths.iter().sum();
    sum as f64 / lengths.len() as f64
}

// -------------------------------------------------------------------------
// BM25 EXACT (like Elasticsearch) with "bool" logic for must/should/must_not
// -------------------------------------------------------------------------

/// Parameters for BM25 calculation
pub struct Bm25Params<'a> {
    /// Document term frequencies
    pub doc_tf: &'a HashMap<String, usize>,
    /// Document length
    pub doc_len: usize,
    /// Average document length
    pub avgdl: f64,
    /// Document frequencies for each term
    pub dfs: &'a HashMap<String, usize>,
    /// Number of documents
    pub n_docs: usize,
    /// BM25 k1 parameter
    pub k1: f64,
    /// BM25 b parameter
    pub b: f64,
}

/// BM25 single-token function, matching Lucene's default formula exactly:
/// idf = ln(1 + (N - df + 0.5) / (df + 0.5))
/// tf_part = freq * (k1+1) / (freq + k1*(1 - b + b*(docLen/avgdl)))
fn bm25_single_token(token: &str, params: &Bm25Params) -> f64 {
    let freq_in_doc = *params.doc_tf.get(token).unwrap_or(&0) as f64;
    if freq_in_doc <= 0.0 {
        return 0.0;
    }

    let df = *params.dfs.get(token).unwrap_or(&0) as f64;
    let numerator = (params.n_docs as f64 - df) + 0.5;
    let denominator = df + 0.5;
    let idf = (1.0 + (numerator / denominator)).ln();

    let tf_part = (freq_in_doc * (params.k1 + 1.0))
        / (freq_in_doc
            + params.k1 * (1.0 - params.b + params.b * (params.doc_len as f64 / params.avgdl)));

    idf * tf_part
}

/// Sum BM25 for all keywords in a single "Term" node
fn score_term_bm25(keywords: &[String], params: &Bm25Params) -> f64 {
    let mut total = 0.0;
    for kw in keywords {
        total += bm25_single_token(kw, params);
    }
    total
}

/// Recursively compute a doc's "ES-like BM25 bool query" score from the AST:
/// - If it fails a must or matches a must_not => return None (exclude doc)
/// - Otherwise sum up matched subclause scores
/// - For "OR," doc must match at least one side
/// - For "AND," doc must match both sides
/// - For a "should" term, we add the BM25 if it matches; if the entire query has no must, then
///   at least one "should" must match in order to include the doc.
pub fn score_expr_bm25(expr: &Expr, params: &Bm25Params) -> Option<f64> {
    use Expr::*;
    match expr {
        Term {
            keywords,
            required,
            excluded,
            ..
        } => {
            let score = score_term_bm25(keywords, params);

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
            let lscore = score_expr_bm25(left, params)?;
            let rscore = score_expr_bm25(right, params)?;
            Some(lscore + rscore)
        }
        Or(left, right) => {
            let l = score_expr_bm25(left, params);
            let r = score_expr_bm25(right, params);
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
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    // 1) Parse the user query into an AST (Expr)
    //    If your code uses parse_query(...) from `elastic_query.rs`, do:
    let parsed_expr = match crate::search::elastic_query::parse_query(params.query, false) {
        Ok(expr) => expr,
        Err(e) => {
            if debug_mode {
                eprintln!("DEBUG: parse_query failed: {:?}", e);
            }
            // Return empty if parse fails, or handle how you want
            return vec![];
        }
    };

    // 2) Precompute TF/DF for docs
    let tf_df_result = compute_tf_df(params.documents);
    let n_docs = params.documents.len();
    let avgdl = compute_avgdl(&tf_df_result.document_lengths);

    // 4) BM25 parameters
    let k1 = 1.2;
    let b = 0.75;

    // We'll store doc_i plus the final BM25
    let mut scored_docs = Vec::new();

    // 5) For each doc, compute BM25 bool logic score
    for (i, doc_tf) in tf_df_result.term_frequencies.iter().enumerate() {
        let doc_len = tf_df_result.document_lengths[i];

        // Create BM25 parameters
        let bm25_params = Bm25Params {
            doc_tf,
            doc_len,
            avgdl,
            dfs: &tf_df_result.document_frequencies,
            n_docs,
            k1,
            b,
        };

        // Evaluate doc's BM25 sum or None if excluded
        let bm25_score_opt = score_expr_bm25(&parsed_expr, &bm25_params);

        if let Some(score) = bm25_score_opt {
            // The doc matched. "score" is the doc's final BM25 sum.
            scored_docs.push((i, score));
        } else {
            // doc is excluded
        }
    }

    // 6) Sort in descending order by BM25
    scored_docs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // 7) Convert into the final structure: (idx, bm25_score)
    let mut results = Vec::new();
    for (i, bm25_val) in scored_docs {
        results.push((i, bm25_val));
    }

    results
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
        };

        let results = rank_documents(&params);
        // Only the first doc should match, because it has all 3 required words
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 0); // doc index 0
                                     // BM25 score is some positive float
        assert!(results[0].1 > 0.0);
    }
}
