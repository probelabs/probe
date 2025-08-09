fn main() {
    println!("Simple LSP Test Project");
    
    let result = calculate_result(10, 20);
    display_result(result);
}

fn calculate_result(a: i32, b: i32) -> i32 {
    let intermediate = perform_calculation(a, b);
    apply_modifier(intermediate)
}

fn perform_calculation(x: i32, y: i32) -> i32 {
    x + y + get_bonus()
}

fn get_bonus() -> i32 {
    42
}

fn apply_modifier(value: i32) -> i32 {
    value * 2
}

fn display_result(result: i32) {
    println!("Final result: {}", result);
    
    if result > 100 {
        print_large_result(result);
    } else {
        print_small_result(result);
    }
}

fn print_large_result(value: i32) {
    println!("That's a large result: {}", value);
}

fn print_small_result(value: i32) {
    println!("That's a small result: {}", value);
}