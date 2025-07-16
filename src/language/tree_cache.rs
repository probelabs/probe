use anyhow::{Context, Result};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Mutex;
use tree_sitter::Tree;

lazy_static::lazy_static! {
    /// A cache for parsed syntax trees to avoid redundant parsing
    ///
    /// This cache stores parsed ASTs keyed by file path and content hash.
    /// When the same file is parsed multiple times, this avoids the overhead
    /// of re-parsing unchanged files.
    static ref TREE_CACHE: Mutex<HashMap<String, (Tree, u64)>> = Mutex::new(HashMap::new());

    /// A counter for cache hits, used for testing
    static ref CACHE_HITS: Mutex<usize> = Mutex::new(0);

    /// A mutex for test synchronization to prevent concurrent test execution
    static ref TEST_MUTEX: Mutex<()> = Mutex::new(());
}

/// Compute a hash of the content for cache validation
fn compute_content_hash(content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    hasher.finish()
}

/// Get a cached tree if available, otherwise parse and cache the result
///
/// This function checks if a valid cached tree exists for the given file path
/// and content. If found and the content hash matches, it returns the cached tree.
/// Otherwise, it parses the content, caches the result, and returns the new tree.
///
/// # Arguments
///
/// * `file_path` - The path of the file being parsed
/// * `content` - The content to parse
/// * `parser` - The tree-sitter parser to use if parsing is needed
///
/// # Returns
///
/// A Result containing the parsed tree, either from cache or freshly parsed
pub fn get_or_parse_tree(
    file_path: &str,
    content: &str,
    parser: &mut tree_sitter::Parser,
) -> Result<Tree> {
    let content_hash = compute_content_hash(content);

    // Check if debug mode is enabled
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    // Try to get from cache first
    {
        let mut cache = TREE_CACHE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some((cached_tree, cached_hash)) = cache.get(file_path) {
            if cached_hash == &content_hash {
                // Increment cache hit counter
                {
                    let mut hits = CACHE_HITS
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    *hits += 1;
                }

                if debug_mode {
                    println!("[DEBUG] Cache hit for file: {file_path}");
                }
                return Ok(cached_tree.clone());
            } else {
                // Content changed, explicitly remove the old entry
                cache.remove(file_path);
                if debug_mode {
                    println!(
                        "[DEBUG] Cache invalidated for file: {} (content changed)",
                        file_path
                    );
                }
            }
        } else if debug_mode {
            println!("[DEBUG] Cache miss for file: {file_path}");
        }
    }

    // Not in cache or content changed, parse and store
    let tree = parser
        .parse(content, None)
        .context(format!("Failed to parse file: {file_path}"))?;

    // Store in cache
    {
        let mut cache = TREE_CACHE
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        cache.insert(file_path.to_string(), (tree.clone(), content_hash));

        if debug_mode {
            println!("[DEBUG] Cached parsed tree for file: {file_path}");
            println!("[DEBUG] Current cache size: {} entries", cache.len());
        }
    }

    Ok(tree)
}

/// Clear the entire tree cache
///
/// This function can be used to free memory or force re-parsing of all files.
#[allow(dead_code)]
pub fn clear_tree_cache() {
    let mut cache = TREE_CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    if debug_mode {
        println!("[DEBUG] Clearing tree cache ({} entries)", cache.len());
    }

    cache.clear();

    // Also reset the cache hit counter
    let mut hits = CACHE_HITS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *hits = 0;
}

/// Remove a specific file from the tree cache
///
/// # Arguments
///
/// * `file_path` - The path of the file to remove from the cache
#[allow(dead_code)]
pub fn invalidate_cache_entry(file_path: &str) {
    let mut cache = TREE_CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    if cache.remove(file_path).is_some() && debug_mode {
        println!("[DEBUG] Removed file from cache: {file_path}");
    }
}

/// Acquire the test mutex for test synchronization
///
/// This function is used by tests to prevent concurrent access to the cache
/// during test execution, which can lead to flaky tests.
#[allow(dead_code)]
pub fn acquire_test_mutex() -> std::sync::MutexGuard<'static, ()> {
    TEST_MUTEX
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Get the current size of the tree cache
#[allow(dead_code)]
pub fn get_cache_size() -> usize {
    let cache = TREE_CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache.len()
}

/// Check if a specific file exists in the cache
#[allow(dead_code)]
pub fn is_in_cache(file_path: &str) -> bool {
    let cache = TREE_CACHE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    cache.contains_key(file_path)
}

/// Reset the cache hit counter (for testing)
#[allow(dead_code)]
pub fn reset_cache_hit_counter() {
    let mut hits = CACHE_HITS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *hits = 0;
}

/// Get the current cache hit count (for testing)
#[allow(dead_code)]
pub fn get_cache_hit_count() -> usize {
    let hits = CACHE_HITS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *hits
}
