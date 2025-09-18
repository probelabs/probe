use anyhow::Result;
use std::fs;
use tempfile::TempDir;

mod common;
use common::TestContext;

#[test]
fn test_rust_outline_basic_symbols() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("basic.rs");

    let content = r#"use std::collections::HashMap;

/// A simple calculator struct for arithmetic operations.
pub struct Calculator {
    pub name: String,
    history: Vec<f64>,
}

impl Calculator {
    /// Creates a new calculator instance.
    pub fn new(name: String) -> Self {
        Self {
            name,
            history: Vec::new(),
        }
    }

    /// Adds two numbers and returns the result.
    pub fn add(&mut self, x: f64, y: f64) -> f64 {
        let result = x + y;
        self.history.push(result);
        result
    }

    /// Gets the calculation history.
    pub fn get_history(&self) -> &[f64] {
        &self.history
    }
}

/// Trait for mathematical operations.
pub trait MathOperations {
    fn multiply(&self, a: f64, b: f64) -> f64;
    fn divide(&self, a: f64, b: f64) -> Option<f64>;
}

/// Enumeration of supported number types.
#[derive(Debug, Clone)]
pub enum NumberType {
    Integer(i64),
    Float(f64),
    Complex { real: f64, imaginary: f64 },
}

/// Main function to demonstrate calculator usage.
pub fn main() {
    let mut calc = Calculator::new("Test Calculator".to_string());
    let result = calc.add(10.0, 20.0);
    println!("Result: {}", result);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculator_add() {
        let mut calc = Calculator::new("Test".to_string());
        assert_eq!(calc.add(2.0, 3.0), 5.0);
    }

    #[test]
    fn test_calculator_history() {
        let mut calc = Calculator::new("Test".to_string());
        calc.add(1.0, 2.0);
        assert_eq!(calc.get_history().len(), 1);
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "Calculator", // Search term that will match multiple symbols
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    // Verify Rust symbols are extracted in outline format
    assert!(
        output.contains("pub struct Calculator"),
        "Missing Calculator struct - output: {}",
        output
    );
    assert!(
        output.contains("Calculator::new"),
        "Missing Calculator impl methods - output: {}",
        output
    );
    assert!(
        output.contains("pub fn main"),
        "Missing main function - output: {}",
        output
    );
    // In outline format, we should see the search results properly formatted
    assert!(
        output.contains("---\nFile:"),
        "Missing file delimiter - output: {}",
        output
    );
    // We expect multiple search results since Calculator appears in multiple places
    assert!(
        output.contains("Found") && output.contains("search results"),
        "Missing search results summary - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_rust_outline_control_flow_statements() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("control_flow.rs");

    let content = r#"use std::collections::HashMap;

/// Function demonstrating various control flow statements with gaps.
pub fn complex_algorithm(data: Vec<i32>, threshold: i32) -> HashMap<String, i32> {
    let mut result = HashMap::new();
    let mut counter = 0;

    // First processing phase
    for item in &data {
        if *item > threshold {
            counter += 1;

            // Complex nested conditions
            if counter % 2 == 0 {
                result.insert(format!("even_{}", counter), *item);
            } else {
                result.insert(format!("odd_{}", counter), *item);
            }
        }
    }

    // Second processing phase
    let mut index = 0;
    while index < data.len() {
        match data[index] {
            x if x < 0 => {
                result.insert(format!("negative_{}", index), x);
                index += 1;
            }
            x if x == 0 => {
                result.insert("zero".to_string(), 0);
                index += 1;
            }
            x => {
                result.insert(format!("positive_{}", index), x);
                index += 1;
            }
        }
    }

    result
}

/// Function with nested loops demonstrating closing brace comments.
pub fn process_matrix(matrix: Vec<Vec<i32>>) -> Vec<Vec<i32>> {
    let mut processed = Vec::new();

    for row in matrix.iter() {
        let mut new_row = Vec::new();

        for &cell in row.iter() {
            let processed_cell = if cell > 0 {
                cell * 2
            } else if cell < 0 {
                cell.abs()
            } else {
                1
            };

            new_row.push(processed_cell);
        }

        processed.push(new_row);
    }

    processed
}

/// Function with match statements and complex patterns.
pub fn analyze_data(input: &str) -> Result<String, String> {
    match input.trim() {
        "" => Err("Empty input".to_string()),
        s if s.len() > 100 => {
            Ok("Large input detected".to_string())
        }
        s if s.chars().all(|c| c.is_ascii_digit()) => {
            let num: i32 = s.parse().map_err(|e| format!("Parse error: {}", e))?;

            match num {
                0..=10 => Ok("Small number".to_string()),
                11..=100 => Ok("Medium number".to_string()),
                _ => Ok("Large number".to_string()),
            }
        }
        _ => Ok("Text input".to_string()),
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "algorithm", // Search for function names that will match
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Verify control flow functions are found
    assert!(
        output.contains("complex_algorithm") || output.contains("process_matrix"),
        "Missing control flow functions - output: {}",
        output
    );

    // Should contain control flow keywords highlighted in the outline
    let has_control_flow = output.contains("for ")
        || output.contains("while ")
        || output.contains("match ")
        || output.contains("if ");
    assert!(
        has_control_flow,
        "Missing control flow statements - output: {}",
        output
    );

    // Should contain closing braces for large blocks with gaps
    assert!(
        output.contains("}"),
        "Missing closing braces - output: {}",
        output
    );

    // Should be in outline format with file delimiter
    assert!(
        output.contains("---\nFile:"),
        "Missing file delimiter in outline format - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_rust_outline_macros_and_attributes() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("macros.rs");

    let content = r#"use serde::{Deserialize, Serialize};
use std::fmt;

/// Custom derive macro demonstration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Person {
    pub name: String,
    pub age: u32,
    pub email: String,
}

/// Custom attribute macro demonstration.
#[cfg(feature = "advanced")]
pub fn advanced_feature() -> String {
    "This is an advanced feature".to_string()
}

/// Procedural macro definition.
macro_rules! create_function {
    ($func_name:ident, $return_type:ty, $value:expr) => {
        pub fn $func_name() -> $return_type {
            $value
        }
    };
}

// Generate functions using macro
create_function!(get_pi, f64, 3.14159);
create_function!(get_greeting, String, "Hello, World!".to_string());
create_function!(get_answer, i32, 42);

/// Implementation with custom Display trait.
impl fmt::Display for Person {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (age: {}, email: {})", self.name, self.age, self.email)
    }
}

/// Generic implementation with constraints.
impl<T> From<T> for Person
where
    T: AsRef<str>,
{
    fn from(name: T) -> Self {
        Person {
            name: name.as_ref().to_string(),
            age: 0,
            email: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_person_creation() {
        let person = Person {
            name: "John".to_string(),
            age: 30,
            email: "john@example.com".to_string(),
        };
        assert_eq!(person.name, "John");
    }

    #[test]
    fn test_macro_generated_functions() {
        assert_eq!(get_pi(), 3.14159);
        assert_eq!(get_answer(), 42);
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "create_function", // Search for a term that will match
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    // Verify macro handling in outline format
    assert!(
        output.contains("macro_rules!") || output.contains("create_function"),
        "Missing macro definition - output: {}",
        output
    );

    // Should be in outline format with file delimiter
    assert!(
        output.contains("---\nFile:"),
        "Missing file delimiter in outline format - output: {}",
        output
    );

    // Should have found at least one result
    assert!(
        output.contains("Found") && output.contains("search results"),
        "Missing search results summary - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_rust_outline_async_and_errors() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("async_errors.rs");

    let content = r#"use std::error::Error;
use std::fmt;
use tokio::time::{sleep, Duration};

/// Custom error type for demonstration.
#[derive(Debug)]
pub enum ProcessingError {
    NetworkError(String),
    TimeoutError,
    ValidationError { field: String, message: String },
}

impl fmt::Display for ProcessingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProcessingError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            ProcessingError::TimeoutError => write!(f, "Operation timed out"),
            ProcessingError::ValidationError { field, message } => {
                write!(f, "Validation error in {}: {}", field, message)
            }
        }
    }
}

impl Error for ProcessingError {}

/// Async function with error handling.
pub async fn fetch_data(url: &str) -> Result<String, ProcessingError> {
    // Simulate network delay
    sleep(Duration::from_millis(100)).await;

    // Simulate validation
    if url.is_empty() {
        return Err(ProcessingError::ValidationError {
            field: "url".to_string(),
            message: "URL cannot be empty".to_string(),
        });
    }

    if url.len() > 1000 {
        return Err(ProcessingError::ValidationError {
            field: "url".to_string(),
            message: "URL too long".to_string(),
        });
    }

    // Simulate successful fetch
    Ok(format!("Data from {}", url))
}

/// Async function with complex error handling and retries.
pub async fn fetch_with_retry(url: &str, max_retries: u32) -> Result<String, ProcessingError> {
    let mut attempts = 0;

    while attempts < max_retries {
        match fetch_data(url).await {
            Ok(data) => return Ok(data),
            Err(ProcessingError::NetworkError(msg)) => {
                eprintln!("Attempt {} failed: {}", attempts + 1, msg);
                attempts += 1;

                if attempts < max_retries {
                    sleep(Duration::from_millis(1000)).await;
                }
            }
            Err(other_error) => return Err(other_error),
        }
    }

    Err(ProcessingError::TimeoutError)
}

/// Generic async function with bounds.
pub async fn process_items<T, E, F>(items: Vec<T>, processor: F) -> Result<Vec<String>, E>
where
    T: Send + Sync,
    E: Send,
    F: Fn(T) -> Result<String, E> + Send + Sync,
{
    let mut results = Vec::new();

    for item in items {
        match processor(item) {
            Ok(result) => results.push(result),
            Err(e) => return Err(e),
        }
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;

    #[tokio::test]
    async fn test_fetch_data_success() {
        let result = fetch_data("https://example.com").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_fetch_data_validation_error() {
        let result = fetch_data("").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_fetch_with_retry() {
        let result = fetch_with_retry("https://example.com", 3).await;
        assert!(result.is_ok());
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "fetch_data", // Search for a function name that exists
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    // Verify async functions are found in outline format
    assert!(
        output.contains("fetch_data") || output.contains("async"),
        "Missing async functions - output: {}",
        output
    );

    // Should be in outline format with file delimiter
    assert!(
        output.contains("---\nFile:"),
        "Missing file delimiter in outline format - output: {}",
        output
    );

    // Should have found at least one result
    assert!(
        output.contains("Found") && output.contains("search results"),
        "Missing search results summary - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_rust_outline_large_function_closing_braces() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("large_function.rs");

    // Create a large function with multiple control flow blocks and gaps
    let content = r#"/// Large function with multiple nested blocks to test closing brace comments.
pub fn complex_data_processor(data: Vec<i32>) -> Vec<String> {
    let mut results = Vec::new();
    let mut categories = std::collections::HashMap::new();

    // Phase 1: Categorization
    for (index, &value) in data.iter().enumerate() {
        let category = match value {
            x if x < 0 => "negative",
            0 => "zero",
            x if x < 100 => "small_positive",
            x if x < 1000 => "medium_positive",
            _ => "large_positive",
        };

        categories.entry(category.to_string())
            .or_insert_with(Vec::new)
            .push((index, value));
    }

    // Phase 2: Processing each category
    for (category, items) in categories {
        if category == "negative" {
            for (index, value) in items {
                results.push(format!("NEG[{}]: {}", index, value.abs()));
            }
        } else if category == "zero" {
            for (index, _) in items {
                results.push(format!("ZERO[{}]: neutral", index));
            }
        } else {
            // Positive number processing
            for (index, value) in items {
                let processed = if value < 10 {
                    format!("SINGLE_DIGIT[{}]: {}", index, value)
                } else if value < 100 {
                    format!("DOUBLE_DIGIT[{}]: {}", index, value)
                } else {
                    format!("MULTI_DIGIT[{}]: {}", index, value)
                };
                results.push(processed);
            }
        }
    }

    // Phase 3: Final sorting and validation
    results.sort();

    // Validation phase
    let mut validated_results = Vec::new();
    for result in results {
        if result.len() > 5 {
            validated_results.push(result);
        }
    }

    validated_results
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "complex_data_processor", // Search for the large function name
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Verify large function is shown with closing braces for major blocks
    assert!(
        output.contains("complex_data_processor"),
        "Missing complex_data_processor function - output: {}",
        output
    );

    // Should have a closing brace with comment for the large function
    let has_closing_brace_comment = output.contains("} //") || output.contains("} /*");
    assert!(
        has_closing_brace_comment,
        "Should have closing brace comment for large function - output: {}",
        output
    );

    // Should be in outline format with file delimiter
    assert!(
        output.contains("---\nFile:"),
        "Missing file delimiter in outline format - output: {}",
        output
    );

    Ok(())
}

#[test]
fn test_rust_outline_keyword_highlighting() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("keywords.rs");

    let content = r#"/// Function containing various Rust keywords for highlighting test.
pub fn keyword_demonstration() {
    // Keywords in variables and control structures
    let mut counter = 0;
    let const_value = 42;

    while counter < 10 {
        if counter % 2 == 0 {
            println!("Even number: {}", counter);
        } else {
            println!("Odd number: {}", counter);
        }

        match counter {
            0 => println!("Starting"),
            5 => println!("Halfway"),
            9 => println!("Almost done"),
            _ => println!("Continuing..."),
        }

        counter += 1;
    }

    // Loop and break/continue keywords
    loop {
        if counter > 20 {
            break;
        }

        if counter % 3 == 0 {
            counter += 1;
            continue;
        }

        counter += 1;
    }

    // Return keyword
    return;
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "counter",
        test_file.to_str().unwrap(),
        "--format",
        "outline",
    ])?;

    // Should find the keyword in the function
    assert!(
        output.contains("keyword_demonstration") || output.contains("counter"),
        "Should find keyword 'counter' - output: {}",
        output
    );

    // Should have reasonable length (truncated for outline)
    let line_count = output.lines().count();
    assert!(
        line_count < 100,
        "Output should be reasonably sized, got {} lines",
        line_count
    );

    Ok(())
}

#[test]
fn test_rust_outline_test_detection() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let test_file = temp_dir.path().join("test_detection.rs");

    let content = r#"use std::collections::HashMap;

pub fn add_numbers(a: i32, b: i32) -> i32 {
    a + b
}

pub fn multiply_numbers(a: i32, b: i32) -> i32 {
    a * b
}

// Unit tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_positive_numbers() {
        assert_eq!(add_numbers(2, 3), 5);
    }

    #[test]
    fn test_add_negative_numbers() {
        assert_eq!(add_numbers(-2, -3), -5);
    }

    #[test]
    fn test_multiply_positive_numbers() {
        assert_eq!(multiply_numbers(4, 5), 20);
    }

    #[test]
    fn test_multiply_with_zero() {
        assert_eq!(multiply_numbers(0, 5), 0);
    }

    #[should_panic]
    #[test]
    fn test_panic_scenario() {
        panic!("This test is expected to panic");
    }

    #[ignore]
    #[test]
    fn test_ignored() {
        // This test is ignored
        assert!(true);
    }
}

// Integration test module
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_integration_scenario() {
        let sum = add_numbers(10, 20);
        let product = multiply_numbers(sum, 2);
        assert_eq!(product, 60);
    }
}

// Benchmark tests (using criterion would be more typical)
#[cfg(test)]
mod benches {
    use super::*;
    use std::hint::black_box;

    #[test]
    fn bench_add_numbers() {
        for i in 0..1000 {
            black_box(add_numbers(i, i + 1));
        }
    }
}
"#;

    fs::write(&test_file, content)?;

    let ctx = TestContext::new();
    let output = ctx.run_probe(&[
        "search",
        "test", // Search for test keyword
        test_file.to_str().unwrap(),
        "--format",
        "outline",
        "--allow-tests",
    ])?;

    // Verify test-related content is found in outline format
    assert!(
        output.contains("test") && output.contains("#[test]"),
        "Missing test annotations - output: {}",
        output
    );

    // Should be in outline format with file delimiter
    assert!(
        output.contains("---\nFile:"),
        "Missing file delimiter in outline format - output: {}",
        output
    );

    // Should have found at least one result
    assert!(
        output.contains("Found") && output.contains("search results"),
        "Missing search results summary - output: {}",
        output
    );

    // Should include test functions or modules
    let has_test_content = output.contains("mod tests")
        || output.contains("fn test_")
        || output.contains("add_numbers");
    assert!(
        has_test_content,
        "Missing test content - output: {}",
        output
    );

    Ok(())
}
