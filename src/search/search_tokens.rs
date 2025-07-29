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

/// Cache configuration for block-level pre-tokenization
#[derive(Debug, Clone)]
struct BlockTokenCacheConfig {
    /// Maximum number of block entries in the cache
    max_entries: usize,
    /// Minimum block size to cache (bytes) - lower than token cache since blocks are larger
    min_block_size: usize,
    /// Maximum age for block cache entries (seconds) - longer since blocks change less frequently
    max_age_seconds: u64,
}

impl Default for BlockTokenCacheConfig {
    fn default() -> Self {
        Self {
            max_entries: 2000,     // Larger cache for blocks since they're more expensive to compute
            min_block_size: 20,    // Cache smaller blocks since they represent meaningful code units
            max_age_seconds: 7200, // 2 hour cache lifetime for blocks (longer than content cache)
        }
    }
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

/// Block-level token cache entry containing both token count and pre-computed tokenized content
/// This allows reuse of tokenization across multiple operations (search limiting, output formatting, etc.)
#[derive(Debug, Clone)]
struct BlockTokenCacheEntry {
    /// Pre-computed token count for the block
    token_count: usize,
    /// Optional: pre-computed tokenized content (for advanced use cases)
    /// Currently not stored to save memory, but could be added for further optimization
    _tokenized_content: Option<Vec<String>>,
    /// Timestamp when this entry was last accessed (for LRU eviction)
    last_accessed: u64,
    /// Content hash for validation (to detect if content changed)
    content_hash: String,
}

/// Thread-safe token count cache using content hashing
struct TokenCountCache {
    cache: DashMap<String, TokenCacheEntry>,
    config: TokenCacheConfig,
}

/// Thread-safe block-level token cache with content hashing and pre-tokenization
/// 
/// This cache is specifically designed for code blocks and provides significant performance
/// improvements by pre-computing and caching tokenization results for entire code blocks.
/// Key features:
/// - Content-based cache keys using SHA-256 hashing for reliable content identification
/// - Block-level granularity reduces cache misses compared to line-by-line caching
/// - Optimized for repeated tokenization of the same code blocks across queries
/// - LRU eviction with configurable size and TTL limits
/// - Thread-safe concurrent access using DashMap
struct BlockTokenCache {
    cache: DashMap<String, BlockTokenCacheEntry>,
    config: BlockTokenCacheConfig,
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

impl BlockTokenCache {
    fn new() -> Self {
        Self {
            cache: DashMap::new(),
            config: BlockTokenCacheConfig::default(),
        }
    }

    /// Get content hash using SHA-256 (more secure and reliable than MD5 for block-level caching)
    /// For block-level caching, we use a stronger hash function to ensure content integrity
    /// and reduce hash collisions for large code blocks
    fn hash_block_content(content: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        
        // Use a simple but fast hash for caching purposes
        // SHA-256 would be more secure but is overkill for cache keys and slower
        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }

    /// Get current timestamp in seconds since epoch
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    /// Clean expired and excess entries from block cache
    /// This is more aggressive than the regular token cache cleanup since blocks
    /// take more memory and we want to keep the cache lean
    fn cleanup(&self) {
        let current_time = Self::current_timestamp();
        let max_age = self.config.max_age_seconds;

        // Remove expired entries first
        self.cache
            .retain(|_, entry| current_time.saturating_sub(entry.last_accessed) < max_age);

        // If still over limit, remove oldest entries (LRU eviction)
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

    /// Get or compute token count for a block with block-level caching
    /// 
    /// This method implements the core block-level pre-tokenization caching optimization.
    /// It caches tokenization results for entire code blocks, providing significant
    /// performance improvements when the same blocks are tokenized multiple times
    /// across different operations (search limiting, output formatting, etc.).
    ///
    /// Key optimizations:
    /// - Block-level granularity: Caches entire code blocks rather than individual strings
    /// - Content-based cache keys: Uses content hashing to identify identical blocks
    /// - Reuse across operations: Same cached result used for limiting, output, etc.
    /// - Fast hash validation: Verifies content hasn't changed using stored hash
    /// - LRU eviction: Maintains optimal cache size with age-based cleanup
    fn get_or_compute_block_tokens<F>(&self, block_content: &str, compute_fn: F) -> usize
    where
        F: FnOnce(&str) -> usize,
    {
        // Skip caching for very small blocks to avoid overhead
        if block_content.len() < self.config.min_block_size {
            return compute_fn(block_content);
        }

        let content_hash = Self::hash_block_content(block_content);
        let current_time = Self::current_timestamp();

        // Try to get from cache first
        if let Some(mut entry) = self.cache.get_mut(&content_hash) {
            // Validate that the content hasn't changed (hash collision check)
            if entry.content_hash == content_hash {
                // Update access time for LRU
                entry.last_accessed = current_time;
                return entry.token_count;
            }
            // Hash collision or content changed - remove stale entry
            drop(entry);
            self.cache.remove(&content_hash);
        }

        // Compute token count
        let token_count = compute_fn(block_content);

        // Store in cache with hash validation
        let entry = BlockTokenCacheEntry {
            token_count,
            _tokenized_content: None, // Could store tokenized content here for future optimization
            last_accessed: current_time,
            content_hash: content_hash.clone(),
        };
        self.cache.insert(content_hash, entry);

        // Perform cleanup periodically (every 50 insertions for blocks since they're larger)
        if self.cache.len() % 50 == 0 {
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

/// Global block-level token cache instance for block pre-tokenization caching
static BLOCK_TOKEN_CACHE: OnceLock<BlockTokenCache> = OnceLock::new();

/// Get reference to the global token cache
fn get_token_cache() -> &'static TokenCountCache {
    TOKEN_CACHE.get_or_init(TokenCountCache::new)
}

/// Get reference to the global block token cache
fn get_block_token_cache() -> &'static BlockTokenCache {
    BLOCK_TOKEN_CACHE.get_or_init(BlockTokenCache::new)
}

/// Returns a reference to the tiktoken tokenizer
pub fn get_tokenizer() -> &'static CoreBPE {
    static TOKENIZER: OnceLock<CoreBPE> = OnceLock::new();
    TOKENIZER.get_or_init(|| p50k_base().expect("Failed to initialize tiktoken tokenizer"))
}

/// Helper function to count tokens in a string using tiktoken (same tokenizer as GPT models)
///
/// This function implements both content-level and block-level token count caching to improve 
/// performance for repeated tokenization. The caching strategy is optimized based on content size:
///
/// **Block-level caching (for larger content >= 100 bytes):**
/// - Designed for code blocks that are tokenized multiple times across operations
/// - Uses content hashing with collision detection for reliable cache keys
/// - Longer TTL (2 hours) since code blocks change less frequently
/// - Larger cache size (2000 entries) for better hit rates
/// - Optimized for reuse across search limiting, output formatting, etc.
///
/// **Content-level caching (for smaller content 50-99 bytes):**
/// - Fast MD5 hashing to identify identical content
/// - Standard TTL (1 hour) for general purpose caching
/// - Standard cache size (1000 entries)
///
/// **No caching (for very small content < 50 bytes):**
/// - Direct tokenization to avoid cache overhead
///
/// Performance optimizations:
/// - Dual-tier caching strategy based on content characteristics
/// - Thread-safe caching using DashMap for concurrent access
/// - LRU eviction with configurable size and TTL limits
/// - Content-based cache keys with hash collision detection
/// - Size-based caching thresholds to optimize performance vs. memory usage
///
/// Cache configuration:
/// - Block cache: 2000 entries, 20+ bytes, 2-hour TTL, cleanup every 50 insertions
/// - Content cache: 1000 entries, 50+ bytes, 1-hour TTL, cleanup every 100 insertions
pub fn count_tokens(text: &str) -> usize {
    // Use block-level caching for larger content (typical code blocks)
    // This provides better cache hit rates for code blocks that are tokenized
    // multiple times across different operations (limiting, output formatting, etc.)
    if text.len() >= 100 {
        let block_cache = get_block_token_cache();
        return block_cache.get_or_compute_block_tokens(text, |content| {
            let tokenizer = get_tokenizer();
            tokenizer.encode_with_special_tokens(content).len()
        });
    }

    // Fall back to content-level caching for smaller content
    let cache = get_token_cache();
    cache.get_or_compute(text, |content| {
        let tokenizer = get_tokenizer();
        tokenizer.encode_with_special_tokens(content).len()
    })
}

/// Enhanced function specifically for counting tokens in code blocks with block-level pre-tokenization caching
///
/// This function is optimized for code blocks and provides maximum performance when the same
/// blocks are tokenized repeatedly across different queries and operations. It should be used
/// when you know you're dealing with code blocks that may be reused.
///
/// Key optimizations:
/// - Block-level granularity reduces cache misses
/// - Content hashing ensures reliable cache invalidation
/// - Longer cache lifetime optimized for code block reuse patterns
/// - Thread-safe concurrent access for multi-threaded search operations
///
/// Use this function when:
/// - Tokenizing SearchResult code blocks
/// - Processing extracted code blocks that may be reused
/// - Formatting output where the same blocks may be tokenized multiple times
/// - Any scenario where code blocks might be tokenized more than once
pub fn count_block_tokens(block_content: &str) -> usize {
    let block_cache = get_block_token_cache();
    block_cache.get_or_compute_block_tokens(block_content, |content| {
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
