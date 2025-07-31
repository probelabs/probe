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
///
/// This function adds buffer/overlap to ensure we process enough files to meet or slightly
/// exceed the requested token/result limits, rather than falling short due to estimation errors.
///
/// The estimation strategy:
/// 1. For token limits: Apply 1.5x buffer to account for estimation variance
/// 2. For result limits: Apply 1.5x buffer to ensure we get the best results after final ranking
/// 3. Add minimum file count (20) to ensure reasonable coverage
/// 4. Use conservative assumptions about results per file (2.5 instead of 3)
/// 5. Better to slightly exceed limits than to fall short
pub fn estimate_files_needed(
    max_results: Option<usize>,
    max_tokens: Option<usize>,
    avg_tokens_per_result: usize,
) -> usize {
    // Minimum files to process for reasonable coverage
    const MIN_FILES_TO_PROCESS: usize = 20;

    // Conservative estimate: assume each file produces ~2.5 results on average
    // (slightly lower than previous 3.0 to be more conservative)
    const AVG_RESULTS_PER_FILE: f64 = 2.5;

    // Apply 1.5x buffer to result limits as well to ensure we get the best results after final ranking
    let result_limit = max_results.unwrap_or(1000);
    let buffered_result_limit = (result_limit as f64 * 1.5).ceil() as usize;

    if let Some(token_limit) = max_tokens {
        // Estimate files needed based on token limit with buffer
        let results_needed_for_tokens = token_limit / avg_tokens_per_result.max(1);

        // Apply 1.5x buffer for token-based estimation
        // This accounts for variance in actual tokens per result and ensures we don't fall short
        let buffered_token_results = (results_needed_for_tokens as f64 * 1.5).ceil() as usize;

        // Convert results to files needed (using conservative avg results per file)
        let files_for_tokens =
            (buffered_token_results as f64 / AVG_RESULTS_PER_FILE).ceil() as usize;

        // For result-based limits, apply buffer and estimate conservatively
        let files_for_results =
            (buffered_result_limit as f64 / AVG_RESULTS_PER_FILE).ceil() as usize;

        // Take the smaller of the two file estimates, ensuring we meet the minimum
        let final_estimate = files_for_tokens.min(files_for_results);
        final_estimate.max(MIN_FILES_TO_PROCESS)
    } else {
        // If no token limit, estimate based on buffered result limit only
        let files_needed = (buffered_result_limit as f64 / AVG_RESULTS_PER_FILE).ceil() as usize;

        // Apply minimum file count for reasonable coverage
        files_needed.max(MIN_FILES_TO_PROCESS)
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

    #[test]
    fn test_estimate_files_needed_token_limits() {
        // Test with token limit - should apply 1.5x buffer
        let result = estimate_files_needed(None, Some(5000), 250);
        // Expected: 5000 tokens / 250 avg = 20 results needed
        // With 1.5x buffer: 30 results needed
        // At 2.5 results per file: 12 files needed
        // But minimum is 20, so should return 20
        assert_eq!(result, 20);

        // Test with larger token limit where buffer matters
        let result = estimate_files_needed(None, Some(25000), 250);
        // Expected: 25000 / 250 = 100 results needed
        // With 1.5x buffer: 150 results needed
        // At 2.5 results per file: 60 files needed
        // Should return 60 (above minimum)
        assert_eq!(result, 60);

        // Test with very small token limit - should enforce minimum
        let result = estimate_files_needed(None, Some(500), 250);
        // Expected: 500 / 250 = 2 results needed
        // With 1.5x buffer: 3 results needed
        // At 2.5 results per file: 2 files needed
        // But minimum is 20, so should return 20
        assert_eq!(result, 20);
    }

    #[test]
    fn test_estimate_files_needed_result_limits() {
        // Test with only result limit - now applies 1.5x buffer
        let result = estimate_files_needed(Some(50), None, 250);
        // Expected: 50 results * 1.5 buffer = 75 results / 2.5 results per file = 30 files
        // Should return 30 (above minimum)
        assert_eq!(result, 30);

        // Test with larger result limit
        let result = estimate_files_needed(Some(100), None, 250);
        // Expected: 100 results * 1.5 buffer = 150 results / 2.5 results per file = 60 files
        // Should return 60 (above minimum)
        assert_eq!(result, 60);

        // Test with small result limit - should enforce minimum
        let result = estimate_files_needed(Some(10), None, 250);
        // Expected: 10 results * 1.5 buffer = 15 results / 2.5 results per file = 6 files
        // But minimum is 20, so should return 20
        assert_eq!(result, 20);
    }

    #[test]
    fn test_estimate_files_needed_both_limits() {
        // Test with both limits - should take the more restrictive
        let result = estimate_files_needed(Some(30), Some(7500), 250);
        // Result limit: 30 * 1.5 buffer = 45 results / 2.5 = 18 files (but min 20) = 20 files
        // Token limit: 7500 / 250 = 30 results * 1.5 buffer = 45 results / 2.5 = 18 files (but min 20) = 20 files
        // Should take minimum of both = 20
        assert_eq!(result, 20);

        // Test where result limit is more restrictive
        let result = estimate_files_needed(Some(25), Some(50000), 250);
        // Result limit: 25 * 1.5 buffer = 37.5 results / 2.5 = 15 files (but min 20) = 20 files
        // Token limit: 50000 / 250 = 200 results * 1.5 buffer = 300 results / 2.5 = 120 files
        // Should take minimum = 20
        assert_eq!(result, 20);

        // Test where token limit is more restrictive
        let result = estimate_files_needed(Some(500), Some(5000), 250);
        // Result limit: 500 * 1.5 buffer = 750 results / 2.5 = 300 files
        // Token limit: 5000 / 250 = 20 results * 1.5 buffer = 30 results / 2.5 = 12 files (but min 20) = 20 files
        // Should take minimum = 20
        assert_eq!(result, 20);
    }

    #[test]
    fn test_estimate_files_needed_edge_cases() {
        // Test with no limits - should use default result limit with buffer
        let result = estimate_files_needed(None, None, 250);
        // Expected: 1000 default results * 1.5 buffer = 1500 results / 2.5 results per file = 600 files
        assert_eq!(result, 600);

        // Test with zero avg_tokens_per_result - should handle gracefully
        let result = estimate_files_needed(None, Some(5000), 0);
        // Should handle division by zero by using max(1) = 1
        // 5000 / 1 = 5000 results * 1.5 buffer = 7500 results / 2.5 = 3000 files
        // Should be very large number (>= minimum)
        assert!(result >= 20);

        // Test buffer effectiveness - ensure we get more files than without buffer
        let without_buffer_calc = 10000 / 250 / 3; // Old logic without buffer: ~13 files
        let with_buffer = estimate_files_needed(None, Some(10000), 250);
        // New logic: 10000 / 250 = 40 results * 1.5 buffer = 60 results / 2.5 = 24 files
        // Should be significantly more than old calculation
        assert!(with_buffer > without_buffer_calc);
        assert_eq!(with_buffer, 24);
    }
}
