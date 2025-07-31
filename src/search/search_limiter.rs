use probe_code::models::{LimitedSearchResults, SearchLimits, SearchResult};
use probe_code::search::search_tokens::count_block_tokens;

/// Helper function to apply limits (max results, max bytes, max tokens) to search results
///
/// This function implements pre-computed token limits optimization with running totals and early termination.
/// Token counting is expensive (tiktoken-rs calls taking ~31ms for small result sets,
/// up to 11.72s for large result sets), so we optimize by:
/// 1. Skip token counting entirely if max_tokens is None (most common case - saves 100% of time)
/// 2. Track running totals (bytes, tokens, result count) and terminate early when any limit is reached
/// 3. Use progressive token counting - only start counting when we approach the estimated limit
/// 4. Estimate tokens based on byte count to avoid counting until necessary
/// 5. Process results in rank order (best first) to ensure optimal result quality within limits
///
/// Performance optimizations:
/// - Pre-computed limits: Track running totals instead of processing all results then applying limits
/// - Early termination: Stop processing immediately when any limit (max_results, max_tokens, max_bytes) is reached
/// - Zero token counting if max_tokens is None (saves 31ms-11.72s on result sets)
/// - Progressive evaluation: only count tokens when we estimate we're approaching the limit
/// - Byte-based early estimation (1 token ≈ 4 bytes is rough approximation)
/// - Result quality: Process ranked results first to ensure best results within limits
pub fn apply_limits(
    results: Vec<SearchResult>,
    max_results: Option<usize>,
    max_bytes: Option<usize>,
    max_tokens: Option<usize>,
) -> LimitedSearchResults {
    // Early return if no limits are specified - avoids all token counting and processing
    if max_results.is_none() && max_bytes.is_none() && max_tokens.is_none() {
        return LimitedSearchResults {
            results,
            skipped_files: Vec::new(),
            limits_applied: None,
            cached_blocks_skipped: None,
            files_skipped_early_termination: None,
        };
    }

    let mut results = results;

    // EARLY TERMINATION OPTIMIZATION: Use partial sort instead of full sort
    // Only sort the top results we might need, reducing complexity from O(n log n) to O(k log n)
    // where k (estimated results needed) << n (total results)
    let estimated_results_needed = max_results.unwrap_or(1000).min(results.len());

    // Use select_nth_unstable_by for partial sorting - much faster for large result sets
    // This ensures the first 'estimated_results_needed' results are the best ranked ones
    if results.len() > estimated_results_needed {
        results.select_nth_unstable_by(estimated_results_needed - 1, |a, b| {
            match (a.rank, b.rank) {
                (Some(a_r), Some(b_r)) => a_r.cmp(&b_r),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                _ => std::cmp::Ordering::Equal,
            }
        });
        // Only sort the selected portion for consistent ordering
        results[..estimated_results_needed].sort_by(|a, b| match (a.rank, b.rank) {
            (Some(a_r), Some(b_r)) => a_r.cmp(&b_r),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        });
    } else {
        // For small result sets, full sort is still efficient
        results.sort_by(|a, b| match (a.rank, b.rank) {
            (Some(a_r), Some(b_r)) => a_r.cmp(&b_r),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            _ => std::cmp::Ordering::Equal,
        });
    }

    let mut limited = Vec::new();
    let mut skipped = Vec::new();

    // PRE-COMPUTED LIMITS OPTIMIZATION:
    // Instead of processing all results and then applying limits, we track running totals
    // and terminate early as soon as any limit is reached. This provides significant
    // performance improvements:
    //
    // 1. Early termination: Stop processing immediately when limits are reached
    // 2. Running totals: Track bytes/tokens/count incrementally during processing
    // 3. Optimal result ordering: Process best-ranked results first to ensure quality
    // 4. Reduced token counting: Only count tokens when approaching the limit
    let mut running_bytes = 0; // Running total of bytes in accepted results
    let mut running_tokens = 0; // Running total of tokens in accepted results
    let mut running_count = 0; // Running count of accepted results

    // Performance optimization: Determine if we need token counting and when to start
    let max_token_limit = max_tokens.unwrap_or(usize::MAX);
    let mut token_counting_started = false;

    // EARLY TERMINATION: Track if any limit has been reached to exit the loop completely
    let mut limit_reached = false;

    // Ultra-lazy token counting approach:
    // - Skip token counting entirely if max_tokens is None (saves 31ms-11.72s)
    // - Use byte-based estimation until we approach the token limit (1 token ≈ 4 bytes)
    // - Only start precise token counting when within 90% of the limit
    // - This minimizes expensive tiktoken-rs calls while maintaining accuracy

    // EARLY TERMINATION OPTIMIZATION: Process only the results we might need
    // Limit the iteration to the estimated_results_needed to avoid processing beyond limits
    let max_iterations = estimated_results_needed;

    for (index, r) in results.into_iter().enumerate() {
        // EARLY TERMINATION: Stop processing if any limit has been reached
        if limit_reached || index >= max_iterations {
            // Add remaining results to skipped if they have valid ranking
            if r.rank.is_some()
                && (r.tfidf_score.unwrap_or(0.0) > 0.0 || r.bm25_score.unwrap_or(0.0) > 0.0)
            {
                skipped.push(r);
            }
            continue;
        }
        let r_bytes = r.code.len();

        // PRE-COMPUTED LIMITS: Check result count limit first (fastest check)
        if let Some(max_res) = max_results {
            if running_count >= max_res {
                // Early termination: we've reached max results, collect remaining as skipped
                limit_reached = true;
                if r.rank.is_some()
                    && (r.tfidf_score.unwrap_or(0.0) > 0.0 || r.bm25_score.unwrap_or(0.0) > 0.0)
                {
                    skipped.push(r);
                }
                continue;
            }
        }

        // PRE-COMPUTED LIMITS: Check byte limit before processing (second fastest check)
        if let Some(max_bytes_limit) = max_bytes {
            if running_bytes + r_bytes > max_bytes_limit {
                // Early termination: adding this result would exceed byte limit
                limit_reached = true;
                if r.rank.is_some()
                    && (r.tfidf_score.unwrap_or(0.0) > 0.0 || r.bm25_score.unwrap_or(0.0) > 0.0)
                {
                    skipped.push(r);
                }
                continue;
            }
        }

        // PRE-COMPUTED LIMITS: Ultra-lazy token counting with running totals
        let r_tokens = if max_tokens.is_some() {
            // Use rough estimation and only start precise counting if we're very close to the limit
            let estimated_tokens = (r_bytes / 4).max(1);
            let estimated_total_after = running_tokens + estimated_tokens;

            // Only start precise counting if we're within 90% of the limit based on estimation
            if !token_counting_started
                && estimated_total_after >= (max_token_limit as f64 * 0.9) as usize
            {
                token_counting_started = true;
                // When we start counting, we need to recalculate tokens for already included results
                // Use block-level caching for better performance on code blocks
                running_tokens = limited
                    .iter()
                    .map(|result: &SearchResult| count_block_tokens(&result.code))
                    .sum();
                // Now count this result precisely too using block-level caching
                count_block_tokens(&r.code)
            } else if token_counting_started {
                // We've already started precise counting - use block-level caching
                count_block_tokens(&r.code)
            } else {
                // Still using estimation
                estimated_tokens
            }
        } else {
            0 // No token limit specified, so we don't need any count
        };

        // PRE-COMPUTED LIMITS: Check token limit with running totals
        if let Some(max_tokens_limit) = max_tokens {
            if running_tokens + r_tokens > max_tokens_limit {
                // Early termination: adding this result would exceed token limit
                limit_reached = true;
                if r.rank.is_some()
                    && (r.tfidf_score.unwrap_or(0.0) > 0.0 || r.bm25_score.unwrap_or(0.0) > 0.0)
                {
                    skipped.push(r);
                }
                continue;
            }
        }

        // PRE-COMPUTED LIMITS: Result passes all limits, add it and update running totals
        running_bytes += r_bytes;
        running_tokens += r_tokens;
        running_count += 1;
        limited.push(r);
    }

    // Final token count calculation: only do expensive precise counting if needed
    let final_total_tokens =
        if max_tokens.is_some() && !token_counting_started && !limited.is_empty() {
            // We only used estimations, but we need to provide accurate final count for the user
            // This is still more efficient than counting every result during the loop
            // Use block-level caching for final token count calculation
            limited
                .iter()
                .map(|result: &SearchResult| count_block_tokens(&result.code))
                .sum()
        } else {
            running_tokens
        };

    LimitedSearchResults {
        results: limited,
        skipped_files: skipped,
        limits_applied: Some(SearchLimits {
            max_results,
            max_bytes,
            max_tokens,
            total_bytes: running_bytes,
            total_tokens: final_total_tokens,
        }),
        cached_blocks_skipped: None,
        files_skipped_early_termination: None,
    }
}
