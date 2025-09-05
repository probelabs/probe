//! Simple test to verify tree-sitter integration works

fn main() {
    println!("Testing tree-sitter dependency integration...");

    // Test basic tree-sitter parser creation
    let mut parser = tree_sitter::Parser::new();

    #[cfg(feature = "tree-sitter-rust")]
    {
        println!("Testing Rust parser...");
        match parser.set_language(&tree_sitter_rust::LANGUAGE.into()) {
            Ok(()) => {
                let code = "fn main() { println!(\"Hello, world!\"); }";
                match parser.parse(code, None) {
                    Some(tree) => println!(
                        "✓ Rust parser works! Root node: {:?}",
                        tree.root_node().kind()
                    ),
                    None => println!("✗ Failed to parse Rust code"),
                }
            }
            Err(e) => println!("✗ Failed to set Rust language: {:?}", e),
        }
    }

    #[cfg(feature = "tree-sitter-python")]
    {
        println!("Testing Python parser...");
        match parser.set_language(&tree_sitter_python::LANGUAGE.into()) {
            Ok(()) => {
                let code = "def main():\n    print('Hello, world!')";
                match parser.parse(code, None) {
                    Some(tree) => println!(
                        "✓ Python parser works! Root node: {:?}",
                        tree.root_node().kind()
                    ),
                    None => println!("✗ Failed to parse Python code"),
                }
            }
            Err(e) => println!("✗ Failed to set Python language: {:?}", e),
        }
    }

    #[cfg(feature = "tree-sitter-typescript")]
    {
        println!("Testing TypeScript parser...");
        match parser.set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()) {
            Ok(()) => {
                let code = "function main(): void { console.log('Hello, world!'); }";
                match parser.parse(code, None) {
                    Some(tree) => println!(
                        "✓ TypeScript parser works! Root node: {:?}",
                        tree.root_node().kind()
                    ),
                    None => println!("✗ Failed to parse TypeScript code"),
                }
            }
            Err(e) => println!("✗ Failed to set TypeScript language: {:?}", e),
        }
    }

    #[cfg(feature = "tree-sitter-javascript")]
    {
        println!("Testing JavaScript parser...");
        match parser.set_language(&tree_sitter_javascript::LANGUAGE.into()) {
            Ok(()) => {
                let code = "function main() { console.log('Hello, world!'); }";
                match parser.parse(code, None) {
                    Some(tree) => println!(
                        "✓ JavaScript parser works! Root node: {:?}",
                        tree.root_node().kind()
                    ),
                    None => println!("✗ Failed to parse JavaScript code"),
                }
            }
            Err(e) => println!("✗ Failed to set JavaScript language: {:?}", e),
        }
    }

    println!("Tree-sitter dependency test completed!");
}
