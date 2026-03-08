# Tree-Sitter Position Test Fixtures

This directory contains test fixtures for validating tree-sitter position extraction across different programming languages. These fixtures are used by the comprehensive test suite in `tests/tree_sitter_positions.rs`.

## Overview

The position tests validate that:
1. Symbol positions are extracted consistently across languages
2. Positions follow tree-sitter's 0-based coordinate system
3. Identifiers are located at their exact token positions
4. Different symbol types are correctly identified per language

## Test Fixtures

### Language-Specific Fixtures

- **rust_positions.rs** - Rust language constructs (functions, structs, traits, enums, etc.)
- **javascript_positions.js** - JavaScript constructs (functions, classes, arrow functions, etc.)
- **typescript_positions.ts** - TypeScript constructs (interfaces, types, namespaces, etc.)
- **go_positions.go** - Go constructs (functions, structs, interfaces, methods, etc.)
- **python_positions.py** - Python constructs (functions, classes, decorators, etc.)
- **java_positions.java** - Java constructs (classes, interfaces, methods, enums, etc.)
- **c_positions.c** - C constructs (functions, structs, unions, enums, etc.)
- **cpp_positions.cpp** - C++ constructs (classes, namespaces, templates, etc.)

### Position Patterns Discovered

Through comprehensive testing, we've documented these key patterns:

#### 1. Coordinate System
- Tree-sitter uses **0-based** line and column numbers
- Line 1, column 1 in editor = line 0, column 0 in tree-sitter
- All positions are relative to file start

#### 2. Identifier Position Rules
- Symbol positions point to the **exact start** of the identifier token
- NOT the start of the declaration (e.g., `pub fn` vs `fn`)
- Column position is the exact character where identifier begins

#### 3. Language-Specific Node Types

**Rust:**
- `function_item`, `struct_item`, `impl_item`, `trait_item`
- `enum_item`, `type_item`, `const_item`, `static_item`
- `mod_item`, `macro_definition`

**JavaScript:**
- `function_declaration`, `function_expression`, `arrow_function`
- `method_definition`, `class_declaration`, `variable_declarator`
- `export_statement`

**TypeScript:**
- All JavaScript types plus:
- `interface_declaration`, `type_alias_declaration`
- `enum_declaration`, `namespace_declaration`

**Go:**
- `function_declaration`, `method_declaration`, `type_declaration`
- `const_declaration`, `var_declaration`, `package_clause`

**Python:**
- `function_definition`, `class_definition`, `assignment`

**Java:**
- `method_declaration`, `constructor_declaration`, `class_declaration`
- `interface_declaration`, `enum_declaration`, `field_declaration`

**C/C++:**
- `function_definition`, `function_declarator`
- `struct_specifier`, `union_specifier`, `enum_specifier`
- `class_specifier` (C++), `namespace_definition` (C++)
- `template_declaration` (C++), `typedef_declaration`

#### 4. Position Consistency
- Multiple parses of the same file produce identical positions
- Positions are deterministic across runs
- Parent node boundaries always encompass identifier positions

#### 5. Edge Cases Handled
- Single character identifiers (e.g., `fn a()`)
- Unicode identifiers (e.g., `fn 测试()`)
- Very long function names
- Nested structures (methods within classes/impl blocks)

## Usage in Tests

The test suite validates:
1. **Symbol Discovery** - Key symbols are found for each language
2. **Position Validation** - Positions are within reasonable bounds
3. **Consistency** - Multiple extractions produce identical results
4. **Edge Cases** - Unicode, short names, and nested structures work
5. **Pattern Documentation** - Exact position rules are verified

## Implementation Notes

- Uses `probe_code::language::factory::get_language_impl()` for language detection
- Tree-sitter parsers are managed through the parser pool
- Symbol extraction is recursive through the AST
- Keyword filtering prevents false positives (e.g., `fn`, `class` keywords)

## Future Enhancements

Consider adding fixtures for:
- Ruby constructs (methods, classes, modules)
- PHP constructs (functions, classes, traits)
- Swift constructs (functions, classes, protocols)
- More complex nested structures
- Generic/template constructs across languages