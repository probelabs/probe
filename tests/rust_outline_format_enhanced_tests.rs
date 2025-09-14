use anyhow::Result;
use std::fs;
use tempfile::TempDir;

mod common;
use common::TestContext;

#[test]
fn test_rust_outline_smart_closing_brace_comments() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("smart_braces.rs");

    let content = r#"/// Small function that should NOT get closing brace comments.
pub fn small_function(x: i32) -> i32 {
    let result = x * 2;
    result + 1
}

/// Large function that SHOULD get closing brace comments when there are gaps.
pub fn large_function_with_gaps(data: Vec<i32>) -> Vec<String> {
    let mut results = Vec::new();
    let mut processor = DataProcessor::new();

    // Phase 1: Initial processing
    for (index, value) in data.iter().enumerate() {
        if value > &100 {
            processor.process_large_value(value, index);
        } else if value < &0 {
            processor.process_negative_value(value, index);
        } else {
            processor.process_small_value(value, index);
        }
    }

    // Phase 2: Complex transformation logic
    let transformed_data = processor.get_transformed_data();
    for item in transformed_data {
        match item.category {
            Category::High => {
                results.push(format!("HIGH: {}", item.value));
            }
            Category::Medium => {
                results.push(format!("MED: {}", item.value));
            }
            Category::Low => {
                results.push(format!("LOW: {}", item.value));
            }
        }
    }

    // Phase 3: Final validation and cleanup
    let mut validated_results = Vec::new();
    for result in results {
        if result.len() > 5 {
            validated_results.push(result);
        }
    }

    validated_results
}

/// Another large function to test closing brace behavior
pub fn another_large_function(items: &[Item]) -> ProcessedResult {
    let mut accumulator = Accumulator::new();

    // Main processing loop with complex nested logic
    for item in items {
        match item.item_type {
            ItemType::Primary => {
                if item.weight > 50.0 {
                    accumulator.add_heavy_primary(item);
                } else {
                    accumulator.add_light_primary(item);
                }
            }
            ItemType::Secondary => {
                accumulator.add_secondary(item);
            }
            ItemType::Auxiliary => {
                accumulator.add_auxiliary(item);
            }
        }
    }

    accumulator.finalize()
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "large_function", // Search for large functions
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should find the large functions
    assert!(
        output.contains("large_function_with_gaps") || output.contains("another_large_function"),
        "Missing large functions - output: {}",
        output
    );

    // Should have closing brace comments for large functions with gaps
    let has_closing_brace_comment = output.contains("} //") || output.contains("} /*");
    assert!(
        has_closing_brace_comment,
        "Large functions should have closing brace comments - output: {}",
        output
    );

    // Should be in outline format
    assert!(
        output.contains("---\nFile:"),
        "Missing file delimiter in outline format - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_rust_outline_array_truncation_with_keyword_preservation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("large_arrays.rs");

    let content = r#"use std::collections::HashMap;

/// Function containing large array that should be truncated but preserve keywords.
pub fn process_large_dataset() -> Vec<String> {
    let large_configuration = vec![
        "database_connection_string",
        "api_key_primary",
        "api_key_secondary",
        "cache_timeout_value",
        "retry_attempt_count",
        "batch_size_limit",
        "queue_max_capacity",
        "worker_thread_count",
        "memory_allocation_limit",
        "disk_space_threshold",
        "network_timeout_value",
        "authentication_token_lifetime",
        "session_expiry_duration",
        "log_rotation_interval",
        "backup_retention_period",
        "monitoring_check_interval",
        "alert_notification_threshold",
        "performance_metrics_collection",
        "security_audit_frequency",
        "data_encryption_algorithm",
        "compression_ratio_target",
        "indexing_strategy_preference",
        "query_optimization_level",
        "connection_pool_sizing",
        "load_balancer_configuration",
        "failover_mechanism_timeout",
        "disaster_recovery_checkpoint",
        "data_replication_strategy",
        "cache_invalidation_policy",
        "resource_allocation_priority"
    ];

    let mut results = Vec::new();
    for config_item in large_configuration {
        if config_item.contains("api_key") {
            results.push(format!("SECURE: {}", config_item));
        } else if config_item.contains("timeout") {
            results.push(format!("TIMING: {}", config_item));
        } else {
            results.push(format!("CONFIG: {}", config_item));
        }
    }

    results
}

/// Function with large struct initialization that should be truncated.
pub fn create_large_configuration() -> Config {
    let config = Config {
        database_host: "localhost".to_string(),
        database_port: 5432,
        database_name: "production_db".to_string(),
        connection_timeout: Duration::from_secs(30),
        max_connections: 100,
        idle_timeout: Duration::from_secs(300),
        query_timeout: Duration::from_secs(60),
        ssl_enabled: true,
        ssl_cert_path: "/etc/ssl/certs/db.pem".to_string(),
        ssl_key_path: "/etc/ssl/private/db.key".to_string(),
        backup_enabled: true,
        backup_interval: Duration::from_secs(3600),
        backup_retention_days: 30,
        log_level: LogLevel::Info,
        log_file_path: "/var/log/app.log".to_string(),
        max_log_file_size: 100_000_000,
        api_key: "super_secret_api_key_here".to_string(),
        api_rate_limit: 1000,
        cache_size: 1024 * 1024 * 100,
        cache_ttl: Duration::from_secs(3600),
        feature_flags: FeatureFlags {
            enable_new_feature_alpha: true,
            enable_experimental_optimization: false,
            enable_beta_ui: true,
            enable_advanced_analytics: true,
        },
        monitoring: MonitoringConfig {
            metrics_enabled: true,
            traces_enabled: true,
            logs_enabled: true,
            sample_rate: 0.1,
        }
    };

    config
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "api_key", // Search for keyword that appears in large structures
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should find the functions containing api_key
    assert!(
        output.contains("process_large_dataset") || output.contains("create_large_configuration"),
        "Missing functions with api_key - output: {}",
        output
    );

    // Should contain the search keyword even in truncated output
    assert!(
        output.contains("api_key"),
        "Should preserve api_key keyword even in truncated arrays - output: {}",
        output
    );

    // Should show truncation with ellipsis for large arrays
    assert!(
        output.contains("..."),
        "Should show ellipsis for truncated large arrays - output: {}",
        output
    );

    // Should be reasonably sized (not show the full large array)
    let line_count = output.lines().count();
    assert!(
        line_count < 100,
        "Output should be truncated to reasonable size, got {} lines",
        line_count
    );

    Ok(())
}

#[test]
fn test_rust_outline_complex_generics_and_lifetimes() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("complex_rust.rs");

    let content = r#"use std::collections::HashMap;
use std::marker::PhantomData;

/// Complex generic struct with lifetimes and type bounds.
pub struct ComplexProcessor<'a, T, U>
where
    T: Clone + Send + Sync + 'static,
    U: AsRef<str> + Into<String>,
{
    data: &'a [T],
    lookup: HashMap<String, U>,
    phantom: PhantomData<T>,
}

impl<'a, T, U> ComplexProcessor<'a, T, U>
where
    T: Clone + Send + Sync + 'static,
    U: AsRef<str> + Into<String>,
{
    /// Create a new processor with lifetime and generic constraints.
    pub fn new(data: &'a [T]) -> Self {
        Self {
            data,
            lookup: HashMap::new(),
            phantom: PhantomData,
        }
    }

    /// Async function with complex generic processing.
    pub async fn process_with_constraints<F, R>(&mut self, processor_func: F) -> Result<Vec<R>, ProcessError>
    where
        F: Fn(&T) -> Result<R, ProcessError> + Send + Sync,
        R: Clone + Send + 'static,
    {
        let mut results = Vec::new();

        for item in self.data.iter() {
            match processor_func(item) {
                Ok(result) => {
                    results.push(result);
                }
                Err(e) => {
                    log::error!("Processing error: {:?}", e);
                    return Err(e);
                }
            }
        }

        Ok(results)
    }
}

/// Trait with associated types and complex bounds.
pub trait DataProcessor<'a, T>: Send + Sync
where
    T: Clone + Send + Sync,
{
    type Output: Clone + Send;
    type Error: std::error::Error + Send + Sync + 'static;

    /// Process data with lifetime constraints.
    fn process_data(&self, data: &'a [T]) -> Result<Self::Output, Self::Error>;

    /// Async processing method with complex constraints.
    async fn async_process<F>(&self, data: &'a [T], mapper: F) -> Result<Vec<Self::Output>, Self::Error>
    where
        F: Fn(&T) -> Result<Self::Output, Self::Error> + Send + Sync;
}

/// Function with higher-ranked trait bounds (HRTB).
pub fn apply_closure<F>(data: &[String], func: F) -> Vec<String>
where
    F: for<'a> Fn(&'a str) -> String,
{
    data.iter()
        .map(|s| func(s.as_str()))
        .collect()
}

/// Complex match expression with patterns and guards.
pub fn pattern_matching_example<T>(value: Option<Result<T, String>>) -> String
where
    T: std::fmt::Display,
{
    match value {
        Some(Ok(val)) if val.to_string().len() > 5 => {
            format!("Long value: {}", val)
        }
        Some(Ok(val)) => {
            format!("Short value: {}", val)
        }
        Some(Err(e)) if e.contains("critical") => {
            format!("Critical error: {}", e)
        }
        Some(Err(e)) => {
            format!("Error: {}", e)
        }
        None => {
            "No value provided".to_string()
        }
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "ComplexProcessor", // Search for the complex struct
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should find complex Rust constructs
    assert!(
        output.contains("ComplexProcessor") || output.contains("where"),
        "Missing complex generic structures - output: {}",
        output
    );

    // Should handle complex generic syntax properly in outline format
    assert!(
        output.contains("---\nFile:"),
        "Missing file delimiter in outline format - output: {}",
        output
    );

    // Should be in outline format with proper structure
    assert!(
        output.contains("Found") && output.contains("search results"),
        "Missing search results summary - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_rust_outline_nested_control_flow_closing_braces() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("nested_control_flow.rs");

    let content = r#"/// Function with deeply nested control flow structures.
pub fn deeply_nested_processing(data: &[ProcessingItem]) -> ProcessingResult {
    let mut result = ProcessingResult::new();

    // Level 1: Main iteration
    for (outer_index, item) in data.iter().enumerate() {
        if item.is_valid() {
            // Level 2: Category processing
            match item.category {
                Category::TypeA => {
                    // Level 3: Sub-category processing
                    for sub_item in &item.sub_items {
                        if sub_item.weight > 10.0 {
                            // Level 4: Weight-based processing
                            match sub_item.processing_type {
                                ProcessingType::Intensive => {
                                    // Level 5: Intensive processing branch
                                    if sub_item.requires_validation {
                                        while !sub_item.is_validated() {
                                            sub_item.validate();
                                            if sub_item.validation_attempts > MAX_ATTEMPTS {
                                                break;
                                            }
                                        }
                                    }
                                    result.add_intensive_result(sub_item.process());
                                }
                                ProcessingType::Standard => {
                                    result.add_standard_result(sub_item.process());
                                }
                                ProcessingType::Quick => {
                                    result.add_quick_result(sub_item.quick_process());
                                }
                            }
                        } else {
                            // Light-weight item processing
                            result.add_lightweight_result(sub_item.lightweight_process());
                        }
                    }
                }
                Category::TypeB => {
                    // Different processing path for TypeB
                    for (index, sub_item) in item.sub_items.iter().enumerate() {
                        if index % 2 == 0 {
                            // Even index processing
                            result.add_even_result(sub_item.process_even());
                        } else {
                            // Odd index processing
                            result.add_odd_result(sub_item.process_odd());
                        }
                    }
                }
                Category::TypeC => {
                    // TypeC has special processing requirements
                    let mut batch_processor = BatchProcessor::new();
                    loop {
                        let batch = item.get_next_batch();
                        if batch.is_empty() {
                            break;
                        }

                        for batch_item in batch {
                            if batch_item.needs_special_handling() {
                                match batch_item.special_type {
                                    SpecialType::Critical => {
                                        result.add_critical_result(batch_item.critical_process());
                                    }
                                    SpecialType::Urgent => {
                                        result.add_urgent_result(batch_item.urgent_process());
                                    }
                                    SpecialType::Normal => {
                                        result.add_normal_result(batch_item.normal_process());
                                    }
                                }
                            }
                        }

                        batch_processor.process_batch(batch);
                    }
                }
            }
        } else {
            // Invalid item handling
            result.add_error(format!("Invalid item at index {}", outer_index));
        }
    }

    result
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "nested_processing", // Search for the function
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should find the deeply nested function
    assert!(
        output.contains("deeply_nested_processing"),
        "Missing deeply nested function - output: {}",
        output
    );

    // The function should be shown with all its nested structure
    // Note: Closing brace comments only appear when there are gaps (ellipsis) in the outline
    // If the entire function is shown, no closing brace comments are needed
    let has_complete_function = !output.contains("...");
    let has_closing_brace_comments = output.contains("} //") || output.contains("} /*");

    // Either we should see closing brace comments (if there are gaps)
    // OR we should see the complete function (if no gaps)
    assert!(
        has_closing_brace_comments || has_complete_function,
        "Should either have closing brace comments (with gaps) or show complete function (no gaps) - output: {}",
        output
    );

    // Should show nested control flow keywords
    let control_flow_count = output.matches("for ").count()
        + output.matches("if ").count()
        + output.matches("match ").count()
        + output.matches("while ").count()
        + output.matches("loop").count();
    assert!(
        control_flow_count >= 3,
        "Should show multiple nested control flow keywords - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_rust_outline_small_functions_no_closing_brace_comments() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("small_functions.rs");

    let content = r#"/// Small helper function - should not have closing brace comments.
pub fn small_helper(x: i32) -> i32 {
    x * 2
}

/// Another small function - also should not have closing brace comments.
pub fn another_small_function(s: &str) -> String {
    s.to_uppercase()
}

/// Small function with a few lines - still should not have closing brace comments.
pub fn small_with_few_lines(data: &[i32]) -> i32 {
    let sum: i32 = data.iter().sum();
    let count = data.len() as i32;
    if count > 0 {
        sum / count
    } else {
        0
    }
}

/// Utility function - very small.
pub fn utility_function(a: i32, b: i32) -> i32 {
    a + b
}

/// Simple validation function.
pub fn validate_input(input: &str) -> bool {
    !input.is_empty() && input.len() < 100
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "small", // Search for small functions
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should find the small functions
    assert!(
        output.contains("small_helper") || output.contains("small_function"),
        "Missing small functions - output: {}",
        output
    );

    // Small functions should NOT have closing brace comments
    let has_closing_brace_comment = output.contains("} //") || output.contains("} /*");
    assert!(
        !has_closing_brace_comment,
        "Small functions should NOT have closing brace comments - output: {}",
        output
    );

    // Should be in outline format
    assert!(
        output.contains("---\nFile:"),
        "Missing file delimiter in outline format - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_rust_outline_keyword_highlighting_preservation() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("keyword_highlighting.rs");

    let content = r#"use std::sync::Arc;

/// Function containing the search term "synchronization" in multiple contexts.
pub fn advanced_synchronization_handler() -> SyncResult {
    let synchronization_config = SyncConfig {
        timeout: Duration::from_secs(30),
        retry_attempts: 3,
        enable_synchronization_logging: true,
    };

    // Synchronization logic with multiple references
    if synchronization_config.enable_synchronization_logging {
        log::info!("Starting synchronization process");
    }

    let sync_manager = SyncManager::new(synchronization_config);
    sync_manager.perform_synchronization()
}

/// Another function with synchronization in comments and code.
pub fn background_synchronization_worker() {
    // This function handles background synchronization tasks
    // The synchronization process is critical for data consistency

    loop {
        match synchronization_queue.pop() {
            Some(task) => {
                // Process synchronization task
                task.execute_synchronization();
            }
            None => {
                // No synchronization tasks pending
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

/// Complex function with synchronization in various contexts.
pub fn multi_context_synchronization() {
    let synchronization_items = vec![
        "user_data_synchronization",
        "config_synchronization",
        "cache_synchronization",
        "backup_synchronization"
    ];

    for item in synchronization_items {
        println!("Processing synchronization for: {}", item);
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "synchronization", // Search term that appears multiple times
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should find functions containing synchronization
    assert!(
        output.contains("synchronization_handler") || output.contains("synchronization_worker"),
        "Missing functions with synchronization - output: {}",
        output
    );

    // Should contain the search keyword multiple times (highlighted)
    let synchronization_count = output.matches("synchronization").count();
    assert!(
        synchronization_count >= 3,
        "Should preserve synchronization keyword multiple times - found {}, output: {}",
        synchronization_count,
        output
    );

    // Should be in outline format
    assert!(
        output.contains("---\nFile:"),
        "Missing file delimiter in outline format - output: {}",
        output
    );

    // Should have search results summary
    assert!(
        output.contains("Found") && output.contains("search results"),
        "Missing search results summary - output: {}",
        output
    );

    Ok(())
}
