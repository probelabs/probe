use probe_code::models::{LimitedSearchResults, SearchLimits, SearchResult};
use probe_code::search::search_tokens::count_tokens;

/// Helper function to apply limits (max results, max bytes, max tokens) to search results
///
/// This function implements aggressive lazy token counting with early termination to improve performance.
/// Token counting is expensive (tiktoken-rs calls taking ~4.5s for large result sets), so we optimize by:
/// 1. Skip token counting entirely if max_tokens is None (most common case)
/// 2. Use progressive token counting - only start counting when we're getting close to potential limits
/// 3. Early termination when any limit is exceeded
/// 4. Estimate tokens based on byte count to avoid counting until necessary
///
/// Performance optimizations:
/// - No token counting if max_tokens is None (saves ~4.5s on large result sets)
/// - Progressive evaluation: only count tokens when we estimate we're approaching the limit
/// - Byte-based early estimation (1 token ≈ 4 bytes is rough approximation)
/// - Early termination when limits are exceeded to avoid processing remaining results
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

    // Rough estimation: 1 token ≈ 4 bytes (GPT-style tokenization)
    // We'll start precise token counting when we reach ~80% of estimated limit
    let token_counting_threshold = if max_tokens.is_some() {
        (max_token_limit as f64 * 0.8 * 4.0) as usize // 80% of max tokens * 4 bytes per token
    } else {
        usize::MAX // Never start counting if no token limit
    };

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

        // Lazy token counting: only start counting when we approach the estimated limit
        let r_tokens = if max_tokens.is_some() {
            // Check if we should start precise token counting
            if !token_counting_started && total_bytes >= token_counting_threshold {
                token_counting_started = true;
                // When we start counting, we need to recalculate tokens for already included results
                total_tokens = limited
                    .iter()
                    .map(|result: &SearchResult| count_tokens(&result.code))
                    .sum();
            }

            if token_counting_started {
                count_tokens(&r.code)
            } else {
                // Use rough estimation until we need precise counting
                (r_bytes / 4).max(1) // Rough approximation: 1 token per 4 bytes, minimum 1 token
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

    // If we used estimation and never started precise counting, calculate final precise token count
    let final_total_tokens =
        if max_tokens.is_some() && !token_counting_started && !limited.is_empty() {
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
