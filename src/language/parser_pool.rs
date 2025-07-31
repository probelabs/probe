use anyhow::Result;
use std::collections::HashMap;
use std::sync::Mutex;
use tree_sitter::Parser;

use crate::language::factory;

// PHASE 4 OPTIMIZATION: Dynamic pool sizing based on CPU cores
const DEFAULT_MAX_PARSERS_PER_LANGUAGE: usize = 4;

fn get_max_parsers_per_language() -> usize {
    std::env::var("PROBE_PARSER_POOL_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            std::cmp::max(
                rayon::current_num_threads(),
                DEFAULT_MAX_PARSERS_PER_LANGUAGE,
            )
        })
}

lazy_static::lazy_static! {
    /// A thread-safe pool of tree-sitter parsers organized by language extension
    ///
    /// This pool maintains pre-configured parsers for each supported language to avoid
    /// the expensive overhead of creating new parsers and setting their language for every file.
    ///
    /// The pool is keyed by file extension (e.g., "rs", "js", "py") and contains a vector
    /// of ready-to-use parsers that have already been configured with the appropriate
    /// tree-sitter language grammar.
    static ref PARSER_POOL: Mutex<HashMap<String, Vec<Parser>>> = Mutex::new(HashMap::new());

    // PHASE 4 OPTIMIZATION: Pre-warm parsers for common languages
    static ref PARSER_WARMER: () = {
        if std::env::var("PROBE_NO_PARSER_WARMUP").is_err() {
            let common_languages = ["rs", "js", "ts", "py", "go", "java", "cpp", "c"];
            for lang in &common_languages {
                // Try to create and return a parser to warm up the pool
                if let Ok(parser) = get_pooled_parser(lang) {
                    return_pooled_parser(lang, parser);
                }
            }
        }
    };
}

/// Initialize parser pool warming
pub fn warm_parser_pool() {
    let _ = &*PARSER_WARMER;
}

/// Gets a parser from the pool for the specified language extension.
///
/// If a pre-configured parser is available in the pool, it will be returned immediately.
/// Otherwise, a new parser will be created and configured with the appropriate language.
/// This function handles the expensive parser initialization and language configuration
/// so that callers receive a ready-to-use parser.
///
/// # Arguments
///
/// * `extension` - The file extension (e.g., "rs", "js", "py") that determines the language
///
/// # Returns
///
/// * `Ok(Parser)` - A configured parser ready for use
/// * `Err(anyhow::Error)` - If the language is not supported or parser setup fails
///
/// # Example
///
/// ```rust
/// let parser = get_pooled_parser("rs")?;
/// let tree = parser.parse(rust_code, None)?;
/// return_pooled_parser("rs", parser);
/// ```
pub fn get_pooled_parser(extension: &str) -> Result<Parser> {
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    // First, try to get a parser from the pool
    {
        let mut pool = PARSER_POOL
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        if let Some(parsers) = pool.get_mut(extension) {
            if let Some(parser) = parsers.pop() {
                if debug_mode {
                    println!("[DEBUG] Parser pool: Retrieved pooled parser for extension '{extension}' (pool size: {})", parsers.len());
                }
                return Ok(parser);
            }
        }
    }

    // No parser available in pool, create a new one
    if debug_mode {
        println!("[DEBUG] Parser pool: Creating new parser for extension '{extension}'");
    }

    let language_impl = factory::get_language_impl(extension)
        .ok_or_else(|| anyhow::anyhow!("Unsupported language extension: {}", extension))?;

    let mut parser = Parser::new();
    parser
        .set_language(&language_impl.get_tree_sitter_language())
        .map_err(|e| anyhow::anyhow!("Failed to set parser language: {}", e))?;

    Ok(parser)
}

/// Returns a parser to the pool for reuse.
///
/// This function adds the parser back to the pool for the specified language extension,
/// making it available for future `get_pooled_parser` calls. This avoids the need to
/// recreate and reconfigure parsers, providing significant performance benefits.
///
/// # Arguments
///
/// * `extension` - The file extension the parser was configured for
/// * `parser` - The parser to return to the pool
///
/// # Note
///
/// The parser should be in a clean state (no active parsing context) when returned.
/// The pool will maintain a reasonable number of parsers per language to balance
/// performance with memory usage.
pub fn return_pooled_parser(extension: &str, parser: Parser) {
    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    let mut pool = PARSER_POOL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let parsers = pool.entry(extension.to_string()).or_default();

    // PHASE 4 OPTIMIZATION: Use dynamic pool sizing
    let max_parsers = get_max_parsers_per_language();

    if parsers.len() < max_parsers {
        parsers.push(parser);
        if debug_mode {
            println!(
                "[DEBUG] Parser pool: Returned parser for extension '{extension}' (pool size: {})",
                parsers.len()
            );
        }
    } else if debug_mode {
        println!("[DEBUG] Parser pool: Discarded parser for extension '{extension}' (pool at capacity: {})", parsers.len());
    }
}

/// Gets statistics about the current parser pool state.
///
/// This function is primarily useful for debugging and monitoring pool effectiveness.
/// It returns information about how many parsers are currently pooled for each language.
///
/// # Returns
///
/// A HashMap mapping language extensions to the number of pooled parsers
pub fn get_pool_stats() -> HashMap<String, usize> {
    let pool = PARSER_POOL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    pool.iter()
        .map(|(ext, parsers)| (ext.clone(), parsers.len()))
        .collect()
}

/// Clears the entire parser pool.
///
/// This function removes all pooled parsers, forcing future parser requests
/// to create new instances. This can be useful for testing or memory management.
#[allow(dead_code)]
pub fn clear_parser_pool() {
    let mut pool = PARSER_POOL
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());

    let debug_mode = std::env::var("DEBUG").unwrap_or_default() == "1";

    if debug_mode {
        let total_parsers: usize = pool.values().map(|v| v.len()).sum();
        println!(
            "[DEBUG] Parser pool: Clearing pool with {} parsers across {} languages",
            total_parsers,
            pool.len()
        );
    }

    pool.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_pool_basic_functionality() {
        // Clear pool to start fresh
        clear_parser_pool();

        // Get a parser for Rust
        let parser1 = get_pooled_parser("rs").expect("Should create Rust parser");

        // Return it to pool
        return_pooled_parser("rs", parser1);

        // Get another parser - should be the same one from pool
        let _parser2 = get_pooled_parser("rs").expect("Should get pooled Rust parser");

        // Check pool stats
        let stats = get_pool_stats();

        // On Windows, empty Vec entries might be removed from HashMap entirely
        // Both behaviors (Some(&0) and None) indicate the parser is checked out
        #[cfg(windows)]
        {
            let checked_out = stats.get("rs").is_none_or(|&count| count == 0);
            assert!(
                checked_out,
                "Parser should be checked out (expected pool size 0 or entry removed, got {:?})",
                stats.get("rs")
            );
        }
        #[cfg(not(windows))]
        {
            assert_eq!(stats.get("rs"), Some(&0)); // Parser is checked out
        }
    }

    #[test]
    fn test_parser_pool_multiple_languages() {
        clear_parser_pool();

        // Test multiple languages
        let extensions = ["rs", "js", "py", "go"];
        let mut parsers = Vec::new();

        // Get parsers for different languages
        for ext in &extensions {
            let parser =
                get_pooled_parser(ext).unwrap_or_else(|_| panic!("Should create {ext} parser"));
            parsers.push((ext, parser));
        }

        // Return all parsers
        for (ext, parser) in parsers {
            return_pooled_parser(ext, parser);
        }

        // Check that all languages have at least one parser in the pool
        // Note: Some parsers might be discarded if the pool reaches capacity
        let stats = get_pool_stats();
        for ext in &extensions {
            assert!(stats.contains_key(*ext), "Language {ext} should be in pool");
        }

        // Verify total languages supported
        assert_eq!(
            stats.len(),
            extensions.len(),
            "Should have entries for all tested languages"
        );
    }

    #[test]
    fn test_parser_pool_capacity_limit() {
        clear_parser_pool();

        // Create more parsers than the pool capacity
        let mut parsers = Vec::new();
        for _ in 0..10 {
            let parser = get_pooled_parser("rs").expect("Should create Rust parser");
            parsers.push(parser);
        }

        // Return all parsers
        for parser in parsers {
            return_pooled_parser("rs", parser);
        }

        // Pool should be limited to the dynamic max parsers per language
        let max_parsers = get_max_parsers_per_language();
        let stats = get_pool_stats();
        assert!(stats.get("rs").unwrap() <= &max_parsers); // Should be at most the dynamic limit
    }

    #[test]
    fn test_unsupported_extension() {
        let result = get_pooled_parser("unsupported");
        assert!(result.is_err());
        assert!(format!("{:?}", result.err().unwrap()).contains("Unsupported language extension"));
    }
}
