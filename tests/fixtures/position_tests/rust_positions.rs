// Test fixture for Rust tree-sitter position validation
// Line numbers and symbol positions are tested precisely

fn simple_function() {} // simple_function at position (line 3, col 3)

pub fn public_function() -> i32 {
    42
} // public_function at position (line 5, col 7)

async fn async_function() {} // async_function at position (line 9, col 10)

fn function_with_params(param1: i32, param2: &str) -> String {
    param2.to_string()
} // function_with_params at position (line 11, col 3)

struct SimpleStruct {
    field1: i32,
    field2: String,
} // SimpleStruct at position (line 15, col 7)

pub struct PublicStruct {
    pub value: i32,
} // PublicStruct at position (line 20, col 11)

impl SimpleStruct {
    fn new(field1: i32, field2: String) -> Self {
        Self { field1, field2 }
    } // new at position (line 25, col 7)
    
    pub fn get_field1(&self) -> i32 {
        self.field1
    } // get_field1 at position (line 29, col 11)
    
    fn private_method(&mut self) {
        self.field1 += 1;
    } // private_method at position (line 33, col 7)
}

trait MyTrait {
    fn trait_method(&self); // trait_method at position (line 38, col 7)
    
    fn default_implementation(&self) -> i32 {
        42
    } // default_implementation at position (line 41, col 7)
}

impl MyTrait for SimpleStruct {
    fn trait_method(&self) {
        println!("Implemented trait method");
    } // trait_method at position (line 47, col 7)
}

enum Color {
    Red,    // Red at position (line 52, col 4)
    Green,  // Green at position (line 53, col 4)
    Blue,   // Blue at position (line 54, col 4)
} // Color at position (line 51, col 5)

enum ComplexEnum {
    Variant1(i32),              // Variant1 at position (line 58, col 4)
    Variant2 { x: i32, y: i32 }, // Variant2 at position (line 59, col 4)
} // ComplexEnum at position (line 57, col 5)

type MyAlias = HashMap<String, i32>; // MyAlias at position (line 62, col 5)

const CONSTANT: i32 = 42; // CONSTANT at position (line 64, col 6)

static STATIC_VAR: &str = "hello"; // STATIC_VAR at position (line 66, col 7)

mod inner_module {
    pub fn module_function() {} // module_function at position (line 69, col 11)
} // inner_module at position (line 68, col 4)

macro_rules! simple_macro {
    () => {
        println!("Hello from macro!");
    };
} // simple_macro at position (line 73, col 13)

use std::collections::HashMap;