# Adding Support for New Languages

This guide lists the exact files you must modify to add a new language to Probe.

## Required File Modifications

Adding a new language requires modifying exactly **11 files**. Missing any of these will result in incomplete language support.

### Step 1: Core Language Implementation (4 files)
### Step 2: Search & Extract Integration (4 files)
### Step 3: CLI & User Interface (2 files)
### Step 4: Documentation (1 file)

## Step 1: Core Language Implementation

### File 1: `Cargo.toml`
Add tree-sitter dependency:
```toml
tree-sitter-yourlang = "X.Y.Z"
```

### File 2: `src/language/yourlang.rs`
Create language implementation (copy from existing language like `rust.rs`):
```rust
use super::language_trait::LanguageImpl;
use tree_sitter::{Language as TSLanguage, Node};

pub struct YourLangLanguage;

impl LanguageImpl for YourLangLanguage {
    fn get_tree_sitter_language(&self) -> TSLanguage {
        tree_sitter_yourlang::language().into()
    }
    // ... implement other required methods
}
```

### File 3: `src/language/mod.rs`
Add module export:
```rust
pub mod yourlang;
```

### File 4: `src/language/factory.rs`
Add import:
```rust
use probe_code::language::yourlang::YourLangLanguage;
```
Add registration:
```rust
"yourext" => Some(Box::new(YourLangLanguage::new())),
```

## Step 2: Search & Extract Integration

### File 5: `src/search/file_list_cache.rs`
Add to `get_language_extensions()` function:
```rust
"yourlang" => vec![".yourext".to_string()],
```

### File 6: `src/extract/formatter.rs`
Add to syntax highlighting mapping:
```rust
"yourext" => "yourlang",
```

### File 7: `src/search/results_formatter.rs`
Add to language mapping:
```rust
"yourext" => "yourlang",
```

### File 8: `src/search/search_output.rs`
Add to language mapping and comment prefix:
```rust
// In syntax highlighting mapping:
"yourext" => "yourlang",

// In comment prefix mapping (choose appropriate section):
// For # comments:
"py" | "rb" | "sh" | "bash" | "pl" | "r" | "yourext" => "#",
// For // comments:
"rs" | "c" | "h" | "cpp" | "yourext" => "//",
```

## Step 3: CLI & User Interface

### File 9: `src/cli.rs`
Add to language completion lists (2 locations):
```rust
"yourlang", "yourext",
```

### File 10: `npm/src/mcp/index.ts`
Add to supported languages description:
```rust
'Supported languages: rust, javascript, typescript, python, go, c, cpp, java, ruby, php, swift, csharp, yourlang.',
```

## Step 4: Documentation

### File 11: `docs/supported-languages.md`
Add table entry and language-specific features section.

## Testing

Create comprehensive tests in your language file:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    // Add tests for all LanguageImpl methods
}
```

## LanguageImpl Methods to Implement

Copy implementation patterns from similar languages:

- `get_tree_sitter_language()` - Return tree-sitter language
- `is_acceptable_parent()` - Define extractable code blocks
- `is_test_node()` - Detect test code
- `get_symbol_signature()` - Extract clean signatures
- `find_parent_function()` - Optional method

Use tree-sitter playground to understand AST structure: https://tree-sitter.github.io/tree-sitter/playground

## Verification

After modifying all 11 files:
1. `cargo test` - All tests pass
2. `cargo run -- search "term" file.yourext` - Search works
3. `cargo run -- extract file.yourext:N` - Extract works
4. `cargo run -- search "test" --allow-tests` - Test detection works
