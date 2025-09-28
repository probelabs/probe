//! Centralized FQN extraction utilities
use anyhow::Result;
use pathdiff::diff_paths;
use std::path::{Component, Path};

use crate::workspace_utils;

/// Extract FQN using tree-sitter AST parsing with optional language hint
pub fn get_fqn_from_ast(
    file_path: &Path,
    line: u32,
    column: u32,
    language_hint: Option<&str>,
) -> Result<String> {
    use std::fs;
    let content = fs::read_to_string(file_path)?;
    get_fqn_from_ast_with_content(file_path, &content, line, column, language_hint)
}

/// Extract FQN using provided file content to avoid I/O (preferred in analyzers)
pub fn get_fqn_from_ast_with_content(
    file_path: &Path,
    content: &str,
    line: u32,
    column: u32,
    language_hint: Option<&str>,
) -> Result<String> {
    // Select parser based on hint or file extension
    let extension = language_hint
        .and_then(language_to_extension)
        .or_else(|| file_path.extension().and_then(|e| e.to_str()))
        .unwrap_or("");

    // Create a simple parser for FQN extraction
    let mut parser = tree_sitter::Parser::new();

    // Set the language based on file extension
    let language = match extension {
        "rs" => Some(tree_sitter_rust::LANGUAGE),
        "py" => Some(tree_sitter_python::LANGUAGE),
        "js" | "jsx" => Some(tree_sitter_javascript::LANGUAGE),
        "ts" | "tsx" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT),
        "java" => Some(tree_sitter_java::LANGUAGE),
        "go" => Some(tree_sitter_go::LANGUAGE),
        "c" => Some(tree_sitter_c::LANGUAGE),
        "cpp" | "cc" | "cxx" => Some(tree_sitter_cpp::LANGUAGE),
        _ => None,
    };

    if let Some(lang_fn) = language {
        parser
            .set_language(&lang_fn.into())
            .map_err(|e| anyhow::anyhow!("Failed to set parser language: {}", e))?;
    } else {
        // No language-specific parser available – use a generic fallback
        let ident = extract_identifier_at(&content, line, column);
        let module = get_generic_module_prefix(file_path);
        return Ok(match (module, ident) {
            (Some(m), Some(id)) if !id.is_empty() => format!("{}::{}", m, id),
            (Some(m), None) => m,
            (None, Some(id)) => id,
            _ => String::new(),
        });
    }

    // Parse the file content
    let tree = parser
        .parse(content.as_bytes(), None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse file"))?;

    // Find node at the specified position
    let root = tree.root_node();
    let point = tree_sitter::Point::new(line as usize, column as usize);
    let node = find_node_at_point(root, point)?;

    // Build FQN by traversing up the AST
    let mut fqn = build_fqn_from_node(node, content.as_bytes(), extension)?;

    // Prepend the path-based package/module information
    if let Some(path_prefix) = get_path_based_prefix(file_path, extension) {
        if !path_prefix.is_empty() {
            if fqn.is_empty() {
                fqn = path_prefix;
            } else {
                fqn = format!("{}::{}", path_prefix, fqn);
            }
        }
    }

    Ok(fqn)
}

/// Map common language names to an extension key used for parser selection
fn language_to_extension(language: &str) -> Option<&'static str> {
    match language.to_lowercase().as_str() {
        "rust" | "rs" => Some("rs"),
        "python" | "py" => Some("py"),
        "javascript" | "js" | "jsx" => Some("js"),
        "typescript" | "ts" | "tsx" => Some("ts"),
        "java" => Some("java"),
        "go" => Some("go"),
        "c" => Some("c"),
        "cpp" | "c++" | "cxx" => Some("cpp"),
        _ => None,
    }
}

/// Generic identifier extraction around a given position (0-based line/column)
fn extract_identifier_at(content: &str, line: u32, column: u32) -> Option<String> {
    let lines: Vec<&str> = content.lines().collect();
    let l = lines.get(line as usize)?.to_string();
    // Work with characters to handle non-ASCII columns more safely
    let chars: Vec<char> = l.chars().collect();
    let mut idx = column as usize;
    if idx >= chars.len() {
        idx = chars.len().saturating_sub(1);
    }

    // Expand left and right to capture [A-Za-z0-9_]+
    let is_ident = |c: char| c.is_alphanumeric() || c == '_';

    let mut start = idx;
    while start > 0 && is_ident(chars[start]) {
        start -= 1;
        if start == 0 && is_ident(chars[start]) {
            break;
        }
    }
    if !is_ident(chars[start]) && start < chars.len().saturating_sub(1) {
        start += 1;
    }

    let mut end = idx;
    while end + 1 < chars.len() && is_ident(chars[end + 1]) {
        end += 1;
    }

    if start <= end && start < chars.len() && end < chars.len() {
        let slice: String = chars[start..=end].iter().collect();
        if !slice.trim().is_empty() {
            return Some(slice);
        }
    }

    // If cursor not on identifier, try the first identifier on the line
    let mut token = String::new();
    for c in chars {
        if is_ident(c) {
            token.push(c);
        } else if !token.is_empty() {
            break;
        }
    }
    if token.is_empty() {
        None
    } else {
        Some(token)
    }
}

/// Find the most specific node at the given point
fn find_node_at_point<'a>(
    node: tree_sitter::Node<'a>,
    point: tree_sitter::Point,
) -> anyhow::Result<tree_sitter::Node<'a>> {
    let mut current = node;

    // Traverse down to find the most specific node containing the point
    loop {
        let mut found_child = false;

        // Walk children with a temporary cursor to avoid borrow issues
        let mut tmp_cursor = current.walk();
        let mut selected_child: Option<tree_sitter::Node<'a>> = None;
        for child in current.children(&mut tmp_cursor) {
            let start = child.start_position();
            let end = child.end_position();

            // Check if point is within this child's range
            if (start.row < point.row || (start.row == point.row && start.column <= point.column))
                && (end.row > point.row || (end.row == point.row && end.column >= point.column))
            {
                selected_child = Some(child);
                found_child = true;
                break;
            }
        }

        if let Some(child) = selected_child {
            current = child;
        }

        if !found_child {
            break;
        }
    }

    Ok(current)
}

/// Build FQN by traversing up the AST and collecting namespace/class/module names
fn build_fqn_from_node(
    start_node: tree_sitter::Node,
    content: &[u8],
    extension: &str,
) -> anyhow::Result<String> {
    let mut components = Vec::new();
    let mut current = Some(start_node);
    let mut method_name_added = false;

    // Detect the language-specific separator
    let separator = get_language_separator(extension);

    // Traverse up from the current node
    while let Some(node) = current {
        // Check if this is a method/function node
        if is_method_node(&node, extension) && !method_name_added {
            if let Some(method_name) = extract_node_name(node, content) {
                // Avoid duplicating method name if it was already added from an identifier node
                let duplicate = components
                    .last()
                    .map(|s| s == &method_name)
                    .unwrap_or(false);
                if !duplicate {
                    components.push(method_name);
                }
                method_name_added = true;
            }
            if let Some(receiver_type) = extract_method_receiver(&node, content, extension) {
                components.push(receiver_type);
            }
        }
        // Namespace/module/class/struct
        else if is_namespace_node(&node, extension) {
            if let Some(name) = extract_node_name(node, content) {
                components.push(name);
            }
        }
        // Initial node fallback: only if it's the starting node AND has an identifier-like name
        else if components.is_empty() && node.id() == start_node.id() {
            if let Some(name) = extract_node_name(node, content) {
                components.push(name);
            }
        }

        current = node.parent();
    }

    // Reverse to get proper order (root to leaf)
    components.reverse();

    Ok(components.join(separator))
}

/// Get language-specific separator for FQN components
fn get_language_separator(extension: &str) -> &str {
    match extension {
        "rs" | "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "rb" => "::",
        "py" | "js" | "ts" | "jsx" | "tsx" | "java" | "go" | "cs" => ".",
        "php" => "\\",
        _ => "::", // Default to Rust-style for unknown languages
    }
}

/// Check if a node represents a method/function
fn is_method_node(node: &tree_sitter::Node, extension: &str) -> bool {
    let kind = node.kind();
    match extension {
        // For Rust, methods and functions are both "function_item"; whether it is a method
        // is determined by having an enclosing impl block (handled separately).
        "rs" => matches!(kind, "function_item"),
        "py" => kind == "function_definition",
        "js" | "ts" | "jsx" | "tsx" => matches!(
            kind,
            "function_declaration" | "method_definition" | "arrow_function"
        ),
        "java" | "cs" => kind == "method_declaration",
        "go" => kind == "function_declaration",
        "cpp" | "cc" | "cxx" => matches!(kind, "function_definition" | "method_declaration"),
        _ => kind.contains("function") || kind.contains("method"),
    }
}

/// Check if a node represents a namespace/module/class/struct
fn is_namespace_node(node: &tree_sitter::Node, extension: &str) -> bool {
    let kind = node.kind();
    match extension {
        // For Rust, exclude impl_item to avoid duplicating receiver type names
        "rs" => matches!(
            kind,
            "struct_item" | "enum_item" | "trait_item" | "mod_item"
        ),
        "py" => matches!(kind, "class_definition" | "module"),
        "js" | "ts" | "jsx" | "tsx" => matches!(
            kind,
            "class_declaration" | "namespace_declaration" | "module"
        ),
        "cpp" | "cc" | "cxx" => matches!(
            kind,
            "class_specifier" | "struct_specifier" | "namespace_definition"
        ),
        _ => {
            // Fallback for unknown languages: try to detect common node types
            kind.contains("class") || kind.contains("struct") || kind.contains("namespace")
        }
    }
}

/// Extract name from a tree-sitter node
fn extract_node_name(node: tree_sitter::Node, content: &[u8]) -> Option<String> {
    // Prefer field-based name if available
    if let Some(name_node) = node.child_by_field_name("name") {
        if let Ok(text) = name_node.utf8_text(content) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    // Otherwise, look for common identifier node types
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "identifier"
            | "field_identifier"
            | "type_identifier"
            | "property_identifier"
            | "scoped_identifier"
            | "scoped_type_identifier"
            | "name"
            | "constant" => {
                if let Ok(text) = child.utf8_text(content) {
                    let t = text.trim();
                    // Skip common keywords/tokens that are not names
                    if !matches!(
                        t,
                        "pub"
                            | "const"
                            | "let"
                            | "var"
                            | "function"
                            | "fn"
                            | "class"
                            | "struct"
                            | "enum"
                            | "impl"
                            | "mod"
                            | "namespace"
                            | "interface"
                            | "trait"
                    ) {
                        return Some(t.to_string());
                    }
                }
            }
            _ => {}
        }
    }

    // Do NOT fall back to raw node text to avoid capturing tokens like 'pub'
    None
}

/// Extract method receiver type (for method FQN construction)
fn extract_method_receiver(
    node: &tree_sitter::Node,
    content: &[u8],
    extension: &str,
) -> Option<String> {
    // Look for receiver/self parameter or parent struct/class
    match extension {
        "rs" => {
            // For Rust, look for impl block parent
            let mut current = node.parent();
            while let Some(parent) = current {
                if parent.kind() == "impl_item" {
                    // Find the type being implemented
                    // In Rust, impl blocks have structure like: impl [TypeParams] Type [where clause] { ... }
                    // We need to find the type, which comes after "impl" and optional type parameters
                    let mut cursor = parent.walk();
                    let mut found_impl_keyword = false;

                    for child in parent.children(&mut cursor) {
                        // Skip the "impl" keyword
                        if child.kind() == "impl" {
                            found_impl_keyword = true;
                            continue;
                        }

                        // Skip generic parameters if present
                        if child.kind() == "type_parameters" {
                            continue;
                        }

                        // The next type-related node after impl (and optional generics) is our target
                        if found_impl_keyword
                            && (child.kind() == "type_identifier"
                                || child.kind() == "scoped_type_identifier"
                                || child.kind() == "scoped_identifier"
                                || child.kind() == "generic_type")
                        {
                            // For generic types, try to extract just the base type name
                            if child.kind() == "generic_type" {
                                let mut type_cursor = child.walk();
                                for type_child in child.children(&mut type_cursor) {
                                    if type_child.kind() == "type_identifier" {
                                        return Some(
                                            type_child.utf8_text(content).unwrap_or("").to_string(),
                                        );
                                    }
                                }
                            }
                            return Some(child.utf8_text(content).unwrap_or("").to_string());
                        }
                    }
                }
                current = parent.parent();
            }
        }
        "py" => {
            // For Python, look for class parent
            let mut current = node.parent();
            while let Some(parent) = current {
                if parent.kind() == "class_definition" {
                    return extract_node_name(parent, content);
                }
                current = parent.parent();
            }
        }
        "java" | "cs" => {
            // For Java/C#, look for class parent
            let mut current = node.parent();
            while let Some(parent) = current {
                if parent.kind() == "class_declaration" {
                    return extract_node_name(parent, content);
                }
                current = parent.parent();
            }
        }
        _ => {}
    }
    None
}

/// Get path-based package/module prefix from file path
fn get_path_based_prefix(file_path: &Path, extension: &str) -> Option<String> {
    match extension {
        "rs" => get_rust_module_prefix(file_path),
        "py" => get_python_package_prefix(file_path),
        "java" => get_java_package_prefix(file_path),
        "go" => get_go_package_prefix(file_path),
        "js" | "ts" | "jsx" | "tsx" => get_javascript_module_prefix(file_path),
        _ => None,
    }
}

/// Rust module prefix from file path
fn get_rust_module_prefix(file_path: &Path) -> Option<String> {
    // 1) Prefer the crate/package name from the nearest Cargo.toml that defines [package]
    if let Some(crate_name) = find_rust_crate_name(file_path) {
        // Use the package name verbatim for display (may contain '-')
        return Some(crate_name);
    }

    // 2) Next, try to derive crate directory name relative to detected workspace root
    if let Some(workspace_root) = crate::workspace_utils::find_workspace_root(file_path) {
        if let Ok(rel) = file_path.strip_prefix(&workspace_root) {
            if let Some(first) = rel.components().next() {
                if let std::path::Component::Normal(os) = first {
                    let name = os.to_string_lossy().to_string();
                    if !name.is_empty() {
                        return Some(name);
                    }
                }
            }
        }
    }

    // 3) Fallback: derive module path after the last 'src/' component
    use std::path::Component;
    let mut seen_src = false;
    let mut parts_after_src: Vec<String> = Vec::new();
    for comp in file_path.components() {
        match comp {
            Component::Normal(os) => {
                let s = os.to_string_lossy();
                if s == "src" {
                    seen_src = true;
                    parts_after_src.clear();
                    continue;
                }
                if seen_src {
                    parts_after_src.push(s.to_string());
                }
            }
            _ => {}
        }
    }

    if parts_after_src.is_empty() {
        return None;
    }

    let mut module_components: Vec<String> = Vec::new();
    if parts_after_src.len() > 1 {
        for dir in &parts_after_src[..parts_after_src.len() - 1] {
            let ident = dir.replace('-', "_");
            if !ident.is_empty() {
                module_components.push(ident);
            }
        }
    }

    if let Some(filename) = file_path.file_name().and_then(|os| os.to_str()) {
        if let Some(stem) = filename.strip_suffix(".rs") {
            if stem != "lib" && stem != "main" && stem != "mod" && !stem.is_empty() {
                module_components.push(stem.replace('-', "_"));
            }
        }
    }

    if module_components.is_empty() {
        None
    } else {
        Some(module_components.join("::"))
    }
}

/// Walk up from file_path to find a Cargo.toml with [package] and return its name
fn find_rust_crate_name(file_path: &Path) -> Option<String> {
    use std::fs;
    let mut current = file_path.parent()?;
    for _ in 0..15 {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(contents) = fs::read_to_string(&cargo_toml) {
                // Skip workspace-only Cargo.toml
                let has_package = contents.contains("[package]");
                if has_package {
                    // Extract name = "..."
                    if let Some(name_line) = contents
                        .lines()
                        .skip_while(|l| !l.trim_start().starts_with("[package]"))
                        .skip(1)
                        .take_while(|l| !l.trim_start().starts_with('['))
                        .find(|l| l.trim_start().starts_with("name"))
                    {
                        // naive parse: name = "value"
                        if let Some(idx) = name_line.find('=') {
                            let value = name_line[idx + 1..].trim();
                            // Strip quotes if present
                            let value = value.trim_matches(|c| c == '"' || c == '\'');
                            if !value.is_empty() {
                                return Some(value.to_string());
                            }
                        }
                    }
                }
            }
        }
        if let Some(parent) = current.parent() {
            current = parent;
        } else {
            break;
        }
    }
    None
}

/// Python package prefix from file path
fn get_python_package_prefix(file_path: &Path) -> Option<String> {
    let path_str = file_path.to_str()?;
    let without_ext = path_str.strip_suffix(".py")?;

    let components: Vec<&str> = without_ext
        .split('/')
        .filter(|&component| !matches!(component, "." | ".." | "" | "__pycache__"))
        .collect();

    if components.is_empty() {
        return None;
    }

    // Convert __init__.py to its parent directory name
    let mut module_components = Vec::new();
    for component in components {
        if component != "__init__" {
            module_components.push(component);
        }
    }

    if module_components.is_empty() {
        None
    } else {
        Some(module_components.join("."))
    }
}

/// Java package prefix from file path
fn get_java_package_prefix(file_path: &Path) -> Option<String> {
    let path_str = file_path.to_str()?;
    let without_ext = path_str.strip_suffix(".java")?;

    // Look for src/main/java pattern or similar
    let components: Vec<&str> = without_ext.split('/').collect();

    // Find java directory and take everything after it
    if let Some(java_idx) = components.iter().position(|&c| c == "java") {
        let package_components: Vec<&str> = components[(java_idx + 1)..].to_vec();
        if !package_components.is_empty() {
            return Some(package_components.join("."));
        }
    }

    None
}

/// Go package prefix from file path (directory name)
fn get_go_package_prefix(file_path: &Path) -> Option<String> {
    file_path
        .parent()?
        .file_name()?
        .to_str()
        .map(|s| s.to_string())
}

/// JavaScript/TypeScript module prefix from file path
fn get_javascript_module_prefix(file_path: &Path) -> Option<String> {
    // Determine a workspace root so we can normalize the path. For JavaScript projects this
    // typically spots a package.json, but the helper also handles generic fallbacks.
    let workspace_root = workspace_utils::find_workspace_root_with_fallback(file_path).ok();

    // Compute a path relative to the workspace root when possible to avoid leaking absolute
    // directories such as "/home/..." into the FQN.
    let mut relative_path = if let Some(root) = workspace_root.as_ref() {
        if let Ok(stripped) = file_path.strip_prefix(root) {
            stripped.to_path_buf()
        } else {
            diff_paths(file_path, root).unwrap_or_else(|| file_path.to_path_buf())
        }
    } else {
        file_path.to_path_buf()
    };

    // Remove the file extension; only proceed for common JS/TS extensions.
    match relative_path.extension().and_then(|ext| ext.to_str()) {
        Some("tsx") | Some("jsx") | Some("ts") | Some("js") => {
            relative_path.set_extension("");
        }
        _ => return None,
    }

    // Exclude common folder names that don't add semantic value to the module path.
    const IGNORED: [&str; 12] = [
        "",
        ".",
        "..",
        "src",
        "lib",
        "components",
        "pages",
        "utils",
        "node_modules",
        "dist",
        "build",
        "public",
    ];

    let mut components: Vec<String> = Vec::new();
    for component in relative_path.components() {
        if let Component::Normal(os) = component {
            let value = os.to_string_lossy();
            if IGNORED.contains(&value.as_ref()) || value.starts_with('.') {
                continue;
            }
            components.push(value.replace('-', "_"));
        }
    }

    // Drop a trailing "index" when it is part of the path and we already have a directory prefix.
    if components.len() > 1 {
        if let Some(last) = components.last() {
            if last.eq_ignore_ascii_case("index") {
                components.pop();
            }
        }
    }

    if components.is_empty() {
        None
    } else {
        Some(components.join("."))
    }
}

/// Generic module prefix for unknown languages based on path structure
fn get_generic_module_prefix(file_path: &Path) -> Option<String> {
    // Build from last few path components and file stem
    let ignored = [
        "node_modules",
        "dist",
        "build",
        "target",
        ".git",
        "bin",
        "obj",
    ];
    let mut parts: Vec<String> = Vec::new();
    for comp in file_path.parent()?.components() {
        if let std::path::Component::Normal(os) = comp {
            let s = os.to_string_lossy().to_string();
            if s.is_empty() || ignored.contains(&s.as_str()) {
                continue;
            }
            parts.push(s);
        }
    }
    // Only keep the last two directories to avoid very long prefixes
    if parts.len() > 2 {
        parts.drain(..parts.len() - 2);
    }
    // Add file stem if meaningful
    if let Some(stem) = file_path.file_stem().and_then(|s| s.to_str()) {
        if !matches!(stem, "index" | "main" | "mod" | "lib") && !stem.is_empty() {
            parts.push(stem.to_string());
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts.join("::"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn test_rust_impl_method_fqn_no_duplicates_and_no_pub() {
        // Simulate a simple Rust file structure
        let content = r#"
pub struct MessageCodec;

impl MessageCodec {
    pub fn encode(msg: &str) -> String {
        msg.to_string()
    }
}
"#;

        // Use repository-relative path so crate detection finds lsp-daemon/Cargo.toml
        let file_path = PathBuf::from("lsp-daemon/src/protocol.rs");
        // Cursor at start of 'pub fn encode' line (0-based line/col)
        let line = 4u32; // line containing 'pub fn encode'
        let column = 0u32;

        let fqn = get_fqn_from_ast_with_content(&file_path, content, line, column, Some("rust"))
            .expect("FQN extraction should succeed");

        // Expect crate name + type + method, without duplicate type or trailing ::pub
        assert_eq!(fqn, "lsp-daemon::MessageCodec::encode");
    }

    #[test]
    fn test_javascript_module_prefix_uses_workspace_relative_path() {
        let temp_dir = TempDir::new().unwrap();
        let workspace = temp_dir.path();

        // Simulate a Node workspace marker so the resolver detects the project root.
        std::fs::write(workspace.join("package.json"), "{\"name\": \"test-app\"}").unwrap();

        let file_path = workspace
            .join("examples")
            .join("chat")
            .join("npm")
            .join("index.ts");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        std::fs::write(&file_path, "export const ProbeChat = {};").unwrap();

        let prefix = get_javascript_module_prefix(&file_path).expect("module prefix");
        assert_eq!(prefix, "examples.chat.npm");
    }
}
