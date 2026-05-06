use serde::Serialize;
use std::path::Path;
use tree_sitter::Node;

use crate::language::{factory::get_language_impl, get_pooled_parser, return_pooled_parser};

#[derive(Debug, Clone, Serialize)]
pub struct SourceRange {
    pub lines: [usize; 2],
    pub columns: [usize; 2],
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceComment {
    pub kind: String,
    pub start_line: usize,
    pub end_line: usize,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceMatch {
    pub text: String,
    pub start_line: usize,
    pub start_column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment_role: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnclosingSymbol {
    pub kind: String,
    pub name: String,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnclosingCall {
    pub callee: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_arg_literal: Option<String>,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchContext {
    pub node_type: String,
    pub content: String,
    pub lines: [usize; 2],
    pub columns: [usize; 2],
}

#[derive(Debug, Clone, Serialize)]
pub struct OwnerContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub qualified_symbol: Option<String>,
    pub node_type: String,
    pub scope: String,
    pub lines: [usize; 2],
    pub columns: [usize; 2],
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<SourceComment>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub enclosing_symbols: Vec<EnclosingSymbol>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enclosing_call: Option<EnclosingCall>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub enclosing_calls: Vec<EnclosingCall>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuerySourceContext {
    pub language: String,
    pub r#match: MatchContext,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<OwnerContext>,
}

pub fn language_name_for_path(path: &Path) -> Option<&'static str> {
    match path.extension().and_then(|ext| ext.to_str()).unwrap_or("") {
        "rs" => Some("rust"),
        "js" | "jsx" | "mjs" => Some("javascript"),
        "ts" | "tsx" => Some("typescript"),
        "py" => Some("python"),
        "go" => Some("go"),
        "c" | "h" => Some("c"),
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" => Some("cpp"),
        "java" => Some("java"),
        "rb" => Some("ruby"),
        "php" => Some("php"),
        "swift" => Some("swift"),
        "cs" => Some("csharp"),
        "html" | "htm" => Some("html"),
        "md" | "markdown" => Some("markdown"),
        "yaml" | "yml" => Some("yaml"),
        _ => None,
    }
}

pub fn build_query_source_context(
    path: &Path,
    byte_start: usize,
    byte_end: usize,
    fallback_text: &str,
) -> Option<QuerySourceContext> {
    let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
    let language = language_name_for_path(path)?.to_string();
    let source = std::fs::read_to_string(path).ok()?;
    let source_bytes = source.as_bytes();

    let _language_impl = get_language_impl(extension)?;
    let mut parser = get_pooled_parser(extension).ok()?;
    let tree = parser.parse(&source, None)?;
    let root = tree.root_node();
    let matched = find_smallest_covering_node(root, byte_start, byte_end).unwrap_or(root);

    let matched_text = matched
        .utf8_text(source_bytes)
        .map(|text| text.to_string())
        .unwrap_or_else(|_| fallback_text.to_string());

    let owner = find_owner_node(matched)
        .map(|owner_node| build_owner_context(owner_node, matched, &source, source_bytes));

    let context = QuerySourceContext {
        language,
        r#match: MatchContext {
            node_type: matched.kind().to_string(),
            content: matched_text,
            lines: [
                matched.start_position().row + 1,
                matched.end_position().row + 1,
            ],
            columns: [
                matched.start_position().column + 1,
                matched.end_position().column + 1,
            ],
        },
        owner,
    };

    return_pooled_parser(extension, parser);
    Some(context)
}

pub fn build_search_owner_context(
    path: &Path,
    start_line: usize,
    end_line: usize,
    fallback_code: &str,
) -> Option<OwnerContext> {
    let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("");
    let source = std::fs::read_to_string(path).ok()?;
    let source_bytes = source.as_bytes();

    let _language_impl = get_language_impl(extension)?;
    let mut parser = get_pooled_parser(extension).ok()?;
    let tree = parser.parse(&source, None)?;
    let root = tree.root_node();
    let owner = find_search_owner_node(root, start_line, end_line);
    let context = owner.map(|owner| build_owner_context(owner, owner, &source, source_bytes));

    return_pooled_parser(extension, parser);

    let context = context?;
    if context.content.is_some() {
        Some(context)
    } else {
        Some(OwnerContext {
            content: Some(fallback_code.to_string()),
            ..context
        })
    }
}

pub fn extract_owner_symbol_from_source(code: &str, node_type: &str) -> Option<String> {
    if matches!(node_type, "section" | "document" | "paragraph" | "heading") {
        return None;
    }

    for line in code.lines().take(20) {
        let trimmed = line.trim();

        if let Some(name) = extract_name_after_keyword(trimmed, "func ") {
            return Some(name);
        }
        if trimmed.starts_with("func (") {
            if let Some(after_recv) = trimmed.split(')').nth(1) {
                let name = after_recv.trim().split('(').next().unwrap_or("").trim();
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }
        if let Some(fn_pos) = trimmed.find("fn ") {
            let valid_prefix = fn_pos == 0
                || trimmed
                    .as_bytes()
                    .get(fn_pos - 1)
                    .map_or(false, |&b| b == b' ' || b == b')');
            if valid_prefix {
                let name = trimmed[fn_pos + 3..]
                    .split(|c: char| c == '(' || c == '<' || c == ' ')
                    .next()
                    .unwrap_or("")
                    .trim();
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }
        if let Some(name) = extract_name_after_keyword(trimmed, "def ") {
            return Some(name);
        }
        if let Some(name) = extract_name_after_keyword(trimmed, "class ") {
            return Some(name);
        }
        if let Some(name) = extract_name_after_keyword(trimmed, "export class ") {
            return Some(name);
        }
        if let Some(name) = extract_name_after_keyword(trimmed, "interface ") {
            return Some(name);
        }
        if let Some(name) = extract_name_after_keyword(trimmed, "export interface ") {
            return Some(name);
        }
        if let Some(name) = extract_name_after_keyword(trimmed, "type ") {
            return Some(name);
        }
        if let Some(name) = extract_name_after_keyword(trimmed, "export type ") {
            return Some(name);
        }
        for prefix in [
            "function ",
            "async function ",
            "export function ",
            "export default function ",
            "export async function ",
            "export default async function ",
        ] {
            if let Some(name) = extract_name_after_keyword(trimmed, prefix) {
                return Some(name);
            }
        }
        for prefix in [
            "export const ",
            "export let ",
            "export var ",
            "const ",
            "let ",
            "var ",
        ] {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                let name = rest
                    .split(|c: char| c == '=' || c == ':' || c == '(' || c.is_whitespace())
                    .next()
                    .unwrap_or("")
                    .trim();
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }
        if node_type == "method_definition" || node_type.contains("method") {
            let without_modifiers = trimmed
                .trim_start_matches("async ")
                .trim_start_matches("public ")
                .trim_start_matches("private ")
                .trim_start_matches("protected ")
                .trim_start_matches("static ");
            let name = without_modifiers
                .split(|c: char| c == '(' || c == '<' || c == ':')
                .next()
                .unwrap_or("")
                .trim();
            if !name.is_empty()
                && !matches!(name, "if" | "for" | "while" | "switch" | "return")
                && !name.starts_with("//")
            {
                return Some(name.to_string());
            }
        }
    }

    None
}

pub fn leading_comments_from_block(code: &str, start_line: usize) -> Vec<SourceComment> {
    let mut comments = Vec::new();
    let mut pending: Option<(usize, usize, Vec<String>)> = None;
    let mut in_block_comment = false;

    for (idx, line) in code.lines().enumerate() {
        let line_no = start_line + idx;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if pending.is_some() {
                break;
            }
            continue;
        }

        let starts_comment = is_comment_line(trimmed) || in_block_comment;
        if starts_comment {
            match &mut pending {
                Some((_, end, lines)) => {
                    *end = line_no;
                    lines.push(trimmed.to_string());
                }
                None => {
                    pending = Some((line_no, line_no, vec![trimmed.to_string()]));
                }
            }

            if starts_block_comment(trimmed) && !ends_block_comment(trimmed) {
                in_block_comment = true;
            }
            if in_block_comment && ends_block_comment(trimmed) {
                in_block_comment = false;
            }
            continue;
        }
        break;
    }

    if let Some((start, end, lines)) = pending {
        comments.push(SourceComment {
            kind: "leading".to_string(),
            start_line: start,
            end_line: end,
            text: lines.join("\n"),
        });
    }

    comments
}

pub fn classify_text_matches_in_block(
    code: &str,
    start_line: usize,
    matched_keywords: Option<&Vec<String>>,
    leading_comments: &[SourceComment],
) -> Vec<SourceMatch> {
    let mut matches = Vec::new();
    let Some(keywords) = matched_keywords else {
        return matches;
    };

    for keyword in keywords {
        if keyword.is_empty() {
            continue;
        }
        let needle = keyword.to_lowercase();
        for (line_idx, line) in code.lines().enumerate() {
            let haystack = line.to_lowercase();
            let mut search_start = 0;
            while let Some(relative_pos) = haystack[search_start..].find(&needle) {
                let byte_start = search_start + relative_pos;
                let byte_end = byte_start + needle.len();
                let line_no = start_line + line_idx;
                let kind = classify_match_kind_at(code, line_idx, byte_start);
                let comment_role = if kind == "comment"
                    && leading_comments
                        .iter()
                        .any(|comment| line_no >= comment.start_line && line_no <= comment.end_line)
                {
                    Some("leading".to_string())
                } else {
                    None
                };

                matches.push(SourceMatch {
                    text: line
                        .get(byte_start..byte_end)
                        .unwrap_or(keyword.as_str())
                        .to_string(),
                    start_line: line_no,
                    start_column: byte_start + 1,
                    end_line: line_no,
                    end_column: byte_end + 1,
                    kind,
                    comment_role,
                });

                search_start = byte_end;
            }
        }
    }

    matches
}

pub fn classify_scope_from_node(
    node_type: &str,
    code: &str,
    is_doc: bool,
    is_example: bool,
    is_test: bool,
) -> &'static str {
    if is_test {
        return "test";
    }
    if is_example {
        return "example";
    }
    if is_doc {
        return "doc";
    }
    if is_function_like(node_type) || code.lines().take(5).any(|line| line.contains("=>")) {
        return "function";
    }
    if is_module_like(node_type) {
        return "module";
    }
    "declaration"
}

fn build_owner_context(
    owner: Node,
    matched: Node,
    source: &str,
    source_bytes: &[u8],
) -> OwnerContext {
    let symbol = extract_symbol_name_from_node(owner, source_bytes);
    let enclosing_symbols = collect_enclosing_symbols(owner, source_bytes);
    let qualified_symbol = symbol.as_ref().map(|name| {
        let mut parts: Vec<String> = enclosing_symbols
            .iter()
            .filter(|sym| sym.kind == "class" || sym.kind == "module" || sym.kind == "impl")
            .map(|sym| sym.name.clone())
            .collect();
        parts.push(name.clone());
        parts.join(".")
    });
    let comments = leading_comments_before_node(owner, source);
    let owner_start_line = comments
        .first()
        .map(|comment| comment.start_line)
        .unwrap_or_else(|| owner.start_position().row + 1);
    let owner_text_start_byte =
        line_start_byte(source, owner_start_line).unwrap_or(owner.start_byte());
    let owner_content = source
        .get(owner_text_start_byte..owner.end_byte())
        .map(|text| text.to_string());
    let enclosing_calls = collect_enclosing_calls(matched, source_bytes);

    OwnerContext {
        symbol,
        qualified_symbol,
        node_type: owner.kind().to_string(),
        scope: classify_scope_from_node(
            owner.kind(),
            owner_content.as_deref().unwrap_or(""),
            false,
            false,
            false,
        )
        .to_string(),
        lines: [owner_start_line, owner.end_position().row + 1],
        columns: [
            owner.start_position().column + 1,
            owner.end_position().column + 1,
        ],
        comments,
        enclosing_symbols,
        enclosing_call: enclosing_calls.last().cloned(),
        enclosing_calls,
        content: owner_content,
    }
}

fn find_smallest_covering_node<'a>(node: Node<'a>, start: usize, end: usize) -> Option<Node<'a>> {
    if node.start_byte() > start || node.end_byte() < end {
        return None;
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(found) = find_smallest_covering_node(child, start, end) {
            return Some(found);
        }
    }

    Some(node)
}

fn find_owner_node<'a>(node: Node<'a>) -> Option<Node<'a>> {
    let mut current = Some(node);
    while let Some(candidate) = current {
        if is_owner_node(candidate) {
            return Some(normalize_owner_node(candidate));
        }
        current = candidate.parent();
    }
    None
}

fn find_search_owner_node<'a>(
    root: Node<'a>,
    start_line: usize,
    end_line: usize,
) -> Option<Node<'a>> {
    let mut candidates = Vec::new();
    collect_owner_nodes_in_line_range(root, start_line, end_line, &mut candidates);

    candidates
        .into_iter()
        .min_by_key(|node| {
            let line_span = node
                .end_position()
                .row
                .saturating_sub(node.start_position().row);
            (line_span, node.end_byte().saturating_sub(node.start_byte()))
        })
        .map(normalize_owner_node)
}

fn collect_owner_nodes_in_line_range<'a>(
    node: Node<'a>,
    start_line: usize,
    end_line: usize,
    candidates: &mut Vec<Node<'a>>,
) {
    let node_start = node.start_position().row + 1;
    let node_end = node.end_position().row + 1;

    if node_end < start_line || node_start > end_line {
        return;
    }

    if is_owner_node(node) && node_start >= start_line && node_end <= end_line {
        candidates.push(node);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_owner_nodes_in_line_range(child, start_line, end_line, candidates);
    }
}

fn normalize_owner_node(node: Node) -> Node {
    if is_function_like(node.kind()) {
        if let Some(parent) = node.parent() {
            if parent.kind() == "variable_declarator" {
                return parent;
            }
            if parent.kind() == "arguments" {
                if let Some(call) = parent.parent() {
                    if call.kind() == "call_expression" {
                        return node;
                    }
                }
            }
        }
    }
    node
}

fn is_owner_node(node: Node) -> bool {
    let kind = node.kind();
    if matches!(
        kind,
        "function_declaration"
            | "function_definition"
            | "function_item"
            | "method_definition"
            | "method_declaration"
            | "class_declaration"
            | "class_definition"
            | "struct_item"
            | "type_declaration"
            | "impl_item"
            | "variable_declarator"
            | "arrow_function"
            | "function_expression"
    ) {
        return true;
    }
    false
}

fn is_function_like(kind: &str) -> bool {
    kind.contains("function")
        || kind.contains("method")
        || kind.contains("fn")
        || kind.contains("func")
        || kind == "arrow_function"
        || kind == "closure"
}

fn is_module_like(kind: &str) -> bool {
    kind.contains("module")
        || matches!(
            kind,
            "program" | "source_file" | "compilation_unit" | "package_clause"
        )
}

fn extract_symbol_name_from_node(node: Node, source: &[u8]) -> Option<String> {
    if let Some(name_node) = node
        .child_by_field_name("name")
        .or_else(|| node.child_by_field_name("type"))
    {
        return name_node
            .utf8_text(source)
            .ok()
            .map(|text| text.to_string());
    }

    if node.kind() == "variable_declarator" {
        if let Some(name_node) = node.child_by_field_name("name") {
            return name_node
                .utf8_text(source)
                .ok()
                .map(|text| text.to_string());
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if matches!(
            child.kind(),
            "identifier" | "type_identifier" | "property_identifier"
        ) {
            return child.utf8_text(source).ok().map(|text| text.to_string());
        }
    }

    None
}

fn collect_enclosing_symbols(node: Node, source: &[u8]) -> Vec<EnclosingSymbol> {
    let mut symbols = Vec::new();
    let mut current = node.parent();
    while let Some(parent) = current {
        if matches!(
            parent.kind(),
            "class_declaration"
                | "class_definition"
                | "impl_item"
                | "mod_item"
                | "module_declaration"
                | "namespace_declaration"
        ) {
            if let Some(name) = extract_symbol_name_from_node(parent, source) {
                symbols.push(EnclosingSymbol {
                    kind: normalize_symbol_kind(parent.kind()).to_string(),
                    name,
                    line: parent.start_position().row + 1,
                });
            }
        }
        current = parent.parent();
    }
    symbols.reverse();
    symbols
}

fn collect_enclosing_calls(node: Node, source: &[u8]) -> Vec<EnclosingCall> {
    let mut calls = Vec::new();
    let mut current = Some(node);
    while let Some(candidate) = current {
        if candidate.kind() == "call_expression" {
            if let Some(call) = build_enclosing_call(candidate, source) {
                calls.push(call);
            }
        }
        current = candidate.parent();
    }
    calls.reverse();
    calls
}

fn build_enclosing_call(node: Node, source: &[u8]) -> Option<EnclosingCall> {
    let callee_node = node
        .child_by_field_name("function")
        .or_else(|| node.named_child(0))?;
    let callee = callee_node.utf8_text(source).ok()?.to_string();
    let first_arg_literal = first_string_literal_arg(node, source);

    Some(EnclosingCall {
        callee,
        first_arg_literal,
        line: node.start_position().row + 1,
    })
}

fn first_string_literal_arg(node: Node, source: &[u8]) -> Option<String> {
    let arguments = node.child_by_field_name("arguments").or_else(|| {
        let mut cursor = node.walk();
        let found = node
            .children(&mut cursor)
            .find(|child| child.kind() == "arguments");
        found
    })?;
    let mut cursor = arguments.walk();
    for child in arguments.named_children(&mut cursor) {
        if matches!(
            child.kind(),
            "string" | "string_fragment" | "interpreted_string_literal"
        ) {
            let raw = child.utf8_text(source).ok()?.trim();
            return Some(raw.trim_matches(['"', '\'', '`']).to_string());
        }
    }
    None
}

fn leading_comments_before_node(node: Node, source: &str) -> Vec<SourceComment> {
    let lines: Vec<&str> = source.lines().collect();
    let mut idx = node.start_position().row;
    let mut comments = Vec::new();

    while idx > 0 {
        let line = lines.get(idx - 1).copied().unwrap_or("");
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if comments.is_empty() {
                idx -= 1;
                continue;
            }
            break;
        }
        if !is_comment_line(trimmed) {
            break;
        }
        comments.push((idx, trimmed.to_string()));
        idx -= 1;
    }

    comments.reverse();
    comments
        .into_iter()
        .map(|(line, text)| SourceComment {
            kind: "leading".to_string(),
            start_line: line,
            end_line: line,
            text,
        })
        .collect()
}

fn line_start_byte(source: &str, one_based_line: usize) -> Option<usize> {
    if one_based_line == 0 {
        return None;
    }
    if one_based_line == 1 {
        return Some(0);
    }
    let mut line = 1;
    for (idx, ch) in source.char_indices() {
        if ch == '\n' {
            line += 1;
            if line == one_based_line {
                return Some(idx + 1);
            }
        }
    }
    None
}

fn is_comment_line(trimmed: &str) -> bool {
    trimmed.starts_with("//")
        || trimmed.starts_with("///")
        || trimmed.starts_with("#")
        || trimmed.starts_with("*")
        || trimmed.starts_with("/*")
        || trimmed.starts_with("*/")
        || trimmed.starts_with("<!--")
        || trimmed.starts_with("-->")
}

fn starts_block_comment(trimmed: &str) -> bool {
    trimmed.starts_with("/*") || trimmed.starts_with("<!--")
}

fn ends_block_comment(trimmed: &str) -> bool {
    trimmed.contains("*/") || trimmed.contains("-->")
}

fn classify_match_kind_at(code: &str, target_line_idx: usize, target_byte_start: usize) -> String {
    let mut state = LexicalState::default();

    for (line_idx, line) in code.lines().enumerate() {
        state.line_comment = false;
        let stop_at = if line_idx == target_line_idx {
            Some(target_byte_start)
        } else {
            None
        };

        if scan_line_until(line, stop_at, &mut state) {
            return if state.block_comment_end.is_some() {
                "comment".to_string()
            } else if state.line_comment {
                "comment".to_string()
            } else if state.quote.is_some() {
                "string".to_string()
            } else {
                "code".to_string()
            };
        }
    }

    "code".to_string()
}

fn scan_line_until(line: &str, stop_at: Option<usize>, state: &mut LexicalState) -> bool {
    let mut idx = 0;
    let line_len = line.len();
    let stop = stop_at.unwrap_or(line_len).min(line_len);

    while idx < stop {
        let rest = &line[idx..];

        if let Some(end_marker) = state.block_comment_end {
            if let Some(end_offset) = rest.find(end_marker) {
                let end_idx = idx + end_offset + end_marker.len();
                if end_idx > stop {
                    return true;
                }
                state.block_comment_end = None;
                idx = end_idx;
                continue;
            }
            return stop_at.is_some();
        }

        if let Some(quote) = state.quote {
            let Some(ch) = rest.chars().next() else {
                break;
            };
            if state.escaped {
                state.escaped = false;
            } else if ch == '\\' {
                state.escaped = true;
            } else if ch == quote {
                state.quote = None;
            }
            idx += ch.len_utf8();
            continue;
        }

        if rest.starts_with("//") || rest.starts_with('#') {
            if stop_at.is_none() {
                return false;
            }
            state.line_comment = true;
            return true;
        }
        if rest.starts_with("/*") {
            state.block_comment_end = Some("*/");
            idx += 2;
            continue;
        }
        if rest.starts_with("<!--") {
            state.block_comment_end = Some("-->");
            idx += 4;
            continue;
        }

        let Some(ch) = rest.chars().next() else {
            break;
        };
        if matches!(ch, '"' | '\'' | '`') {
            state.quote = Some(ch);
            state.escaped = false;
        }
        idx += ch.len_utf8();
    }

    stop_at.is_some()
}

#[derive(Default)]
struct LexicalState {
    block_comment_end: Option<&'static str>,
    line_comment: bool,
    quote: Option<char>,
    escaped: bool,
}

fn extract_name_after_keyword(line: &str, prefix: &str) -> Option<String> {
    let rest = line.strip_prefix(prefix)?;
    let name = rest
        .split(|c: char| c == '(' || c == '<' || c == ':' || c.is_whitespace())
        .next()
        .unwrap_or("")
        .trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

fn normalize_symbol_kind(kind: &str) -> &'static str {
    match kind {
        "class_declaration" | "class_definition" => "class",
        "impl_item" => "impl",
        "mod_item" | "module_declaration" | "namespace_declaration" => "module",
        "method_definition" | "method_declaration" => "method",
        kind if is_function_like(kind) => "function",
        _ => "declaration",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_text_matches_in_comments_and_strings() {
        let code = r#"/*
 * Implements: SYS-REQ-0
 */
// Implements: SYS-REQ-1
export const literalOnly = "SYS-REQ-2";
const templateLiteral = `
  SYS-REQ-4
`;
export function run() {
  return SYS_REQ_3;
}"#;
        let comments = leading_comments_from_block(code, 10);
        let keywords = vec![
            "SYS-REQ-0".to_string(),
            "SYS-REQ-1".to_string(),
            "SYS-REQ-2".to_string(),
            "SYS_REQ_3".to_string(),
            "SYS-REQ-4".to_string(),
        ];

        let matches = classify_text_matches_in_block(code, 10, Some(&keywords), &comments);

        assert_eq!(matches.len(), 5);
        assert_eq!(matches[0].kind, "comment");
        assert_eq!(matches[0].comment_role.as_deref(), Some("leading"));
        assert_eq!(matches[1].kind, "comment");
        assert_eq!(matches[1].comment_role.as_deref(), Some("leading"));
        assert_eq!(matches[2].kind, "string");
        assert_eq!(matches[3].kind, "code");
        assert_eq!(matches[4].kind, "string");

        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].start_line, 10);
        assert_eq!(comments[0].end_line, 13);
        assert!(comments[0].text.contains("SYS-REQ-0"));
        assert!(comments[0].text.contains("SYS-REQ-1"));
    }
}
