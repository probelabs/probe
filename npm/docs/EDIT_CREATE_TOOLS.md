# Edit and Create Tools

## Overview

The Probe Agent provides file editing and creation capabilities through two tools: `edit` and `create`. The `edit` tool supports four modes — text-based find/replace with fuzzy matching, AST-aware symbol replacement, symbol insertion, and line-targeted editing with optional hash-based integrity verification. These tools are disabled by default and must be explicitly enabled.

## Edit Tool

### Four Editing Modes

Choose the mode based on the scope of your change:

| Mode | Parameters | When to Use |
|------|-----------|-------------|
| **Text edit** | `old_string` + `new_string` | Small, precise changes: fix a condition, rename a variable, update a value |
| **Symbol replace** | `symbol` + `new_string` | Replace an entire function, class, or method by name (no exact text matching needed) |
| **Symbol insert** | `symbol` + `new_string` + `position` | Insert new code before or after an existing symbol |
| **Line-targeted edit** | `start_line` + `new_string` | Edit specific lines from extract/search output; ideal for changes inside large functions |

### Parameters

| Parameter | Required | Description |
|-----------|----------|-------------|
| `file_path` | Yes | Path to the file to edit (absolute or relative to cwd) |
| `new_string` | Yes | Replacement text or new code content |
| `old_string` | No | Text to find and replace. Copy verbatim from the file. |
| `replace_all` | No | Replace all occurrences (default: `false`, text mode only) |
| `symbol` | No | Code symbol name for AST-aware editing (e.g. `"myFunction"`, `"MyClass.myMethod"`) |
| `position` | No | `"before"` or `"after"` — insert near the symbol or line instead of replacing it |
| `start_line` | No | Line reference for line-targeted editing (e.g. `"42"` or `"42:ab"` with hash) |
| `end_line` | No | End of line range, inclusive (e.g. `"55"` or `"55:cd"`). Defaults to `start_line`. |

### Mode Selection Rules (Priority Order)

- If `symbol` is provided, the tool uses AST-aware mode (symbol replace or symbol insert depending on `position`)
- If `start_line` is provided (without `symbol`), the tool uses line-targeted mode
- If `old_string` is provided (without `symbol` or `start_line`), the tool uses text-based mode
- If none are provided, the tool returns an error with guidance

### Text Mode — Find and Replace

Provide `old_string` with text copied from the file and `new_string` with the replacement.

```xml
<edit>
<file_path>src/main.js</file_path>
<old_string>return false;</old_string>
<new_string>return true;</new_string>
</edit>
```

#### Fuzzy Matching

If exact matching fails, the tool automatically tries progressively relaxed matching strategies:

1. **Exact match** — verbatim string comparison
2. **Line-trimmed** — strips leading/trailing whitespace from each line before comparing (handles indentation differences)
3. **Whitespace-normalized** — collapses all runs of whitespace to single spaces (handles extra/missing spaces)
4. **Indent-flexible** — matches code structure regardless of base indentation level (handles different indent depths)

This means you don't need to perfectly match the file's whitespace. For example:

```javascript
// File content:  "    const x = 1;"  (4-space indent)
// Your old_string: "const x = 1;"    (no indent)
// Result: fuzzy match via line-trimmed strategy — edit succeeds
```

The success message tells you which strategy was used:
```
Successfully edited src/main.js (1 replacement, matched via line-trimmed)
```

#### Replace All

When `old_string` appears multiple times and you want to replace all occurrences:

```xml
<edit>
<file_path>config.json</file_path>
<old_string>"debug": false</old_string>
<new_string>"debug": true</new_string>
<replace_all>true</replace_all>
</edit>
```

Without `replace_all`, the tool returns an error if the string appears more than once, with instructions to either add more context or set `replace_all=true`.

### Symbol Replace Mode — Rewrite by Name

Provide `symbol` with the name of a function, class, or method and `new_string` with the complete new implementation. No need to quote the old code.

```xml
<edit>
<file_path>src/utils.js</file_path>
<symbol>calculateTotal</symbol>
<new_string>function calculateTotal(items) {
  return items.reduce((sum, item) => sum + item.price * item.quantity, 0);
}</new_string>
</edit>
```

The tool uses tree-sitter AST parsing to find the symbol by name, then replaces the entire definition (from its first line to its last line) with your `new_string`.

**Supported languages** (16): JavaScript, TypeScript, Python, Rust, Go, Java, C, C++, Ruby, PHP, Swift, Kotlin, Scala, C#, Lua, Zig.

**Auto-indentation**: The tool detects the original symbol's indentation level and reindents your `new_string` to match. Write your code at any indentation — it will be adjusted automatically.

**Symbol naming**: Use the name as it appears in the source code:
- Functions: `"calculateTotal"`, `"handleClick"`
- Classes: `"UserService"`, `"DatabaseConnection"`
- Methods: `"MyClass.myMethod"` (dot notation for class methods)

### Symbol Insert Mode — Add Code Near a Symbol

Provide `symbol`, `new_string`, and `position` (`"before"` or `"after"`) to insert code near an existing symbol.

```xml
<edit>
<file_path>src/utils.js</file_path>
<symbol>calculateTotal</symbol>
<position>after</position>
<new_string>function calculateTax(total, rate) {
  return total * rate;
}</new_string>
</edit>
```

- `position="after"` inserts a blank line after the symbol, then the new code
- `position="before"` inserts the new code and a blank line before the symbol

Indentation is automatically adjusted to match the reference symbol.

### Line-Targeted Mode — Edit by Line Number

Use line numbers from `extract` or `search` output to make precise edits. Ideal for editing inside large functions without rewriting the entire symbol.

#### Replace a Line

```xml
<edit>
<file_path>src/main.js</file_path>
<start_line>42</start_line>
<new_string>  return processItems(order.items);</new_string>
</edit>
```

#### Replace a Range of Lines

```xml
<edit>
<file_path>src/main.js</file_path>
<start_line>42</start_line>
<end_line>55</end_line>
<new_string>  // simplified implementation
  return processItems(order.items);</new_string>
</edit>
```

#### Insert After a Line

```xml
<edit>
<file_path>src/main.js</file_path>
<start_line>42</start_line>
<position>after</position>
<new_string>  const validated = validate(input);</new_string>
</edit>
```

#### Delete Lines

```xml
<edit>
<file_path>src/main.js</file_path>
<start_line>42</start_line>
<end_line>45</end_line>
<new_string></new_string>
</edit>
```

#### Hash-Based Integrity Verification

When `allowEdit` is enabled, `hashLines` is on by default — search/extract output includes content hashes (e.g. `42:ab | function foo() {}`). Use these in your edit references for integrity verification:

```xml
<edit>
<file_path>src/main.js</file_path>
<start_line>42:ab</start_line>
<end_line>55:cd</end_line>
<new_string>  return processItems(order.items);</new_string>
</edit>
```

If the hash doesn't match (file changed since last read), the error includes the updated reference so you can retry.

#### Heuristic Auto-Corrections

Line-targeted edits automatically correct common LLM mistakes:

1. **Prefix stripping**: If `new_string` contains `42:ab | code`, the line-number prefixes are stripped
2. **Echo stripping**: If `new_string` echoes boundary lines (the line before/after the edit range), they are removed
3. **Indent restoration**: If `new_string` has different base indentation than the original lines, it's reindented to match

The success message notes any auto-corrections applied, e.g.: `[auto-corrected: stripped line-number prefixes, stripped echoed line before range]`

## Create Tool

Creates new files with specified content. Parent directories are created automatically.

### Parameters

| Parameter | Required | Description |
|-----------|----------|-------------|
| `file_path` | Yes | Path where the file should be created |
| `content` | Yes | Content to write to the file |
| `overwrite` | No | Whether to overwrite if file exists (default: `false`) |

### Examples

```xml
<create>
<file_path>src/newFile.js</file_path>
<content>export function hello() {
  return "Hello, world!";
}</content>
</create>
```

Overwrite an existing file:

```xml
<create>
<file_path>src/config.json</file_path>
<content>{"debug": true, "verbose": false}</content>
<overwrite>true</overwrite>
</create>
```

## Enabling Edit Tools

### CLI Agent

```bash
# Enable with --allow-edit flag
probe agent "Fix the bug in auth.js" --allow-edit --path ./my-project

# Enable with environment variable
ALLOW_EDIT=1 probe agent "Refactor the login flow" --path ./my-project

# Combine with other options
probe agent --allow-edit --enable-bash --path ./src "Set up a new React component with tests"

# hashLines is on by default with --allow-edit; disable with --no-hash-lines
probe agent --allow-edit --no-hash-lines --path ./src "Fix the bug"
```

### SDK — ProbeAgent

```javascript
import { ProbeAgent } from '@probelabs/probe';

const agent = new ProbeAgent({
  path: '/path/to/project',
  allowEdit: true,       // enables edit + create tools
  // hashLines defaults to true when allowEdit is true (disable with hashLines: false)
  provider: 'anthropic',
  allowedFolders: ['./src', './tests']  // restrict to specific directories
});

// The agent can now modify files when answering questions
const answer = await agent.answer("Fix the off-by-one error in calculateTotal");
```

Both `allowEdit: true` AND the tool being permitted by `allowedTools` are required:

```javascript
// Edit tools available — allowEdit=true and edit is in allowedTools
const agent1 = new ProbeAgent({
  allowEdit: true,
  allowedTools: ['search', 'edit', 'create', 'attempt_completion']
});

// Edit tools NOT available — not in allowedTools list
const agent2 = new ProbeAgent({
  allowEdit: true,
  allowedTools: ['search', 'extract']  // edit/create not listed
});
```

### SDK — Standalone Tools

Use the tools directly without the ProbeAgent wrapper:

```javascript
import { editTool, createTool } from '@probelabs/probe';

const edit = editTool({
  allowedFolders: ['/path/to/project'],
  cwd: '/path/to/project',
  debug: false
});

const create = createTool({
  allowedFolders: ['/path/to/project'],
  cwd: '/path/to/project'
});

// Text edit
const result1 = await edit.execute({
  file_path: 'src/main.js',
  old_string: 'return false;',
  new_string: 'return true;'
});
// => "Successfully edited src/main.js (1 replacement)"

// Symbol replace
const result2 = await edit.execute({
  file_path: 'src/utils.js',
  symbol: 'calculateTotal',
  new_string: `function calculateTotal(items) {
  return items.reduce((sum, item) => sum + item.price * item.quantity, 0);
}`
});
// => "Successfully replaced symbol "calculateTotal" in src/utils.js (was lines 10-15, now 3 lines)"

// Symbol insert
const result3 = await edit.execute({
  file_path: 'src/utils.js',
  symbol: 'calculateTotal',
  position: 'after',
  new_string: `function calculateTax(total, rate) {
  return total * rate;
}`
});
// => "Successfully inserted 3 lines after symbol "calculateTotal" in src/utils.js (at line 16)"

// Create file
const result4 = await create.execute({
  file_path: 'src/newModule.js',
  content: 'export function greet(name) { return `Hello, ${name}!`; }'
});
// => "Successfully created src/newModule.js (58 bytes)"
```

### Delegate Subagents

When using the `delegate` tool, subagents automatically inherit `allowEdit` from the parent agent. No extra configuration needed.

## Security

### Allowed Folders

Both tools enforce `allowedFolders` restrictions. File operations outside allowed directories are blocked:

```javascript
const edit = editTool({
  allowedFolders: ['/project/src', '/project/tests']
});

// This works — path is within allowed folders
await edit.execute({ file_path: '/project/src/main.js', ... });

// This fails — "Permission denied - ../../etc/passwd is outside allowed directories"
await edit.execute({ file_path: '/etc/passwd', ... });
```

Path traversal attacks (e.g. `../../../etc/passwd`) are detected and blocked. Symlinks are resolved before checking permissions.

### Default Disabled

File modification tools are disabled by default. They require explicit opt-in via `--allow-edit` (CLI) or `allowEdit: true` (SDK).

### Dual Authorization

Tools require both the `allowEdit` flag AND the tool being permitted in `allowedTools`. This prevents accidental enabling when tool filtering is used.

## Error Handling — Self-Healing Messages

All error messages include specific recovery instructions. When an edit fails, the error tells the LLM (or you) exactly how to fix the call and retry.

### Error Reference

| Error | Cause | Recovery Instructions in Message |
|-------|-------|----------------------------------|
| **Invalid file_path** | Empty or non-string path | Suggests correct format with example |
| **Invalid new_string** | Missing or non-string value | Notes that empty string is valid for deletions |
| **Permission denied** | Path outside allowed directories | Tells you to use a path within the workspace |
| **File not found** | File doesn't exist | Suggests `search` to find files, `create` to make new ones |
| **Invalid symbol** | Empty or non-string symbol | Shows valid format examples, suggests text mode fallback |
| **Invalid position** | Not "before" or "after" | Explains each option, notes omitting replaces instead |
| **Symbol not found** | Symbol doesn't exist in file | Suggests `search`/`extract` to find correct name, offers text mode fallback |
| **Must provide old_string or symbol** | Neither parameter provided | Explains both modes with examples |
| **String not found** | old_string not in file (even with fuzzy matching) | Three steps: read file content, try symbol mode, verify path |
| **Multiple occurrences** | old_string appears N times | Two options: set `replace_all=true`, or add more context |
| **No changes made** | Replacement identical to original | Explains fuzzy matching edge case |

### Example Error Flow

```
1. LLM calls: edit(file_path="src/utils.js", symbol="calcTotal", new_string="...")
2. Error: Symbol "calcTotal" not found in src/utils.js. Verify the symbol name matches
   a top-level function, class, method, or other named definition exactly as declared in
   the source. Use 'search' or 'extract' to inspect the file and find the correct symbol
   name. Alternatively, use old_string + new_string for text-based editing instead.
3. LLM reads the error, uses extract to check the file, finds the symbol is "calculateTotal"
4. LLM retries: edit(file_path="src/utils.js", symbol="calculateTotal", new_string="...")
5. Success!
```

## Best Practices

### Choosing the Right Mode

1. **Small, surgical edits** → Text mode (`old_string` + `new_string`)
   - Renaming a variable, fixing a condition, updating a value
   - Works best when you can copy the exact text from the file

2. **Rewriting entire definitions** → Symbol mode (`symbol` + `new_string`)
   - Replacing a whole function, class, or method
   - No need to match the old code exactly — just provide the name
   - Indentation is handled automatically

3. **Adding new code** → Symbol insert (`symbol` + `new_string` + `position`)
   - Adding a new function next to a related one
   - Adding imports or comments near specific code

4. **Editing inside large functions** → Extract symbol + line-targeted edit
   - First: `extract` with a symbol target (e.g. `"src/main.js#processOrder"`) to see the function with line numbers
   - Then: `edit` with `start_line`/`end_line` to surgically modify specific lines within it
   - With `allowEdit`, `hashLines` is on by default — extract output includes content hashes (e.g. `42:ab |`) for integrity verification in the edit call

### Workflow

```
1. Use 'search' to find relevant files and code
2. Use 'extract' to see the full context (exact content with line numbers)
3. Choose the appropriate edit mode:
   - Copy exact text for old_string (text mode)
   - Use the symbol name directly (symbol mode)
   - Use line numbers from extract/search output (line-targeted mode)
4. For large functions: extract the symbol first, then use line-targeted edits
5. If edit fails, read the error message and follow its instructions
6. Use 'extract' again to verify the change
```

### Common Patterns

**Rename a function** (text mode is simpler here):
```xml
<edit>
<file_path>src/utils.js</file_path>
<old_string>function oldName(</old_string>
<new_string>function newName(</new_string>
<replace_all>true</replace_all>
</edit>
```

**Rewrite a function** (symbol mode avoids quoting the old code):
```xml
<edit>
<file_path>src/utils.js</file_path>
<symbol>processData</symbol>
<new_string>async function processData(input) {
  const validated = validate(input);
  return await transform(validated);
}</new_string>
</edit>
```

**Add a helper function** (symbol insert places it logically):
```xml
<edit>
<file_path>src/utils.js</file_path>
<symbol>processData</symbol>
<position>before</position>
<new_string>function validate(input) {
  if (!input) throw new Error('Input required');
  return input;
}</new_string>
</edit>
```

## File State Tracking (Multi-Edit Safety)

When using ProbeAgent with `allowEdit: true`, the system automatically tracks which files and symbols the LLM has seen via `search` or `extract`. This uses two-tier content-aware tracking to prevent edits based on stale content while allowing edits to proceed when only unrelated parts of a file changed.

### How It Works

1. **File-level "seen" tracking**: Every `extract` and `search` call marks files as "seen". Before any `edit`, the tracker checks if the file was previously read. If not, the edit is blocked with a message guiding the LLM to use `extract` first
2. **Symbol-level content hashing**: When extracting a symbol (e.g., `file.js#myFunction`), the system computes a SHA-256 content hash of the symbol's code. Before a symbol edit, it re-reads the symbol via tree-sitter AST and compares the hash. If the symbol changed, the edit is blocked
3. **Smart staleness**: Edits proceed when the target symbol hasn't changed, even if other parts of the file were modified. This is more granular than file-level mtime tracking
4. **Chained edit support**: After every successful symbol write, the tracker re-reads the symbol to capture its new content hash and position, enabling subsequent edits to the same symbol

### Error Messages

| Scenario | Error Message |
|----------|--------------|
| **File never read** | `This file has not been read yet in this session. Use 'extract' to read the file first...` |
| **Symbol changed** | `Symbol "foo" has changed since you last read it. Use extract to re-read...` |

Both error messages include an `<extract>` example for the LLM to recover automatically.

### Standalone Usage

When using `editTool()` directly (without ProbeAgent), file tracking is not enabled — all edits proceed without staleness checks, preserving backward compatibility.

To enable tracking for standalone tools, pass a `FileTracker` instance via options:

```javascript
import { editTool } from '@probelabs/probe';
import { FileTracker } from '@probelabs/probe';

const tracker = new FileTracker();
const edit = editTool({
  allowedFolders: ['/path/to/project'],
  fileTracker: tracker
});

// Mark a file as seen before editing
tracker.markFileSeen('/path/to/project/src/main.js');

// Edit succeeds — file was seen
await edit.execute({
  file_path: 'src/main.js',
  old_string: 'return false;',
  new_string: 'return true;'
});
// Tracker is automatically updated after write, so chained edits work
```

### Design Decisions

- **Content hash, not mtime**: SHA-256 of normalized content detects actual changes, not filesystem metadata. Enables "symbol unchanged = edit allowed" even when other parts of the file changed.
- **Two-tier tracking**: File-level "seen" flag (from search) guards against blind edits. Symbol-level content hashes (from extract) verify staleness at the right granularity.
- **Per-session isolation**: Each ProbeAgent instance gets its own FileTracker. Subagents get their own tracker too (natural isolation via fresh ProbeAgent instances).
- **Graceful degradation**: When `fileTracker` is `undefined` (standalone tools, read-only sessions), all checks are no-ops.

## Limitations

- **Single file per call**: Each tool call operates on one file
- **Symbol mode requires tree-sitter support**: Only works with the 16 supported languages
- **Symbol mode finds definitions only**: Cannot target variable declarations, imports, or arbitrary code blocks — use text mode for those
- **Fuzzy matching is not semantic**: It handles whitespace/indentation differences but not code reformatting (e.g. single-line vs multi-line)
- **CRLF edge case**: Files with `\r\n` line endings may not fuzzy-match when the search uses `\n` — use exact text with matching line endings

## Testing

```bash
# Run all edit/create tool tests
cd npm && NODE_OPTIONS=--experimental-vm-modules npx jest tests/unit/edit-create-tools.test.js

# Run symbol mode tests
cd npm && NODE_OPTIONS=--experimental-vm-modules npx jest tests/unit/symbol-edit-tools.test.js

# Run fuzzy matching tests
cd npm && NODE_OPTIONS=--experimental-vm-modules npx jest tests/unit/fuzzy-match.test.js

# Run line-targeted mode tests
cd npm && NODE_OPTIONS=--experimental-vm-modules npx jest tests/unit/line-edit-mode.test.js

# Run hashline utility tests
cd npm && NODE_OPTIONS=--experimental-vm-modules npx jest tests/unit/hashline.test.js

# Run file tracker tests
cd npm && NODE_OPTIONS=--experimental-vm-modules npx jest tests/unit/fileTracker.test.js

# Run line-edit heuristics tests
cd npm && NODE_OPTIONS=--experimental-vm-modules npx jest tests/unit/line-edit-heuristics.test.js

# Run XML parsing tests (includes edit tool XML parsing)
cd npm && NODE_OPTIONS=--experimental-vm-modules npx jest tests/unit/xmlParsing.test.js

# Run all npm tests
cd npm && npm test
```
