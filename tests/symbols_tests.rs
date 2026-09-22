use probe_code::extract::symbols::extract_symbols;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_extract_symbols_with_rust_code() {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let test_file = temp_dir.path().join("test.rs");

    let rust_code = r#"
pub struct User {
    pub name: String,
    pub age: u32,
}

impl User {
    pub fn new(name: String, age: u32) -> Self {
        Self { name, age }
    }

    pub fn greet(&self) -> String {
        format!("Hello, I'm {}", self.name)
    }
}

pub fn main() {
    let user = User::new("Alice".to_string(), 30);
    println!("{}", user.greet());
}

pub const MAX_USERS: usize = 1000;
"#;

    fs::write(&test_file, rust_code).expect("Failed to write test file");

    let symbols = extract_symbols(&test_file, false).expect("Symbol extraction should succeed");
    let signatures: Vec<_> = symbols
        .symbols
        .iter()
        .map(|symbol| symbol.signature.as_str())
        .collect();

    assert!(
        signatures.iter().any(|sig| sig.contains("struct User")),
        "Should extract Rust struct signatures: {signatures:?}"
    );
    assert!(
        signatures.iter().any(|sig| sig.contains("impl User")),
        "Should extract Rust impl signatures: {signatures:?}"
    );
    assert!(
        signatures.iter().any(|sig| sig.contains("pub fn main()")),
        "Should extract Rust function signatures: {signatures:?}"
    );
    assert!(
        signatures.iter().any(|sig| sig.contains("MAX_USERS")),
        "Should extract Rust constant signatures: {signatures:?}"
    );
}

#[test]
fn test_extract_symbols_with_python_code() {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let test_file = temp_dir.path().join("test.py");

    let python_code = r#"
class User:
    def __init__(self, name: str, age: int):
        self.name = name
        self.age = age

    def greet(self) -> str:
        return f"Hello, I'm {self.name}"

def create_user(name: str, age: int) -> User:
    return User(name, age)

async def async_function(data: list) -> dict:
    return {"length": len(data)}

MAX_USERS = 1000
add = lambda x, y: x + y
"#;

    fs::write(&test_file, python_code).expect("Failed to write test file");

    let symbols = extract_symbols(&test_file, false).expect("Symbol extraction should succeed");
    let signatures: Vec<_> = symbols
        .symbols
        .iter()
        .map(|symbol| symbol.signature.as_str())
        .collect();

    assert!(
        signatures.iter().any(|sig| sig.contains("class User")),
        "Should extract Python class signatures: {signatures:?}"
    );
    assert!(
        signatures
            .iter()
            .any(|sig| sig.contains("def create_user(")),
        "Should extract Python function signatures: {signatures:?}"
    );
    assert!(
        signatures
            .iter()
            .any(|sig| sig.contains("async def async_function(")),
        "Should extract Python async function signatures: {signatures:?}"
    );
    assert!(
        signatures.iter().any(|sig| sig.contains("MAX_USERS = ...")),
        "Should extract Python constant signatures: {signatures:?}"
    );
    assert!(
        !signatures.iter().any(|sig| sig.contains("lambda")),
        "Should not extract Python lambda assignments as symbols: {signatures:?}"
    );
}
