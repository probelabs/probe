# Protocol Converter

The `ProtocolConverter` module provides utilities for converting between database types (`Edge`, `SymbolState`) and LSP protocol types (`Location`, `CallHierarchyItem`, `CallHierarchyCall`).

## Overview

The converter handles:
- Database Edge/SymbolState to LSP Location
- Database SymbolState to LSP CallHierarchyItem  
- Database Edge to LSP CallHierarchyCall
- Proper URI formatting (file:// scheme)
- Position and range mapping

## Usage

```rust
use lsp_daemon::database::{ProtocolConverter, SymbolState, Edge, EdgeRelation};
use std::path::Path;

let converter = ProtocolConverter::new();
```

### Converting Symbols to CallHierarchyItem

```rust
let symbol = SymbolState {
    symbol_uid: "rust_function_123".to_string(),
    file_version_id: 1,
    language: "rust".to_string(),
    name: "parse_config".to_string(),
    fqn: Some("config::parser::parse_config".to_string()),
    kind: "function".to_string(),
    signature: Some("fn parse_config(path: &Path) -> Result<Config>".to_string()),
    visibility: Some("public".to_string()),
    def_start_line: 42,
    def_start_char: 0,
    def_end_line: 58,
    def_end_char: 1,
    is_definition: true,
    documentation: Some("Parse configuration from file".to_string()),
    metadata: None,
};

let file_path = Path::new("/src/config/parser.rs");
let call_hierarchy_item = converter.symbol_to_call_hierarchy_item(&symbol, file_path);

// Result:
// CallHierarchyItem {
//     name: "parse_config",
//     kind: "Function",
//     uri: "file:///src/config/parser.rs",
//     range: Range { start: Position { line: 42, character: 0 }, end: Position { line: 58, character: 1 } },
//     selection_range: Range { start: Position { line: 42, character: 0 }, end: Position { line: 58, character: 1 } }
// }
```

### Converting Edges to Locations

```rust
let edges = vec![
    Edge {
        relation: EdgeRelation::Calls,
        source_symbol_uid: "caller_123".to_string(),
        target_symbol_uid: "target_456".to_string(),
        anchor_file_version_id: 1,
        start_line: Some(15),
        start_char: Some(8),
        confidence: 0.95,
        language: "rust".to_string(),
        metadata: None,
    }
];

let locations = converter.edges_to_locations(edges);

// Result: Vec<Location> with one entry:
// Location {
//     uri: "file://placeholder_file_1",
//     range: Range { start: Position { line: 15, character: 8 }, end: Position { line: 15, character: 8 } }
// }
```

### Converting Edges to CallHierarchyCall

```rust
let caller_symbol = SymbolState {
    symbol_uid: "caller_456".to_string(),
    name: "main".to_string(),
    // ... other fields
};

let call_edge = Edge {
    relation: EdgeRelation::Calls,
    source_symbol_uid: "caller_456".to_string(),
    target_symbol_uid: "target_123".to_string(),
    start_line: Some(15),
    start_char: Some(4),
    // ... other fields
};

let symbols = vec![caller_symbol];
let edges = vec![call_edge];
let calls = converter.edges_to_calls(edges, &symbols);

// Result: Vec<CallHierarchyCall> with call information
```

### URI Conversion

```rust
// Convert file path to URI
let path = Path::new("/home/user/project/src/main.rs");
let uri = converter.path_to_uri(path);
// Result: "file:///home/user/project/src/main.rs"

// Convert URI back to path
let converted_path = converter.uri_to_path(&uri)?;
// Result: PathBuf("/home/user/project/src/main.rs")
```

## Symbol Kind Mapping

The converter maps internal symbol kinds to LSP SymbolKind values:

| Internal Kind | LSP Kind |
|---------------|----------|
| function | Function |
| method | Method |
| constructor | Constructor |
| class | Class |
| interface | Interface |
| struct | Struct |
| enum | Enum |
| variable | Variable |
| constant | Constant |
| field | Field |
| property | Property |
| module | Module |
| namespace | Namespace |
| package | Package |
| * | Unknown |

## Platform Support

The converter handles URI formatting for different platforms:

- **Unix**: `/path/to/file.rs` → `file:///path/to/file.rs`
- **Windows**: `C:\path\to\file.rs` → `file:///C:\path\to\file.rs`
- **Already formatted**: `file://...` → unchanged

## Error Handling

- Invalid URIs return error results from `uri_to_path()`
- Missing symbols in edges return `None` from conversion methods
- File path resolution failures use placeholder paths

## Example Usage

See `/lsp-daemon/examples/converter_example.rs` for a complete working example.

## Testing

Run the converter tests with:

```bash
cargo test database::converters -p lsp-daemon --lib
```

All conversion methods have unit tests covering:
- Basic functionality
- Edge cases (empty data, invalid inputs)
- Platform-specific URI handling
- Symbol kind mapping
- Range and position conversion