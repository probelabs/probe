#!/usr/bin/env rust-script

//! ```cargo
//! [dependencies]
//! tree-sitter = "0.24.5"
//! tree-sitter-rust = "0.23.2"
//! tree-sitter-python = "0.23.6"
//! tree-sitter-typescript = "0.23.2" 
//! tree-sitter-javascript = "0.23.1"
//! ```

//! Standalone test to verify tree-sitter dependencies work

fn main() {
    println!("Testing tree-sitter dependency integration...");
    
    // Test basic tree-sitter parser creation
    let mut parser = tree_sitter::Parser::new();
    
    // Test Rust parser
    println!("Testing Rust parser...");
    match parser.set_language(&tree_sitter_rust::LANGUAGE.into()) {
        Ok(()) => {
            let code = "fn main() { println!(\"Hello, world!\"); }";
            match parser.parse(code, None) {
                Some(tree) => {
                    println!("✓ Rust parser works! Root node: {:?}", tree.root_node().kind());
                    println!("  Tree: {:?}", tree.root_node().to_sexp());
                }
                None => println!("✗ Failed to parse Rust code"),
            }
        }
        Err(e) => println!("✗ Failed to set Rust language: {:?}", e),
    }
    
    // Test Python parser  
    println!("Testing Python parser...");
    match parser.set_language(&tree_sitter_python::LANGUAGE.into()) {
        Ok(()) => {
            let code = "def main():\n    print('Hello, world!')";
            match parser.parse(code, None) {
                Some(tree) => {
                    println!("✓ Python parser works! Root node: {:?}", tree.root_node().kind());
                    println!("  Tree: {:?}", tree.root_node().to_sexp());
                }
                None => println!("✗ Failed to parse Python code"),
            }
        }
        Err(e) => println!("✗ Failed to set Python language: {:?}", e),
    }
    
    // Test TypeScript parser
    println!("Testing TypeScript parser...");
    match parser.set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()) {
        Ok(()) => {
            let code = "function main(): void { console.log('Hello, world!'); }";
            match parser.parse(code, None) {
                Some(tree) => {
                    println!("✓ TypeScript parser works! Root node: {:?}", tree.root_node().kind());
                    println!("  Tree: {:?}", tree.root_node().to_sexp());
                }
                None => println!("✗ Failed to parse TypeScript code"),
            }
        }
        Err(e) => println!("✗ Failed to set TypeScript language: {:?}", e),
    }
    
    // Test JavaScript parser
    println!("Testing JavaScript parser...");
    match parser.set_language(&tree_sitter_javascript::LANGUAGE.into()) {
        Ok(()) => {
            let code = "function main() { console.log('Hello, world!'); }";
            match parser.parse(code, None) {
                Some(tree) => {
                    println!("✓ JavaScript parser works! Root node: {:?}", tree.root_node().kind());
                    println!("  Tree: {:?}", tree.root_node().to_sexp());
                }
                None => println!("✗ Failed to parse JavaScript code"),
            }
        }
        Err(e) => println!("✗ Failed to set JavaScript language: {:?}", e),
    }
    
    println!("Tree-sitter dependency test completed!");
}