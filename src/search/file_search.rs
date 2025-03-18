use anyhow::{Context, Result};
use grep::regex::RegexMatcherBuilder;
use grep::searcher::sinks::UTF8;
use grep::searcher::{BinaryDetection, SearcherBuilder};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;
// No need for term_exceptions import

use crate::search::file_list_cache;

/// Helper function to format duration in a human-readable way
fn format_duration(duration: std::time::Duration) -> String {
    if duration.as_millis() < 1000 {
        format!("{}ms", duration.as_millis())
    } else {
        format!("{:.2}s", duration.as_secs_f64())
    }
}

/// Searches a file for a pattern and returns whether it matched, along with the matching line numbers and content
pub fn search_file_for_pattern(
    file_path: &Path,
    pattern: &str,
) -> Result<(bool, HashMap<usize, Vec<String>>)> {
    let start_time = Instant::now();

    let mut matched = false;
    let mut line_matches = HashMap::new();
    let file_name = file_path.file_name().unwrap_or_default().to_string_lossy();

    // Check if debug mode is enabled
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    if debug_mode {
        println!(
            "DEBUG: Searching file {:?} for pattern: '{}'",
            file_path, pattern
        );
    }

    // Check if the pattern already has word boundaries or parentheses (indicating a grouped pattern)
    // or if it's a combined pattern (starts with "(?i)")
    let pattern_adjustment_start = Instant::now();
    let _has_word_boundaries = pattern.contains("\\b") || pattern.starts_with("(");
    let _is_combined_pattern = pattern.starts_with("(?i)");

    // Use the pattern as-is if exact mode is specified, it already has word boundaries,
    // it's a grouped pattern (starts with parenthesis), or it's a combined pattern
    let adjusted_pattern = pattern.to_string();

    let pattern_adjustment_duration = pattern_adjustment_start.elapsed();

    if debug_mode {
        println!(
            "DEBUG: Pattern adjustment completed in {} - Adjusted pattern: '{}'",
            format_duration(pattern_adjustment_duration),
            adjusted_pattern
        );
    }

    // Create a case-insensitive regex matcher for the pattern
    // Note: For combined patterns that already include (?i), this is redundant but harmless
    let matcher_creation_start = Instant::now();
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(true)
        .build(&adjusted_pattern)
        .context(format!(
            "Failed to create regex matcher for: {}",
            adjusted_pattern
        ))?;

    let matcher_creation_duration = matcher_creation_start.elapsed();

    if debug_mode {
        println!(
            "DEBUG: Matcher creation completed in {}",
            format_duration(matcher_creation_duration)
        );
    }

    // Configure the searcher
    let searcher_start = Instant::now();
    let mut searcher = SearcherBuilder::new()
        .binary_detection(BinaryDetection::quit(b'\x00'))
        .build();

    let searcher_creation_duration = searcher_start.elapsed();

    if debug_mode {
        println!(
            "DEBUG: Searcher creation completed in {}",
            format_duration(searcher_creation_duration)
        );
    }

    // Search the file
    let search_start = Instant::now();
    if let Err(err) = searcher.search_path(
        &matcher,
        file_path,
        UTF8(|line_number, line| {
            // Check if line is longer than 2000 characters
            if line.len() > 2000 {
                if debug_mode {
                    println!(
                        "  Skipping line {} in file {} - line too long ({} characters)",
                        line_number,
                        file_name,
                        line.len()
                    );
                }
                return Ok(true); // Skip this line but continue searching
            }

            // Find all matches in the line
            let line_num = line_number as usize;
            matched = true;
            // Use regex to find all matches in the line
            if let Ok(re) = regex::Regex::new(&adjusted_pattern) {
                let matches: Vec<String> = re.find_iter(line)
                    .map(|m| m.as_str().to_string())
                    .collect();
                if !matches.is_empty() {
                    line_matches.entry(line_num)
                        .or_insert_with(Vec::new)
                        .extend(matches);
                    if debug_mode {
                        println!(
                            "  Match found in file: {} (term: {}) at line {} - matches: {:?}",
                            file_name, adjusted_pattern, line_number,
                            line_matches.get(&line_num).unwrap()
                        );
                    }
                }
            } else {
                // If regex creation fails, just store the line number without content
                line_matches.entry(line_num).or_insert_with(Vec::new);
                if debug_mode {
                    println!(
                        "  Match found in file: {} (term: {}) at line {} - failed to extract match content",
                        file_name, adjusted_pattern, line_number
                    );
                }
            }

            Ok(true) // Continue searching for all matches
        }),
    ) {
        // Just convert the error to anyhow::Error
        return Err(err.into());
    }

    let search_duration = search_start.elapsed();

    if debug_mode {
        println!(
            "DEBUG: File search completed in {} - Found matches on {} lines",
            format_duration(search_duration),
            line_matches.len()
        );
    }

    let total_duration = start_time.elapsed();

    if debug_mode {
        println!(
            "DEBUG: Total file pattern search completed in {}",
            format_duration(total_duration)
        );
    }

    Ok((matched, line_matches))
}

/// Finds files containing a specific pattern
pub fn find_files_with_pattern(
    path: &Path,
    pattern: &str,
    custom_ignores: &[String],
    allow_tests: bool,
) -> Result<Vec<PathBuf>> {
    // Use the cached file list implementation
    file_list_cache::find_files_with_pattern(path, pattern, custom_ignores, allow_tests)
}

/// Function to find files whose names match query words
/// Returns a map of file paths to the term indices that matched the filename
pub fn find_matching_filenames(
    path: &Path,
    queries: &[String],
    already_found_files: &HashSet<PathBuf>,
    custom_ignores: &[String],
    allow_tests: bool,
    term_indices: &HashMap<String, usize>,
) -> Result<HashMap<PathBuf, HashSet<usize>>> {
    // Use the cached file list implementation
    file_list_cache::find_matching_filenames(
        path,
        queries,
        already_found_files,
        custom_ignores,
        allow_tests,
        term_indices,
    )
}

/// Compatibility function for the old get_filename_matched_queries signature
/// This is now a simplified version that returns a set with all query indices
/// since we're adding the filename to the top of each code block
#[allow(dead_code)]
pub fn get_filename_matched_queries_compat(
    filename: &str,
    queries_terms: &[Vec<(String, String)>],
) -> HashSet<usize> {
    let start_time = Instant::now();
    let mut matched_terms = HashSet::new();

    // Check if debug mode is enabled
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    if debug_mode {
        println!(
            "DEBUG: Starting filename matched queries compatibility function for '{}'",
            filename
        );
    }

    // Add all query indices to the matched_terms set
    // This ensures that all terms are considered "matched" by the filename
    for (query_idx, term_pairs) in queries_terms.iter().enumerate() {
        matched_terms.insert(query_idx);

        if debug_mode {
            for (term_idx, (original_term, _)) in term_pairs.iter().enumerate() {
                println!(
                    "DEBUG:   Term '{}' (index {}) considered matched by filename '{}'",
                    original_term, term_idx, filename
                );
            }
        }
    }

    let total_duration = start_time.elapsed();

    if debug_mode {
        println!(
            "DEBUG: Filename matched queries compatibility function completed in {} - Added {} term indices for filename '{}'",
            format_duration(total_duration),
            matched_terms.len(),
            filename
        );
    }

    matched_terms
}
