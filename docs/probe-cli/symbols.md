# Symbols Command

List all symbols (functions, structs, classes, constants, etc.) in files — a table of contents with line numbers and nesting.

## TL;DR

```bash
# List symbols in a file
probe symbols src/main.rs

# JSON output (for programmatic use)
probe symbols src/main.rs --format json

# Multiple files
probe symbols src/main.rs src/lib.rs

# Include test functions
probe symbols src/main.rs --allow-tests
```

## Basic Syntax

```
probe symbols <FILES> [OPTIONS]
```

### Options

| Option | Description | Default |
|--------|-------------|---------|
| `-o, --format` | Output format: `text` or `json` | `text` |
| `--allow-tests` | Include test functions/methods | `false` |

## Output Formats

### Text Output

Shows symbols with indentation for nesting:

```
src/main.rs:
  1:795   fn main()
  15:28   struct Config { ... }
  30:55   impl Config
    32:44   fn new() -> Config
    45:54   fn validate(&self) -> bool
  60      const MAX_SIZE: usize
  65:72   enum Status
```

### JSON Output

Returns structured data with nested children:

```json
[{
  "file": "src/main.rs",
  "symbols": [
    {
      "name": "main",
      "kind": "function",
      "signature": "fn main()",
      "line": 1,
      "end_line": 795,
      "children": []
    },
    {
      "name": "Config",
      "kind": "impl",
      "signature": "impl Config { ... }",
      "line": 30,
      "end_line": 55,
      "children": [
        {
          "name": "new",
          "kind": "function",
          "signature": "fn new() -> Config",
          "line": 32,
          "end_line": 44
        }
      ]
    }
  ]
}]
```

## Symbol Types

The symbols command detects the following symbol types across languages:

| Kind | Languages | Examples |
|------|-----------|---------|
| `function` | All | `fn main()`, `def greet()`, `function hello()` |
| `method` | All | Methods inside classes/impl blocks |
| `struct` | Rust, Go | `struct Config { ... }` |
| `class` | Python, TS/JS, Java | `class App { ... }` |
| `interface` | TS, Go | `interface Config { ... }` |
| `trait` | Rust | `trait Handler { ... }` |
| `impl` | Rust | `impl Config { ... }` |
| `enum` | Rust, TS, Java | `enum Status { ... }` |
| `const` | Rust, Go, TS/JS | `const MAX_SIZE: usize` |
| `static` | Rust | `static INSTANCE: ...` |
| `type` | Rust, TS, Go | `type Alias = ...` |
| `module` | Rust, TS | `mod utils`, `namespace Api` |
| `macro` | Rust | `macro_rules! my_macro` |
| `variable` | TS/JS, Python | `const config = ...`, `MAX_COUNT = 100` |

## Nesting

Container symbols (impl blocks, classes, traits, modules) show their children indented:

- **Rust**: methods inside `impl` and `trait` blocks
- **TypeScript/JavaScript**: methods inside `class` declarations
- **Python**: methods inside `class` definitions
- **Go**: methods are shown at top level (Go uses receiver syntax, not nesting)

## Use Cases

- **Quick file overview**: Understand a file's structure before reading it
- **Find the right symbol name**: Get exact names for `probe extract file.rs#symbol`
- **Navigate large files**: Find line numbers for functions of interest
- **Code review**: See what changed at a structural level

## SDK Usage

```javascript
import { symbols } from '@probelabs/probe';

const result = await symbols({
  files: ['src/main.rs'],
  cwd: './project'
});

console.log(result[0].symbols);
```

See also: [Search](./search.md) | [Extract](./extract.md) | [Query](./query.md) | [CLI Reference](./cli-reference.md)
