use std::collections::HashSet;

/// Merges adjacent or overlapping line ranges into a minimal set of non-overlapping ranges
/// This improves efficiency by reducing the number of blocks that need to be processed
///
/// # Arguments
/// * `line_numbers` - A HashSet of line numbers to merge
///
/// # Returns
/// * A vector of (start, end) tuples representing merged line ranges
///
/// # Example
/// ```
/// use std::collections::HashSet;
/// use crate::language::block_optimization::merge_line_ranges;
///
/// let mut line_numbers = HashSet::new();
/// line_numbers.insert(1);
/// line_numbers.insert(2);
/// line_numbers.insert(3);
/// line_numbers.insert(5);
/// line_numbers.insert(6);
///
/// let ranges = merge_line_ranges(&line_numbers);
/// assert_eq!(ranges.len(), 2);
/// assert!(ranges.contains(&(1, 3)));
/// assert!(ranges.contains(&(5, 6)));
/// ```
pub fn merge_line_ranges(line_numbers: &HashSet<usize>) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();

    // Handle empty set case
    if line_numbers.is_empty() {
        return ranges;
    }

    // Sort the line numbers
    let mut sorted_lines: Vec<_> = line_numbers.iter().cloned().collect();
    sorted_lines.sort_unstable();

    // Initialize with the first line
    let mut start = sorted_lines[0];
    let mut prev = start;

    // Process remaining lines
    for &line in sorted_lines.iter().skip(1) {
        if line > prev + 1 {
            // Gap found, end the current range and start a new one
            ranges.push((start, prev));
            start = line;
        }
        prev = line;
    }

    // Add the final range
    ranges.push((start, prev));

    ranges
}

/// Optimized version of parse_file_for_code_blocks that merges adjacent line numbers
/// before processing to reduce the number of blocks that need to be processed.
///
/// This function is not currently used in the main code path to maintain compatibility
/// with existing tests. It can be enabled in the future by setting the PROBE_OPTIMIZE_BLOCKS
/// environment variable to "1".
///
/// # Implementation Notes
///
/// To use this optimization in the parse_file_for_code_blocks function:
///
/// 1. Merge adjacent line numbers into ranges before processing:
///    ```rust
///    let merged_ranges = merge_line_ranges(line_numbers);
///    ```
///
/// 2. Get or build the line map for the requested lines:
///    ```rust
///    let line_map = tree_cache::get_or_build_line_map(
///        &cache_key,
///        &tree,
///        line_numbers,  // Use the original line_numbers for compatibility
///        content,
///        extension,
///        language_impl.as_ref(),
///        allow_tests,
///    );
///    ```
///
/// 3. Process each range of lines:
///    ```rust
///    for &(range_start, range_end) in &merged_ranges {
///        // Process each line in the range
///        for line in range_start..=range_end {
///            // Only process lines that were in the original line_numbers set
///            if !line_numbers.contains(&line) {
///                continue;
///            }
///            
///            // Process the line as usual...
///        }
///    }
///    ```
///
/// This optimization can significantly reduce the number of blocks that need to be processed
/// when there are many adjacent line numbers, improving performance for large files.
pub fn optimize_block_extraction() -> bool {
    std::env::var("PROBE_OPTIMIZE_BLOCKS").unwrap_or_default() == "1"
}
