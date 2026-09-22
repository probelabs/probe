# Bash and QML Language Support Plan

Status: implemented on `feat/bash-qml-support` (search/extract layer in
`961eba11` + `f2c5772d`; LSP-daemon/query/indexing parity layer in the follow-up
commits). This document mirrors `crystal-language-support-plan.md` (PR #571,
commit `53ed5860`) and records the analogous design decisions for Bash and QML,
including the surfaces where parity was deliberately skipped.

## Scope and Baseline

The branch is based on `88434b11` (v0.6.0-rc312), which **predates both the
Solidity PR (#563) and the Crystal PR (#571)**. Consequences:

- `src/semantic_context.rs` does not exist on this branch (introduced by #563).
  The Crystal PR's one-line `semantic_context` addition has no analog here;
  skipped, and the source-context test from `crystal_language_tests.rs` is
  replaced with a `SearchFilters`-only alias test.
- No Solidity match arms exist anywhere; every bash/qml arm was added against
  the pre-Solidity map shapes.
- The Crystal PR's `is_keyword_or_invalid` helper already existed in this
  branch's `lsp_database_adapter.rs`; it was also ported into
  `analyzer/tree_sitter_analyzer.rs` (with QML keywords `property`/`signal`/
  `readonly`/`required` and bash keywords `local`/`declare`/`typeset` added)
  because QML `ui_property`/`function_declaration` name extraction otherwise
  returned keywords as symbol names. Name extraction there also prefers the
  grammar's `name` field, as in the Crystal PR.

## Dependency Plan

- Root crate: `tree-sitter-bash = "0.23.3"`, `tree-sitter-qmljs = "0.3.1"`
  (added in `961eba11`). Both expose the tree-sitter 0.24 `LANGUAGE` constant.
- `lsp-daemon/Cargo.toml`: same two crates added (crates.io versions, not git
  pins — unlike Crystal's git-pinned grammar, both grammars have published
  crates compatible with tree-sitter 0.24).
- `ast-grep-language 0.36` **bundles Bash** (`SupportLang::Bash`), so Bash
  needs no custom wiring in `src/query.rs`. ast-grep has **no QML support**, so
  QML follows the Crystal pattern: a `ProbeQueryLang` wrapper enum
  (`Builtin(SupportLang)` / `Qml`) implementing `ast_grep_core::Language` with
  `tree_sitter_qmljs::LANGUAGE`.

## Node-Kind Choices

### Bash (tree-sitter-bash)

| Node kind | Symbol kind |
|---|---|
| `function_definition` | Function |
| `variable_assignment` | Variable |
| `declaration_command` (`local`/`readonly`/`export`/`declare`/`typeset`) | Variable |

- Scope-creating nodes: `function_definition`, `if_statement`, `case_statement`,
  `for_statement`, `while_statement`, `subshell`, `compound_statement`.
- Identifier nodes differ from other languages: function names are `word`
  nodes, assignment names are `variable_name` nodes. Both were added to
  `is_identifier_node` / name extraction paths in the daemon.
- Bash has **no namespace constructs**: `is_namespace_node` returns `false` for
  `sh`/`bash` (FQN is flat, `.` separator).

### QML (tree-sitter-qmljs)

| Node kind | Symbol kind |
|---|---|
| `ui_object_definition`, `ui_object_definition_binding`, `ui_inline_component` | Class (component object) |
| `ui_property`, `ui_binding` | Field |
| `ui_signal` | Method (signal is callable-shaped) |
| `ui_import` | Import |
| `function_declaration`, `method_definition` (embedded JS) | Function |

- `ui_object_definition` is the enclosing component scope for FQN purposes
  (`.` separator).
- The daemon `SymbolKind` enum has no `Property`/`Event` variants, so QML
  properties map to `Field` and signals to `Method`.

## Query Support (`src/query.rs`)

- `get_language` / `get_file_extension` / auto-detect arms:
  `bash`/`sh` → `SupportLang::Bash` (extensions `.sh`, `.bash`);
  `qml` → `ProbeQueryLang::Qml` (extension `.qml`).
- **Known grammar limitation (verified empirically):** tree-sitter-qmljs only
  forms standalone ast-grep pattern nodes for object definitions. Patterns like
  `Item {\n  $$$\n}` or `MouseArea {\n  $$$\n}` match; standalone
  property/signal/JS-statement patterns (`property int count: 0`,
  `signal foo(...)`, `count += 1`) do not parse as single pattern nodes and
  return zero matches. Tests therefore use object patterns. Bash patterns
  (functions, statements) work normally.

## Search and Extraction Wiring

- `filters.rs`: `sh` normalizes to `bash`; `bash`/`sh` and `qml` extension
  sets added to `get_extensions_for_type` (drives `lang:` hints).
- `search_runner.rs` + `main.rs`: `sh` → `bash` alias normalization.
- `file_list_cache.rs`: test-file globs (`test_*.sh`, `*_test.sh`, `*.bats`,
  `tst_*.qml`) plus the Crystal PR's `is_test_path` guard ported over (it
  handles searching a test directory *directly*, which glob overrides miss)
  extended with `.bats` and `tst_*.qml`.
- `test_detection.rs`: `test_*.sh` / `*_test.sh` / `*.bats` and Qt Test
  `tst_*.qml` file patterns.
- `extract/symbols.rs`: bash/qml node kinds added to `normalize_kind`
  (`variable`, `class`/`component`, `property`, `binding`, `signal`, `import`,
  `pragma`) and `is_container_node` / body kinds (`ui_object_initializer`).
- **Skipped: the Crystal `elastic_query.rs` `::` namespace tokenizer change.**
  It exists so `HTTP::Server`-style Crystal constants parse as one term.
  Neither Bash nor QML uses `::` namespacing, so there is nothing to port; a
  `lang:bash`/`lang:qml` field-hint parse test was added to
  `elastic_query_tests.rs` instead.
- **Skipped: `.github/workflows/lsp-tests.yml`.** The Crystal PR's only change
  there was an unrelated PHP 8.1→8.2 CI bump. There is no per-language matrix
  to extend; neither Crystal nor this change installs its LSP server in CI.
- `parser_pool.rs`: `sh` and `qml` added to tier-3 warm lists.

## LSP Daemon Integration

Decision: **register real LSP servers for both languages**, matching the
Crystal PR's approach (crystalline), since both languages have usable servers:

- **Bash → `bash-language-server start`** (npm `bash-language-server`).
  Root marker: `.git` only — Bash has no package manifest. Capabilities:
  `references: true`, no call hierarchy, no implementations.
- **QML → `qmlls`** (ships with the Qt SDK / Qt Creator). Root markers:
  `qmldir`, `*.qmlproject`. Capabilities conservative: `references: false`
  (qmlls reference support is partial/version-dependent), no call hierarchy,
  no implementations.
- Both are marked "⚠️ Configured; install required" in `lsp-daemon/README.md`,
  exactly like Crystal. Tree-sitter indexing works without either server.

Surfaces wired (all analogous to the Crystal diff):

- `language_detector.rs`: `Language::Bash` / `Language::Qml` variants,
  `as_str`/`from_str`, extension map (`sh`, `bash`, `qml`), and a bash shebang
  pattern (`#!/.*\b(?:ba|z)?sh\b`) — new vs Crystal, needed because shell
  scripts are commonly extensionless.
- `analyzer/tree_sitter_analyzer.rs`: parser pool arms, extension→language
  names, `map_bash_node_to_symbol` / `map_qml_node_to_symbol`, scope kinds,
  `supported_languages`, unit tests (parser pool + node mapping + async
  symbol extraction through `analyze_file`).
- `fqn.rs`: parser arms, `language_to_extension`, `.` separator, method/namespace
  node arms (bash: none; qml: `ui_object_definition`).
- `indexing/ast_extractor.rs`: tree-sitter language arms + per-language
  node-kind extraction maps; name extraction prefers the `name` field
  (Crystal pattern).
- `indexing/config.rs`: env-config language loop, default extensions
  (`sh`/`bash`, `qml`), `FromStr`, feature flags (`extract_functions` /
  `extract_variables` for Bash; `extract_bindings` / `extract_signals` for
  QML).
- `indexing/file_detector.rs`: `qml` added (`sh`/`bash` were already listed).
- `indexing/lsp_enrichment_worker.rs`, `indexing/pipelines.rs`: language arms
  (pipeline configs with the same feature flags).
- `lsp_database_adapter.rs`: tree-sitter parser arms, symbol-defining node
  kinds, identifier kinds (`word`, `variable_name`, `constant`),
  node-kind→SymbolKind arms, `language_to_extension`, separators,
  method/namespace arms, field-based `extract_node_name`, and
  `find_symbol_at_position` tests for both languages.
- `lsp_registry.rs`: server configs (above) + string→Language map.
- `lsp_server.rs`: LSP `languageId` mapping (`sh`/`bash` → `bash`,
  `qml` → `qml`).
- `relationship/tree_sitter_extractor.rs`: parser pool arms.
- `symbol/language_support.rs`: `LanguageRules::bash()` / `::qml()` (both
  `.` scope separator, no overloading; QML keywords cover `property`,
  `signal`, `function`, `component`, etc.), factory arms.
- `symbol/uid_generator.rs`: extension→language names + rules registration.
- `workspace/config.rs`, `workspace/project.rs`: supported-language lists and
  extension detection.
- `workspace_resolver.rs`: root markers (see LSP decision above).
- `daemon.rs`: `parse_with_tree_sitter` arms for `sh`/`bash`/`qml`.
- `src/lsp_integration/{client,management,readiness}.rs`: `parse_language`
  / language-string maps (`bash`/`sh`, `qml`).
- `npm/src/agent/acp/tools.js`: language description mentions bash/qml.

Not touched (checked, no language maps): `indexing/language_strategies.rs`,
`analyzer/language_analyzers/*`, `relationship/language_patterns/*` — the
Crystal PR did not touch them either; generic fallbacks cover bash/qml.

## Test Fixtures

- `tests/fixtures/bash/project1/`: `src/deploy.sh` (functions + variables),
  `src/lib/utils.sh` (library functions), `tests/test_utils.bats` (Bats).
- `tests/fixtures/qml/project1/`: `src/Main.qml` (ApplicationWindow with
  properties, signal, functions, nested component), 
  `src/components/RequirementCard.qml` (property alias, readonly property,
  signal, function, nested `Text`/`MouseArea`),
  `tests/tst_requirement_card.qml` (QtTest `TestCase`).

## Test Strategy

`tests/bash_language_tests.rs` and `tests/qml_language_tests.rs` mirror
`tests/crystal_language_tests.rs` (9 tests each):

1. Symbol tree extraction (`extract_symbols`) — QML asserts object children
   (property/signal/function).
2. Extraction by symbol name (`process_file_for_extraction`).
3. Extraction by line target.
4. `query` with explicit `--language`.
5. `query` auto-detection by extension.
6. Search with language filter + test exclusion (`.bats` / `tst_*` excluded).
7. Direct test-directory search returns empty without `--allow-tests`
   (exercises the ported `is_test_path` guard).
8. `lang:` hint search.
9. `SearchFilters` alias behavior (`sh`→`bash`, `qml`).

Plus unit tests in `src/query.rs` (3), `src/debug_tree_sitter.rs` (2),
`src/language/tests.rs` (map arms), `tests/search_hints_tests.rs` (aliases),
and daemon-side tests in `tree_sitter_analyzer.rs` and
`lsp_database_adapter.rs`.

## Verification Checklist

- [x] `cargo +1.96.1 build` (root)
- [x] `cd lsp-daemon && cargo +1.96.1 build`
- [x] `cargo +1.96.1 test --lib`
- [x] `cargo +1.96.1 test --test bash_language_tests --test qml_language_tests`
- [x] lsp-daemon tests for touched modules
- [x] Smoke: `debug-tree-sitter` prints symbol kinds for `.sh` and `.qml`

## Known Limitations

- QML ast-grep patterns only match object definitions (grammar limitation,
  see "Query Support").
- `qmlls` enrichment is conservative (`references: false`); bash-language-server
  and qmlls are optional — all tree-sitter indexing works without them.
- Extensionless shell scripts are detected by shebang in the daemon's
  `LanguageDetector`, but the CLI/search layer remains extension-driven.
