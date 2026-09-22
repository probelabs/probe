# Crystal Language Support Plan

This plan describes how to add first-class Crystal support to Probe. The goal is
parity with existing tree-sitter-backed languages: `search`, `extract`,
`symbols`, `query`, language filtering, source context, documentation, and
best-effort LSP daemon integration.

## Current Feasibility

- Crystal source files use the `.cr` extension.
- The Crystal language repository is `crystal-lang/crystal`.
- The current Crystal release observed during planning was `1.20.2`, published
  on 2026-05-15.
- The likely tree-sitter grammar is
  `https://github.com/crystal-lang-tools/tree-sitter-crystal`.
- `tree-sitter-crystal` has Rust bindings and tree-sitter metadata for `.cr`
  files, but no GitHub release was available during planning.
- `cargo search tree-sitter-crystal --limit 5` returned no crates.io package in
  this environment, so implementation should assume a pinned git dependency
  unless crates.io availability changes.
- Probe currently uses `tree-sitter = "0.24.5"`, so the first implementation
  task must verify grammar/runtime compatibility before broader wiring.

## Branching

Start from latest `main`, not from an active feature branch:

```bash
git fetch origin
git checkout main
git pull --ff-only origin main
git checkout -b add-crystal-language-support
```

If the work starts while another PR branch is checked out, keep those changes
separate and do not commit Crystal work on top of the old branch.

## Dependency Plan

Add `tree-sitter-crystal` to both Rust crates that directly use tree-sitter
grammars:

- `Cargo.toml`
- `lsp-daemon/Cargo.toml`

Expected shape if no crates.io release exists:

```toml
tree-sitter-crystal = { git = "https://github.com/crystal-lang-tools/tree-sitter-crystal", rev = "<pinned-commit>" }
```

Use a specific commit rather than a floating branch. After adding the dependency,
run at least:

```bash
cargo check -p probe-code
cargo check -p lsp-daemon
```

If the grammar fails with a tree-sitter ABI/runtime mismatch, try an older
grammar commit before considering vendoring. Do not proceed with the full
integration until a stable dependency path is proven.

Implementation note: `50ca9e6fcfb16a2cbcad59203cfd8ad650e25c49` built but
failed at runtime with tree-sitter language ABI 15. Pinning `f71f4ca62ac0`
keeps the Rust `LANGUAGE` binding and uses language ABI 14, which is compatible
with Probe's current tree-sitter runtime.

## Core Language Integration

Add a Crystal implementation:

- `src/language/crystal.rs`
- `src/language/mod.rs`
- `src/language/factory.rs`

The language module should follow the style of `src/language/solidity.rs` and
`src/language/ruby.rs`, because Crystal is syntactically Ruby-like but needs
explicit symbol support beyond Ruby's current minimal implementation.

`CrystalLanguage::get_tree_sitter_language()` should return:

```rust
tree_sitter_crystal::LANGUAGE.into()
```

Verify the exact exported symbol from the dependency. The upstream Rust binding
observed during planning exposes `LANGUAGE`.

Candidate symbol node kinds from `tree-sitter-crystal`:

- `class_def`
- `module_def`
- `struct_def`
- `enum_def`
- `method_def`
- `abstract_method_def`
- `macro_def`
- `lib_def`
- `fun_def`
- `alias`
- `annotation_def`
- `type_def`
- `union_def`

Implement:

- `is_acceptable_parent()`
- `is_symbol_node()`
- `is_test_node()`
- `find_parent_function()`
- `get_symbol_signature()`

Recommended parent/function handling:

- Treat `method_def`, `abstract_method_def`, `macro_def`, and `fun_def` as
  function-like parents.
- Treat `class_def`, `module_def`, `struct_def`, `enum_def`, and `lib_def` as
  containers.
- Include `alias`, `annotation_def`, `type_def`, and `union_def` as symbol
  nodes even if they are not large extraction parents.

Recommended test detection:

- Test files: rely on existing file-level test directory and naming filters
  where possible.
- Crystal spec files commonly use `_spec.cr`; ensure existing test-file
  detection catches that extension pattern.
- Node-level tests should detect common spec DSL calls such as `describe`,
  `context`, `it`, and `pending` where the grammar exposes call nodes clearly.

Recommended signature handling:

- For container nodes, return the declaration header and replace the body with
  a compact form such as `class User ... end` or `module API ... end`.
- For method-like nodes, return the `def` or `macro` signature without the body.
- For aliases, annotations, type defs, and union defs, return the full one-line
  declaration when possible.

## Query Support

Update `src/query.rs`.

`ast-grep-language` may not include Crystal. If not, extend the existing local
wrapper pattern:

```rust
enum ProbeQueryLang {
    Builtin(SupportLang),
    Solidity,
    Crystal,
}
```

Then map:

- `crystal`
- `cr`

to `ProbeQueryLang::Crystal`, and return
`tree_sitter_crystal::LANGUAGE.into()` from `get_ts_language()`.

Add `.cr` to query file extension matching and auto-detection. Verify both:

```bash
cargo run -- query 'def active? : Bool' tests/fixtures/crystal/project1 --language crystal
cargo run -- query 'class User < Serializable' tests/fixtures/crystal/project1
```

## Search and Extraction Wiring

Add Crystal mappings wherever Probe maps languages to extensions or display
names:

- `src/cli.rs`
- `src/main.rs`
- `src/semantic_context.rs`
- `src/search/filters.rs`
- `src/search/file_list_cache.rs`
- `src/search/results_formatter.rs`
- `src/search/search_output.rs`
- `src/extract/formatter.rs`

Expected mappings:

- Language names: `crystal`, `cr`
- Extension: `.cr`
- Syntax label: `crystal`
- Comment prefix: `#`

Search language filters must work through both CLI option and query hints:

```bash
cargo run -- search "HTTP::Server" tests/fixtures/crystal/project1 --language crystal --no-gitignore
cargo run -- search "HTTP::Server AND lang:crystal" tests/fixtures/crystal/project1 --no-gitignore
```

## LSP Daemon Integration

Crystal LSP support should be best-effort and must not block tree-sitter support
unless the ticket specifically requires LSP behavior.

Update:

- `lsp-daemon/src/language_detector.rs`
- `lsp-daemon/src/lsp_registry.rs`
- `lsp-daemon/src/lsp_server.rs`
- `lsp-daemon/src/workspace_resolver.rs`
- `lsp-daemon/src/indexing/pipelines.rs`
- `lsp-daemon/src/indexing/lsp_enrichment_worker.rs`
- `lsp-daemon/src/lsp_database_adapter.rs`

Add:

- `Language::Crystal`
- `Language::Crystal.as_str() == "crystal"`
- `.cr` extension detection
- LSP `languageId = "crystal"`
- Workspace markers: `shard.yml`, `shard.lock`
- Pipeline extension list: `["cr"]`
- Tree-sitter parser map for `crystal` and `cr`

LSP server candidates to evaluate:

- `crystalline`
- `crystal-language-server`

`crystalline` was observed as passively maintained and explicitly limited. Pick
the default only after checking installability and basic initialize/open
behavior. If neither server is reliable in this environment, document Crystal
as tree-sitter supported and LSP configurable by user override.

## Documentation

Update public docs after implementation:

- `README.md`
- `docs/reference/supported-languages.md`
- `docs/reference/adding-languages.md` if the generic checklist changes
- `lsp-daemon/README.md` if a default Crystal LSP is added
- npm MCP/tool descriptions if they enumerate supported languages

Documentation should state Crystal support covers `.cr` files and tree-sitter
AST extraction. Only claim LSP support if an LSP server was configured and
smoke-tested.

## Test Fixtures

Create a realistic Crystal fixture:

```text
tests/fixtures/crystal/project1/
  shard.yml
  src/
    server.cr
    calculator.cr
    models/user.cr
  spec/
    calculator_spec.cr
```

Fixture should include:

- `module`
- `class`
- `struct`
- `enum`
- instance method
- class method
- abstract method
- macro
- alias
- annotation or annotation definition if the grammar handles it cleanly
- `lib`/`fun` declaration if practical
- spec DSL calls in `_spec.cr`

Add a test file such as `tests/crystal_language_tests.rs`, mirroring
`tests/solidity_language_tests.rs`.

Minimum regression tests:

- `extract_symbols()` returns top-level modules/classes and nested methods.
- `process_file_for_extraction(..., Some("symbol_name"), ...)` extracts a
  Crystal method without pulling unrelated methods.
- `perform_query()` supports `--language crystal`.
- `perform_probe()` with `language: Some("crystal")` returns only `.cr` files.
- Test exclusion skips `_spec.cr` when `allow_tests` is false.
- Source context reports `"language": "crystal"` for `.cr` files.
- Language aliases normalize `cr` to `crystal`.

## Required Real-Repository Dogfood

Testing on a real Crystal project is required before raising the PR. Use the
official Crystal compiler repository as the primary dogfood target:

- `https://github.com/crystal-lang/crystal`

Run this after fixture and focused unit tests pass:

```bash
tmpdir=$(mktemp -d)
git clone --depth 1 https://github.com/crystal-lang/crystal "$tmpdir/crystal"
```

Run representative commands against that checkout:

```bash
cargo run -- symbols "$tmpdir/crystal/src/compiler/crystal/compiler.cr"
cargo run -- query 'def run' "$tmpdir/crystal/src" --language crystal --max-results 20
cargo run -- search "SemanticVisitor" "$tmpdir/crystal/src" --language crystal --max-results 20 --no-gitignore
cargo run -- extract "$tmpdir/crystal/src/compiler/crystal/compiler.cr#compile"
```

Adjust exact files and symbol names based on the current upstream tree, but do
not replace this with only synthetic fixtures. At minimum, verify `symbols`,
`extract`, `query`, and `search --language crystal` on files from
`crystal-lang/crystal`. Save the successful command output summaries for the PR
body.

Current branch verification on an up-to-date local `crystal-lang/crystal`
checkout:

- `probe symbols src/compiler/crystal/compiler.cr` extracted `module Crystal`,
  `class Compiler`, enums, nested `CompilationUnit`, and `compile` methods.
- `probe query 'def compile' src/compiler/crystal --language crystal --format json`
  returned 4 method matches across `command.cr`, `compiler.cr`, and
  `interpreter/compiler.cr`.
- `probe query 'def compile' src/compiler/crystal --language cr --max-results 3 --with-context --format json`
  accepted the `cr` alias and returned Crystal query context metadata.
- `probe query 'class Compiler' src/compiler/crystal/compiler.cr --format json`
  auto-detected `.cr` and returned the full `class Compiler` block.
- `probe search 'Crystal::System::Dir AND lang:crystal' . --no-gitignore --max-results 5`
  parsed `Crystal::System::Dir` as a namespaced term and returned only `.cr`
  files.
- `probe search '"Crystal::System::Dir" AND lang:crystal' . --strict-elastic-syntax --max-results 3 --no-gitignore`
  verified strict syntax works for quoted Crystal namespaced constants.
- `probe search 'describe AND lang:crystal' spec --max-results 5 --no-gitignore --format json`
  returned zero results by default, while adding `--allow-tests --max-bytes 700 --max-tokens 250`
  returned Crystal spec blocks within the requested limits.
- `probe extract src/compiler/crystal/compiler.cr#compile --format plain`
  found both `compile` method definitions in `compiler.cr`.
- `probe extract src/compiler/crystal/compiler.cr:228 --format plain`
  extracted the enclosing `compile` method from a line target.
- `probe extract src/compiler/crystal/compiler.cr#compile --dry-run --format plain`
  reported both matching method ranges without returning code.

Current branch LSP verification:

- `cargo test -p lsp-daemon test_crystal_parser_pool_and_node_mapping`
  verified Crystal parser pool creation for `crystal` and `cr`.
- `cargo test -p lsp-daemon test_crystal_symbol_extraction_uses_parser_pool`
  verified the tree-sitter analyzer extracts Crystal module, class, and method
  names instead of keyword tokens.
- `cargo test -p lsp-daemon test_find_symbol_at_position_uses_crystal_tree_sitter`
  verified Crystal `find_symbol_at_position()` resolution for both `crystal`
  and `cr`.
- Crystal LSP tool version checks could not run in this environment because the
  tools are not installed: `crystalline --version`,
  `crystal-language-server --version`, and `crystal --version` all failed with
  `command not found`.

## Verification Checklist

Run focused checks first:

```bash
cargo fmt --all -- --check
cargo test --test crystal_language_tests
cargo test query::tests::test_crystal_query_support
cargo test search::filters::tests::test_normalize_language_names
```

Then run broader checks appropriate to the touched surfaces:

```bash
cargo check --workspace
cargo test --test integration_tests
cargo test --test symbols_tests
cargo test --test query_command_tests
cargo test --test query_command_json_tests
```

If LSP daemon files are changed, add:

```bash
cargo test -p lsp-daemon language_detector
cargo test -p lsp-daemon lsp_registry
cargo test -p lsp-daemon test_crystal_parser_pool_and_node_mapping
cargo test -p lsp-daemon test_crystal_symbol_extraction_uses_parser_pool
cargo test -p lsp-daemon test_find_symbol_at_position_uses_crystal_tree_sitter
crystalline --version
crystal-language-server --version
crystal --version
```

Use exact test names after implementation, because existing test module names
may differ.

## PR Criteria

The PR should not be raised until all of these are true:

- Crystal dependency is pinned and compatible with Probe's tree-sitter runtime.
- `.cr` files are parsed through the normal parser pool.
- `probe symbols` works on fixture and real Crystal files.
- `probe extract` works by symbol name and line target.
- `probe query --language crystal` works through ast-grep.
- `probe search --language crystal` and `lang:crystal` filtering work.
- Test exclusion handles Crystal spec files.
- LSP daemon maps recognize Crystal, or the PR explicitly states LSP is
  configurable/out of scope.
- Docs and supported-language lists are updated.
- Real dogfood on `https://github.com/crystal-lang/crystal` has been run and is
  included in the PR description.

## Known Risks

- `tree-sitter-crystal` may require pinning an exact git commit because it has
  no release in the observed repository state.
- The grammar may use a `tree-sitter-language` binding style that must be
  checked against Probe's existing `LANGUAGE.into()` pattern.
- Crystal LSP options may not provide full call hierarchy or reference
  behavior. Avoid claiming full LSP feature parity without a live smoke test.
- Crystal macros and Ruby-like DSL calls may produce broad AST nodes. Keep
  extraction tests realistic so Probe returns useful blocks rather than entire
  files.
