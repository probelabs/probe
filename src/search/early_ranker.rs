use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Structure to hold file match information for early ranking
#[derive(Debug, Clone)]
pub struct FileMatchInfo {
    pub path: PathBuf,
    pub content_matches: HashMap<usize, Vec<usize>>, // term_index -> line numbers
    pub filename_matched_terms: HashSet<usize>,      // term indices that matched in filename
    pub estimated_lines: usize,                      // Estimated file size in lines
}

/// Structure to hold early ranking results
#[derive(Debug, Clone)]
pub struct EarlyRankResult {
    pub path: PathBuf,
    pub score: f64,
    pub match_info: FileMatchInfo,
}

/// Calculate IDF (Inverse Document Frequency) for early ranking
fn calculate_idf(total_files: usize, files_with_term: usize) -> f64 {
    ((total_files as f64 + 1.0) / (files_with_term as f64 + 1.0)).ln()
}

/// Calculate early BM25-like score using line match information
pub fn calculate_early_score(
    file_info: &FileMatchInfo,
    query_terms: &[String],
    document_frequencies: &HashMap<usize, usize>, // term_index -> number of files containing term
    total_files: usize,
    avg_doc_length: f64,
) -> f64 {
    // BM25 parameters
    let k1 = 1.2;
    let b = 0.75;

    // Document length normalization
    let doc_length = file_info.estimated_lines as f64;
    let length_norm = 1.0 - b + b * (doc_length / avg_doc_length);

    let mut score = 0.0;

    // Score content matches
    for (term_idx, line_matches) in &file_info.content_matches {
        // Use line count as proxy for term frequency
        let tf = line_matches.len() as f64;

        // Calculate IDF
        let df = document_frequencies.get(term_idx).copied().unwrap_or(1);
        let idf = calculate_idf(total_files, df);

        // BM25 formula for this term
        let term_score = idf * ((tf * (k1 + 1.0)) / (tf + k1 * length_norm));
        score += term_score;
    }

    // Boost for filename matches (significant signal of relevance)
    let filename_boost = 2.0;
    for term_idx in &file_info.filename_matched_terms {
        let df = document_frequencies.get(term_idx).copied().unwrap_or(1);
        let idf = calculate_idf(total_files, df);
        score += idf * filename_boost;
    }

    // Coverage boost - files matching more query terms are likely more relevant
    let matched_terms = file_info.content_matches.len() + file_info.filename_matched_terms.len();
    let coverage = matched_terms as f64 / query_terms.len() as f64;
    score *= 1.0 + (coverage * 0.5);

    score
}

/// Calculate filename matching score
pub fn get_filename_matches(
    path: &Path,
    _query_terms: &[String],
    term_indices: &HashMap<String, usize>,
) -> HashSet<usize> {
    let mut matched_indices = HashSet::new();

    if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
        let filename_lower = filename.to_lowercase();

        for (term, &idx) in term_indices {
            let term_lower = term.to_lowercase();
            if filename_lower.contains(&term_lower) {
                matched_indices.insert(idx);
            }
        }
    }

    matched_indices
}

/// Build document frequency map from all file matches
pub fn build_document_frequencies(
    all_matches: &[(PathBuf, HashMap<usize, Vec<usize>>)],
) -> HashMap<usize, usize> {
    let mut doc_frequencies = HashMap::new();

    for (_path, term_matches) in all_matches {
        for &term_idx in term_matches.keys() {
            *doc_frequencies.entry(term_idx).or_insert(0) += 1;
        }
    }

    doc_frequencies
}

/// Perform early ranking on all matched files
pub fn rank_files_early(
    file_matches: Vec<(PathBuf, HashMap<usize, Vec<usize>>)>,
    query_terms: &[String],
    term_indices: &HashMap<String, usize>,
    file_sizes: &HashMap<PathBuf, usize>, // Pre-computed file sizes (in lines)
) -> Vec<EarlyRankResult> {
    let total_files = file_matches.len();
    if total_files == 0 {
        return Vec::new();
    }

    // Build document frequencies
    let doc_frequencies = build_document_frequencies(&file_matches);

    // Calculate average document length
    let total_lines: usize = file_sizes.values().sum();
    let avg_doc_length = total_lines as f64 / total_files as f64;

    // Calculate early scores for each file
    let mut results = Vec::with_capacity(file_matches.len());

    for (path, content_matches) in file_matches {
        // Get filename matches
        let filename_matched_terms = get_filename_matches(&path, query_terms, term_indices);

        // Get estimated file size
        let estimated_lines = file_sizes.get(&path).copied().unwrap_or(100);

        let file_info = FileMatchInfo {
            path: path.clone(),
            content_matches,
            filename_matched_terms,
            estimated_lines,
        };

        let score = calculate_early_score(
            &file_info,
            query_terms,
            &doc_frequencies,
            total_files,
            avg_doc_length,
        );

        results.push(EarlyRankResult {
            path,
            score,
            match_info: file_info,
        });
    }

    // Sort by score (descending)
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    results
}

/// Estimate how many files we need to process to satisfy limits
pub fn estimate_files_needed(
    max_results: Option<usize>,
    max_tokens: Option<usize>,
    avg_tokens_per_result: usize,
) -> usize {
    let result_limit = max_results.unwrap_or(1000);

    if let Some(token_limit) = max_tokens {
        // Estimate files needed based on token limit
        // Assume each file produces ~3 results on average
        // Add 100% buffer for safety
        let results_for_tokens = token_limit / avg_tokens_per_result;
        let files_for_tokens = (results_for_tokens / 3).max(1) * 2;

        // Take minimum of result-based and token-based estimates
        result_limit.min(files_for_tokens)
    } else {
        // If no token limit, just need enough files for result limit
        // Assume 3 results per file on average
        (result_limit / 3).max(10)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_early_scoring() {
        let mut file_info = FileMatchInfo {
            path: PathBuf::from("src/auth_controller.rs"),
            content_matches: HashMap::new(),
            filename_matched_terms: HashSet::new(),
            estimated_lines: 200,
        };

        // Add some content matches
        file_info.content_matches.insert(0, vec![10, 20, 30]); // "auth" on 3 lines
        file_info.content_matches.insert(1, vec![15, 25]); // "controller" on 2 lines

        // Add filename matches
        file_info.filename_matched_terms.insert(0); // "auth" in filename
        file_info.filename_matched_terms.insert(1); // "controller" in filename

        let query_terms = vec![
            "auth".to_string(),
            "controller".to_string(),
            "user".to_string(),
        ];
        let mut doc_frequencies = HashMap::new();
        doc_frequencies.insert(0, 10); // "auth" in 10 files
        doc_frequencies.insert(1, 5); // "controller" in 5 files
        doc_frequencies.insert(2, 20); // "user" in 20 files

        let score = calculate_early_score(
            &file_info,
            &query_terms,
            &doc_frequencies,
            100,   // total files
            150.0, // avg doc length
        );

        assert!(score > 0.0);
        // Should have high score due to filename matches and good coverage
        assert!(score > 5.0);
    }
}
