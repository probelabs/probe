use dashmap::DashMap;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};
use tiktoken_rs::{p50k_base, CoreBPE};

/// Cache configuration for token counting
#[derive(Debug, Clone)]
struct TokenCacheConfig {
    /// Maximum number of entries in the cache
    max_entries: usize,
    /// Minimum content size to cache (bytes)
    min_content_size: usize,
    /// Maximum age for cache entries (seconds)
    max_age_seconds: u64,
}

impl Default for TokenCacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 1000,     // Reasonable limit for memory usage
            min_content_size: 50,  // Only cache content >= 50 bytes
            max_age_seconds: 3600, // 1 hour cache lifetime
        }
    }
}

/// Cache entry containing token count and access timestamp
#[derive(Debug, Clone)]
struct TokenCacheEntry {
    token_count: usize,
    last_accessed: u64,
}

/// Thread-safe token count cache using content hashing
struct TokenCountCache {
    cache: DashMap<String, TokenCacheEntry>,
    config: TokenCacheConfig,
}

impl TokenCountCache {
    fn new() -> Self {
        Self {
            cache: DashMap::new(),
            config: TokenCacheConfig::default(),
        }
    }

    /// Get content hash using MD5 (fast, good distribution for caching)
    fn hash_content(content: &str) -> String {
        let digest = md5::compute(content.as_bytes());
        format!("{:x}", digest)
    }

    /// Get current timestamp in seconds since epoch
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Clean expired and excess entries from cache
    fn cleanup(&self) {
        let current_time = Self::current_timestamp();
        let max_age = self.config.max_age_seconds;

        // Remove expired entries
        self.cache
            .retain(|_, entry| current_time.saturating_sub(entry.last_accessed) < max_age);

        // If still over limit, remove oldest entries (simple LRU)
        if self.cache.len() > self.config.max_entries {
            let mut entries: Vec<_> = self
                .cache
                .iter()
                .map(|item| (item.key().clone(), item.value().last_accessed))
                .collect();

            // Sort by access time (oldest first)
            entries.sort_by_key(|(_, timestamp)| *timestamp);

            // Remove oldest entries to get under limit
            let to_remove = self.cache.len() - self.config.max_entries;
            for (key, _) in entries.into_iter().take(to_remove) {
                self.cache.remove(&key);
            }
        }
    }

    /// Get token count from cache or compute and cache it
    fn get_or_compute<F>(&self, content: &str, compute_fn: F) -> usize
    where
        F: FnOnce(&str) -> usize,
    {
        // Skip caching for small content to avoid overhead
        if content.len() < self.config.min_content_size {
            return compute_fn(content);
        }

        let hash = Self::hash_content(content);
        let current_time = Self::current_timestamp();

        // Try to get from cache first
        if let Some(mut entry) = self.cache.get_mut(&hash) {
            // Update access time for LRU
            entry.last_accessed = current_time;
            return entry.token_count;
        }

        // Compute token count
        let token_count = compute_fn(content);

        // Store in cache
        let entry = TokenCacheEntry {
            token_count,
            last_accessed: current_time,
        };
        self.cache.insert(hash, entry);

        // Perform cleanup periodically (every 100 insertions approximately)
        if self.cache.len() % 100 == 0 {
            self.cleanup();
        }

        token_count
    }

    /// Get cache statistics for debugging
    #[allow(dead_code)]
    fn stats(&self) -> (usize, usize) {
        (self.cache.len(), self.config.max_entries)
    }
}

/// Global token count cache instance
static TOKEN_CACHE: OnceLock<TokenCountCache> = OnceLock::new();

/// Get reference to the global token cache
fn get_token_cache() -> &'static TokenCountCache {
    TOKEN_CACHE.get_or_init(TokenCountCache::new)
}

/// Returns a reference to the tiktoken tokenizer
pub fn get_tokenizer() -> &'static CoreBPE {
    static TOKENIZER: OnceLock<CoreBPE> = OnceLock::new();
    TOKENIZER.get_or_init(|| p50k_base().expect("Failed to initialize tiktoken tokenizer"))
}

/// Helper function to count tokens in a string using tiktoken (same tokenizer as GPT models)
///
/// This function implements token count caching to improve performance for repeated tokenization
/// of identical content. The cache uses content hashing (MD5) as keys and implements LRU eviction
/// with size limits and TTL expiration.
///
/// Performance optimizations:
/// - Content hashing: Fast MD5 hashing to identify identical content
/// - Thread-safe caching: DashMap for concurrent access across threads
/// - LRU eviction: Removes least recently used entries when cache fills
/// - Size thresholds: Only caches content >= 50 bytes to avoid overhead
/// - Periodic cleanup: Removes expired entries and enforces size limits
///
/// Cache configuration:
/// - Max entries: 1000 (configurable)
/// - Min content size: 50 bytes
/// - TTL: 1 hour
/// - Cleanup frequency: Every 100 insertions
pub fn count_tokens(text: &str) -> usize {
    let cache = get_token_cache();

    cache.get_or_compute(text, |content| {
        let tokenizer = get_tokenizer();
        tokenizer.encode_with_special_tokens(content).len()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_cache_basic_functionality() {
        let cache = TokenCountCache::new();

        // Test content caching
        let content = "This is a test string that should be cached because it's longer than 50 bytes in total length.";

        // First call should compute and cache
        let count1 = cache.get_or_compute(content, |text| {
            let tokenizer = get_tokenizer();
            tokenizer.encode_with_special_tokens(text).len()
        });

        // Second call should use cached result (we can't directly verify this,
        // but we can ensure the result is consistent)
        let count2 = cache.get_or_compute(content, |text| {
            let tokenizer = get_tokenizer();
            tokenizer.encode_with_special_tokens(text).len()
        });

        assert_eq!(count1, count2);
        assert!(count1 > 0);
    }

    #[test]
    fn test_token_cache_small_content_not_cached() {
        let cache = TokenCountCache::new();

        // Content smaller than min_content_size (50 bytes) should not be cached
        let small_content = "short";

        let count = cache.get_or_compute(small_content, |text| {
            let tokenizer = get_tokenizer();
            tokenizer.encode_with_special_tokens(text).len()
        });

        // Should still return correct count
        assert_eq!(count, 1); // "short" should be 1 token

        // Cache should be empty (can't directly test, but size should be 0)
        let (cache_size, _) = cache.stats();
        assert_eq!(cache_size, 0);
    }

    #[test]
    fn test_count_tokens_consistency() {
        // Test that our cached count_tokens function returns same results as direct tiktoken
        let test_cases = vec![
            "short",
            "This is a medium-length string that should give consistent results.",
            "fn main() {\n    println!(\"Hello, world!\");\n}",
            "const calculateTokens = (text) => {\n    return text.split(' ').length;\n};\n\nexport default calculateTokens;",
        ];

        for content in test_cases {
            let cached_count = count_tokens(content);

            // Direct tiktoken call for comparison
            let tokenizer = get_tokenizer();
            let direct_count = tokenizer.encode_with_special_tokens(content).len();

            assert_eq!(
                cached_count, direct_count,
                "Cached count should match direct tiktoken count for content: {:?}",
                content
            );
        }
    }

    #[test]
    fn test_token_cache_hash_consistency() {
        // Test that identical content produces same hash
        let content = "This is test content for hash consistency validation and should be long enough to be cached.";

        let hash1 = TokenCountCache::hash_content(content);
        let hash2 = TokenCountCache::hash_content(content);

        assert_eq!(hash1, hash2);
        assert!(!hash1.is_empty());

        // Different content should produce different hashes
        let different_content = "This is different test content for hash consistency validation and should be long enough.";
        let hash3 = TokenCountCache::hash_content(different_content);

        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_token_cache_thread_safety() {
        use std::sync::Arc;
        use std::thread;

        let cache = Arc::new(TokenCountCache::new());
        let content = "This is test content for thread safety validation and should be long enough to be cached by the system.";

        let handles: Vec<_> = (0..4)
            .map(|_| {
                let cache_clone = Arc::clone(&cache);
                let content_clone = content.to_string();

                thread::spawn(move || {
                    cache_clone.get_or_compute(&content_clone, |text| {
                        let tokenizer = get_tokenizer();
                        tokenizer.encode_with_special_tokens(text).len()
                    })
                })
            })
            .collect();

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // All threads should return the same count
        let first_result = results[0];
        for result in &results[1..] {
            assert_eq!(*result, first_result);
        }

        assert!(first_result > 0);
    }
}
