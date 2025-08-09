// Simple test file for rust-analyzer
fn main() {
    let result = add_numbers(5, 10);
    println!("Result: {}", result);
    process_result(result);
}

fn add_numbers(a: i32, b: i32) -> i32 {
    let sum = a + b;
    multiply_by_two(sum)
}

fn multiply_by_two(n: i32) -> i32 {
    n * 2
}

fn process_result(value: i32) {
    if value > 20 {
        print_large_number(value);
    } else {
        print_small_number(value);
    }
}

fn print_large_number(n: i32) {
    println!("Large number: {}", n);
}

fn print_small_number(n: i32) {
    println!("Small number: {}", n);
}