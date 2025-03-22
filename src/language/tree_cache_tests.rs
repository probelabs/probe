use crate::language::tree_cache;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use tree_sitter::Parser;

// Create a test mutex for synchronization
lazy_static::lazy_static! {
    static ref TEST_MUTEX: Mutex<()> = Mutex::new(());
}

#[test]
fn test_tree_cache_basic() {
    // Acquire the test mutex to prevent concurrent test execution
    let _guard = TEST_MUTEX.lock().unwrap();
    // Clear the cache before starting the test
    tree_cache::clear_tree_cache();
    tree_cache::reset_cache_hit_counter();

    // Create a parser
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .unwrap();

    // Sample Rust code
    let content = r#"
        fn test_function() {
            println!("Hello, world!");
        }
    "#;

    // First parse - should be a cache miss
    let tree1 = tree_cache::get_or_parse_tree("test_file.rs", content, &mut parser).unwrap();

    // Verify cache hit count is still 0
    assert_eq!(tree_cache::get_cache_hit_count(), 0);

    // Second parse of the same content - should be a cache hit
    let tree2 = tree_cache::get_or_parse_tree("test_file.rs", content, &mut parser).unwrap();

    // Verify cache hit count is now 1
    assert_eq!(tree_cache::get_cache_hit_count(), 1);

    // Verify both trees have the same structure
    assert_eq!(tree1.root_node().kind(), tree2.root_node().kind());
    assert_eq!(
        tree1.root_node().start_position(),
        tree2.root_node().start_position()
    );
    assert_eq!(
        tree1.root_node().end_position(),
        tree2.root_node().end_position()
    );

    // Check that the cache has one entry
    assert_eq!(tree_cache::get_cache_size(), 1);
    assert!(tree_cache::is_in_cache("test_file.rs"));
}

#[test]
fn test_tree_cache_invalidation() {
    // Acquire the test mutex to prevent concurrent test execution
    let _guard = TEST_MUTEX.lock().unwrap();
    // Clear the cache before starting the test
    tree_cache::clear_tree_cache();

    // Create a parser
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .unwrap();

    // Sample Rust code
    let content1 = r#"
        fn test_function() {
            println!("Hello, world!");
        }
    "#;

    let content2 = r#"
        fn test_function() {
            println!("Hello, modified world!");
        }
    "#;

    // First parse
    let tree1 = tree_cache::get_or_parse_tree("test_file2.rs", content1, &mut parser).unwrap();

    // Verify cache has one entry
    assert_eq!(tree_cache::get_cache_size(), 1);
    assert!(tree_cache::is_in_cache("test_file2.rs"));

    // Parse with modified content - should invalidate cache and reparse
    let tree2 = tree_cache::get_or_parse_tree("test_file2.rs", content2, &mut parser).unwrap();

    // Verify trees have different structure
    assert_eq!(tree1.root_node().kind(), tree2.root_node().kind()); // Same kind (source_file)
    assert_eq!(
        tree1.root_node().start_position(),
        tree2.root_node().start_position()
    ); // Same start
    assert_eq!(
        tree1.root_node().end_position().row,
        tree2.root_node().end_position().row
    ); // Same number of rows

    // But the content is different, so the byte positions should differ
    assert_ne!(tree1.root_node().end_byte(), tree2.root_node().end_byte());

    // Check that the cache still has one entry (the updated one)
    assert_eq!(tree_cache::get_cache_size(), 1);
    assert!(tree_cache::is_in_cache("test_file2.rs"));
}

#[test]
fn test_tree_cache_clear() {
    // Acquire the test mutex to prevent concurrent test execution
    let _guard = TEST_MUTEX.lock().unwrap();
    // Clear the cache before starting the test
    tree_cache::clear_tree_cache();

    // Create a parser
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .unwrap();

    // Sample Rust code
    let content = r#"
        fn another_function() {
            println!("Testing cache clear");
        }
    "#;

    // Parse a file
    tree_cache::get_or_parse_tree("test_file3.rs", content, &mut parser).unwrap();

    // Parse a file to add an entry to the cache
    tree_cache::get_or_parse_tree("test_file3.rs", content, &mut parser).unwrap();

    // Verify cache has entries
    assert!(tree_cache::get_cache_size() > 0);

    // Clear the cache
    tree_cache::clear_tree_cache();

    // Verify cache is empty
    assert_eq!(tree_cache::get_cache_size(), 0);
}

#[test]
fn test_tree_cache_invalidate_entry() {
    // Acquire the test mutex to prevent concurrent test execution
    let _guard = TEST_MUTEX.lock().unwrap();
    // Clear the cache before starting the test
    tree_cache::clear_tree_cache();

    // Create a parser
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .unwrap();

    // Sample Rust code
    let content = r#"
        fn yet_another_function() {
            println!("Testing cache invalidation");
        }
    "#;

    // Parse a file to add an entry to the cache
    tree_cache::get_or_parse_tree("test_file4.rs", content, &mut parser).unwrap();

    // Verify cache has exactly one entry
    assert_eq!(tree_cache::get_cache_size(), 1);
    assert!(tree_cache::is_in_cache("test_file4.rs"));

    // Invalidate the specific entry
    tree_cache::invalidate_cache_entry("test_file4.rs");

    // Verify cache is now empty and the specific entry is gone
    assert_eq!(tree_cache::get_cache_size(), 0);
    assert!(!tree_cache::is_in_cache("test_file4.rs"));
}

#[test]
fn test_tree_cache_concurrent_access() {
    // Acquire the test mutex to prevent concurrent test execution
    let _guard = TEST_MUTEX.lock().unwrap();
    // Clear the cache before starting the test
    tree_cache::clear_tree_cache();

    // Create multiple threads that access the cache simultaneously
    let handles: Vec<_> = (0..5)
        .map(|i| {
            thread::spawn(move || {
                // Create a parser
                let mut parser = Parser::new();
                parser
                    .set_language(&tree_sitter_rust::LANGUAGE.into())
                    .unwrap();

                // Sample Rust code with thread-specific content
                let content = format!(
                    r#"
                    fn thread_function_{0}() {{
                        println!("Hello from thread {0}");
                    }}
                    "#,
                    i
                );

                // Parse in a loop to test concurrent access
                for j in 0..10 {
                    let file_name = format!("thread_{}_iteration_{}.rs", i, j);
                    tree_cache::get_or_parse_tree(&file_name, &content, &mut parser).unwrap();

                    // Small sleep to increase chance of thread interleaving
                    thread::sleep(Duration::from_millis(1));
                }
            })
        })
        .collect();

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify cache has entries (exact count depends on thread execution order)
    assert!(tree_cache::get_cache_size() > 0);

    // Clean up
    tree_cache::clear_tree_cache();
}
