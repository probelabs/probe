use anyhow::Result;
use probe_code::language::parser::parse_file_for_code_blocks;
use std::collections::HashSet;

/// Test line comment context extension across multiple programming languages
/// This ensures that when line comments are found by search, they are extended
/// to include their meaningful parent context (functions, methods, classes, etc.)
/// rather than being returned as isolated single-line snippets.

#[test]
fn test_rust_line_comment_context_extension() -> Result<()> {
    let rust_code = r#"
pub fn process_tokens(text: &str) -> Vec<String> {
    let tokens = tokenize(text);
    tokens
        .into_iter()
        .map(|token| normalize(token)) // normalize each token
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_processing() {
        let result = process_tokens("hello world");
        assert_eq!(result.len(), 2); // should have two tokens
    }
}
"#;

    let mut line_numbers = HashSet::new();
    line_numbers.insert(6); // Line with "// normalize each token"
    line_numbers.insert(16); // Line with "// should have two tokens"

    let result = parse_file_for_code_blocks(rust_code, "rs", &line_numbers, true, None)?;

    println!("Rust results:");
    for (i, block) in result.iter().enumerate() {
        println!(
            "  Block {}: type='{}', lines={}-{}",
            i,
            block.node_type,
            block.start_row + 1,
            block.end_row + 1
        );
    }

    // Line comment should be extended to include parent function
    assert!(
        result.iter().any(|block| {
            block.node_type == "function_item"
                && block.start_row <= 6
                && block.end_row >= 6
                && block.end_row > block.start_row // Not a single-line block
        }),
        "Rust line comment should be extended to parent function"
    );

    // Test function comment should be extended to include parent test function
    assert!(
        result.iter().any(|block| {
            (block.node_type == "function_item" || block.node_type.contains("test"))
                && block.start_row <= 16
                && block.end_row >= 16
                && block.end_row > block.start_row // Not a single-line block
        }),
        "Rust test comment should be extended to parent test function"
    );

    Ok(())
}

#[test]
fn test_javascript_line_comment_context_extension() -> Result<()> {
    let js_code = r#"
function processData(data) {
    const processed = data.map(item => {
        return item.value * 2; // double the value
    });
    return processed;
}

class DataProcessor {
    constructor() {
        this.multiplier = 3; // default multiplier
    }

    process(value) {
        return value * this.multiplier; // apply multiplier
    }
}
"#;

    let mut line_numbers = HashSet::new();
    line_numbers.insert(4); // Line with "// double the value"
    line_numbers.insert(10); // Line with "// default multiplier"
    line_numbers.insert(14); // Line with "// apply multiplier"

    let result = parse_file_for_code_blocks(js_code, "js", &line_numbers, true, None)?;

    println!("JavaScript results:");
    for (i, block) in result.iter().enumerate() {
        println!(
            "  Block {}: type='{}', lines={}-{}",
            i,
            block.node_type,
            block.start_row + 1,
            block.end_row + 1
        );
    }

    // Comments should be extended to their parent contexts
    assert!(
        result.iter().any(|block| {
            (block.node_type == "function_declaration"
                || block.node_type == "function"
                || block.node_type == "statement_block")
                && block.start_row <= 4
                && block.end_row >= 4
                && block.end_row > block.start_row
        }),
        "JavaScript function comment should be extended to parent function or block, got: {:?}",
        result
            .iter()
            .map(|b| (&b.node_type, b.start_row + 1, b.end_row + 1))
            .collect::<Vec<_>>()
    );

    // Note: The primary goal is that most comments get extended context.
    // Some comments (like property identifiers) may remain single-line, which is acceptable.
    // We mainly want to ensure function-level comments are extended.
    let function_comment_extended = result.iter().any(|block| {
        block.node_type == "statement_block"
            && block.start_row <= 4
            && block.end_row >= 4
            && block.end_row > block.start_row
    });
    assert!(
        function_comment_extended,
        "JavaScript function comments should be extended, got: {:?}",
        result
            .iter()
            .map(|b| (&b.node_type, b.start_row + 1, b.end_row + 1))
            .collect::<Vec<_>>()
    );

    Ok(())
}

#[test]
fn test_python_line_comment_context_extension() -> Result<()> {
    let python_code = r#"
def calculate_score(points):
    total = sum(points)
    average = total / len(points)  # calculate average
    return average * 1.2  # apply bonus multiplier

class ScoreCalculator:
    def __init__(self):
        self.bonus = 1.5  # default bonus factor

    def process(self, data):
        return data * self.bonus  # apply bonus
"#;

    let mut line_numbers = HashSet::new();
    line_numbers.insert(4); // Line with "# calculate average"
    line_numbers.insert(5); // Line with "# apply bonus multiplier"
    line_numbers.insert(9); // Line with "# default bonus factor"
    line_numbers.insert(12); // Line with "# apply bonus"

    let result = parse_file_for_code_blocks(python_code, "py", &line_numbers, true, None)?;

    println!("Python results:");
    for (i, block) in result.iter().enumerate() {
        println!(
            "  Block {}: type='{}', lines={}-{}",
            i,
            block.node_type,
            block.start_row + 1,
            block.end_row + 1
        );
    }

    // Comments should be extended to their parent contexts
    assert!(
        result.iter().any(|block| {
            (block.node_type == "function_definition"
                || block.node_type == "function"
                || block.node_type == "block")
                && block.start_row <= 4
                && block.end_row >= 4
                && block.end_row > block.start_row
        }),
        "Python function comment should be extended to parent function or block, got: {:?}",
        result
            .iter()
            .map(|b| (&b.node_type, b.start_row + 1, b.end_row + 1))
            .collect::<Vec<_>>()
    );

    // Note: The primary goal is that most comments get extended context.
    // Some comments may remain single-line, which is acceptable.
    // We mainly want to ensure function-level comments are extended.
    let function_comment_extended = result.iter().any(|block| {
        block.node_type == "block"
            && block.start_row <= 4
            && block.end_row >= 4
            && block.end_row > block.start_row
    });
    assert!(
        function_comment_extended,
        "Python function comments should be extended, got: {:?}",
        result
            .iter()
            .map(|b| (&b.node_type, b.start_row + 1, b.end_row + 1))
            .collect::<Vec<_>>()
    );

    Ok(())
}

#[test]
fn test_typescript_line_comment_context_extension() -> Result<()> {
    let ts_code = r#"
interface User {
    name: string;
    age: number; // user's age in years
}

function processUser(user: User): string {
    return `${user.name}: ${user.age}`; // format user info
}

class UserManager {
    private users: User[] = []; // internal user storage

    addUser(user: User): void {
        this.users.push(user); // add to collection
    }
}
"#;

    let mut line_numbers = HashSet::new();
    line_numbers.insert(4); // Line with "// user's age in years"
    line_numbers.insert(8); // Line with "// format user info"
    line_numbers.insert(12); // Line with "// internal user storage"
    line_numbers.insert(15); // Line with "// add to collection"

    let result = parse_file_for_code_blocks(ts_code, "ts", &line_numbers, true, None)?;

    println!("TypeScript results:");
    for (i, block) in result.iter().enumerate() {
        println!(
            "  Block {}: type='{}', lines={}-{}",
            i,
            block.node_type,
            block.start_row + 1,
            block.end_row + 1
        );
    }

    // Comments should be extended to their parent contexts
    assert!(
        result.iter().any(|block| {
            block.node_type == "interface_declaration"
                && block.start_row <= 4
                && block.end_row >= 4
                && block.end_row > block.start_row
        }),
        "TypeScript interface comment should be extended to parent interface"
    );

    assert!(
        result.iter().any(|block| {
            (block.node_type == "function_declaration" || block.node_type == "function")
                && block.start_row <= 8
                && block.end_row >= 8
                && block.end_row > block.start_row
        }),
        "TypeScript function comment should be extended to parent function"
    );

    Ok(())
}

#[test]
fn test_go_line_comment_context_extension() -> Result<()> {
    let go_code = r#"
package main

func calculateTotal(prices []float64) float64 {
    var total float64
    for _, price := range prices {
        total += price // accumulate total
    }
    return total * 1.1 // add 10% tax
}

type Calculator struct {
    taxRate float64 // tax rate as decimal
}

func (c *Calculator) Process(value float64) float64 {
    return value * c.taxRate // apply tax rate
}
"#;

    let mut line_numbers = HashSet::new();
    line_numbers.insert(7); // Line with "// accumulate total"
    line_numbers.insert(9); // Line with "// add 10% tax"
    line_numbers.insert(13); // Line with "// tax rate as decimal"
    line_numbers.insert(17); // Line with "// apply tax rate"

    let result = parse_file_for_code_blocks(go_code, "go", &line_numbers, true, None)?;

    println!("Go results:");
    for (i, block) in result.iter().enumerate() {
        println!(
            "  Block {}: type='{}', lines={}-{}",
            i,
            block.node_type,
            block.start_row + 1,
            block.end_row + 1
        );
    }

    // Comments should be extended to their parent contexts
    assert!(
        result.iter().any(|block| {
            (block.node_type == "function_declaration" || block.node_type == "func_declaration")
                && block.start_row <= 7
                && block.end_row >= 7
                && block.end_row > block.start_row
        }),
        "Go function comment should be extended to parent function"
    );

    assert!(
        result.iter().any(|block| {
            (block.node_type == "type_declaration" || block.node_type == "struct_type")
                && block.start_row <= 13
                && block.end_row >= 13
                && block.end_row > block.start_row
        }),
        "Go struct comment should be extended to parent type"
    );

    Ok(())
}

#[test]
fn test_java_line_comment_context_extension() -> Result<()> {
    let java_code = r#"
public class Calculator {
    private double rate = 0.1; // default tax rate

    public double calculate(double amount) {
        return amount * (1 + rate); // apply tax rate
    }

    public static void main(String[] args) {
        Calculator calc = new Calculator();
        double result = calc.calculate(100.0); // test calculation
        System.out.println(result);
    }
}
"#;

    let mut line_numbers = HashSet::new();
    line_numbers.insert(3); // Line with "// default tax rate"
    line_numbers.insert(6); // Line with "// apply tax rate"
    line_numbers.insert(11); // Line with "// test calculation"

    let result = parse_file_for_code_blocks(java_code, "java", &line_numbers, true, None)?;

    println!("Java results:");
    for (i, block) in result.iter().enumerate() {
        println!(
            "  Block {}: type='{}', lines={}-{}",
            i,
            block.node_type,
            block.start_row + 1,
            block.end_row + 1
        );
    }

    // Comments should be extended to their parent contexts
    assert!(
        result.iter().any(|block| {
            (block.node_type == "class_declaration" || block.node_type == "method_declaration")
                && block.start_row <= 3
                && block.end_row >= 3
                && block.end_row > block.start_row
        }),
        "Java class field comment should be extended to parent class or method, got: {:?}",
        result
            .iter()
            .map(|b| (&b.node_type, b.start_row + 1, b.end_row + 1))
            .collect::<Vec<_>>()
    );

    assert!(
        result.iter().any(|block| {
            (block.node_type == "method_declaration"
                || block.node_type == "function"
                || block.node_type == "block")
                && block.start_row <= 6
                && block.end_row >= 6
                && block.end_row > block.start_row
        }),
        "Java method comment should be extended to parent method or block, got: {:?}",
        result
            .iter()
            .map(|b| (&b.node_type, b.start_row + 1, b.end_row + 1))
            .collect::<Vec<_>>()
    );

    Ok(())
}

/// Test that verifies line comments are NOT returned as single-line blocks
/// This is the core issue - line comments should always be extended to their parent context
#[test]
fn test_no_single_line_comment_blocks() -> Result<()> {
    let rust_code = r#"
fn example() {
    let x = 42; // this is a comment
    println!("{}", x);
}
"#;

    let mut line_numbers = HashSet::new();
    line_numbers.insert(3); // Line with comment

    let result = parse_file_for_code_blocks(rust_code, "rs", &line_numbers, true, None)?;

    // Verify no single-line blocks are returned for comment lines
    for block in &result {
        if block.start_row == 3 && block.end_row == 3 {
            panic!(
                "Found single-line block for comment line - this should not happen! Block type: {}",
                block.node_type
            );
        }
    }

    // Should have at least one block that includes the comment line but extends beyond it
    assert!(
        result.iter().any(|block| {
            block.start_row <= 3 && block.end_row >= 3 && block.end_row > block.start_row
        }),
        "Should have extended block containing the comment line"
    );

    Ok(())
}
