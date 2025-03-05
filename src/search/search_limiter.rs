use crate::models::{LimitedSearchResults, SearchLimits, SearchResult};
use crate::search::search_tokens::count_tokens;

/// Helper function to apply limits (max results, max bytes, max tokens) to search results
pub fn apply_limits(
    results: Vec<SearchResult>,
    max_results: Option<usize>,
    max_bytes: Option<usize>,
    max_tokens: Option<usize>,
) -> LimitedSearchResults {
    if max_results.is_none() && max_bytes.is_none() && max_tokens.is_none() {
        return LimitedSearchResults {
            results,
            skipped_files: Vec::new(),
            limits_applied: None,
        };
    }

    let mut results = results;
    // Sort results by rank if available
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

    for r in results {
        let r_bytes = r.code.len();
        let r_tokens = count_tokens(&r.code);

        let would_exceed_results = max_results.map_or(false, |mr| limited.len() >= mr);
        let would_exceed_bytes = max_bytes.map_or(false, |mb| total_bytes + r_bytes > mb);
        let would_exceed_tokens = max_tokens.map_or(false, |mt| total_tokens + r_tokens > mt);

        if would_exceed_results || would_exceed_bytes || would_exceed_tokens {
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

    LimitedSearchResults {
        results: limited,
        skipped_files: skipped,
        limits_applied: Some(SearchLimits {
            max_results,
            max_bytes,
            max_tokens,
            total_bytes,
            total_tokens,
        }),
    }
}
