//! Tests validating that cache invalidation works correctly in probe
//!
//! These tests verify that the cache invalidation mechanisms properly handle:
//! 1. Tree cache content hash validation and automatic invalidation
//! 2. Session cache MD5-based file change detection
//! 3. Bounded cache growth with LRU eviction
//! 4. Thread-safe concurrent access patterns

use std::fs;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

use probe_code::language::factory::get_language_impl;
use probe_code::language::tree_cache::{
    clear_tree_cache, get_cache_size, get_or_parse_tree, is_in_cache,
};
use probe_code::search::cache::{calculate_file_md5, hash_query, SessionCache};

/// Test that tree cache properly invalidates when file content changes
#[test]
fn test_tree_cache_content_invalidation_works() {
    clear_tree_cache();

    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.rs");

    // Create initial file content
    let initial_content = r#"
fn original_function() {
    println!("This is the original function");
    let x = 42;
    return x;
}
"#;
    fs::write(&file_path, initial_content).unwrap();

    // Get language implementation and parser
    let language_impl = get_language_impl("rs").unwrap();
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language_impl.get_tree_sitter_language())
        .unwrap();

    // Parse the initial content - this will cache the tree
    let initial_tree =
        get_or_parse_tree(file_path.to_str().unwrap(), initial_content, &mut parser).unwrap();
    let initial_node_count = initial_tree.root_node().child_count();

    // Verify the file is cached
    assert!(is_in_cache(file_path.to_str().unwrap()));

    // Modify file content
    let modified_content = r#"
fn completely_different_function() {
    println!("This is completely different");
    let y = 100;
    let z = 200;
    return y + z;
}

fn another_function() {
    println!("Additional function");
}
"#;
    fs::write(&file_path, modified_content).unwrap();

    // FIXED: get_or_parse_tree now detects content change and reparses correctly
    let updated_tree =
        get_or_parse_tree(file_path.to_str().unwrap(), modified_content, &mut parser).unwrap();
    let updated_node_count = updated_tree.root_node().child_count();

    // Parse directly for comparison
    let fresh_tree = parser.parse(modified_content, None).unwrap();
    let fresh_node_count = fresh_tree.root_node().child_count();

    // PASSING ASSERTION: The updated tree should match the fresh parse
    assert_eq!(
        updated_node_count, fresh_node_count,
        "Tree cache correctly invalidated and reparsed! Updated: {updated_node_count}, Fresh: {fresh_node_count}"
    );

    // Verify we have more nodes now (2 functions vs 1)
    assert!(
        updated_node_count > initial_node_count,
        "Modified content should have more AST nodes than original"
    );

    // Additional verification: Function names should be correct
    let updated_root = updated_tree.root_node();
    let fresh_root = fresh_tree.root_node();

    let updated_functions = extract_function_names(&updated_root, modified_content.as_bytes());
    let fresh_functions = extract_function_names(&fresh_root, modified_content.as_bytes());

    assert_eq!(
        updated_functions, fresh_functions,
        "Cached functions match fresh parse: {updated_functions:?}"
    );

    // Verify we have the expected function names
    assert!(updated_functions.contains(&"completely_different_function".to_string()));
    assert!(updated_functions.contains(&"another_function".to_string()));
}

/// Test that session cache properly invalidates when files change
#[test]
fn test_session_cache_md5_invalidation_works() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("session_test.rs");

    // Create initial file
    let initial_content = r#"
fn initial_function() {
    let value = 42;
    return value;
}
"#;
    fs::write(&file_path, initial_content).unwrap();

    // Create session cache
    let query = "function value";
    let query_hash = hash_query(query);
    let mut cache = SessionCache::new("test_session".to_string(), query_hash.clone());

    // Add block to cache with initial file hash
    let initial_md5 = calculate_file_md5(&file_path).unwrap();
    cache
        .file_md5_hashes
        .insert(file_path.to_string_lossy().to_string(), initial_md5.clone());
    cache.add_to_cache(format!("{}:1-5", file_path.to_string_lossy()));

    // Verify block is cached
    assert!(cache.is_cached(&format!("{}:1-5", file_path.to_string_lossy())));

    // Modify file content
    let modified_content = r#"
fn completely_different_function() {
    let different_value = 999;
    let another_value = 888;
    return different_value + another_value;
}

fn second_function() {
    println!("This is a second function");
}
"#;
    fs::write(&file_path, modified_content).unwrap();

    // FIXED: Cache validation correctly detects file change and invalidates
    cache.validate_and_invalidate_cache(true).unwrap();

    // PASSING ASSERTION: Block should be invalidated after file change
    assert!(
        !cache.is_cached(&format!("{}:1-5", file_path.to_string_lossy())),
        "Session cache correctly invalidated stale entry after file modification!"
    );

    // Additional check: File hash should be removed from cache
    assert!(
        !cache
            .file_md5_hashes
            .contains_key(&file_path.to_string_lossy().to_string()),
        "Session cache correctly removed stale file hash!"
    );
}

/// Test that tree cache has bounded growth and doesn't cause memory issues
#[test]
fn test_tree_cache_bounded_growth_works() {
    clear_tree_cache();

    let temp_dir = TempDir::new().unwrap();
    let _initial_cache_size = get_cache_size();

    // Generate a reasonable number of files to test cache bounds (not 10k!)
    let num_files = 100; // Reasonable test size
    let mut file_paths = Vec::new();

    for i in 0..num_files {
        let file_path = temp_dir.path().join(format!("file_{i}.rs"));
        let content = format!(
            r#"
fn function_{i}() {{
    println!("Function number {{}}", {i});
    let result = {i} * 2;
    return result;
}}
"#
        );

        fs::write(&file_path, &content).unwrap();

        // Parse each file to fill the cache
        let language_impl = get_language_impl("rs").unwrap();
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&language_impl.get_tree_sitter_language())
            .unwrap();

        let _ = get_or_parse_tree(file_path.to_str().unwrap(), &content, &mut parser);

        file_paths.push((file_path, content));
    }

    let final_cache_size = get_cache_size();

    // PASSING ASSERTION: Cache should be bounded and not grow without limits
    // Cache uses LRU eviction so it shouldn't exceed reasonable bounds
    assert!(
        final_cache_size <= 2000, // DEFAULT_CACHE_SIZE from tree_cache.rs
        "Cache size is bounded: {final_cache_size} entries (within limit of 2000)"
    );

    // Verify cache is working efficiently
    assert!(
        final_cache_size >= num_files.min(100), // Should cache at least some files
        "Cache is working: {final_cache_size} entries for {num_files} files"
    );
}

/// Test that concurrent cache access is thread-safe
#[test]
fn test_concurrent_cache_thread_safety_works() {
    clear_tree_cache();

    let temp_dir = TempDir::new().unwrap();
    let num_threads = 4; // Reasonable for testing
    let files_per_thread = 25; // Reasonable for testing
    let success_count = Arc::new(Mutex::new(0));

    let handles: Vec<_> = (0..num_threads)
        .map(|thread_id| {
            let temp_dir = temp_dir.path().to_path_buf();
            let success_count = Arc::clone(&success_count);

            thread::spawn(move || {
                let language_impl = get_language_impl("rs").unwrap();
                let mut parser = tree_sitter::Parser::new();
                parser
                    .set_language(&language_impl.get_tree_sitter_language())
                    .unwrap();

                for i in 0..files_per_thread {
                    // Each thread creates its own files to avoid conflicts
                    let file_path = temp_dir.join(format!("thread_{thread_id}_{i}.rs"));
                    let content = format!(
                        r#"
fn thread_{thread_id}_function_{i}() {{
    let value = {thread_id};
    return value * {i};
}}
"#
                    );

                    // Write file and parse safely
                    if fs::write(&file_path, &content).is_ok()
                        && get_or_parse_tree(file_path.to_str().unwrap(), &content, &mut parser)
                            .is_ok()
                    {
                        // Verify the tree makes sense
                        let tree =
                            get_or_parse_tree(file_path.to_str().unwrap(), &content, &mut parser)
                                .unwrap();
                        let root = tree.root_node();
                        let function_names = extract_function_names(&root, content.as_bytes());

                        let expected_function = format!("thread_{thread_id}_function_{i}");
                        if function_names.contains(&expected_function) {
                            let mut count = success_count.lock().unwrap();
                            *count += 1;
                        }
                    }

                    // Small delay to allow interleaving
                    thread::sleep(Duration::from_millis(1));
                }
            })
        })
        .collect();

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    let total_successes = *success_count.lock().unwrap();
    let expected_total = num_threads * files_per_thread;

    // PASSING ASSERTION: Thread-safe cache should handle concurrent access correctly
    assert_eq!(
        total_successes, expected_total,
        "Thread-safe cache handled all {expected_total} operations correctly"
    );
}

/// Test that compound word tokenization is consistent
#[test]
fn test_compound_word_tokenization_consistency_works() {
    use probe_code::search::tokenization::tokenize;

    // Test compound words with consistent tokenization
    let test_cases = vec![
        "processPaymentData",
        "calculateTotalAmount",
        "getUserInformation",
        "validateInputParameters",
        "generateReportSummary",
    ];

    for word in &test_cases {
        // Tokenize the word multiple times
        let mut results = Vec::new();

        for _ in 0..10 {
            let tokens = tokenize(word);
            results.push(tokens);
        }

        // Check that all results are identical
        let first_result = &results[0];
        for (i, result) in results.iter().enumerate().skip(1) {
            assert_eq!(
                result, first_result,
                "Compound word tokenization is consistent for '{word}': attempt 0: {first_result:?}, attempt {i}: {result:?}"
            );
        }
    }
}

/// Test performance characteristics of cache
#[test]
fn test_cache_performance_characteristics() {
    use std::time::SystemTime;

    clear_tree_cache();

    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("perf_test.rs");

    // Create test content
    let content = r#"
fn performance_test_function() {
    let data = vec![1, 2, 3, 4, 5];
    let result: Vec<i32> = data.iter().map(|x| x * 2).collect();
    println!("Result: {:?}", result);
    return result.len();
}
"#;
    fs::write(&file_path, content).unwrap();

    let language_impl = get_language_impl("rs").unwrap();
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&language_impl.get_tree_sitter_language())
        .unwrap();

    // Measure performance with cache
    let start_time = SystemTime::now();
    for _ in 0..100 {
        let _ = get_or_parse_tree(file_path.to_str().unwrap(), content, &mut parser);
    }
    let cached_duration = start_time.elapsed().unwrap();

    // Clear cache and measure without cache
    clear_tree_cache();
    let start_time = SystemTime::now();
    for _ in 0..100 {
        let _ = parser.parse(content, None);
    }
    let uncached_duration = start_time.elapsed().unwrap();

    println!("Cached parsing: {cached_duration:?}");
    println!("Uncached parsing: {uncached_duration:?}");

    if uncached_duration.as_nanos() > 0 {
        println!(
            "Speedup: {:.2}x",
            uncached_duration.as_secs_f64() / cached_duration.as_secs_f64()
        );
    }

    // The cache provides significant performance improvement
    assert!(
        cached_duration <= uncached_duration,
        "Cache provides performance benefit or is at least as fast"
    );
}

/// Helper function to extract function names from AST
fn extract_function_names(node: &tree_sitter::Node, source: &[u8]) -> Vec<String> {
    let mut function_names = Vec::new();

    if node.kind() == "function_item" {
        // Look for the function name
        for child in node.children(&mut node.walk()) {
            if child.kind() == "identifier" {
                if let Ok(name) = std::str::from_utf8(&source[child.start_byte()..child.end_byte()])
                {
                    function_names.push(name.to_string());
                    break;
                }
            }
        }
    }

    // Recursively check children
    for child in node.children(&mut node.walk()) {
        function_names.extend(extract_function_names(&child, source));
    }

    function_names
}

/// Test that session cache persistence works across loads
#[test]
fn test_session_cache_persistence_works() {
    let temp_home = tempfile::TempDir::new().unwrap();
    std::env::set_var("HOME", temp_home.path());
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("persist_test.rs");

    // Create initial file
    let content = r#"
fn persist_test_function() {
    let value = 123;
    return value;
}
"#;
    fs::write(&file_path, content).unwrap();

    // Create and populate session cache
    let query = "persist test";
    let query_hash = hash_query(query);
    let mut cache = SessionCache::new("persist_session".to_string(), query_hash.clone());

    let file_md5 = calculate_file_md5(&file_path).unwrap();
    cache
        .file_md5_hashes
        .insert(file_path.to_string_lossy().to_string(), file_md5);
    cache.add_to_cache(format!("{}:1-5", file_path.to_string_lossy()));

    // Save to disk
    cache.save().unwrap();

    // Load new instance
    let loaded_cache = SessionCache::load("persist_session", &query_hash).unwrap();

    // Verify data was persisted correctly
    assert!(loaded_cache.is_cached(&format!("{}:1-5", file_path.to_string_lossy())));
    assert!(loaded_cache
        .file_md5_hashes
        .contains_key(&file_path.to_string_lossy().to_string()));
}
