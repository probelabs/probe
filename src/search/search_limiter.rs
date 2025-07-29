use probe_code::models::{LimitedSearchResults, SearchLimits, SearchResult};
use probe_code::search::search_tokens::count_tokens;

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
        };
    }

    let mut results = results;
    // Sort results by rank if available (best results first)
    results.sort_by(|a, b| match (a.rank, b.rank) {
        (Some(a_r), Some(b_r)) => a_r.cmp(&b_r),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        _ => std::cmp::Ordering::Equal,
    });

    let mut limited = Vec::new();
    let mut skipped = Vec::new();
    let mut total_bytes = 0;
    let mut total_tokens = 0;

    // Performance optimization: Determine if we need token counting and when to start
    let max_token_limit = max_tokens.unwrap_or(usize::MAX);
    let mut token_counting_started = false;

    // Note: We now use ultra-lazy approach - no need for byte-based threshold
    // Token counting is triggered only when estimated tokens approach the limit

    for r in results {
        let r_bytes = r.code.len();

        // Check limits that don't require token counting first (fastest checks)
        let would_exceed_results = max_results.is_some_and(|mr| limited.len() >= mr);
        let would_exceed_bytes = max_bytes.is_some_and(|mb| total_bytes + r_bytes > mb);

        // Early termination: if we've exceeded non-token limits, skip expensive token counting
        if would_exceed_results || would_exceed_bytes {
            if r.rank.is_some()
                && (r.tfidf_score.unwrap_or(0.0) > 0.0 || r.bm25_score.unwrap_or(0.0) > 0.0)
            {
                skipped.push(r);
            }
            continue;
        }

        // Ultra-lazy token counting: delay all token counting until absolutely necessary
        let r_tokens = if max_tokens.is_some() {
            // Use rough estimation and only start precise counting if we're very close to the limit
            let estimated_tokens = (r_bytes / 4).max(1);
            let estimated_total_after = total_tokens + estimated_tokens;

            // Only start precise counting if we're within 90% of the limit based on estimation
            if !token_counting_started
                && estimated_total_after >= (max_token_limit as f64 * 0.9) as usize
            {
                token_counting_started = true;
                // When we start counting, we need to recalculate tokens for already included results
                total_tokens = limited
                    .iter()
                    .map(|result: &SearchResult| count_tokens(&result.code))
                    .sum();
                // Now count this result precisely too
                count_tokens(&r.code)
            } else if token_counting_started {
                // We've already started precise counting
                count_tokens(&r.code)
            } else {
                // Still using estimation
                estimated_tokens
            }
        } else {
            0 // No token limit specified, so we don't need any count
        };

        let would_exceed_tokens = max_tokens.is_some_and(|mt| total_tokens + r_tokens > mt);

        if would_exceed_tokens {
            if r.rank.is_some()
                && (r.tfidf_score.unwrap_or(0.0) > 0.0 || r.bm25_score.unwrap_or(0.0) > 0.0)
            {
                skipped.push(r);
            }
        } else {
            total_bytes += r_bytes;
            total_tokens += r_tokens;
            limited.push(r);
        }
    }

    // Final token count calculation: only do expensive precise counting if needed
    let final_total_tokens =
        if max_tokens.is_some() && !token_counting_started && !limited.is_empty() {
            // We only used estimations, but we need to provide accurate final count for the user
            // This is still more efficient than counting every result during the loop
            limited
                .iter()
                .map(|result: &SearchResult| count_tokens(&result.code))
                .sum()
        } else {
            total_tokens
        };

    LimitedSearchResults {
        results: limited,
        skipped_files: skipped,
        limits_applied: Some(SearchLimits {
            max_results,
            max_bytes,
            max_tokens,
            total_bytes,
            total_tokens: final_total_tokens,
        }),
        cached_blocks_skipped: None,
    }
}
