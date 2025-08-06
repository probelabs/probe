# Simple LSP Integration Example

This is a simplified example that shows how to interact with rust-analyzer as an LSP server to get call hierarchy information. All concurrency and complex abstractions have been removed to make the code as straightforward as possible.

The example uses only standard library features and serde_json for JSON handling. Everything happens synchronously over stdio using JSON-RPC.

## How to run

```bash
cargo run -- path/to/src/lib.rs "function_name"
```

The example will search for the specified pattern in the file and use that position for call hierarchy analysis.

**Examples:**
```bash
# Search for a function definition
cargo run -- src/main.rs "fn main"

# Search for any function name
cargo run -- src/lib.rs "handle_search"

# Search for method calls
cargo run -- src/parser.rs "parse_file"
```

You should see:
1. The location where the pattern was found (line and column)
2. A complete call graph showing both incoming calls (who calls this function) and outgoing calls (what this function calls)
3. A Graphviz DOT file saved for visualization
4. Instructions on how to generate visual graphs

**Pro tip**: Search for function definitions like `"fn function_name"` for best results, as these positions are most likely to support call hierarchy.

If no call hierarchy items are found, it usually means:
1. The pattern was found but not on a function definition
2. The symbol at that position doesn't support call hierarchy  
3. rust-analyzer hasn't finished analyzing the workspace yet


## Requirements

- `rust-analyzer` must be installed and available in your PATH
- The target file must be part of a valid Rust project (with Cargo.toml)

## Example Output

```
🚀 Starting simple LSP example...

Found 'send_message' at line 74, column 3
Starting rust-analyzer...
Initializing LSP...
Initialize response received
Opening document...

Preparing call hierarchy...
Found function: send_message

Getting outgoing calls...
Getting incoming calls...

📊 Call hierarchy for 'send_message':

  Outgoing calls (this function calls):
    → write!
    → Write::flush

  Incoming calls (functions that call this):
    ← build_call_graph
    ← main
    ← wait_for_workspace_ready

Shutting down...
Done!
```

## What it demonstrates

1. **Simple synchronous LSP communication** – No async/await, no concurrency, just straightforward blocking I/O
2. **Minimal dependencies** – Only uses standard library and serde_json
3. **Basic call hierarchy** – Shows both incoming and outgoing function calls
4. **Pattern-based search** – Find functions by name without counting lines/columns
5. **Three key LSP messages**:
   - `textDocument/prepareCallHierarchy` → gets the function handle
   - `callHierarchy/outgoingCalls` → what this function calls
   - `callHierarchy/incomingCalls` → what calls this function

## Taking it further

- **Add error handling** – Currently uses simple unwrap/expect patterns
- **Add graph visualization** – Convert results to Graphviz DOT format
- **Support multiple queries** – Reuse the rust-analyzer session for multiple lookups
- **Add more LSP features** – Explore other capabilities like hover, goto definition, etc.

This simplified example provides a clear foundation for understanding LSP communication without the complexity of async code or concurrency.