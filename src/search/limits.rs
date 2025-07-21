use probe_code::models::{LimitedSearchResults, SearchResult};
use probe_code::search::token_utils::count_tokens;

/// Helper function to apply limits to search results
pub fn apply_limits(
    results: Vec<SearchResult>,
    max_results: Option<usize>,
    max_bytes: Option<usize>,
    max_tokens: Option<usize>,
) -> LimitedSearchResults {
    // If no limits are specified, return all results
    if max_results.is_none() && max_bytes.is_none() && max_tokens.is_none() {
        return LimitedSearchResults {
            results,
            truncated: false,
            total_results: results.len(),
            total_bytes: results.iter().map(|r| r.content.len()).sum(),
            total_tokens: results.iter().map(|r| count_tokens(&r.content)).sum(),
        };
    }

    let mut limited_results = Vec::new();
    let mut current_bytes = 0;
    let mut current_tokens = 0;
    let mut truncated = false;

    // Calculate total bytes and tokens for all results
    let total_bytes = results.iter().map(|r| r.content.len()).sum();
    let total_tokens = results.iter().map(|r| count_tokens(&r.content)).sum();

    // Apply limits
    for result in results {
        // Check if we've reached the maximum number of results
        if let Some(max) = max_results {
            if limited_results.len() >= max {
                truncated = true;
                break;
            }
        }

        // Check if adding this result would exceed the maximum bytes
        if let Some(max) = max_bytes {
            if current_bytes + result.content.len() > max {
                truncated = true;
                break;
            }
        }

        // Check if adding this result would exceed the maximum tokens
        if let Some(max) = max_tokens {
            let result_tokens = count_tokens(&result.content);
            if current_tokens + result_tokens > max {
                truncated = true;
                break;
            }
            current_tokens += result_tokens;
        }

        // Add the result to the limited results
        current_bytes += result.content.len();
        limited_results.push(result);
    }

    LimitedSearchResults {
        results: limited_results,
        truncated,
        total_results: limited_results.len(),
        total_bytes: current_bytes,
        total_tokens: current_tokens,
    }
}
