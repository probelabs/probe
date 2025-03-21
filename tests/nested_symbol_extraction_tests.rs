use anyhow::Result;
use std::fs;
use std::path::PathBuf;

#[test]
fn test_nested_symbol_extraction() -> Result<()> {
    // Create a temporary test file with nested symbols
    let test_content = r#"
struct ProbeAgentServer {
    client: Client,
    config: Config,
}

impl ProbeAgentServer {
    pub fn new(client: Client, config: Config) -> Self {
        Self { client, config }
    }

    pub fn setupToolHandlers(&self) {
        // Setup tool handlers
        println!("Setting up tool handlers");
        
        // Register search handler
        self.register_handler("search", |params| {
            // Search implementation
        });
    }
    
    fn register_handler(&self, name: &str, handler: impl Fn(&str)) {
        // Register handler implementation
    }
}
"#;

    // Write the test content to a temporary file
    let temp_dir = tempfile::tempdir()?;
    let file_path = temp_dir.path().join("test_nested_symbols.rs");
    fs::write(&file_path, test_content)?;

    // Test extracting the nested symbol
    let result = extract_nested_symbol(&file_path, "ProbeAgentServer.setupToolHandlers")?;

    // Verify the result contains the setupToolHandlers method
    assert!(result.contains("pub fn setupToolHandlers"));
    assert!(result.contains("Setting up tool handlers"));

    // Clean up
    temp_dir.close()?;

    Ok(())
}

// Helper function to extract a nested symbol from a file
fn extract_nested_symbol(path: &PathBuf, symbol: &str) -> Result<String> {
    // Read the file content
    let content = fs::read_to_string(path)?;

    // Call the symbol finder function
    let result = probe::extract::symbol_finder::find_symbol_in_file(
        path, symbol, &content, true, // allow_tests
        0,    // context_lines
    )?;

    Ok(result.code)
}

#[test]
fn test_simple_symbol_extraction() -> Result<()> {
    // Create a temporary test file with a simple symbol
    let test_content = r#"
struct Config {
    pub path: String,
    pub timeout: u64,
}

impl Config {
    pub fn new(path: String, timeout: u64) -> Self {
        Self { path, timeout }
    }
}
"#;

    // Write the test content to a temporary file
    let temp_dir = tempfile::tempdir()?;
    let file_path = temp_dir.path().join("test_simple_symbol.rs");
    fs::write(&file_path, test_content)?;

    // Test extracting a simple symbol
    let result = extract_nested_symbol(&file_path, "Config")?;

    // Verify the result contains the Config struct
    assert!(result.contains("struct Config"));
    assert!(result.contains("pub path: String"));

    // Clean up
    temp_dir.close()?;

    Ok(())
}
